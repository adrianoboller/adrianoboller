# Estado global de processo entre testes do mesmo binário

*Descoberto em 09/09/2026, 06h10, fechando o pedido 234 (C20-QA-ODBC).*

## 1. O que aconteceu

`config::testes_recursos::threads_e_cpu_viram_o_teto_do_paralelo`
(`crates/phxsql-server/src/config.rs`) montava um `Config` com
`threads:4, cpu_percentual:25`, chamava `c.recursos.aplicar()` e conferia
`phxsql_core::paralelo::nucleos() == 1`. `paralelo::TETO`
(`crates/phxsql-core/src/paralelo.rs`) é um `static AtomicUsize` — **global
do PROCESSO**, não do teste — e `Recursos::aplicar()` é chamado por
`Config::ler`, que uma dezena de outros testes deste mesmo arquivo também
chamam (`servidor.rs` tem mais oito). Cada um redefine o mesmo `TETO` com o
valor que o SEU `config.json` de teste produz.

O `cargo test` roda os testes de um binário em PARALELO por padrão. Nada
impede um vizinho de chamar `Config::ler` (e portanto `definir_teto`) entre
o `aplicar()` e o `assert_eq!` deste teste — e nesse instante o `nucleos()`
lido não é mais o `1` que este teste armou, é o que o vizinho acabou de
gravar.

Rodando o binário de teste compilado, filtrado por `config::`, direto (sem
passar pelo `cargo test`, para eliminar a variação de tempo de build):
**4 quedas em 200 corridas** (2%), sempre a mesma asserção, sempre com o
mesmo padrão —

```
assertion `left == right` failed: o teto nao valeu
  left: 4
 right: 1
```

— `left` batendo com `nucleos()` da máquina (4, o `nproc` deste contêiner),
nunca um número aleatório: é exatamente o valor que um teste vizinho (com
`threads`/`cpu_percentual` de fábrica, sem teto efetivo) grava no meio do
caminho.

## 2. O que eu concluí primeiro, e estava errado

A primeira leitura do pedido (`docs/PENDENCIAS.md` linha 258 e
`docs/propostas/comparativo-19.md`, seção "O teste 234 e o estado global")
já vinha com o diagnóstico certo escrito por quem nomeou o pedido — não teve
tempo de errar aqui. Onde quase errei foi no CONSERTO: a primeira ideia, ao
ver `phxsql_core::paralelo::tests::o_teto_configurado_vale` já existindo (e
já testando `definir_teto`/`nucleos` isolado), foi **acrescentar** um
segundo teste ao lado dele em `paralelo.rs` para cobrir a propriedade que
faltava ("o teto corta, nunca inventa acima do disponível") — dois `#[test]`
separados, cada um chamando `definir_teto` e conferindo um valor exato.

Isso **reintroduzia o mesmo defeito, num escopo menor**: dois testes do
MESMO binário (`phxsql-core`), cada um gravando e lendo o global `TETO`,
correndo um contra o outro exatamente como `threads_e_cpu_viram_o_teto_do_
paralelo` corria contra `Config::ler`. A diferença de escala (2 testes
contra 1, em vez de 1 contra uma dezena) muda a PROBABILIDADE da colisão,
não a existência dela — e "mais raro" não é "consertado". Só percebi ao
revisar a própria contagem de quem mais toca `paralelo::definir_teto` neste
crate antes de publicar: dois usos no MESMO arquivo de teste, os dois
pinando um valor exato do global. O conserto certo foi juntar as duas
asserções num teste SÓ.

## 3. O que a medição disse

| onde | quantas corridas | quedas | tempo |
|---|---:|---:|---:|
| binário de teste, ANTES do conserto, filtro `config::`, 4 threads | 200 | **4** | ~13s |
| binário de teste, DEPOIS do conserto, mesmo filtro | 300 | **0** | ~29s |

E o achado do item 2, medido à parte e não só argumentado: recompus por um
instante os dois `#[test]` separados que quase entraram em
`phxsql-core::paralelo::tests` (cada um pinando um valor exato do mesmo
`TETO`, num laço de 2.000 repetições para apertar a janela da corrida em vez
de depender de sorte de agendamento), rodei o binário de teste **30 vezes**
filtrado por `paralelo::`, e as **30 caíram** — a maioria já na primeira
repetição do laço interno. A superfície de colisão real (2 testes, chamados
uma vez cada por `cargo test`) é bem menor que a de `Config::ler` (uma
dezena de chamadas por corrida), e por isso não teria caído sozinha sem o
laço de 2.000 apertando a janela — mas o MECANISMO é idêntico, e a medição
confirma que a diferença entre "caiu 4 vezes em 200" e "caiu 30 vezes em 30"
é só o tamanho da janela de exposição, não a ausência do defeito. Os dois
testes temporários foram removidos depois de medir; o arquivo fica só com o
teste único (`o_teto_configurado_vale`, as duas asserções em sequência, sem
teste vizinho entre elas).

## 4. A regra

**Teste que confere um valor EXATO de estado global de processo só pode
existir SOZINHO — nem dividido entre módulos do mesmo binário, nem
dividido em duas funções `#[test]` no mesmo arquivo.** Se duas provas
precisam do mesmo global, elas têm de estar na MESMA função de teste (uma
depois da outra, sem chamada de teste entre elas) ou atrás de um mutex de
teste dedicado — nunca duas funções `#[test]` soltas, por mais perto que
pareçam uma da outra no arquivo.

O corolário que fecha o pedido: **um teste que precisa mexer no global
prova o CÁLCULO em isolamento (sem tocar o global) sempre que o cálculo é
separável da gravação** — foi isso que permitiu `threads_e_cpu_viram_o_
teto_do_paralelo` continuar existindo e provando a mesma coisa
(`Recursos::nucleos()`) sem depender do processo inteiro ficar quieto.

## 5. Como está guardado hoje

- `crates/phxsql-server/src/config.rs::testes_recursos::threads_e_cpu_
  viram_o_teto_do_paralelo` prova só `Recursos::nucleos()` — não chama
  `aplicar()`, não lê `phxsql_core::paralelo` nenhuma vez.
- `crates/phxsql-core/src/paralelo.rs::tests::o_teto_configurado_vale`
  prova as DUAS metades do global (`definir_teto(1)` corta, e um teto acima
  do disponível não inventa núcleo) na MESMA função — de propósito, pelo
  motivo do item 2 acima.
- O comentário acima de `Recursos::aplicar()` nomeia a composição
  (`definir_teto(self.nucleos())`) como revisada e não testada de ponta a
  ponta, e diz por quê.
- `bancada/guardas/catalogo.py` **não** ganhou uma entrada para este defeito
  especificamente: o executor (`provar-guardas.py`) roda o binário nomeado
  UMA vez e espera falha determinística; uma colisão de ~2% por corrida não
  cabe nesse contrato — reportaria "NAO PEGOU" na esmagadora maioria das
  vezes mesmo com o defeito reposto. Fica registrado aqui e no
  `docs/QA-PDCA.md` (seção "Configuração") em vez de forçar uma entrada que
  o próprio catálogo marcaria como suspeita.

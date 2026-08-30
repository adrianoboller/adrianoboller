# As guardas — provar que a prova pega

A casa exige que todo teste novo **falhe com o defeito reposto**. Isso sempre
foi feito à mão, uma vez, por quem escreveu o teste — e depois se perdia.
Ninguém conseguia dizer, hoje, quais das 1.229 asserções ainda pegariam o
defeito que as motivou.

```bash
python3 bancada/guardas/provar-guardas.py            # todas
python3 bancada/guardas/provar-guardas.py --listar   # o catálogo
python3 bancada/guardas/provar-guardas.py --so profiler
```

Três arquivos, e a divisão entre os dois primeiros é o ponto:

| arquivo | o que é |
|---|---|
| `catalogo.py` | **só dados**: cada defeito, o trecho de hoje, o trecho de antes, e quais testes têm de cair |
| `provar-guardas.py` | o executor: copia a árvore, repõe um defeito por vez, roda os testes nomeados, desfaz, e julga |
| `tabela-no-testes.py` | regrava a tabela das guardas no `docs/TESTES.md` a partir do `--json` de uma rodada — número visível que não sai de gerador está errado e ninguém percebeu ainda |

## O que sai

```
--- a arvore limpa, antes de qualquer defeito ---
  phxsql-server --lib                verde    8.5 s  540 testes
  phxsql-store  --lib                verde    0.7 s  133 testes
  phxsql-store  --test cifra-dos-dados verde  5.1 s    8 testes

--- com o defeito reposto, um de cada vez ---
  profiler-recorta         PROVADA      9.9 s  5/5 cairam
  cadeia-sem-teto          PROVADA      4.0 s  0/1 cairam
      o binario abortou, que e como esta guarda pega
  aad-fora-do-slot         REDUNDANTE   5.7 s  0/0 cairam
      confirmado: tirar so o AAD nao e sentido por teste nenhum
  ...
```

Cinco vereditos, e o terceiro é o motivo desta frente existir:

| veredito | o que quer dizer |
|---|---|
| **PROVADA** | todos os `caem` caíram e todos os `seguem` continuaram de pé |
| **REDUNDANTE** | a entrada declarou `espera: "nada muda"` e nada mudou mesmo — a guarda existe **duas vezes** no código, e tirar uma só não é sentida por teste nenhum. É resultado medido, e não falha |
| **NAO PEGOU** | um `caem` continuou passando — **é um teste que passa por engano**, e a casa considera isso pior que teste que falta |
| **ESTRAGOU** | um `seguem` caiu junto: a troca quebrou mais do que o defeito de origem quebrava, então ela não prova a guarda |
| **QUEBRADA** | o trecho não está mais no arquivo, aparece duas vezes, ou o código trocado nem compila — a entrada envelheceu |

Sai `0` quando todas ficaram provadas ou redundantes, `1` quando alguma não
ficou.

## As duas metades da prova real

O executor confere as **duas**, e nesta ordem:

1. **passa com o conserto** — a árvore limpa roda primeiro, inteira. Se ela não
   estiver verde, nada aqui prova nada e o executor para. Sem essa conferência,
   um teste já vermelho apareceria como guarda provada: o defeito reposto não
   teria feito diferença nenhuma e o teste cairia do mesmo jeito. **Foi ela que
   pegou o defeito do próprio executor** — ver abaixo.
2. **falha com o defeito** — a lista `caem`, teste a teste.

E a lista `seguem`, que é a terceira metade que ninguém pede: os testes que têm
de **continuar passando**. Sem ela, uma troca que quebrasse o arquivo inteiro
pareceria uma guarda excelente.

## Os três cuidados, e o que cada um custou

**Nunca na árvore de verdade.** O executor copia `crates/`, `exemplos/`,
`Cargo.toml` e `Cargo.lock` (5 MB) para `~/.cache/phx-guardas` e mexe só lá.
Cada troca é desfeita num `finally`, e há uma rede no `atexit`: um Ctrl-C no
meio não deixa defeito plantado em lugar nenhum.

E esse caminho é **compartilhado**: duas árvores de trabalho na mesma máquina
disputam a mesma cópia, e o estrago engana — três guardas saíram `QUEBRADA` com
«o código com o defeito reposto não compila», citando campos de uma `struct`
que não existe nesta árvore, porque a cópia era da árvore vizinha. Não é
entrada envelhecida, é cópia trocada. **Quem roda em paralelo passa `--arvore`
com um nome próprio** — a mesma rodada, com `--arvore` privado, deu 19/19 sem
uma quebrada.

**Só o binário de teste que a entrada nomeia.** Medido nesta máquina, com o
`target/` quente e uma recompilação por mutação: o binário nomeado custa
**8,1 s**, o `cargo test --workspace` custa **49,2 s**. Para as 19 entradas são
~2 min contra ~15 min — não é «horas», como eu tinha escrito antes de medir, é
uma ordem de grandeza. O que a escolha compra é caber **dentro** da bateria
única (14m35s inteira) em vez de dobrá-la.

**Prazo em toda rodada.** Defeito que **pendura** em vez de falhar travaria a
bateria — e o `sujas-com-a-trava` é exatamente esse: um `Mutex` não reentrante
pedido duas vezes pela mesma thread. O teste dele já tem prazo próprio de 30 s;
o executor tem o dele por cima, e **mais largo**, senão mataria a rodada antes
de o teste conseguir reprovar. Medido: essa guarda leva 35,3 s, e as outras 18
levam de 1,4 a 13,2 s.

A `trava-atras-da-rede` é a segunda do mesmo naipe, e por outro caminho: o
defeito dela é o laço da réplica segurando a trava de dados durante uma leitura
de rede, e com ele reposto a sonda do teste **pendura** por 30 s em vez de
falhar. O teste tem prazo próprio de 8 s em cada sonda, o executor tem o dele
por cima (120 s), e a mensagem de reprovação já traz o diagnóstico: *«`varrer`
sem resposta em 8 s; o `ping`, que não precisa da trava, respondeu em 570 µs»*.

## Três coisas que só apareceram rodando

**A cópia por `copytree` reintroduziu a regra do binário velho, dentro da
ferramenta que existe para pegá-la.** `copytree` copia com `copy2`, que
**preserva a data**. A rodada anterior compilava o `target/` da cópia a partir
do fonte mutado; a seguinte devolvia o fonte limpo com a data velha; e o cargo,
que decide por data, achava o artefato mais novo que o fonte e não recompilava
nada — a «árvore limpa» rodava o binário **com o defeito ainda dentro**. Quem
pegou foi justamente a conferência da árvore limpa. Hoje a cópia é por
**conteúdo**, com a data de agora no que mudou, e os arquivos que o catálogo
sabe mutar levam `utime` a cada invocação.

**A cópia não pode morar no `/tmp`.** `restaurar.rs` tem um teste que exige que
o palco da restauração **não** caia em `std::env::temp_dir()`, e ele mede isso
contra o diretório de trabalho. Com a cópia em `/tmp/…`, o próprio diretório de
trabalho é temporário e o teste reprova sem haver defeito nenhum. Ler o teste
não mostrava isso.

**`crates/` sozinho nem compila.** O `lib.rs` do servidor faz
`include_str!("../../../exemplos/Config_exemplo_01.json")`. A primeira cópia
levou só `crates/` e o compilador disse exatamente qual arquivo faltava.

## Acrescentar uma guarda

1. escreva a entrada no `catalogo.py`, com o `trecho` copiado **do fonte** — ele
   tem de aparecer uma vez só, porque trocar a errada provaria outra coisa, e
   tem de casar byte a byte: um `\n` que existe de verdade dentro do fonte Rust
   pede uma *raw string* no catálogo, e o executor recusa a entrada quando não
   casa (foi o que aconteceu com o `evento-linha-sem-escape` na primeira
   tentativa);
2. rode `--so <id>`;
3. se der `NAO PEGOU`, **pare**: o achado é seu, e vale mais que a guarda. É um
   teste que passa por engano. Conserte o teste, ou registre com precisão por
   que ele não pega o que dizia pegar.

O `troca` tem de ser o **defeito de origem**, e não um sabotador qualquer que
derruba o teste por outro motivo. Três entradas mostram o que isso custa:

- **`profiler-recorta` e `profiler-recorta-largo` são o mesmo defeito com a mão
  mais e menos pesada**, e cada um derruba um conjunto diferente de testes. A
  entrada nasceu única, listando sete testes «porque o comentário do fonte diz
  que todos caem com um `find` e um corte». Medido: caem cinco, e depende de
  qual corte. «Este teste pega aquele defeito» é uma afirmação como outra
  qualquer.
- **`aad-fora-do-slot`, `nonce-sem-endereco` e `endereco-fora-da-amarracao` são
  três entradas para uma garantia só**, porque o endereço do slot cifrado está
  amarrado **duas vezes** (o dado associado e o nonce) e cada uma segura
  sozinha. As duas primeiras *afirmam* a redundância; a terceira prova a
  garantia. Tirar uma ponta só e chamar de defeito reposto teria dado
  `NAO PEGOU` num teste que está certo.
- **`regra-de-tabela-imposta` é a regra que a casa mais repete**, virada em
  asserção: *guarda nova entra pedida, não imposta*. Com o defeito reposto — sem
  regra de tabela, nega — caem **14 dos 540** testes do `--lib`, e o
  `supervisor_passa_por_cima` sobrevive, que é o `seguem` dela. A largura do
  estrago é o argumento: uma guarda imposta tira o direito de todo mundo que já
  funcionava, e quem trava isso é o teste do comportamento **velho**.

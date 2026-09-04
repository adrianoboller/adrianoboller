# Bancada de comparação

A medição do PhxSql contra outros motores. Tudo aqui é para ser **refeito**:
número de desempenho que ninguém consegue reproduzir é número em que não se
deve acreditar.

| Arquivo | O que é |
|---|---|
| `medir.py` | a bancada: cerca cada fase com os contadores do `/proc` |
| `esta-medindo.sh` | o **portão**: responde se há medição em curso. Sai 0 e lista quando achou, 1 e cala quando não. Consultado pelo `comunicacao.sh` e pelo `zelador.sh` antes de rodarem, e provado no **item 0b** da bateria |
| `graficos.py` | gera a página de comparação a partir do `resultados.json` |
| `resultados.json` | a última medição **completa**, crua — é dela que o dossiê se gera |
| `carga-10-milhoes.log` | o log da corrida de 10.000.000 |
| `resultados-3-milhoes.json` | a corrida de 3.000.000, guardada inteira |
| `carga-3-milhoes.log` | o log dela |
| `bateria/` | a bateria de **ponta a ponta**: os seis itens do pedido feitos como um usuário faria, pelo soquete e pela tela, e a medição do que o gatilho e a chave custam. Ver `bateria/LEIA-ME.md` |
| `exclusao/` | a **prova pelo processo** da janela de durabilidade da exclusão: 150 exclusões pelo soquete e um `SIGKILL` no meio da janela. Ver `exclusao/LEIA-ME.md` |
| `guardas/` | o catálogo dos **defeitos repostos** e o executor que os repõe: prova que cada teste ainda pega o defeito que o motivou. Não mede nada — julga as outras baterias. Ver `guardas/LEIA-ME.md` |
| `embutido/` | a prova do **PhxSql embutido**: um programa em **C** ligado à biblioteca (`crates/phxsql-ffi`), rodado três vezes — contra o `.a` e contra o `.so` em x86-64, e contra o `.a` em **ARM64 sob `qemu-aarch64-static`**. Não mede tempo: prova que a ABI funciona onde ela vai morar. `bancada/embutido/provar.sh` |
| `sqlite/` | a comparação com o **SQLite(R)**, que é a que decide o caso do celular: motor contra motor, o custo do soquete medido à parte, e a durabilidade casada nos três regimes. Ver `sqlite/LEIA-ME.md` e `docs/MOBILE.md` |
| `arm/` | a prova de que o binário **ARM64 roda** — sob `qemu-user-static`, sem VM. `docs/EMPACOTAMENTO.md` §7.3 |
| `windows/` | a mesma prova para o **`.exe`**, sob `wine`. A sonda é a do `arm/`, com o rótulo vindo de fora. `docs/EMPACOTAMENTO.md` §6.1 |

A carga do lado do PhxSql é `crates/phxsql-store/examples/carga.rs`, que roda
cada fase num processo separado — assim os contadores são daquela fase e de
mais nada.

## Antes de lançar qualquer bancada: pergunte ao portão

```bash
bancada/esta-medindo.sh && echo "ha medicao em curso -- espere"
```

Ele existe porque a pergunta «há bancada medindo agora?» era improvisada num
`pgrep -f` a cada vez — e **`pgrep -f` se acha**: o padrão viaja na linha de
comando do próprio `pgrep`, então ele casa o processo que perguntou. Com a
máquina limpa, `pgrep -f "bancada/concorrencia"` acha **1**; o portão acha
**0**.

É a quarta vez que essa armadilha aparece nesta base, e a lei contra ela já
estava escrita — dentro do `comunicacao.sh`, que é o único lugar que ela
protegia. **Lei escrita dentro de um script só vale para aquele script.**

E ela não se aplica aqui ao pé da letra: «o crivo é o nome do executável» não
distingue nada quando o executável é `python3`. Daí os **dois** crivos — nome
do executável onde ele diz algo, caminho do script onde não diz — e a exclusão
do observador **por linhagem** (`/proc/<pid>/stat`), nunca por texto. É por
isso que o shell que chama o portão, carregando o nome dele na própria linha de
comando, não faz o portão achar a si mesmo.

Rodar aviso ou zelador dentro de uma janela de medição **reprovou três
baterias em 04/09/2026** — e o vizinho que as reprovou era o próprio agente
que as tinha lançado.

## Como refazer

```bash
cargo build --release --example carga -p phxsql-store
service mysql start
python3 bancada/medir.py 10000000     # ~50 min: 45 do insert do PhxSql
python3 bancada/graficos.py
```

## O arquivo do repositório só muda no fim

Durante a corrida o progresso vai para `resultados.parcial.json`, que não é
versionado. Só quando a medição fecha inteira ele é promovido a
`resultados.json`, com `os.replace` — que troca de uma vez: ou o arquivo é o
antigo inteiro, ou o novo inteiro, nunca metade de cada.

Isso existe porque uma corrida de dez milhões leva vinte minutos, e durante
esse tempo o arquivo versionado ficava com meia medição dentro. Quem olhasse
via número, não via "faltam quatro fases".

## Duas escalas, e por que as duas ficam

O `resultados.json` guarda uma medição só, e o dossiê se gera dela. Mas
comparar escalas diferentes engana: tabela menor tem árvore mais rasa e cabe
melhor na cache, então parte de qualquer ganho vem do tamanho, não do motor.

Por isso a corrida de 3.000.000 ficou com nome próprio ao lado da de
10.000.000. A comparação limpa entre escalas diferentes é **o primeiro milhão**, que é o
mesmo trabalho nas duas: foi assim que se mediu o efeito do CRC slice-by-8,
5.089/s → 16.063/s.

Melhor ainda é comparar a **mesma escala em dois momentos**, e é para isso que
a corrida de 3.000.000 é refeita a cada rodada de desempenho. Ela isolou o
efeito da rodada da unicidade:

| fase | antes | depois | ganho |
|---|---:|---:|---:|
| inserir | 209,09 s | 159,40 s | 1,31× |
| buscar | 1,06 s | 0,55 s | 1,91× |
| varrer | 1,76 s | 0,66 s | 2,69× |
| atualizar | 0,66 s | 0,56 s | 1,18× |
| excluir | 1,87 s | 1,67 s | 1,12× |

E ensinou uma lição: o conserto que dava nome à rodada — a conferência de
unicidade — foi o **menor** dos cinco ganhos. Quem rendeu foi o `descer`
deixando de reler a folha, que tirou uma página inteira, com o CRC de 4 KB
junto, de *toda* busca do motor.

## O que se mede

Tempo de parede, CPU (`utime+stime`), pico de memória residente (`VmHWM`) e
bytes lidos e escritos (`read_bytes`/`write_bytes`). Do lado do MySQL(R) o
trabalho acontece no `mysqld`, então os contadores dele entram **por
diferença** em volta de cada fase.

## As quatro regras que fazem a comparação valer

1. **Mesmos dados.** O gerador é previsível, sem sorteio: os dois motores
   recebem exatamente as mesmas linhas.
2. **Mesmo esquema.** Chave primária em `id` e índice secundário em `cidade`,
   dos dois lados.
3. **Mesma forma de pergunta.** Uma instrução por operação nos dois. A
   primeira versão desta bancada mandava ao MySQL(R) um único
   `WHERE id IN (…)` e ao PhxSql vinte mil buscas separadas — o número saía
   41× a favor do MySQL(R) pela *forma da pergunta*, não pelo motor.
4. **Mesma quantidade de trabalho.** Forma igual não basta: a varredura por
   faixa pedia ao MySQL(R) um `COUNT(*) + SUM(valor)` sobre 1.250.000 linhas
   e ao PhxSql a leitura de apenas 20.000 delas. Mesma pergunta, 1,6% do
   trabalho — e o resultado saía 5× a favor do PhxSql sem que o motor
   tivesse feito nada por isso. Corrigido: a fase `varrer` lê a faixa inteira
   e soma o valor, como o outro lado — e a prova de que agora está igual é a
   **soma**: os dois devolvem 1.250.000 linhas e 5.576.201.000,00, o mesmo
   total até o centavo, por dois códigos sem uma linha em comum. O resultado
   sobreviveu: a varredura continua a favor do PhxSql, por 3,3× em vez de 5×.

Estas duas regras vieram do mesmo lugar: **os dois erros favoreciam um lado
diferente, e nenhum dos dois era visível no número**. Bancada mal montada mente
com número, que é a mentira mais convincente que existe.

## O que NÃO é comparado

**Durabilidade.** Os dois carregam em massa, com uma sincronização por lote.
Uma bancada com `commit` por linha daria outros números — e é a que importa
para quem grava pedido a pedido.

E uma exceção, que é o **terceiro** caso de trabalho desigual escondido num
número desta bancada — desta vez contra nós, e por isso demorou a aparecer. Na
fase `excluir`, o MySQL(R) recebe as 20.000 instruções dentro de um
`START TRANSACTION … COMMIT`: **um** `fsync` para as vinte mil. O PhxSql
sincronizava o `.trash` **por linha** — vinte mil — porque a garantia da
lixeira é por exclusão e não por janela (`docs/DESEMPENHO.md` §4.12).

O padrão continua sendo esse, porque ele é o comportamento do servidor com o
`config.json` de fábrica. Para comparar durabilidade equivalente:

```bash
PHX_EXCLUSAO_NA_JANELA=1 python3 bancada/medir.py 1000000
```

Medido a 1.000.000 nesta máquina, duas corridas de cada: a fase sai de
**6,30 s / 16,59 s** (perde de 1,45 s / 1,90 s do MySQL(R)) para **0,91 s /
0,96 s** — e vira a única fase em que o motor perdia.

**Transações.** O MySQL(R) tem; o PhxSql não. Parte do custo de escrita dele é
o log de *redo*, que compra algo que o PhxSql ainda não oferece.

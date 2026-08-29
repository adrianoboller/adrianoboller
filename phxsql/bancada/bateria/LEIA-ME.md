# A bateria de ponta a ponta

Os seis itens que o dono pediu, feitos **como um usuário faria** — pelo
soquete e pela tela —, e não como teste unitário isolado:

1. criar um database;
2. criar tabelas dentro dele;
3. **UUID v7 como chave** e **relacionamento 1:N** entre as tabelas;
4. **gatilhos**;
5. **procedimentos**;
6. **carga de 5.000 registros**, medida.

## Como rodar

```bash
cargo build --release
cargo build --release --examples -p phxsql-store   # a regra do binário velho
python3 bancada/bateria/prova-bateria.py                     # só as provas
python3 bancada/bateria/prova-bateria.py --tela              # + o navegador
python3 bancada/bateria/prova-bateria.py --medir             # + a medição
python3 bancada/bateria/prova-bateria.py --tela --medir --rodadas 5
```

Com `--tiros <diretório>` a parte da tela grava as capturas de cada passo.

O script sobe um `phxsqld` **próprio** em **6300** (dados) e **6301** (web),
com cadastro de usuários de verdade, e mata **só o processo que ele criou,
pelo PID**. Nunca toca em `phxsqld` de outra pessoa.

`--medir` grava `resultados.json` ao lado — e ele é a fonte dos números que
`docs/DESEMPENHO.md` cita nesta frente. Número digitado à mão envelhece calado.

## Por que ela sobe um cadastro de usuários

Porque o passo que mais importa do item 5 é o **portão**: existe um usuário
`pedro` que opera a base inteira **menos** a tabela `alunos`, e a prova é que
um `CALL` de um procedimento que lê `alunos` morre no mesmo portão que a porta
da frente — e que o que ele **pode** continua podendo. Sem cadastro não há o
que provar ali.

A senha nunca entra no arquivo: o `config.json` recebe o **hash** gerado na
hora por `phxsqld --senha`, porque o sal muda a cada rodada.

## Por que tem uma metade em Node

Porque metade dos defeitos desta casa só apareceu no navegador, e este
`prova-tela.mjs` já pagou o próprio custo: ele achou que a ficha nova de uma
tabela de chave `Uuid` **não gravava** — o campo prometia «em branco … gera um
v7» e em branco o servidor recusava a linha inteira com «obrigatória e recebeu
NULL».

Ele **não sobe servidor**: quem sobe é o Python, que já montou o banco pelo
soquete e sabe matar o que criou. O Node só olha e clica.

## Os dois defeitos que ela achou, e o que ficou travado

| defeito | onde | o que trava hoje |
|---|---|---|
| `gravar_de_verdade` fechava a janela de durabilidade chamando `descarregar_sujas()`, que **pede a trava de dados já na mão de quem chamou** — abraço mortal com a própria trava, servidor inteiro parado. **Sem gatilho nenhum:** bastavam duas tabelas gravadas alternadamente | `servidor.rs`, `gravar_de_verdade` | `testes_janela_e_cadeia::duas_tabelas_na_mesma_janela_nao_travam_o_servidor` (com prazo — reposto o defeito ele **falha**, não pendura) |
| `AFTER INSERT ON t` gravando em `t` chamava a si mesmo sem fundo, e o Rust **abortava o processo** com *stack overflow* | `servidor.rs`, `rodar_gatilhos_depois` | `testes_janela_e_cadeia::a_cadeia_de_gatilhos_para_no_teto_e_avisa` |

Os dois testes vêm com o par que importa mais que eles: o do comportamento
**velho** — `uma_tabela_so_grava_como_sempre` e
`a_cadeia_curta_de_auditoria_roda_inteira`. Guarda nova que quebra o caminho
de sempre não é guarda, é estrago.

## Quatro coisas que a bateria trava de propósito

* **a chave estrangeira é declarada e NÃO é imposta.** Dois passos gravam esse
  limite: a filha órfã entra e a mãe com filhas sai. O dia em que o motor
  impuser a chave, eles falham — e é o aviso de que a documentação mudou;
* **`WHERE` sobre coluna sem índice é recusado pelo nome**, dentro da rotina
  como fora dela: a camada `SELECT` não varre a tabela inteira escondendo o
  custo, e a rotina não ganha atalho;
* **`COUNT(*)` e a varredura não são a mesma pergunta.** O `COUNT(*)` sai do
  cabeçalho em O(1) e conta o slot da linha excluída suavemente; a varredura
  não a mostra. A prova compara `COUNT(*)` com `COUNT(*)`, e não com a
  contagem da varredura — este passo já apanhou por isso na prova das rotinas;
* **tabela sem gatilho grava exatamente como antes**, com gatilho ligado em
  *outra* tabela — que é justamente quando o portão está verdadeiro e a
  procura acontece. Confere o dado *e a forma da resposta*, chave por chave.

## O que a medição compara

Os quatro cenários da carga fazem **o mesmo trabalho** — as mesmas 5.000
linhas, o mesmo formato de tabela, os mesmos dois índices — e mudam **só o
gatilho**. Cada rodada usa uma tabela **nova**, para nenhum cenário herdar a
árvore quente do anterior, e as rodadas são **intercaladas**: medir em bloco
põe toda a deriva da máquina dentro de um cenário e a chama de custo dele —
foi assim que a primeira medição do custo do portão dos gatilhos inventou um
fantasma de 20%.

E a diferença entre cenários só vira número quando é **maior que o
espalhamento dentro de um cenário sozinho**. O script imprime a régua ao lado
e diz «NAO aparece acima do ruido» quando é o caso, em vez de um número bonito.

A segunda medição (`6b`) é a hipótese do próprio `uuid.rs`, que nunca tinha
sido medida aqui: **v7 contra v4 contra `Sequence`**, mesma carga, mesma
tabela, mesma quantidade de linhas.

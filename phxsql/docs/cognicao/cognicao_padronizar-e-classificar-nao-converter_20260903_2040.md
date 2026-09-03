# «Todas as X são Y» se cumpre CLASSIFICANDO, e a conta que fecha é a das dispensas

## 1. O que aconteceu

O pedido 158 é uma frase do dono: *«todas as `table` são PhxGrid com
agrupamento dinâmico»*. Ele estava «parcial» com o número **24 tabelas à mão**
anotado ao lado, e a tarefa aparente era converter as 24.

Li as 24 uma a uma antes de tocar em qualquer código. **Quatro** eram lista de
dado — o Profiler, as transações abertas, o resultado de consulta da tela da
Claude, e o ajudante `tabela()` que morreu junto com o último chamador. As
outras **vinte** eram formulário com `input` por célula, ficha técnica
`campo → valor → o que faz`, prévia com reticências, um passo de assistente, o
próprio pivot — e uma que não é tabela nenhuma.

A catraca fechou em **zero**: nenhuma tabela à mão **sem motivo**.

## 2. O que eu concluí primeiro, e estava errado

Duas coisas.

**A primeira:** que o item estava «24 de 24 por fazer», porque era esse o
número anotado. Errado — o número mede *ocorrências*, não *trabalho pendente*.
Vinte delas nunca deveriam virar grade, e o pedido não estava 0% cumprido: ele
estava **por classificar**, que é outro estado e ninguém o tinha nomeado.

**A segunda, e essa é a que me pegou de verdade:** achei que a linha 2425 —
a declaração do próprio ajudante `tabela()` — fosse **defeito de contagem**, a
régua contando a régua. Ia escrever isso como achado. Fui ler o módulo antes, e
o cabeçalho dele já dizia o contrário, com todas as letras: *«o dia em que o
último chamador sair, o ajudante vira código morto e sai junto — e a ocorrência
dele desce sozinha»*. Era **deliberado**, e a minha «descoberta» teria
contradito uma decisão registrada quinze linhas acima de onde eu estava olhando.

## 3. O que a medição disse

| | antes | depois |
|---|---:|---:|
| `PhxGrid.criar(` | 51 | **55** |
| à mão, marcação crua | 23 | **0** |
| à mão, pelo ajudante | 1 | **0** |
| dispensas com motivo | 4 | **24** |
| catraca | 24 | **0** |

Quatro conversões, vinte dispensas — e a soma é 24, que era o número inteiro.
**A conta que fecha um «todas as X são Y» não é a das conversões: é a das
conversões MAIS as dispensas.**

E três coisas que só apareceram fazendo, nenhuma delas visível na leitura:

1. **`classeDaLinha` não existia.** Eu a chamei na configuração da grade
   achando que existia. A grade teria ignorado a opção **em silêncio**, e o
   realce vermelho do pedido que falhou — o motivo de um Profiler existir —
   sumiria sem erro nenhum. Achei lendo o contrato da grade, não o meu código.
2. **`profLinhas` era reatribuído** (`= []`) em três lugares. A grade guarda a
   *referência* do array; trocar a referência a deixa pintando o array velho
   para sempre. O comentário do `painelDaProva` já avisava disso — a armadilha
   estava documentada e mesmo assim eu ia cair nela.
3. **A régua tem um falso positivo, e é um só**: a palavra `<table>` dentro do
   texto que explica a importação de HTML. Conferido linha a linha nas 20.

E o quarto, que veio de graça por estar ao lado: a tela ainda listava *«ler o
que a própria transação escreveu»* entre o que **não** existe — a **terceira**
cópia da frase que o pedido 162 tornou falsa, depois das duas que eu tinha
consertado horas antes.

## 4. A regra

**Ordem do tipo «todas as X são Y» se cumpre classificando cada X, e o
entregável é a lista das dispensas com o motivo — não o número de conversões.**
Converter o que não é Y é estrago com cara de padronização.

E o corolário: **antes de chamar uma contagem de «defeito da régua», leia o
cabeçalho da régua.** O que parece bug de medição costuma ser decisão escrita
por quem mediu antes.

## 5. Como está guardado hoje

- `ISENTAS` no `conferidor_grades.rs`, 24 linhas com o motivo de cada uma, e
  `nenhuma_isencao_morta` reprovando a que deixar de corresponder.
- `TETO_TABELA_NA_MAO = 0`, com prova real nos dois sentidos (uma tela crua
  reposta reprova em 1).
- `docs/GRADE.md` §8.1, com a tabela das quatro e das vinte por natureza.

**O buraco que fica nomeado:** a guarda de piso do teste foi aposentada, porque
em zero ela virava `>= 0` e o clippy a reprovou. Ela existia para forçar a
catraca a descer junto da conversão. Em zero não há para onde descer, então não
falta nada hoje — mas se um dia a régua passar a medir mais e nascer uma catraca
nova num número alto, **a nova precisa do piso de volta**. Está escrito no
próprio teste, ao lado da aposentadoria.

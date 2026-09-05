# Leitura inconsistente pode ser o banco, e não o leitor — e a distribuição diz qual

- **Quando:** 2026-09-05, 03:05
- **Onde:** `bancada/acid/prova.py`, letra I; o parágrafo do `docs/ACID.md` §4.3
- **Custo:** zero, porque a conta das quatro frequências fechou antes de eu
  escrever; teria sido um defeito **inventado** no `varrer` e uma frente aberta
  para consertá-lo

## O que aconteceu

Consertada a corrida vazia (a cognição das 02:52), a matriz da leitura
consistente saiu **ao contrário** do que eu previa:

| o leitor pergunta | escritor **sem** transação | escritor **em** transação |
|---|---:|---:|
| **uma** instrução (`varrer`) | 98 de 400 | 0 de 400 |
| **duas** instruções (`ler`+`ler`) | 0 de 400 | 48 de 400 |

Eu esperava o inverso em toda a linha de cima: uma instrução só, servida por
uma única tomada da ficha compartilhada, **não deveria** ver a soma quebrada.

## O que eu concluí primeiro, e estava errado

Concluí que **o `varrer` estava rasgando a leitura** — que a ficha
compartilhada da onda 1 não cobria a varredura inteira, e que havia ali um
defeito de concorrência para nomear e abrir como pedido. Fui ao `op_varrer` já
convencido, e ele desmentiu na primeira leitura: a ficha é tomada **uma vez**,
num escopo que morre depois de a página inteira ser montada.

O erro estava na pergunta. Eu li «a leitura veio inconsistente» como «o leitor
montou uma resposta inconsistente», e as duas coisas não são a mesma: **o banco
também pode estar inconsistente naquele instante**, e aí a resposta certa é
exatamente a que veio.

## O que a medição disse

Contei os **estados**, e não as quebras. O escritor solto faz `ler`, `ler`,
`grava x`, `grava y` — quatro idas e voltas, e o ciclo passa por quatro
estados. Em 400 amostras:

| estado | quantas vezes o `varrer` viu | quanto o ciclo dura nele |
|---|---:|---|
| `(50,50)` — em acordo | 150 | 3 idas e voltas |
| `(49,50)` — no meio da transferência | 50 | 1 |
| `(49,51)` — em acordo | 151 | 3 |
| `(50,51)` — no meio da transferência | 49 | 1 |

**3:1:3:1**, e o medido foi 150:50:151:49. O `varrer` estava amostrando o tempo
uniformemente e reportando **corretamente** um banco que passa 25% do tempo
inconsistente — porque o escritor, sem transação, o deixa assim entre as duas
gravações. Não havia leitura rasgada: havia leitura exata de um estado feio.

E a prova de que a explicação está certa é a coluna ao lado: **com o escritor
em transação, a mesma varredura, na mesma tabela, viu o estado intermediário
zero vez em 400**. A única diferença entre as duas colunas é a transação.

A célula de baixo à direita fecha o quadro por outro caminho: **48 quebras com
o escritor em transação**. O `COMMIT` é atômico, mas ele acontece *inteiro*
entre a primeira leitura do par e a segunda — que é a leitura repetível que não
existe, medida sobre um invariante em vez de sobre uma linha só.

## A regra

**Antes de chamar de defeito do leitor, conte os estados e compare com quanto o
escritor passa em cada um.** Se a frequência bate com a duração, o leitor está
certo e quem está inconsistente é o banco — e a diferença entre os dois
diagnósticos é uma frente inteira de conserto que não precisava existir.

## Como está guardado hoje

A matriz das quatro células está no `docs/ACID.md` §4.3, gerada de
`bancada/acid/resultado.json`, com o parágrafo dizendo que a linha de cima é o
que a transação **compra** e a de baixo é o que **falta**. A afirmação
`a_transacao_conserta_a_leitura_de_uma_instrucao` trava a célula que virou
zero, e o controle escrito ao lado dela é o 98 da coluna vizinha, para que
ninguém a leia como um zero solto.

A contagem por estado que fechou a conta **ficou no repositório**, e essa foi a
segunda decisão desta cognição: ela nasceu numa sonda de mão, e sonda de mão
morre com a sessão. Hoje `transferir()` guarda a distribuição inteira em
`estados_vistos_por_uma_instrucao`, e o documento a imprime ao lado da matriz —
a corrida seguinte saiu `(49,51)` 154 · `(50,50)` 149 · `(49,50)` 49 ·
`(50,51)` 48, que é 3:1:3:1 de novo.

**Onde o buraco fica:** a conta 3:1:3:1 é lida por quem olha, e **não** por uma
afirmação da bancada — não há guarda reprovando uma distribuição que deixe de
bater. Enquanto não houver, uma leitura de fato rasgada e um banco de fato
inconsistente continuam distinguíveis só por alguém que faça a divisão.

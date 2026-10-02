# A evidência de botão é por CHAVE: clicar um sítio dá por provados todos os que dividem o gancho

**Estado:** PENDENTE

## O que aconteceu

Pedido 190, rodada de 02/10/2026. O conferidor listava **90** botões sem prova
em 321. Agrupados pela chave que a evidência grava, eram **80 chaves**: dez
linhas eram repetição (`#btVoltarGer` em 5 sítios, `[data-t]` em 3,
`[data-op]`, `[data-db]`, `#btVoltaMot` e `#btSemear` em 2 cada). Os casos
39–46 cobriram as 80 chaves, e a catraca `TETO_BOTAO_SEM_PROVA` pôde descer a
zero — mas um zero **por chave** não é um zero **por sítio**.

Os sítios que dividem chave com outro sítio e que esta máquina **não alcança**
são os que o número não mostra: `#btVoltarGer` do «Reparar tabela» (a recusa
por falta de espelho `.bkp` impede o resultado de abrir), `[data-t]` da lista
de tabelas do DbLink (precisa de um MySQL/MariaDB de verdade).

## O que eu concluí primeiro, e estava errado

Que «90 botões sem prova» queria dizer 90 casos a escrever, e que o número
chegando a zero era a medida de «todos os botões foram clicados». São duas
perguntas diferentes: `botoes-exercitados.txt` guarda **ganchos** (`#id`,
`[data-x]`, `.classe`) vistos sob o clique, e o conferidor dá por provado todo
botão cuja chave está lá — não importa em qual tela o clique caiu.

## O que a medição disse

`cargo run --example botoes-sem-prova -p phxsql-server`: 90 linhas → 80 chaves
distintas (`sort | uniq` do relatório). Quando a mesma chave mora em duas
formas da mesma tela (a tela de partições tem a forma de arquivo único e a
paginada, ambas com `#btVoltarGer`), um clique na primeira já zera as duas —
o caso 39 passou a clicar as duas **de propósito**, porque o conferidor não
cobraria a segunda.

## A regra

Ao fechar uma chave, **liste os sítios que a dividem** e exercite cada um que
for alcançável; o que não for entra no texto do caso e no documento da área,
dito como lacuna — a catraca não vai dizê-lo.

## Como está guardado hoje

Os dois sítios de `#btVoltarGer` (partições em arquivo único e paginada), os
dois de `#btVoltaMot` (lista e recusa a quem não administra) e os dois de
`#btSemear` (tabela ausente e tabela presente) têm clique próprio no caso 39 e
no 46. **Não está guardado:** `#btVoltarGer` do reparo (sem espelho na
bateria) e `[data-t]` do DbLink (sem banco de fora). O conferidor continua
contando por chave; uma contagem por sítio exigiria a evidência gravar a tela
de origem do clique, e isso não foi feito.

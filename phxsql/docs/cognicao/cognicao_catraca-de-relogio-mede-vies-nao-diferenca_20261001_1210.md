# Catraca de relógio mede o VIÉS, não quantos pares diferem

**Estado:** PENDENTE

## O que aconteceu

Pedido 445, achado B3 da revisão SEC de 434/435: a guarda do relógio do
`o_pulso_nao_diz_quais_nos_tem_pino` (`crates/phxsql-server/tests/identidade-do-pulso.rs`)
aceitava que o `ms` da resposta separasse o nó com pino do sem pino em até
**34 de 40** sondas. A revisão mostrou que uma volta parcial a 30/40 já
classifica com três sondas e passava verde. Apertei o teto para os mesmos 10
do teste irmão do ramo sem prova.

## O que eu concluí primeiro, e estava errado

Que o teto frouxo era só descuido, e que 10 «diferentes em 40» bastava, porque
o medido com o conserto era 0 e 1 em 40. Na bateria inteira do binário
(`--test identidade-do-pulso`, 4 threads), o teste caiu com **11 de 40** numa
de cinco corridas: o `ms` é arredondado, e com o trabalho perto da fronteira
do milissegundo ele vira de um lado para o outro sob carga — para **os dois
lados**. A contagem de pares diferentes mede ruído junto com mapa.

## O que a medição disse

- Teto 10 sobre «pares diferentes»: 1 vermelho em 5 corridas da bateria
  (11/40), 0 em 4 corridas isoladas.
- Teto 10 sobre o **viés** `|noB mais lento − noC mais lento|`: 8 de 8
  corridas da bateria verdes. O defeito do 435 dá viés 40 (o lado com pino
  sempre acima); a volta parcial que o B3 descreve dá 30; o ruído simétrico
  fica perto de zero mesmo com onze pares diferentes.

## A regra

Catraca de canal lateral por tempo conta o que um classificador usaria — o
viés para um lado —, e não quantas amostras diferem.

## Como está guardado hoje

`vies_do_relogio` no `identidade-do-pulso.rs`, usado pelos dois testes do
relógio (com e sem prova), com `LIMITE_DO_RELOGIO_SEM_PROVA = 10`. As guardas
`erro-do-pulso-mapeia-quem-nao-tem-pino` e `resposta-sem-prova-assina-e-esconde`
repõem o defeito de 40/40 contra a régua nova. **Buraco:** nenhuma guarda
repõe uma volta PARCIAL (30/40) — o número dela é dedução, não medida.

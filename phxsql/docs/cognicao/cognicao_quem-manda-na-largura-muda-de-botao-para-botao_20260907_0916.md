# Quem manda na largura muda de botão para botão

**07/09/2026, 09:16 UTC.**

## 1. O que aconteceu

Pedido do dono: *«os botões da Barra podem ficar menores na horizontal pelo
menos 10%»*. A barra é `#ferramentas`, e os botões `.fer` declaram
`min-width:62px` e `padding:5px 9px 4px`.

Medido no navegador **antes** de tocar no CSS, em 1440 px: **23 botões,
1.476,52 px somados, média 64,2 px, máxima 80,03 px**. E o número que decidiu o
conserto foi outro: **15 dos 23 estavam exatamente em 62 px.**

## 2. O que eu concluí primeiro, e estava errado

**Duas vezes**, e as duas caíram por medição.

**A primeira:** que baixar o `min-width` resolveria. Ele manda em 15 botões —
mas nos outros 8 quem manda é o **rótulo**: «Diagrama ER» media 80,03 px porque
o texto pede isso, e nenhum `min-width` menor o move. O simétrico também é
falso: mexer só no `padding` não move os 15, porque neles o `min-width` já é
maior que conteúdo + padding. **Uma propriedade só conserta metade da barra**, e
qual metade depende do rótulo de cada botão — coisa que não se lê no CSS.

**A segunda:** que a regra do topo da folha valia para todas as larguras.
Baixei `min-width` e `padding`, medi de novo, e abaixo de 1025 px a soma mudou
**0,0%** — idêntica ao pixel. Havia uma `@media (max-width:1024px)`
reescrevendo as duas propriedades, e o conserto nunca chegava lá.

Essa segunda é a pétrea do **irmão** de novo, e o alcance novo dela é o que
vale registrar: aqui o irmão **não é uma função** — a lei diz «irmão é quem
chama as mesmas funções na mesma ordem», e uma media query não chama nada. Em
folha de estilo o irmão é **quem redeclara a mesma propriedade**, e ele ganha
por vir depois. A lei escrita não cobria este caso; quem o achou foi o 0,0%.

## 3. O que a medição disse

| | antes | depois | |
|---|---:|---:|---|
| soma das larguras (1440 px) | 1.476,52 px | **1.293,56 px** | −12,4% |
| média | 64,20 px | **56,24 px** | −12,4% |
| máxima («Diagrama ER») | 80,03 px | **69,83 px** | −12,7% |
| pior botão, ≥1025 px | | | **−11,3%** |
| pior botão, ≤1024 px | | | **−10,7%** |
| rótulos cortados | 0 | **0** | |

Todos os 23, em todas as 10 larguras medidas: **≥10%**. Nenhuma ferramenta
inalcançável, nenhuma página rolando de lado.

E um ganho que o pedido não pedia, achado pela mesma corrida: em **1440 px a
barra caiu de 2 fileiras para 1** — 102 px de altura para 56. «Diretivas» e
«Repair» transbordavam para uma segunda fileira, e agora cabem.

O conserto foi em três peças, e nenhuma sozinha bastava: `min-width` 62→55,
`padding` horizontal 9→5, e `letter-spacing:-.02em` **no rótulo** — tracking, e
não fonte menor, porque baixar a fonte custaria legibilidade nos 23 para
consertar 8. No irmão de ≤1024 px, `min-width` 56→50 e `padding` 7→4.

## 4. A regra

**Antes de encolher um componente, meça QUEM manda no tamanho dele — pode ser
outro em cada instância.** `min-width` e conteúdo disputam, e o vencedor muda
de elemento para elemento; mexer no perdedor não faz nada e parece que fez.

E o alcance novo da lei do irmão: **em folha de estilo, irmão é quem redeclara
a mesma propriedade — a media query é irmã da regra do topo.**

## 5. Como está guardado hoje

- `testes-web/barra-em-resolucao-menor.mjs` agora mede **largura por botão**,
  soma, média, máxima e **rótulo cortado** (`scrollWidth > clientWidth` do
  `.rot`) — este último existe para reprovar a saída barata: encolher a caixa
  comendo o texto, que é botão mentindo sobre o que faz.
- O motivo dos três números está no comentário acima da própria regra, com o
  «medido ANTES de mexer» e os 15 de 23.
- **Onde o buraco fica:** não há catraca que trave a largura. Alguém que
  devolva `min-width:62px` amanhã não é reprovado por ninguém — o medidor
  **mede**, mas não é chamado pela suíte, e a comparação com o retrato anterior
  foi feita à mão nesta rodada, com dois JSON lado a lado.

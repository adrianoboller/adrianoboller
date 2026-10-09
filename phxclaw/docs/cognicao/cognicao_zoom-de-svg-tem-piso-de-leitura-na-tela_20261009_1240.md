# Zoom de SVG tem piso de leitura NA TELA, não no CSS

**Estado:** FRUTÍFERO

**Evidência:** `tests/desktop/ui_fluxos.mjs`, checagem «legivel ao abrir: o menor texto do no tem
>= 12 px na tela» (fonte computada × zoom). Cópia da UI abrindo o fluxo pelo AJUSTAR (o primeiro
desenho): **6,96 px** → reprova (46/47 com a outra checagem de zoom). Abrindo em 100%: 12 px →
47/47. E a irmã do mesmo erro: vão entre camadas de 60 px punha «verdadeiro» debaixo da aba do nó
seguinte → a checagem das portas reprova; com 96 px passa.

## O que aconteceu

A tela Fluxos abria o grafo «cabendo no quadro». A 1280 px, com a lista e o painel ao lado, o
quadro tinha 495 px para 1.450 px de grafo: zoom de 35%, e o texto do nó (12 e 13 px no CSS) saía
a 4 px. A captura mostrou; o CSS dizia 12 px e a sonda de tipografia (G11) leria 12 px.

## O que eu concluí primeiro, e estava errado

Que «ajustar ao quadro» era o padrão certo de editor de grafo (é o do n8n) e que o piso de 12 px
estava garantido porque a folha só usa a escala `--fs-*`. O piso vale para o que chega ao olho:
dentro de um `viewBox` escalado, o tamanho da fonte é multiplicado pelo zoom, e nenhuma régua que
lê `font-size` vê isso.

## A regra

Texto dentro de SVG com zoom se mede como `font-size × escala`, e a abertura padrão respeita o
piso; ver o todo é escolha (AJUSTAR, minimapa), não o padrão. Rótulo posto no vão entre formas
mede o vão contra o maior rótulo, não contra o comum.

## Como está guardado hoje

`fluxos.js` abre em `k = 1`; `GX = 96` com o motivo no comentário; as duas checagens no
`ui_fluxos.mjs` (legibilidade ao abrir; portas sem sobreposição e fora de nó), as duas com RED
medido.

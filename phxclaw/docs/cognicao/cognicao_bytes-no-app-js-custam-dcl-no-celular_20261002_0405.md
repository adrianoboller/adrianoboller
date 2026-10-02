# Cada KB no app.js custa DOMContentLoaded no celular: +11 KB foram +270 ms

**Estado:** FRUTÍFERO

**Evidência:** sonda M5 de `tests/desktop/qualificacao/qualificar.mjs` (CPU 4×, 4G lenta, mediana de 3): HEAD 1.440 ms; app.js com o IDE web dentro 1.714 ms; HEAD + 11 KB de **comentário** 1.750 ms; IDE em `assets/ide.js` com `<script async>` 1.415 ms.

## O que aconteceu

O adaptador websocket, o espelho acessível e a trilha entraram no `app.js` (+11 KB) e a
Visão geral no celular passou de 1.440 para 1.714 ms de DCL — acima do teto 1.600 da
sonda M5, que é «médio» e não derruba o veredito, mas é regressão medida.

## O que eu concluí primeiro, e estava errado

Que algum código novo rodava na carga (o adaptador, o `aoTrocar` do espelho). Desliguei
os dois em cópias: 1.700 e 1.745 ms. Não era execução.

## O que a medição disse

HEAD + 11 KB de comentário puro deu 1.750 ms: o custo é **bytes no caminho do DCL** sob a
rede emulada, não o que o código faz. Mover o módulo inteiro do IDE (25 KB) para um
`<script async>` armado no DOMContentLoaded deixou o DCL em 1.415 ms — melhor que HEAD.

## A regra

Tela que não é de celular não entra no `app.js`: vai num arquivo próprio, `<script async>`
depois dos clássicos, armado no DOMContentLoaded (acha `txt`, `el`, `carregadores`). O
conferidor da fábrica continua lendo o arquivo (ele varre os `<script src>` do index.html),
e o `sw.js` precisa listá-lo na casca.

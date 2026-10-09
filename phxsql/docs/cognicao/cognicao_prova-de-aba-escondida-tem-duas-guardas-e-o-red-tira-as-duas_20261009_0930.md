# A prova de «aba escondida faz 0 pedidos» tem duas guardas, e o RED tem de tirar as duas

**Estado:** PENDENTE

## O que aconteceu

Fatia A10 do pedido 707 (`crates/phxsql-server/ui/aquario.js`, `tela()`). O laço de pedidos do
aquário tem DUAS guardas contra a aba escondida: o ouvinte de `visibilitychange`, que limpa o
relógio na hora em que a aba some, e o `!document.hidden` dentro de `ativo()`, que impede a volta
em curso de reagendar. A prova (`testes-web/prova-707-aquario.mjs`, caso «aba escondida faz 0
pedidos») conta os pedidos `aquario_*` em 6 s com a aba escondida.

## O que eu concluí primeiro, e estava errado

Que o defeito reposto era tirar o `!document.hidden` do `ativo()`. Com só ele fora, a prova
**passou**: 0 pedidos em 6 s. A prova não estava errada; o RED estava pela metade. O ouvinte
sozinho já mata o relógio quando nenhum pedido está no ar no instante do esconder, e no Chromium
sem cabeça isso é quase sempre.

## O que a medição disse

- só `ativo()` sem a guarda: **0 pedidos em 6 s**, a prova passa (RED que não reprova);
- ouvinte e guarda fora: **5 pedidos em 6 s**, a prova reprova;
- as duas no lugar: **0 em 6 s**, e **3–4 em 3 s** ao voltar à vista.

E o Chromium sem cabeça **não esconde** a aba de trás: com outra página trazida à frente, o
`document.visibilityState` da primeira continua `visible`. A prova forja o estado pelo próprio
documento e dispara o mesmo evento, e diz isso na linha do resultado.

## A regra

Quando um comportamento tem duas guardas, o RED tira as duas — e cada uma isolada é outro RED,
que pode não ter teste capaz de separá-la.

## Como está guardado hoje

O RED das duas juntas está registrado no relatório da A10; o da guarda de `ativo()` sozinha
**não tem prova que o separe** (ela só importa quando a aba some com um pedido no ar, uma janela
de milissegundos que o teste não alcança sem atrasar o servidor de propósito). A aba escondida de
verdade (não forjada) fica sem prova automática enquanto o Playwright sem cabeça não a esconder.

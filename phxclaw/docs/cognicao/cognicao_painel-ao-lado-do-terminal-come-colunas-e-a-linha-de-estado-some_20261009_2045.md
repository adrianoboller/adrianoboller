# Painel novo ao lado do terminal come colunas, e com elas a linha de estado do Helix

**Estado:** PENDENTE (a prova roda na árvore; falta o commit)

**Evidência:** `tests/desktop/ide_dobra.mjs` em 1280 px, primeira corrida: 34/35. O status do
IDE dizia `Helix • 5×27`; a captura `ide_dobra_1280_depurar.png` mostrava o terminal com 5
colunas, espremido por quatro painéis lado a lado (minimapa, leitura com dobra, compartilhar,
testes). Depois de empilhar os dobráveis numa coluna (`.ide-lado`): 52 colunas e 35/35.

## O que aconteceu

O `:open conf.yaml` chegou ao Helix (a linha do cursor mostrava `a:`), mas a trilha, o
minimapa e a leitura continuaram em `src/main.rs`. Com 5 colunas a linha de estado
(`NOR   conf.yaml …`) não cabe, e o `ide.js` lê o arquivo aberto SÓ dela — sem linha de
estado, ele entende «linha coberta pelo menu do `:`» e não muda nada.

## O que eu concluí primeiro, e estava errado

Que o teclado tinha perdido o foco do canvas depois das teclas no painel (Esc/`:` indo para
outro lugar). O espelho do leitor de tela desmentiu: o Helix tinha aberto o arquivo.

## A regra

Todo painel novo na `.ide-corpo` tira largura do terminal, e o IDE inteiro depende de a linha
de estado do Helix caber. `details` fechado não é largura zero: o `summary` com letra espaçada
pede ~200 px. Painel novo entra na coluna `.ide-lado`, e o roteiro mede em 1280 px que o Helix
continua com a linha de estado.

## Como está guardado hoje

`apps/phxclaw-ui/index.html` (`.ide-lado`, com o motivo no comentário) e o
`tests/desktop/ide_dobra.mjs`, que em 1280 px troca de arquivo pelo Helix e confere a leitura
acompanhando.

# `overflow-x:auto` recorta o menu que desce — o alcance do «CSS global morde»

## O que aconteceu

Exercitando o conjunto de ícones novo (papel E, 24/09/2026), a captura do menu
**Tabelas** saiu com o título aceso e **nenhuma lista**. A lista estava no DOM,
com `display:block`, 238 × 559 px — e `elementFromPoint` no meio dela devolvia
um `BUTTON.fer` da barra de ferramentas.

A causa, em `ui/index.html`: a seção «o que vale em toda tela» deu à barra
`.menubar{overflow-x:auto;overflow-y:hidden}` para ela rolar em tela estreita
(`git log -S` aponta `a671f2d4`, 23/09/2026). A lista de cada menu era
`position:absolute` **filha da barra**, e a barra é `position:relative`: ficou
recortada na altura dela (30 px). **Todo menu do alto abria invisível, em toda
largura de tela, desde 23/09.**

## O que eu concluí primeiro, e estava errado

Que era `z-index`: a barra de ferramentas «ganhando» da barra de menu. Os
números desmentiram — `.menubar` tem `z-index:60` e `#ferramentas` não tem
nenhum. Não era empilhamento; era recorte. Trocar `z-index` não teria mudado
nada, e o próximo teria subido o número de novo.

## O que a medição disse

- `getComputedStyle(lista).display` = `block`; retângulo 238 × 559 px.
- `elementFromPoint(centro da lista)` = `#ferramentas .fer` — o que desenha ali
  é o vizinho, não a lista.
- A regra do CSS que fecha a conta: se um eixo de `overflow` não é `visible`, o
  outro **não pode** ser `visible` (vira `auto`). `overflow-x:auto` sozinho já
  recortaria.
- Com a lista em `position:fixed` (posta pelo `abrirMenu` debaixo do título), o
  mesmo `elementFromPoint` devolve um item da lista.

## A regra

**Quem dá `overflow` a um contêiner procura os filhos `position:absolute` que
saem dele** — menu, popover, dica — e os passa para `fixed` (ou os tira de
dentro). Rolagem horizontal e menu que desce não moram no mesmo contêiner.

## Como está guardado hoje

- O conserto: `.menubar .lista{position:fixed}` + posição no `abrirMenu`, e a
  lista fecha quando a barra rola ou a janela muda de tamanho.
- A prova: `testes-web/casos/34-impressao-e-icones.mjs` pergunta **quem está no
  topo no meio da lista**. Com o defeito reposto (lista `absolute` de novo) ela
  responde `false` — medido.
- **O buraco:** antes do caso 34, nenhum caso da bateria perguntava se a
  lista é VISTA — acionar o item por código ou por clique não mede isso. Teste que aciona sem olhar não prova que
  a pessoa vê.

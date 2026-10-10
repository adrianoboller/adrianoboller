# `width:auto` não segura um `<select>` filho de flex em coluna

**Estado:** PENDENTE

## O que aconteceu

Pedido 783, editor do perfil do aquário (`ui/aquario.js`, `.aqe`). A lista de letras era um
`<select class="aqe-fonte">` dentro de `.aqe-grupo{display:flex;flex-direction:column}`, com
`.aqe select.aqe-fonte{width:auto;min-width:200px}` justamente para desfazer o
`input,select{width:100%}` global do `index.html`. Na captura, o seletor ocupava a coluna
inteira: **1.288 px** a 1600 de largura e **588 px** a 900.

## O que eu concluí primeiro, e estava errado

Que a regra global ganhava a minha — especificidade, ou ordem das folhas. Medido pelo CSSOM, a
minha regra **casava e valia**: `.aqe select.aqe-fonte -> auto` acima de `input, select -> 100%`.
Não era a folha global que mordia.

## O que a medição disse

Quem esticava era o **contêiner**: num flex em coluna o `align-items` padrão é `stretch`, e o
item esticado ganha a largura da coluna mesmo com `width:auto`. Com `align-self:flex-start` no
seletor, ele volta à largura do conteúdo. O `getComputedStyle(e).width` dizia `1288px`, e só a
lista das regras que casavam mostrou que nenhuma delas pedia isso.

## A regra

Quando um controle sai largo demais, liste as regras que **casam** com ele antes de culpar a
folha global: se nenhuma pede a largura, quem a deu foi o pai (`stretch` do flex ou do grid).

## Como está guardado hoje

O caso `perfil-do-aquario` da bateria (`testes-web/casos/53-perfil-do-aquario.mjs`) confere a
largura do seletor de cor e a do `<select>` de letra. O RED desta conferência (repor o
`align-self` e ver o caso reprovar) **não foi medido** — por isso PENDENTE.

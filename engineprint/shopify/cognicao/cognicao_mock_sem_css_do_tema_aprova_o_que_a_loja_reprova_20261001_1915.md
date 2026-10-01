# O mock sem o CSS do tema aprova o que a loja reprova

**Descoberto em** 01/10/2026, 19:15. **Onde:** link "Verificar estoque" na
pagina de produto (`sections/product-whatsapp.liquid`), prova no preview do
tema 20.

## O que aconteceu

O dono pediu que o link nao trocasse de cor ao passar o mouse nem ao
clicar — ele via laranja. Escrevi `color: inherit` em todos os estados com
tres classes de especificidade, rodei a prova no mock: 36 de 36. Subi para
o tema 20 e rodei a mesma prova no preview da loja: **4 falhas** — a cor
computada no hover e com o mouse pressionado era `rgb(255, 138, 31)`, o
laranja `--color-primary-hover` do tema, a 390 e a 1280 px.

## O que eu conclui primeiro, e estava errado

1. **Que o laranja vinha de `p > a:hover { --button-color: ... }`.** Essa
   regra existe no `base.css`, tem especificidade (0,1,2) e eu a "vencia" com
   folga; o comentario no CSS ja dizia que estava resolvido. Nao era ela.
2. **Que o mock provava a cor.** O `render.mjs` monta a pagina so com a
   secao e um `<head>` minimo; o `base.css` do tema nao entra. Toda regra
   global do tema e invisivel ali — o mock so sabe provar o que a secao faz
   sozinha. Trinta e seis "ok" sobre cor eram trinta e seis medicoes de um
   ambiente que nao tinha o adversario.

## O que a medicao disse

Perguntado ao Chromium (CDP `getMatchedStylesForNode` com `:hover` forcado)
no preview do tema 20, a ultima regra a casar com o link era do tema:

    :is(p:not(.h1,.h2,.h3,.h4,.h5,.h6) a:where(:not(.button,.button-secondary)),
        .rte :is(p,ul,ol,table):not(.h1,.h2,.h3,.h4,.h5,.h6) a:where(:not(.button,.button-secondary))):hover
    { color: var(--color-primary-hover) }

Especificidade: o `:is()` vale pelo ramo mais pesado, `.rte` (0,1,0) +
`:is(p,...)` (0,0,1) + `:not(.h1,...)` (0,1,0) + `a` (0,0,1) = (0,2,2), mais
`:hover` = **(0,3,2)**. A minha era (0,3,0). Perde por dois elementos.

Conserto sem disputar: dentro do link, `--color-primary`,
`--color-primary-hover` e `--button-color` passam a valer `currentColor`
(o tema nunca define essas variaveis no proprio `a`, so as le), e
`currentColor` na propriedade `color` e herdar. Prova no preview depois do
conserto: 34 de 34, cor `rgb(29, 122, 58)` em repouso, no hover e no clique.

## A regra

Cor, tamanho ou posicao de um componente so se prova onde o CSS global do
tema esta carregado: o mock sem `base.css` prova a logica da secao, nunca o
que o tema faz com ela. E quando uma regra do tema vence por especificidade,
neutralize a variavel que ela le em vez de empilhar classes — a proxima
regra do tema volta a vencer, a variavel nao.

## Como esta guardado hoje

- `mock/prova-estoque.mjs --tema <id> <url>` roda as mesmas conferencias no
  preview da loja; o LEIA-ME manda rodar as duas, e a da loja e a que vale
  para cor.
- O comentario no CSS da secao diz o seletor real e a especificidade medida.
- **Buraco:** o mock continua sem o `base.css`; qualquer prova de cor so no
  mock vai aprovar de novo o que a loja reprova. Carregar o `base.css` no
  mock exigiria tambem as variaveis do `theme-styles-variables.liquid`, e
  isso nao foi feito.

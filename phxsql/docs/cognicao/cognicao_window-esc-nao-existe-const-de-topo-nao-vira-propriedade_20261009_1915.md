# `window.esc` não existe: `const` de topo não vira propriedade de `window`

**Estado:** PENDENTE

## O que aconteceu

Pedido 771, medindo os 220 `innerHTML`/`insertAdjacentHTML` da `ui/` com um
analisador de sintaxe (`testes-web/medir-innerhtml.mjs`). O `claude.js` escapava
tudo por um ajudante próprio:

```js
const E = s => (window.esc ? esc(s) : String(s));
```

e o `esc` da página é `const esc = t => …` no topo do script do `index.html`.
`const`/`let` de topo moram no escopo global **léxico** e nunca viram
propriedade de `window` (só `var` e `function` viram). Então `window.esc` era
sempre `undefined`, e o `E` devolvia `String(s)` — o texto **cru** — na página
servida, em 102 interpolações `${E(…)}` do módulo da Claude: nomes de tabela e
coluna propostos pelo modelo, mensagens de erro da API, o endereço da API.

O comentário do lado dizia que o degrau existia para o módulo «se exercitar sem
a página em volta». Na página, ele era o caminho de sempre.

## O que eu concluí primeiro, e estava errado

Que o `E` era um escapador com degrau de reserva, e portanto seguro — o
analisador o marcou como suspeito só porque o degrau `String(s)` não escapa, e
a primeira leitura foi «falso positivo do degrau, que nunca roda». Era o
contrário: o degrau era o único que rodava. O `txt` e o `marcado` do mesmo
arquivo usam o mesmo teste `window.X` e funcionam — porque esses são
`function` de topo, que viram propriedade. A mesma forma de código, três
funções, uma quebrada pelo modo de declarar a outra ponta.

## O que a medição disse

No Chromium, com o binário da base `29009965`: `PhxIA._gravar({endpoint:
'https://exemplo.invalid/<b id="inj771">x</b>'}); PhxIA.telaConfig()` deixou
`#inj771` como **elemento** no DOM (caso `csp`, passo 4, vermelho). Com
`typeof esc === "function"` no lugar de `window.esc`, o mesmo endereço aparece
como texto e o elemento não existe (verde, nos dois temas).

## A regra

Para perguntar se um global existe, use `typeof nome`, nunca `window.nome` —
e o degrau de reserva de um escapador escapa também, nunca devolve cru.

## Como está guardado hoje

O caso `testes-web/casos/52-csp.mjs` (passo 4) cai com o `window.esc`
reposto. O analisador `testes-web/medir-innerhtml.mjs` mede os sinks, mas não
é catraca: ele não segue os `formato:` das grades (propriedade de objeto), e
os cinco que eram dado cru ali foram achados lendo, não medindo. **Buraco:**
outro `window.X` que dependa de `const` de topo não tem guarda genérica — o
varrer de hoje — todo `window.<nome>` da `ui/` contra os `const`/`let` de topo
do `index.html` — achou só este.

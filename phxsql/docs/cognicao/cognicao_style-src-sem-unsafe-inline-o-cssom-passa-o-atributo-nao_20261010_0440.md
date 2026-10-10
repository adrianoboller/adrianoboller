# `style-src` sem `'unsafe-inline'`: o CSSOM passa, o atributo e o `<style>` montado não

**Estado:** PENDENTE

## O que aconteceu

Pedido 771, segunda etapa. A página servia `style-src 'unsafe-inline'` porque
havia **97** atributos `style="…"` em modelos de HTML da `ui/` (index 55,
claude 30, telemetria 10, grade 2) e **2** `<style>` montados pelo JS com
conteúdo variável (as colunas fixas da grade e a folha do aquário). Depois:
**0** e **0**; `style-src` com **5** `'sha256-…'` (um por `<style>` embutido) e
`style-src-attr 'none'`.

## O que eu concluí primeiro, e estava errado

1. «Um `<style>` vazio criado pelo JS e preenchido por `insertRule` passa.»
   **Não passa**: medido no Chromium 1194, o `<style>` vazio é barrado (o
   navegador pede o hash de `""`, `47DEQpj8…`) e `elemento.sheet` fica `null` —
   não há folha onde inserir regra.
2. «O jeito genérico é um `data-css` com a declaração inteira, aplicado por um
   observador.» Seria o `style=` de volta com outro nome: HTML injetado voltaria
   a desenhar por cima da tela. O que ficou é **tipado** — um atributo por
   propriedade (`data-e-larg`, `data-e-fundo`, `data-e-tinta`, `data-e-parada`,
   `data-e-n`) e o valor passando por crivo (número 0–100; cor `var()`, `#hex`,
   `rgb()/hsl()` de números ou nome).
3. «A classe substitui o atributo.» Não sozinha: o atributo ganhava de toda
   regra da folha, e a classe perde para `input{width:100%}` e para qualquer
   seletor mais específico. A troca só é fiel com `!important` na classe.

## O que a medição disse

Página de sonda no Chromium da bateria, `style-src` só com o hash da folha:

| caminho | resultado |
|---|---|
| `innerHTML` com `style="…"` | barrado (`style-src-attr`) |
| `setAttribute('style', …)` | barrado |
| `<style>` criado pelo JS, com ou sem texto | barrado |
| `elemento.style.setProperty` / `.style.cssText` | aplicado |
| `new CSSStyleSheet()` + `replaceSync` + `adoptedStyleSheets` | aplicado |
| atributo de apresentação SVG (`stop-color="#f00"`) | aplicado (não é estilo) |

E um achado de brinde: a grade montava o seletor das colunas fixas com o nome
da coluna cru (`[data-campo="' + campo + '"]`) — uma aspa no nome quebrava a
folha inteira. Pelo CSSOM o nome vira chave de objeto, nunca seletor.

## A regra

Sob CSP sem `'unsafe-inline'`, estilo dinâmico entra pelo CSSOM e estilo fixo
pela folha autorizada; nunca por atributo nem por `<style>` montado.

## Como está guardado hoje

- `crates/phxsql-server/src/http.rs::a_pagina_autoriza_cada_estilo_por_hash_e_nenhum_atributo`
  e `::nenhum_atributo_style_nos_modelos_da_pagina` (lê a página inteira, não
  uma lista de arquivos);
- `testes-web/casos/52-csp.mjs`: cabeçalho, veneno `<div style>` barrado e
  relatado, crivo do `PhxEstilo` recusando `url()`, barras do Painel com a
  largura pedida;
- o vigia `securitypolicyviolation` da bateria reprova qualquer caso com
  violação de estilo. RED medido: com um `style=` reposto nas barras de disco,
  `csp` e `entrada` caem nos dois temas (`style-src-attr … :15190`).

Candidato a FRUTÍFERO quando o integrador conferir as provas acima.

# O widget novo do Judge.me nao busca nada: um div colado mostra "seja o primeiro"

**Descoberto em** 28/09/2026, 19:30. **Onde:** estrelas e widget do Judge.me
na pagina de produto da Engine Print (tema 17, `product.whatsapp.liquid`).

## O que aconteceu

O dono instalou o Judge.me e pediu estrelas ao lado do nome do produto, o
widget nos produtos e uma pagina da avaliacao geral da loja. A instalacao
automatica do app so tinha posto um div escondido do carrinho
(`jdgm-cart-drawer-source`); nada nos produtos. O modelo de produto desta loja
e `.liquid`, e bloco de app so entra em modelo JSON — entao o caminho oficial
(arrastar o bloco no editor) nao existe aqui.

Quatro armadilhas, todas vistas lendo o codigo do app e depois medidas no
navegador:

| # | armadilha | de onde vem |
|---|---|---|
| 1 | div `jdgm-review-widget` sozinho mostra "Seja o primeiro a escrever uma avaliacao" mesmo com avaliacao publicada | `review_widget_revamp_enabled = true`: o widget novo le `jdgm.data.reviewWidget[id]` (ReviewWidgetManager, `getReviewDataFromJdgm`) e so busca na API se o idioma do navegador diferir do da loja |
| 2 | selo com `data-template="product"` (ou `collection`, `index`) nunca aparece | o CSS de configuracao da loja traz `.jdgm-preview-badge[data-template="product"]{display:none !important}` porque `preview_badge_product_page_install_preference = false` |
| 3 | `display:inline-flex` + `gap` no selo nao vale | o app forca `.jdgm-prev-badge{display:block !important}` — estrelas e texto colados, fim de um e inicio do outro no mesmo pixel (1100,19) |
| 4 | `.pp__estrelas .jdgm-star{color:#111}` perde | o app pinta com `.jdgm-preview-badge .jdgm-star{color:#108474}` — mesma especificidade, carregado depois |

E na pagina geral: o cabecalho que o app grava pronto (`all_reviews_header`)
sai em ingles ("Be the first to write a review"), porque a conta do Judge.me
esta com `locale: "en"` embora todos os textos configurados estejam em
portugues; e o `all_reviews_widget_v2025` esta desligado, entao vale o widget
antigo, com o verde-azulado do app no botao e no histograma.

## O que eu conclui primeiro, e estava errado

1. **Que bastava o div com `data-id`, como no Judge.me de sempre.** Era o
   plano escrito antes desta rodada. O widget novo nao le o div: le um objeto
   que so o trecho oficial preenche, a partir de
   `product.metafields.judgeme.review_widget_data`. Com zero avaliacoes os dois
   caminhos mostram a mesma tela — o defeito so apareceria no dia da primeira
   avaliacao, calado.
2. **Que o caminho certo era converter o modelo para JSON e usar os blocos do
   app.** Nao da: `themeFilesDelete` e bloqueado, e um `.json` com o mesmo nome
   do `.liquid` nao pode coexistir; um modelo novo exigiria trocar o
   `templateSuffix` de todos os produtos depois da publicacao. O Judge.me
   publica um trecho oficial para tema com modelo em Liquid
   (help center, "Adding Judge.me widgets in Vintage themes") — e ele bate
   atributo por atributo com o que o `loader.js` da extensao procura
   (`data-entry-point="review_widget.js"`, `data-entry-key`, `data-widget`).
3. **Que as estrelas dos cartoes podiam sair do metacampo padrao
   `reviews.rating`**, sem script nenhum. Nao existe definicao `reviews.rating`
   na loja; confiar nele seria estrela que nunca aparece. No cartao ficou o
   selo oficial, mas so quando o produto ja tem `judgeme.badge` — assim colecao
   sem produto avaliado nao carrega o `widget/main.js`.

## O que a medicao disse

Prova no navegador (`mock/prova-judgeme.mjs`), tema 17 contra o 16 publicado,
390 e 1280 px — **37 verificacoes, 0 falhas** na corrida final:

- widget monta em portugues ("Avaliacoes de produtos (0) / Avaliacoes da loja
  (0) / Seja o primeiro...") e o selo some sem avaliacao (`display:none`);
- com um selo simulado de 2 avaliacoes, nota 4,5, no molde do proprio app
  (`shopify_v2/badge`), o texto vira "2 avaliacoes" e as estrelas ficam **ao
  lado do nome**: a 1280, nome em x 754–1002 e selo em 1014–1176; a 390,
  nome em 38–212 e selo em 224–330 (com "(2)" no lugar do texto — "2
  avaliacoes" nao cabia: 174 + 155 px numa linha de 314);
- estrela `rgb(17,17,17)`, folga de 6,4 px entre estrelas e texto;
- pagina geral: botao `rgb(10,10,10)`, nenhum texto em ingles visivel,
  nenhum carregador girando com zero avaliacao, botao alinhado ao placar.

Cada uma das checagens de defeito **reprovou a versao anterior antes de
passar**: cor (#108474), folga (0,0 px), botao/estrela/carregador (6
falhas), alinhamento (x 521 contra 40). E o placar com avaliacao, que a loja
ainda nao mostra, foi exercitado com liquidjs (`mock/placar-avaliacoes.mjs`):
a primeira versao escrevia "5" para 5,00 e "4" para 3,96 — `round: 1` de
numero inteiro sai "5" num motor e "5.0" no outro.

## A regra

Widget de app que troca de versao se prova pelo codigo que carrega, nao pelo
nome da classe: leia o carregador, ache de onde ele tira os dados, e confira
no navegador o estado **com** dado — o estado vazio e igual nos dois caminhos
e nao prova nada.

## Como esta guardado hoje

- `sections/product-whatsapp.liquid`: selo ao lado do `<h1>` sem
  `data-template`, widget pelo trecho oficial com `window.jdgm` defensivo,
  cores e folga com a especificidade medida.
- `blocks/_product-card.liquid` + `assets/wx-vitrine.css`: selo no cartao so
  com `judgeme.badge`.
- `sections/wx-avaliacoes.liquid` + `templates/page.avaliacoes.json`: placar
  proprio no servidor (nota por decimos), lista oficial do app, cores da loja
  so nesta pagina via as variaveis do app.
- `mock/prova-judgeme.mjs` (navegador) e `mock/placar-avaliacoes.mjs`
  (liquidjs) — os dois reprovam as versoes defeituosas.

**Buracos que ficam:** (a) o `review_widget_revamp_dual_publish_end_date` do
app e 02/10/2026 — depois disso o metacampo antigo `judgeme.widget` para de
ser gravado; o trecho ja nao depende dele, mas o `judgeme.badge` do selo pode
seguir o mesmo caminho em alguma versao futura, e ai as estrelas somem sem
erro. A prova simulada nao pegaria isso, porque injeta o selo. (b) O locale
"en" da conta do Judge.me continua la; a pagina esconde o resumo em ingles,
mas outro widget antigo que o dono ligar pode mostrar ingles.

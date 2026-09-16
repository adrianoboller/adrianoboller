# Loja Engine Print (Shopify) — seção de produto e ferramental

Fonte da seção `product-whatsapp` do tema **EnginePrint Industrial** (Horizon
3.4) e os scripts que a provaram antes de ir para a loja. Tudo em
`sections/product-whatsapp.liquid`; o resto é apoio.

## O que a revisão de 16/09/2026 mudou

- **Galeria "sleek"**: o palco virou um canvas único (`#F7F7F7`, o cinza de
  estúdio das fotos oficiais da Bambu Lab), sem borda e sem padding. Antes era
  uma caixa branca com borda de 1 px e padding de até 2,5 rem sobre página
  cinza — a foto oficial aparecia como um quadrado de outra cor dentro dela.
- **Crossfade** de ~260 ms entre imagens (os dois slides se sobrepõem e um
  dissolve no outro), swipe no celular, setas no desktop, teclado, contador e
  zoom em tela cheia. Antes a troca era `display:none → grid` (corte seco).
- **Conversão**: variantes como cartões com preço Pix, linha de estoque a
  partir do dado da variante, faixa de confiança com ícones, barra fixa de
  compra no celular, descrição dividida em sanfonas pelos `<h3>`.
- Dois defeitos herdados corrigidos: `alt: media.alt | default: …` dentro dos
  argumentos do `image_tag` virava o filtro seguinte e engolia `class`; e a
  linha de pagamento prometia "até 4x sem juros" enquanto o modelo de preço
  (comentário WX) é 2x sem juros.

## Provar antes de publicar

    cd engineprint/shopify
    npm i liquidjs@10           # só para o mock
    curl -s https://engineprint.com.br/products/bambu-lab-a1.js > mock/dados/bambu-lab-a1.json
    node mock/render.mjs sections/product-whatsapp.liquid bambu-lab-a1 mock/a1.html '{"Padrão":417,"Combo":2580}'
    node mock/exercita.mjs      # capturas + medições (crossfade, swipe, barra fixa, zoom)
    node mock/mede.mjs          # a foto cabe no palco? (celular e desktop)
    node mock/setas.mjs         # setas não abrem o zoom junto

O mock imita os objetos e filtros da Shopify que a seção usa; **não substitui**
o preview do tema — a prova final é no navegador, na loja.

## Fotos

Quem usa o canvas único precisa de fotos com o mesmo fundo. `imagens/`
traz o baixador da galeria oficial, a folha de contato para escolher
**olhando**, a comparação com a foto atual, e o normalizador (`#FFF → #F7F7F7`
por flood-fill a partir das bordas, e enquadramento 1:1).

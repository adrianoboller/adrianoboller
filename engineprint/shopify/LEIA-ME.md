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
    AJUSTES='{"whatsapp_number":""}' node mock/render.mjs sections/product-whatsapp.liquid bambu-lab-a1 mock/a1-sem.html '{"Padrão":417,"Combo":2580}'
    node mock/prova-estoque.mjs mock/a1.html mock/a1-sem.html   # "Verificar estoque" → WhatsApp, mensagem segue a variante; sem número volta "Em estoque"
    node mock/prova-estoque.mjs --tema <id-do-tema> https://engineprint.com.br/products/bambu-lab-a1   # a mesma prova no preview da loja

**Cor só se prova no preview da loja.** O mock não carrega o `base.css` do
tema, e foi ele que pintou o link de laranja no hover por um seletor de
especificidade (0,3,2) enquanto o mock dizia 36 de 36 — ver
`cognicao/cognicao_mock_sem_css_do_tema_aprova_o_que_a_loja_reprova_20261001_1915.md`.

`AJUSTES` é um JSON que sobrepõe as configurações da seção, para simular o
dono mudando algo no editor sem tocar no esquema. Com a loja fora do ar o
`curl` do produto falha; o mock aceita o mesmo JSON montado à mão a partir de
uma página salva (`data-pp-variants` e `data-pp-media` já trazem o que ele usa).

O mock imita os objetos e filtros da Shopify que a seção usa; **não substitui**
o preview do tema — a prova final é no navegador, na loja.

## Fotos

Quem usa o canvas único precisa de fotos com o mesmo fundo. `imagens/`
traz o baixador da galeria oficial, a folha de contato para escolher
**olhando**, a comparação com a foto atual, e o normalizador (`#FFF → #F7F7F7`
por flood-fill a partir das bordas, e enquadramento 1:1).

## Avaliações (Judge.me) — revisão de 28/09/2026

O modelo de produto desta loja é `.liquid`, e bloco de app só entra em modelo
JSON; por isso o Judge.me entra pelo **trecho oficial para modelo em Liquid**,
não pelo editor de tema. Onde está cada peça:

| peça | arquivo |
|---|---|
| estrelas ao lado do nome | `sections/product-whatsapp.liquid` (`.pp__title-row`) |
| widget de avaliações no fim do produto | `sections/product-whatsapp.liquid` (`#avaliacoes`) |
| estrelas no cartão das coleções (só produto já avaliado) | `blocks/_product-card.liquid` + `assets/wx-vitrine.css` |
| página `/pages/avaliacoes` (nota geral da loja) | `sections/wx-avaliacoes.liquid` + `templates/page.avaliacoes.json` |

Produto sem avaliação não mostra estrela nenhuma — é configuração do próprio
app (`hide_badge_preview_if_no_reviews`). As armadilhas (widget novo que não
busca dados sozinho, selo escondido por `data-template`, `display:block
!important`, cor com a mesma especificidade) estão em
`cognicao/cognicao_judgeme_widget_novo_precisa_dos_dados_20260928_1930.md`.

Provar, no preview do tema (a loja de verdade, não HTML salvo):

    bash mock/confia-ca-do-proxy.sh      # uma vez por contêiner: o Chromium vem sem o CA do proxy
    node mock/prova-judgeme.mjs <id-do-tema> <id-do-tema-publicado>   # 37 verificações, 390 e 1280 px
    node mock/placar-avaliacoes.mjs      # o placar COM avaliação, que a loja ainda não mostra (liquidjs)

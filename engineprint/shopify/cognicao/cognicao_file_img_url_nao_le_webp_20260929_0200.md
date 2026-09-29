# file_img_url nao le webp: o arquivo existe e a pagina mostra "no-image"

**Descoberto em** 29/09/2026, 02:00. **Onde:** pagina Detalhes, painel da
Snapmaker U1 (`sections/wx-detalhes.liquid`).

## O que aconteceu

O dono viu que a U1 nao tinha banner na pagina Detalhes. As outras nove
impressoras tinham. A secao escolhe a arte por nome de arquivo e a pede com
`{{ banner | file_img_url: '2400x' }}`; para a U1 o site entregava
`/cdn/shopifycloud/storefront/assets/no-image-2048-a2addb12_2400x.gif` — o
cinza de "sem imagem" do Shopify.

## O que eu conclui primeiro, e estava errado

Que o arquivo `Snapmaker-U1-Review_0002_Layer-4.webp` nao existia nos
Arquivos (nome digitado errado, ou apagado). A consulta aos Arquivos mostrou
que ele existe, 1200x675, desde 08/08/2026. A diferenca entre a U1 e as
outras nove nao era o arquivo: era a extensao. As nove sao `.jpg`; a U1 e
`.webp`, e o `file_img_url` — filtro antigo, anterior ao `image_url` — so
entende jpg, png e gif.

## O que a medicao disse

- Antes (tema 17): 1 de 9 banners era o `no-image` (a U1); 8 de 9 com 200.
- Depois (tema 18, `images[banner] | image_url: width: 2400`): 9 de 9 com a
  arte real; a da U1 pesa 120.485 bytes e abre no navegador a 1178x560
  (1280 px) e 348x196 (390 px), natural 1200x675.

## A regra

Arte dos Arquivos se pede por `images['nome'] | image_url`, nunca por
`file_img_url`: o filtro antigo nao falha, devolve uma imagem cinza com
status 200 — e ninguem ve o defeito ate abrir o painel certo.

## Como esta guardado hoje

`sections/wx-detalhes.liquid` usa `images[]` para as dez artes e, se um
arquivo sumir, cai na foto do produto em vez do cinza. **Buraco:** nao ha
varredura do tema procurando outros `file_img_url` com arquivo que nao seja
jpg/png/gif; o `grep` de hoje achou so este uso na secao.

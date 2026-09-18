# Remendo no mecanismo errado: ele foi ao ar e nunca teve efeito

**Descoberto em** 18/09/2026, 02:00. **Onde:** vitrine das colecoes da Engine
Print (`sections/wx-collection.liquid`, `sections/wx-hero-banner.liquid`).

## O pedido, duas vezes

Em 16/09 o dono disse que o selo ("5% no Pix" / "Esgotado") e o botao "+"
sofriam *overlapping* ao clicar num produto. Escrevi um remendo, publiquei, e
dei por resolvido. Em 17/09 ele **repetiu o mesmo pedido**. O remendo estava no
ar — conferido, 2 ocorrencias no HTML da pagina — e o defeito tambem.

## O que eu concluiu primeiro, e estava errado

Concluiu que a causa era a **View Transition**: `product-card.js` marca a
galeria com `data-view-transition-type` e o `base.css` promove essa galeria
para a camada `::view-transition`, que pinta acima de tudo. A historia fechava
tao bem que nao a medi — escrevi a regra, ela era plausivel, e plausivel
passou por medido.

Medido depois, na pagina real:

| atributo | na loja |
|---|---|
| `data-view-transition-to-main-product` | **nao existe** (zero ocorrencias) |
| `data-product-transition` | `"false"` |
| `data-page-transition-enabled` | `"false"` |

E `handleViewTransition` comeca com:

    if (... || !this.productTransitionEnabled) return;
    if (!cardGallery || !cardGallery.hasAttribute('data-view-transition-to-main-product')) return;

Ou seja: **aquela transicao nunca roda nesta loja.** A regra era codigo morto
desde o primeiro dia. Pior que nao consertar: um comentario de 30 linhas
explicando com seguranca um mecanismo que nao acontece, no caminho de quem
viesse depois.

## O segundo erro, no meio da investigacao

Para achar quem pintava por cima, usei `document.elementFromPoint()` no centro
do selo. Ele devolveu a **imagem do slide**, e quase escrevi "confirmado, a
imagem cobre o selo".

`elementFromPoint` responde **quem recebe o clique**, nao **quem pinta em
cima**. O selo tem `pointer-events: none` — medido — entao ele e pulado no
teste de acerto mesmo estando visivel na frente. A ferramenta respondeu com
honestidade a pergunta que eu fiz; a pergunta e que era outra.

Quem resolveu foi **olhar**: um recorte a 3x mostrou o selo atravessando a
borda da imagem, metade na foto e metade no branco do cartao.

## A causa de verdade, e o numero

Nao era empilhamento, era **caixa**. O selo e filho de `<product-card>` e se
posiciona a partir dele; a imagem nao ocupa o cartao inteiro, porque
`.product-card__info` tem `padding: 14px 16px 18px`. Medido: cartao 166px,
imagem 134px. Com recuo de 10px a partir do cartao:

    16 - 10 = 6px para fora a direita
    14 - 10 = 4px para fora no topo

Bate com o medido em **9 cartoes, nas duas larguras (390 e 1280)**. E o tema
ja tinha resolvido isso para o botao "+" — `--quick-add-right: calc(offset +
var(--padding-inline-end))` — e nunca fez o mesmo para o selo.

## E o banner, que era outro defeito com o mesmo disfarce

"As imagens do banner estao cortadas." Medi esperando corte de `object-fit:
cover` e achei outra coisa: a foto ocupava **356x200 no topo de uma moldura de
356x480**, com 280px de branco embaixo e o titulo metade fora da foto.

A secao pede `.wx-hero__media img { width:100%; height:100% }`, mas o `<img>`
esta dentro de um `<picture>` — inline, sem altura. Altura em porcentagem
contra pai de altura automatica resolve para `auto`, o `<img>` cai na
proporcao natural dele e o `object-fit: cover` **nunca chega a agir**.

So DEPOIS de consertar isso o corte vira problema real (moldura 0,81 contra
imagens 1,61 e 1,78). E ai o conserto tambem nao era na imagem: era tirar
`.wx-hero__content` do `absolute` para o fluxo, para a caixa ter a altura que
aquele texto pede — 58% da foto fora antes, 37% depois, sem constante nenhuma
no meio.

## As leis que saem daqui

**1. Remendo tem de ser provado no defeito, nao no raciocinio.** Uma
explicacao coerente nao e uma medicao. Antes de publicar: reponha o defeito,
veja o remendo falhar sem ele e passar com ele. Se o defeito nao se repoe, nao
se sabe o que se consertou.

**2. Pedido repetido e a prova de que o conserto anterior nao serviu.**
Quando o dono repete, a primeira hipotese a testar e "o meu remendo nao faz
nada" — nao "ele nao viu".

**3. `elementFromPoint` mede clique, nao pintura.** Com `pointer-events:none`
ele pula elemento visivel. Para ordem de pintura: recorte a pixel, ou olhar.

**4. Codigo morto que se declara conserto e pior que ausencia de conserto.**
O comentario convence o proximo a nao procurar mais. O que ficou no lugar
nomeia o que foi MEDIDO, com o numero.

## O medidor

`mock/mede-vitrine.mjs` — desenha a pagina no navegador, reprova selo que
passa da borda da imagem e banner que nao preenche a caixa. Provado nos dois
sentidos: 4 reprovacoes sem o remendo, zero com ele, e rodado tambem contra a
**previa publicada** (`/t/39`), nao so contra o repro local.

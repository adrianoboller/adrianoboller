# Medicao pela rede que nao imita o navegador mede outra coisa

**Descoberto em** 18/09/2026, 04:30. **Onde:** auditoria de desempenho da
Engine Print.

## O que aconteceu

O dono pediu para aumentar o desempenho do site. Medi, e em sequencia afirmei
QUATRO coisas — todas erradas, todas com numero ao lado:

| # | o que eu afirmei | o que era |
|---|---|---|
| 1 | "15 MB por pagina" | 1 MB |
| 2 | "o gargalo e 777 KiB de JS" | JS sao 102 KiB |
| 3 | "os PNG de origem pesam" | o CDN reencoda, 79 KiB |
| 4 | "361 KiB em miniaturas de menu" | 4 KB cada, 25 KiB no total |

O numero final, medido direito:

| celular 390px | produto | colecao | home |
|---|---|---|---|
| HTML | 57 | 69 | 72 |
| CSS | 64 | 64 | 64 |
| JS | 102 | 100 | 100 |
| IMG | 343 | 745 | 848 |
| **TOTAL** | **567 KiB** | **978 KiB** | **1.084 KiB** |

**Nao ha problema de desempenho.** A nota que eu dei (4) saiu de medicao
minha errada; medido, e 8. Eu ia mexer num tema publicado para consertar um
defeito que nao existe.

## O que eu concluiu primeiro, e estava errado

Que `curl <url>` mede o que o visitante baixa. Nao mede. Um `curl` cru e um
cliente diferente do navegador em tres pontos, e cada um inflou a conta:

1. **Sem `Accept-Encoding: gzip, br`** o servidor manda texto CRU. CSS de
   441 KiB comprime para 64; JS de 777 vira 102. Fator ~7x em codigo.
2. **Sem `Accept: image/avif,image/webp,...`** o CDN da Shopify manda o
   formato original. Um PNG de 691 KiB vira 79 KiB em WebP e 4 KB em AVIF.
3. **A URL vem do HTML, com `&amp;`.** Passar `...?v=1&amp;width=160` ao
   curl faz o parametro virar `amp;width` — ignorado — e o CDN serve a
   imagem INTEIRA. Foi isto que transformou uma miniatura de 4 KB em
   60 KiB na minha planilha.

O terceiro e o mais traicoeiro porque **nao da erro**: a requisicao volta 200,
com uma imagem valida, do tamanho errado.

## Por que sobreviveu a quatro rodadas

Porque cada correcao consertou UM dos tres e eu declarei vitoria. Acrescentei
`Accept-Encoding`, o numero caiu, pareceu resolvido. Acrescentei `Accept`, caiu
de novo, pareceu resolvido. So na quarta — quando 6 miniaturas de 160px deram
361 KiB e a aritmetica ficou impossivel (60 KiB para 160x160) — e que fui
conferir UMA imagem isolada e achei o `&amp;`.

**O que me salvou foi um numero absurdo, nao um teste.** Se as miniaturas
fossem de 800px, 60 KiB seria plausivel e eu teria publicado a besteira.

## As leis

**1. Medicao de rede se faz com os cabecalhos do navegador.** No minimo
`Accept-Encoding: gzip, deflate, br` e o `Accept` de imagem. Sem eles, mede-se
um cliente que nenhum visitante usa.

**2. URL extraida de HTML passa por `html.unescape` antes de virar
requisicao.** `&amp;` nao da erro: da outro resultado, com 200.

**3. Confira UM caso isolado contra a expectativa fisica antes de somar.**
160x160 nao pode pesar 60 KiB. A soma escondeu o absurdo que uma amostra
mostraria na hora — agregado e onde erro de unidade se disfarca.

**4. Nao se conserta o que nao se reproduziu.** Eu estava a um passo de mexer
no tema publicado para resolver um gargalo inexistente — de novo, depois do
remendo morto da View Transition. A diferenca e que desta vez a medicao chegou
antes do commit.

## O corolario para a avaliacao

Nota de desempenho que sai de `curl` sem cabecalho e nota inventada com cara
de medida. A nota do site subiu de 5,15 para 5,55 **sem eu tocar em nada** —
so por medir direito o que ja estava bom.

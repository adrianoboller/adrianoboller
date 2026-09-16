# Arquivos removidos do tema (registro)

## `sections/produto-premium.liquid` — removido em 16/09/2026

**Onde o original continua:** nos temas `EnginePrint Industrial 11` e
`EnginePrint Industrial 12`, que não foram tocados. Não há cópia aqui de
propósito — o que ensina não é o conteúdo do arquivo, é o mecanismo abaixo.

É a **ancestral** de `sections/product-whatsapp.liquid`: mesma seção, versão de
agosto. Ficou no tema depois que a nova entrou com outro nome, sem nenhum
template a referenciar (conferido nos 26 arquivos de `templates/*.json` e nos
grupos de seção: zero referências).

**Arquivo morto não é inofensivo quando ele traz `{% stylesheet %}`.** A
Shopify compila o CSS de *todas* as seções em `compiled_assets/styles.css`,
usadas ou não, e concatena por nome de arquivo: `product-whatsapp` vem antes de
`produto-premium`. As duas usam o mesmo prefixo de classe `.pp__`, com a mesma
especificidade — então a versão velha, por vir depois, ganhava. Medido no
navegador com o CSS real do tema:

| propriedade | valor da seção nova | o que valia na loja |
|---|---|---|
| `.pp__thumb` `padding` | `0` | `4px` |
| `.pp__thumb` fundo | `#F7F7F7` | `#FFF` |
| `.pp__thumb` altura | 56 px | 58 px |
| `.pp__slide` (inativo) `display` | `block` | `none` |
| `.pp__stage` fundo | `#F7F7F7` | `#F2F2F0` |

O sintoma visível era a miniatura: com borda de 1 px e `padding` de 4 px, a
imagem fica 5 px para dentro, onde o canto arredondado do botão já fechou — e
aparece como um quadrado de canto vivo. O invisível era pior: `display:none`
no slide inativo **anulava o crossfade inteiro**, deixando o corte seco que a
seção nova existia para resolver.

**A lição, para não voltar:** duas seções no mesmo tema compartilhando prefixo
de classe é um conflito que nenhuma das duas consegue ver sozinha, e que não
aparece em preview de arquivo nem em mock — só no CSS compilado do tema. Se um
dia for preciso manter as duas instaladas ao mesmo tempo, a saída é renomear o
prefixo de uma delas (`.pp__` → `.epp__`), não disputar especificidade.

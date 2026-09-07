# O desenho embutido e o desenho SOLTO não são o mesmo arquivo

**07/09/2026, 03:45** — descoberto ao gerar o fluxograma e o diagrama de
workflow do medidor comparativo, que saem em dois destinos: embutidos no
dossiê e como `.svg` avulso em `docs/dossie/`.

## 1. O que aconteceu

O mesmo texto SVG, byte a byte, **renderizou perfeitamente dentro do dossiê e
não carregou nada como arquivo**. Medido: `naturalWidth` = 0 nos dois.

Três diferenças, e nenhuma delas dá erro visível:

1. **`xmlns`.** Dentro de HTML o parser assume o namespace SVG; num `.svg`
   solto, sem ele o arquivo não é SVG — e o navegador não reclama, só entrega
   largura zero.
2. **Entidade nomeada.** `&middot;` e `&mdash;` são HTML, não XML. **Uma** só
   derruba o arquivo inteiro.
3. **`<style>` antes do `<svg>`.** Eu pus os tokens de cor no topo do arquivo,
   como se põe num HTML. Aí a raiz deixa de ser `<svg>` e o arquivo vira
   documento HTML com um desenho dentro — que até aparece, e não é um SVG.

## 2. O que eu concluí primeiro, e estava errado

**Três vezes, e as três medindo o EMBRULHO em vez do arquivo.**

Primeiro escrevi um portão que abria o `.svg` e conferia
`document.documentElement.tagName === 'svg'`. Ele reprovou os dois — e a razão
não era o arquivo: o Chromium **embrulha todo `.svg` de `file://`** num
documento sintético, então a raiz é sempre `html`. O portão não media nada.

Segundo, troquei por `getBBox()` em cada `<text>`. Estourou com
*«getBBox is not a function»* — os elementos vinham do embrulho, fora do
namespace SVG. Segunda régua, mesmo erro.

Terceiro, e este é o que quase me enganou: quando o `<img>` finalmente
reprovou, eu tinha **duas causas plausíveis na mesa** — o `xmlns` que faltava
e a política de origem do Chromium para `file://` numa página `about:blank`.
Se eu tivesse consertado só uma e visto passar, teria escrito a lição errada
com toda a confiança. Separei: a página de teste passou a ser um arquivo ao
lado dos SVG, o que mata a hipótese da origem, e aí o `xmlns` respondeu
sozinho.

## 3. O que a medição disse

- **0 × 0 px** com o arquivo «bonito» que renderizava perfeito embutido.
- **199 × 150** e **300 × 134** depois do `xmlns` — carrega.
- **2** defeitos de desenho achados só na captura, e os dois são os mesmos que
  a página dos gráficos já pagou: rótulo invadindo a caixa vizinha, e texto
  **riscado pela própria seta** por estar centrado no eixo dela.
- **0** textos fora do `viewBox`, conferido por `getBBox` contra o
  `viewBox.baseVal` — na cópia embutida, onde o DOM é SVG de verdade.

## 4. A regra

**Desenho que sai em dois destinos se prova nos dois.** Embutido em HTML e
solto em arquivo são formatos diferentes com o mesmo texto dentro, e o segundo
é mais estrito: namespace obrigatório, só as cinco entidades do XML, e a raiz
tem de ser o `<svg>`.

E a regra sobre o portão, que é a que se repetiu três vezes aqui:
**quando o navegador embrulha o que você quer medir, meça o efeito e não a
estrutura.** «Carrega como imagem e tem tamanho» é sobre o arquivo;
`documentElement` é sobre o embrulho.

## 5. Como está guardado hoje

- `docs/dossie/comparativo-no-dossie.py`, função `solto()`: acrescenta o
  `xmlns`, troca as entidades nomeadas pelo caractere e põe os tokens de cor
  **dentro** do `<svg>`. O comentário dela nomeia os três defeitos.
- Os dois destinos saem do **mesmo** gerador, então não podem divergir: a
  cópia do dossiê e o `.svg` avulso vêm da mesma função de desenho.

**O buraco que fica, declarado:** o conferidor do navegador é um script de
sessão, não está versionado, e não roda na bateria única. Quem mexer no
desenho tem de reabrir os dois arquivos à mão — e os defeitos desta rodada
mostram que ler o código não basta.

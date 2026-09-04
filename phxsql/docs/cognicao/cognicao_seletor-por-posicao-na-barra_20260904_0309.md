# Seletor de tela por POSIÇÃO acerta o vizinho em vez de ficar vazio

*Descoberto em 04/09/2026, 03:09 — frente T (tela), papéis E e F.*

## 1. O que aconteceu

A bateria reprovou em **5 dos 8 passos** de `idiomas` e em **3 passos de tela**
do `ponta-a-ponta`. Os cinco de `idiomas` traziam uma mensagem só:

```
page.waitForSelector: Timeout 10000ms exceeded.
  - waiting for locator('#idiomasAqui .idi') to be visible
```

A parte inteira levava **1m38s** — quase tudo esperando por elementos que nunca
iam aparecer. Depois do conserto ela leva **12,1s**.

## 2. O que eu concluí primeiro, e estava errado

O diagnóstico que chegou comigo — e que eu li como provável — era o
`id="idiomasAqui"` **repetido** no `index.html`: duas ocorrências (linhas ~7225
e ~7894), três chamadas a `desenharIdiomas("#idiomasAqui")`, e `querySelector`
devolvendo sempre o primeiro. Explica o sintoma inteiro, tem duas linhas de
prova e o duplicado é anterior à rodada. É um diagnóstico bonito.

Errado — para **este** defeito. E, o que é pior de perceber, **certo para
outro**, que ninguém tinha exercitado. As duas coisas juntas são a lição: um
diagnóstico plausível não deixa de ser plausível por estar no lugar errado, e é
exatamente por isso que ele sobrevive tanto tempo.

## 3. O que a medição disse

**O duplicado, medido no navegador** (`querySelectorAll('#idiomasAqui').length`):

| situação | quantos |
|---|---|
| tela de Configurações aberta | **1** |
| tela de Idiomas aberta | **1** |
| duas ABAS, uma em cada tela | **1** (e `.tela` no documento: 1) |
| duas **REGIÕES** lado a lado, uma em cada tela | **2** |

As três primeiras linhas absolvem o duplicado do defeito da bateria: `folha()`
e `abrirAdmin` trocam o `innerHTML` do `#painel`, e a decisão [2] do
`multitela.js` **desanexa do documento** a aba escondida. A quarta linha o
condena noutro caminho — e nesse caminho o sintoma é o previsto: a tela de
Idiomas abre com **0 bandeiras** e a de Configurações, na paina do lado,
redesenha as **6** dela sozinha.

**A causa real da bateria**, medida com o navegador aberto: a barra de
ferramentas tem **23** botões e **nenhum** se chama Config. O seletor da prova
era

```
.fer[title^="Config"], .fer[title^="Konfig"], #ferramentas .fer >> nth=13
```

e o `nth=13` **não ficou vazio**: passou a acertar o 14º botão, que hoje é
«Restaurar». A tela abria, o clique dava certo, e só dez segundos depois o
passo morria falando do elemento de outra tela.

`git log -S'rot:"Config'` dá **c153d71**, 02/09/2026: *«3. Config sai da barra.
Antes de apagar, CONFERIDO que o caminho continua existindo: menu Configurações
→ Gerais do servidor.»* A remoção foi deliberada e conferida no produto. O que
não foi conferido foi **quem clicava na barra**.

E a bateria `tela` **já sabia**: o caso 17 (`17-barra-e-popup.mjs`) afirma que o
Config NÃO está na barra e ESTÁ no menu, e passa. Duas baterias discordavam
sobre o mesmo fato há dois dias, e a que estava certa era a que falhava alto.

**Os três do `ponta-a-ponta`**, todos da mesma família:

- «as colunas declaradas estão lá»: a grade virou PhxGrid e o `<th>` carrega o
  botão de filtro dentro dele. `textContent` devolve `"cidade▼"`, não
  `"cidade"`.
- «a grade não troca a caixa do dado» dava **`null`**, não `"uppercase"`: o
  `indexOf('cidade')` devolvia -1 pela mesma seta, e a guarda do dado
  maiúsculo reprovava **sem nunca ter olhado uma célula**. Medido de verdade:
  `textTransform` = `none`. O dado estava certo o tempo todo.
- «o recado começa com a frase do dono»: a moldura `[SP000021] ` abre toda
  recusa desde **d4f8563**, *«Toda recusa diz de qual sprint se trata, no molde
  do MySQL»* — e a mesma resposta traz `sprint` como **campo**, justamente para
  ninguém recortar a frase.

## 4. A regra

**Seletor que erra o alvo tem de ficar VAZIO, nunca acertar o vizinho.** Peça
de tela se acha pela CHAVE — `data-campo` na grade, a chave da fábrica de
idiomas no menu — ou por busca que falhe **nomeando o que procurou**. Nunca por
posição numa lista, nunca pela redação de um `title`.

E o corolário do id: **id que o módulo de telas não gerencia é id que se repete
sem sintoma até alguém dividir a tela.** O `#painel` é movido para a tela com
foco; os ids que cada tela traz no próprio HTML, não.

## 5. Como está guardado hoje

- `abrirPeloMenu(page, chave)` em `testes-web/apoio.mjs`, usada nos quatro
  pontos de `prova-idiomas.mjs` (três da tela de Configurações e o da nota da
  multitela, que procurava a palavra «multitela» dentro do rótulo — o irmão,
  que passava por sorte). **Prova real:** apagada a chave do item de menu, a
  bateria reprova dizendo `nao achei item de menu com a chave
  «tela.mi_gerais_servidor»` em vez de esperar 10 s por outra tela.
- `bancada/bateria/prova-tela.mjs` lê o cabeçalho por `data-campo` e a moldura
  da recusa pelo campo `sprint` da resposta. **Prova real:** com
  `text-transform:uppercase` reposto nas células, a guarda agora diz
  `"uppercase"` — antes dizia `null`, que é «não achei a célula» fantasiado de
  «o dado está errado».
- `desenharIdiomas` desenha **todos** os alvos (`$$`, não `$`), e a seta do
  teclado devolve o foco dentro do próprio contêiner. Guarda nova no caso 12
  (`testes-web/casos/12-multitela.mjs`), logo depois do `dividir(2)`.
- **Onde o buraco ficou:** `bancada/profiler/exercicio-tela.mjs:40` ainda usa
  `.fer[title^="Profiler"]` — mesma família (redação), fora do território desta
  frente. Passa hoje porque o botão existe e a prova roda em português. E o
  `id="idiomasAqui"` continua **repetido** no HTML: o comportamento está
  consertado, a duplicidade formal não.

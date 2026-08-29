# DESIGN — o sistema visual do Centro de Controle

O que este documento é: o levantamento do sistema visual que o
`crates/phxsql-server/ui/index.html` **realmente tem** — os tokens, a escala,
os componentes que existem e quando usar cada um —, mais as regras que
impedem os defeitos que já custaram caro aqui.

O que ele não é: uma proposta. Nada aqui é aspiracional. Onde um número
aparece, ele foi **medido no navegador**, não estimado — as três reprovações
de contraste que este documento registra foram encontradas assim, e nenhuma
delas aparecia lendo o código.

> A marca manda. `marca/LEIA-ME.md` é a fonte: Exo 2, fundo `#010418`,
> assinatura *Built to store. Engineered to scale.* Este documento descreve
> como o console aplica a marca, e não uma paleta paralela.

---

## 1. Tokens

Todos vivem em `:root`, e o tema claro os **redefine** em
`:root[data-tema="claro"]`. Nenhuma cor é escrita solta numa regra: se
precisar de uma cor nova, ela nasce token, senão o tema claro não a acompanha.

### 1.1 Superfícies

| Token | Escuro | Claro | Onde |
|---|---|---|---|
| `--fundo` | `#010418` | `#f7f5f2` | o fundo da página; é o `#010418` da marca |
| `--painel` | `#0a1122` | `#ffffff` | barra, árvore, `.rolo`, caixa de diálogo |
| `--painel-2` | `#0f182c` | `#f2efeb` | cartões, cabeçalho de tabela, blocos de formulário |
| `--painel-3` | `#131d33` | `#ece7e1` | **só hover do cromo** (barra de ferramentas, menu) |
| `--realce` | `#152238` | `#e9e4de` | linha selecionada, hover de linha |
| `--linha` | `#1e2940` | `#ded7cf` | divisória fraca |
| `--linha-forte` | `#2b3a56` | `#c4bab0` | borda de campo e de botão |

### 1.2 Texto

| Token | Escuro | Claro | Onde |
|---|---|---|---|
| `--texto` | `#dde2eb` | `#1a1210` | corpo, valor de dado |
| `--texto-2` | `#a8b0c0` | `#4a3f3a` | texto secundário, rótulo de botão |
| `--texto-3` | `#848da0` | `#6b5e57` | legenda, cabeçalho de tabela, rótulo de KPI |

`--texto-3` é o token de texto apagado **mais usado da folha**, e por isso o
mais perigoso: ele mora sobre `--painel-2` (cabeçalho de tabela) e sobre
`--realce` (linha selecionada). Os dois valores acima são os que passam de
4,5:1 no **pior** desses fundos — ver §5.

### 1.3 Marca e arquivos

`--laranja` `--ambar` `--vermelhao` `--vermelho` vêm da folha de marca. No
tema claro o vermelhão escurece para `#c63c0a`, que é a adaptação já decidida
e documentada na marca — não é liberdade deste documento.

`--reg` `--ndx` `--bin` `--memo` `--log` são as cinco cores de arquivo, e
precisam se distinguir **entre si e do acento**.

### 1.4 As cores da ação

| Token | Escuro | Claro | Significa |
|---|---|---|---|
| `--acao-incluir` | `#6cc98c` | `#2f7a3e` | inclui |
| `--acao-alterar` | `#ffc43d` | `#7d5f18` | altera |
| `--acao-marcar` | `#ff8fc7` | `#b5257f` | marca (o excluir que volta) |
| `--acao-excluir` | `#ff5f5f` | `#b71414` | exclui de vez |
| `--acao-consultar` | `#5fa6e8` | `#1f5c93` | consulta |

**Sempre contorno, nunca fundo cheio.** O preenchimento só no `hover`, quando
já há intenção. A razão está num comentário do CSS desde antes de virar regra:
fundo laranja com texto escuro em cima ficava ilegível, e foi assim que o botão
de excluir apareceu. Como as cinco são **cor de texto** num botão de contorno,
o piso delas é 4,5:1 e não 3:1 — é por isso que `--acao-alterar` mudou (§5).

### 1.5 Medidas

`--barra:52px` (altura da barra de cima) · `--arvore:268px` (largura escolhida
do painel lateral, ajustável por arrasto) · `--arvore-col` (a largura **viva**
da coluna: vale `--arvore` só com a lateral fixa e aberta; vale `0` nos outros
dois estados — ver §4).

### 1.6 Tipografia

Exo 2 para tudo; **IBM Plex Mono** para dado, identificador, número e rótulo
técnico — mono aqui não é enfeite: é o que faz coluna de número alinhar e
nome de tabela não se confundir com prosa.

A escala em uso, por frequência: **12,5px** (o padrão do miolo), 11,5 · 12 ·
10,5 · 11 · 10 · 9,5px (legenda e rótulo em versalete), 13px (tabela), 20px
(título de tela), 22 e 26px (números de `.ficha` e `.kpi`).

Raios: 5px é o padrão; 7–9px em cartão; 99px em pílula; 12px na caixa de
diálogo.

---

## 2. Os componentes que existem

### Moldura
| Peça | O que é |
|---|---|
| `.barra` | a barra de cima: marca, versão, avisos, tema, quem sou, Sair, e o botão do painel lateral. **Fica acima do véu da gaveta** (§4) |
| `.menubar` | menu tradicional; `Alt+letra` abre; rola de lado quando não cabe |
| `#ferramentas` | a barra de ferramentas — **pelo id, ver §3** |
| `.lateral` | a casca do painel lateral: topo com o pino, a árvore, e a nota de «neste navegador» |
| `.arvore` / `.no` | a árvore; `.no.db`, `.no.tab`, `.no.esq` são as variações |
| `.corpo` / `.cabecalho` / `.abas` / `#painel` | a área de trabalho — **`#painel` pelo id, ver §3** |

### Dado
| Peça | Quando usar |
|---|---|
| `.rolo` | **o contêiner de rolagem de toda grade larga.** Tabela larga vai dentro de um `.rolo`; a rolagem é dele e nunca da página |
| `table` / `thead th` / `td.dado` / `td.num` / `td.nulo` | a grade. `td.num` alinha à direita com `tabular-nums` |
| `.fichas` / `.ficha` | a faixa de números de resumo no alto de uma tela |
| `.kpis` / `.kpi` | os números do Painel; `.kpi.viva` e `.kpi.mal` colorem o valor |
| `.cartas` / `.carta` | cartão de gráfico; `.carta.larga` ocupa a linha |
| `.pino` | etiqueta de estado em pílula: `.ok` `.nao` `.mal`, e `.reg` `.ndx` `.bin` `.memo` `.log` |

### Entrada
| Peça | Quando usar |
|---|---|
| `.botao` | ação principal. `.secundario`, `.mini`, `.perigo`, e as cinco da ação (§1.4) |
| `.acoes` | a fila de botões de uma tela (nela o `.botao` volta a ter largura própria) |
| `.ficha-edit` | o formulário de uma ficha |
| `.criar` / `.criar .bloco` | o formulário de criação (nova tabela) |
| `.form-dbl` / `.linha-chk` | formulário do DbLink; **`.linha-chk` é o lugar onde o `input{width:100%}` já está consertado — reuse-o em vez de reconsertar** |
| `.forma-job` | formulário de job |
| `.barra-acao` | fila de campos + botões acima de uma lista (campos com largura própria) |
| `.ferramentas` *(classe)* | a fila de filtros **dentro** do painel — não confundir com `#ferramentas` |
| `table.conf` | a comparação de quatro colunas do conflito de escrita |

### Recado
| Peça | Quando usar |
|---|---|
| `.aviso` | bloco de aviso com barra à esquerda; `.mal` e `.bom` |
| `.leg` | legenda curta abaixo de um campo ou de uma tabela |
| `.centro` | o estado vazio de uma tela inteira |
| `.sobre` / `.caixa` | o diálogo sobreposto; `.caixa.larga` para a comparação |

---

## 3. A regra que mais dói: nome de classe é global

`input{width:100%}` virou uma bolinha do tamanho da célula. `label{text-transform:
uppercase}` fez «Blumenau» aparecer «BLUMENAU» — **mentira sobre o dado**, porque
quem olha não sabe se está gravado assim. Os dois já estão registrados no
CLAUDE.md. O que faltava registrar é o **terceiro modo** do mesmo defeito, e ele
é o mais silencioso:

> **Dois componentes diferentes com o mesmo nome de classe. O segundo ganha, e
> o primeiro passa a ter propriedades que ninguém escreveu para ele.**

Não aparece lendo nenhum dos dois — cada um, sozinho, está certo. Quatro casos
reais, todos medidos no navegador:

| Classe | Os dois componentes | O estrago medido |
|---|---|---|
| `.ferramentas` | a barra da moldura **e** a fila de filtros do painel | a fila (que vem depois) ligava `flex-wrap:wrap` numa barra escrita para **rolar**: duas fileiras no desktop e **sete no celular** |
| `.painel` | a área de conteúdo **e** o nó «Painel» da árvore (`class="no painel"`) | o nó da árvore ficava com **76px de altura** e `padding:20px 22px 40px`, contra **28px** e `6px 16px` de todos os outros nós |
| `.recado` | o recado do cartão de entrada **e** a pílula de aviso da barra | a pílula herdava `margin-top:14px` e ficava **fora do eixo** de uma barra de 52px centrada |
| `.modo` | o texto do cartão de entrada **e** o cartão de escolha do diálogo de excluir | o texto virava colunas (`display:flex`) e um parágrafo ganhava `cursor:pointer` |

**O conserto é sempre escopar, nunca renomear em massa:**

- peça de moldura, que é única na página → **pelo id**: `#ferramentas`, `#painel`;
- peça do cartão de entrada → **dentro de `#entrada`**: `#entrada .recado`;
- peça de uma tela → **dentro do componente**: `.form-dbl .cmp`, `.linha-job .leg`.

Renomear a classe conserta igual e **conflita com quem estiver mexendo na mesma
tela em paralelo**; escopar não toca a marcação de ninguém.

**Como achar as próximas:** `docs/design/colisoes.py` varre a folha e lista todo
nome de classe com dois blocos de declaração **fora de `@media`** (dentro de
`@media` é o mesmo componente noutra largura — isso é o propósito, não a
doença). A lista é curta; o julgamento é humano. Foi ela que achou `.painel` e
`.recado`.

Um sinal que a varredura **não** pega e vale procurar à mão: um `.no.painel`,
um `td label`, um `.acoes .botao` — regra que existe só para desfazer o que
outra regra fez é **curativo**, e curativo é onde a colisão está escondida.

### Ainda sobre CSS global

- **Rótulo em versalete, valor nunca.** `td label,td .rot-dado` desfaz o
  `text-transform` dentro de célula. Componente novo que mostre dado dentro de
  `label` precisa do mesmo cuidado.
- **`minmax(160px,1fr)` não encolhe, transborda.** Num painel de 76px a coluna
  continua com 160px e o pai corta. Use sempre
  **`minmax(min(160px,100%),1fr)`** — as onze grades `auto-fit` da folha já usam.
- **Todo filho de grid precisa de `min-width:0`.** O padrão `auto` significa
  «não encolha abaixo do conteúdo», e é isso que faz a tabela **empurrar** a
  coluna em vez de rolar dentro dela. `#app > *{min-width:0;min-height:0}`.

---

## 4. Os quatro tamanhos, e o painel lateral

### O modelo
`#app` é um grid de quatro linhas (`barra`, `menu`, `tools`, e a linha
`arvore | corpo`) e duas colunas. **`body{overflow:hidden}` continua**: é o que
garante que a página nunca role de lado. O que for largo demais rola **dentro do
próprio contêiner** — `.rolo`, `#painel`, `#ferramentas`, `.menubar`, `.abas`.

| Faixa | Lateral | Ferramentas | Formulários |
|---|---|---|---|
| ≤ 640px celular | sempre gaveta sobreposta, com véu | uma fileira que rola | uma coluna; alvos de 40px |
| 641–1024px tablet | recolhível, fixa por padrão | uma fileira que rola | duas colunas onde couber |
| ≥ 1025px desktop | fixa, largura ajustável | duas fileiras (cabe tudo) | como sempre foi |
| larga (ultrawide, multi-monitor) | igual ao desktop | igual ao desktop | **tetos**, não regras novas — §4.1 |

### 4.1 A quarta faixa: o que a largura extra faz

**Medido antes de mexer**, tela do Painel, árvore aberta, tema escuro:

| largura | rola de lado? | maior parágrafo | vão rótulo→valor | escala do texto em SVG |
|---|---|---|---|---|
| 390 | não | 326px | 301px | 1,40× |
| 820 | não | 740px | 715px | 1,40× |
| 1180 | não | 1.100px | 613px | 1,40× |
| 1920 | não | 1.840px | 1.353px | 1,83× |
| 3440 | não | 3.360px | 2.873px | 3,73× |
| 5120 | não | 5.040px | 4.553px | **5,83×** |

A responsividade **segurava**: não há rolagem lateral em largura nenhuma, e isso
é mérito do trabalho da rodada anterior. O que não existia era **teto**. Uma
linha de 5.040px tem umas 630 letras; um valor a 4.553px do próprio rótulo não
se lê, porque o olho perde a linha no meio do caminho; e 11px desenhados com
67px ao lado de um menu de 13px são dois regimes de escala na mesma tela.

**A saída escolhida foi a mista**, confirmada pelo dono com a foto de um IDE
ocupando um ultrawide em três painéis verticais: **a largura extra vira mais
painel, não linha mais comprida.**

- Contra «teto e centraliza»: além de desperdiçar a tela, num monitor duplo o
  bloco central cai **em cima da emenda física**. Um bloco de 74ch centrado em
  5.120px mora de 2.260 a 2.860px, com a emenda em 2.560 no meio da frase.
  Alinhado à esquerda ele mora de 290 a 890px, inteiro no primeiro monitor.
  É por isso que **nada aqui centraliza**: o navegador não sabe onde está a
  emenda, e a única regra defensável é não pôr um bloco único no meio.
- Contra «só mais colunas»: nenhuma quantidade de coluna conserta um parágrafo
  de 630 letras. Texto corrido tem limite de legibilidade que nenhum monitor
  muda.

**As três regras da faixa larga:**

1. **Texto corrido tem teto** — `--medida: 74ch`, que dá **592px** medidos nesta
   folha. Vale para parágrafo, item de lista, célula de legenda, subtítulo da
   tela, e para a caixa de `.nota`/`.aviso`, que passava a moldura em volta de
   um texto certo. Tabela de dados e `<pre>` ficam **de fora**: dado em coluna
   não é linha de leitura, e cortar um `<pre>` só acrescentaria rolagem.
2. **Célula de grade tem teto** — `--teto-cartao: 520px`, `--teto-campo: 340px`.
   `auto-fit` com `1fr` recolhe a trilha vazia e **estica** a que sobrou: era
   assim que um campo de «30» ficava com 850px na configuração. O teto manda a
   sobra para a calha, e não para dentro da célula.
3. **Texto em SVG não cresce com o monitor** — a escala pode ser constante e
   maior que 1 (o medidor de arco vive em 1,4× de propósito, porque ali o
   número grande *é* o desenho); o que não pode é ela **depender da janela**.

**O teto vai no item, nunca no `minmax` da trilha.** Foi a primeira tentativa e
ela quebrou o 1920: com `minmax(132px, 520px)` o navegador conta as repetições
pelo **máximo** quando ele é um comprimento definido, então 1.888px davam três
trilhas de 520 em vez de treze de 132, e a fileira de oito KPIs virou três
colunas em três fileiras. O `1fr` fica onde estava; quem para de crescer é o
cartão dentro da trilha.

**Por que teto e não `@media (min-width:2000px)`:** a área central vai virar
mais de uma região lado a lado, e no dia em que virar, uma regra presa à largura
da **janela** estaria medindo a coisa errada — a região tem a largura dela. Teto
em `ch` e `auto-fill` medem o **contêiner**, e por isso continuam certos sem uma
linha nova.

**Medido depois** (as mesmas seis larguras, agora com o caminho de dados longo
que reproduz o defeito): maior parágrafo **453px** no Painel e 620px na
configuração em todas as larguras acima de 820; maior vão rótulo→valor **328px**;
escala do texto em SVG **1,40× constante**; zero sobreposição; zero rolagem
lateral.

O par rótulo→valor virou coluna com `columns:300px`, e não com uma grade
`auto-fill`, porque a ordem de leitura de uma ficha é de cima para baixo: a
multicoluna preserva isso, e a grade leria em ziguezague. Medido na ficha da
telemetria: **1 coluna a 1180px, 3 a 1920, 8 a 3440 e 13 a 5120**, com o vão
caindo de 4.553px para 276px — porque ele passa a ser o vão da **coluna**, e não
o da tela.

### 4.2 O regime de escala do SVG, e as três saídas

| Onde | O que se fez | Por quê |
|---|---|---|
| `barras` (rede, discos, tabelas, IPs, bancos, usuários) | virou HTML | o desenho é rótulo + barra + valor; o navegador mede o rótulo melhor que qualquer `rotulo:112` chutado na chamada |
| `barrasCheias` (espaço em disco) | virou HTML | **era aqui a sobreposição** — ver §6.1 |
| `areaHoras` (operações por hora) | `viewBox` nasce na **largura medida** | a geometria precisa de SVG, e aqui a largura extra vira mais gráfico: as marcas do eixo passam de sete a uma por hora |
| `anel` (usuários por nível) | largura fixa de 320px, centrado | esticar um círculo não serve a ninguém |
| `medidor` (CPU, memória) | já tinha largura fixa (168px) | 1,4× **constante** é decisão de desenho, não deriva do monitor |

O `viewBox` na largura medida não é invenção desta rodada: o painel de bolhas da
telemetria já fazia isso (`svg.setAttribute("viewBox", ...)` a partir do
`clientWidth`). O que mudou foi generalizar a regra e escrevê-la.

O gráfico de horas segue a largura com um `ResizeObserver` só, criado uma vez, e
guarda a última largura no `dataset` para não repintar a cada tique — arrastar a
pega da árvore dispara dezenas deles.

### O painel lateral: dois booleanos, não três botões

```
aberta=1 pin=1   fixa     ocupa coluna própria e EMPURRA o conteúdo
aberta=1 pin=0   solta    flutua POR CIMA; o conteúdo fica com a tela inteira
aberta=0         fechada  some; a coluna vale zero
```

- **Despinado, ele se fecha sozinho** depois que a pessoa escolhe algo nele —
  é para isso que serve despinar. **Pinado, fica**: é o que «pinar» quer dizer.
  Um lugar só decide isso (`fecharSeSolta()`, chamado de `marcar()`), porque
  toda escolha na árvore passa por ali.
- **O botão de reabrir mora na `.barra`, que nunca some.** Painel que se fecha
  sem deixar por onde voltar é armadilha, e a volta não pode depender de
  lembrar um atalho.
- **`Ctrl+\`** recolhe e reabre. `Alt+letra` já é do menu e `Ctrl+B` já é o
  backup; esta sobra e nenhum navegador a usa.
- A largura anda **por arrasto e pelas setas** (a pega é um `role="separator"`
  com valor), entre 180px e 520px.
- Recolhido, pinado e largura ficam no **`localStorage`** — e a tela diz isso,
  em texto, no rodapé do painel. Não é preferência de servidor.
- Abaixo de 640px **não há escolha**: 268px de uma tela de 390 não são um
  painel ao lado do trabalho, são o trabalho inteiro. O botão de pinar aparece
  desligado dizendo por quê.

### O véu da gaveta
Cobre menu, ferramentas e conteúdo — **e não a barra de cima**. Ele cobria
tudo, e medido com `elementFromPoint` quem respondia no lugar do botão de
recolher, do tema e do Sair era o próprio véu: o painel parecia ter sempre um
botão de voltar e não tinha nenhum no dedo. Botão visível que não responde é
pior que botão ausente.

### Movimento
`@media (prefers-reduced-motion:reduce){*{transition:none!important;
animation:none!important}}` está no fim da folha e vale para as transições
novas também.

---

## 5. Contraste: o que foi medido e o que mudou

Medido no navegador, compondo fundos translúcidos até a primeira superfície
opaca, nos dois temas e nos três tamanhos. O piso é **4,5:1** (texto normal) e
3:1 (texto grande ou grafismo).

**Reprovados, e consertados:**

| Token | Era | Media | Virou | Passou a medir |
|---|---|---|---|---|
| `--texto-3` claro | `#7a6d66` | **4,36:1** sobre `--painel-2`, **3,95:1** sobre `--realce` | `#6b5e57` | 5,45:1 e **4,94:1** |
| `--texto-3` escuro | `#7c8598` | **4,30:1** sobre `--realce` | `#848da0` | 5,30:1 e **4,78:1** |
| `--acao-alterar` claro | `#8a6a1f` | **4,40:1** sobre `--painel-2` | `#7d5f18` | 5,20:1 e 4,72:1 |

Os dois primeiros valiam para **cabeçalho de tabela, rótulo de KPI e legenda
de cartão** — ou seja, apareciam em quase toda tela. O terceiro é cor de texto
de botão, e por isso o piso dele é 4,5:1 e não 3:1.

**A regra que sai daí:** um token de texto se mede contra o **pior** fundo em
que ele aparece, não contra o fundo da página. `--texto-3` passava folgado
sobre `--fundo` e reprovava sobre `--realce`; quem só olha o primeiro par não
vê nada de errado.

**Medido e mantido:** `--laranja` claro (`#c63c0a`) dá 4,10:1 sobre `--realce`.
Fica: sobre `--realce` ele só aparece como **ícone** (`.op-ico`), e grafismo
tem piso 3:1. Como cor de texto ele mora sobre `--fundo`, onde mede 4,76:1. E
o valor é decisão documentada da marca — mudá-lo exigiria mudar a marca.

Reexecutar a medição: `docs/design/exercicio.mjs` (§7).

---

## 6. O que foi consertado nesta rodada

**Erro** (ilegível, inalcançável, mordido pelo CSS global, dado deformado):

1. `--painel-3` e `--pend` eram usados e **nunca definidos**. `var()` sem valor
   não cai no anterior: invalida a declaração inteira. O hover da barra de
   ferramentas e do menu não pintava fundo nenhum, e a **bolinha «a fazer» das
   três ferramentas que ainda não existem era transparente** — o aviso não
   existia na tela.
2. Colisão `.ferramentas` → a barra da moldura passou a `#ferramentas`
   (as sete fileiras no celular saíam daqui).
3. Colisão `.painel` → a área de conteúdo passou a `#painel` (o nó «Painel» da
   árvore tinha 76px em vez de 28px).
4. Colisão `.recado` → o do cartão de entrada passou a `#entrada .recado`
   (a pílula de aviso da barra estava fora do eixo).
5. Contraste: os três tokens de §5.
6. O celular não era apertado, era **quebrado**: a árvore fixa de 268px deixava
   **122px** para o conteúdo e `.corpo` cortava o que passasse — as cinco abas
   mediam 424px nessa faixa e **a aba Integridade não tinha como ser clicada**.
   Resolvido pelo painel retrátil, por `min-width:0` nos filhos do grid e por
   `.abas{overflow-x:auto}`.
7. As onze grades `auto-fit` com piso rígido, que transbordavam em vez de
   encolher.
8. O véu da gaveta cobrindo os botões da barra de cima.
9. `#eu` corta com reticências no celular e ganhou `title` com o texto inteiro
   — corte que avisa é honesto, corte que perde o dado não.

**Gosto** (feito porque era barato e não conflita com ninguém): a barra de
ferramentas rola numa fileira só abaixo de 1025px; `.cartas` cai para 280px de
piso no tablet; `.kpis` e `.fichas` ficam com duas por linha no celular em vez
de uma torre.

**Medido e deixado como está:** o desktop. Ele já estava bom — a barra em duas
fileiras mostra as trinta ferramentas sem esconder nenhuma, e economia de
mudança é virtude aqui.

**Deixado para quem é dono:** a colisão `.modo` (cartão de entrada × cartão de
escolha do diálogo). A frente do login já a consertou, escopando dentro de
`#entrada`; consertá-la aqui também só produziria conflito de merge.

---

## 6.1 O defeito da tela larga: texto de SVG sobreposto

O cartão «A máquina» desenhava o caminho do diretório de dados e o
«9,4 GB livres de 37,0 GB · 214,9 GB reservados» como dois `<text>` do **mesmo
`<g>`**, um em `x=0` e o outro ancorado no fim de um `viewBox` de 800. Texto de
SVG **não quebra, não corta e não empurra ninguém**: quando não cabe, ele passa
por cima do vizinho.

Aparecia já a 1920 e ficava grotesco a 5120 — mas medindo, ele estava **em todas
as larguras**, inclusive 390 e 820: o que decide não é o monitor, é o
comprimento do caminho. Num servidor com o `base` numa pasta funda, o número
que interessa fica ilegível no celular também.

Não foi consertado dando mais espaço ao texto — foi consertado tirando o texto
do SVG. Em HTML a célula quebra em vez de invadir a vizinha, e **sobrepor deixa
de ser possível por construção**, e não por cuidado. O caminho está inteiro (não
há reticências): quem cede espaço é ele, que quebra; o valor nunca quebra,
porque cortado ele perderia o sentido.

**A prova é dos dois lados**, como manda a casa: o caso `responsivo` da bateria
**planta um caminho comprido** antes de medir, porque o diretório temporário da
bateria é curto e sem plantar o teste passaria por engano. Com o defeito
reposto, ele reprova nas cinco larguras.

---

## 7. Como exercitar

Interface só se prova exercitando, e **os quatro números que importam não se
enxergam lendo o código**. Os roteiros estão em `docs/design/`:

| Roteiro | O que faz |
|---|---|
| `exercicio.mjs` | percorre 32 telas × 3 viewports × 2 temas = **192 combinações**; mede rolagem, transbordo, corte-sem-rolo e contraste; guarda uma captura de cada |
| `lateral.mjs` | 27 conferências do painel retrátil: recolhe, expande, pina, despina, sobrevive ao recarregar, arrasta, anda pelo teclado, e o botão de reabrir existe em todos os estados |
| `colisoes.py` | a varredura de nome de classe repetido (§3) |
| `contraste.py` | a conta de contraste fora do navegador, para escolher um token novo antes de gastar uma rodada |

O que `exercicio.mjs` mede, e **por que não basta medir o óbvio**:

- `documentElement.scrollWidth <= innerWidth` — com `body{overflow:hidden}`
  isso é **sempre verdade**, inclusive numa tela cortada. Medir só isso responde
  «não vaza» quando o console está quebrado. Ficou, mas como piso.
- **o que passa da borda direita** sem ter um contêiner rolável no caminho —
  esse é o teste honesto de «a página não rola de lado». Um botão da barra de
  ferramentas em 1806px não é defeito: a barra rola.
- **o que está cortado sem rolo** (`overflow:hidden` com `scrollWidth >
  clientWidth`, ignorando `text-overflow:ellipsis`) — conteúdo inalcançável, que
  é pior que rolagem. Era **28 telas**; é 0.
- **contraste** dos pares texto/fundo dos componentes principais.

Estado na entrega, nas 192 combinações: **0 rolagem de corpo · 0 transbordo
sem rolo · 0 corte inalcançável · 0 par abaixo de 4,5:1** — contra 28 telas com
corte inalcançável e 9 reprovações de contraste antes.

---

## 8. Ao acrescentar um componente

1. **Abra no navegador e olhe** — nos dois temas, em 390px **e em 5120px**. Ler
   o código não acha nenhum dos defeitos deste documento.
2. **Nome de classe já existe?** Rode `colisoes.py`. Se for peça de moldura,
   use o id.
3. **Cor nova nasce token**, nos dois temas, e se mede contra o **pior** fundo
   em que vai aparecer.
4. **Grade `auto-fit`** usa `minmax(min(Npx,100%),1fr)`. O teto vai no **item**
   (`max-width:var(--teto-cartao)`), nunca no `minmax` — ver §4.1.
5. **Grade larga** vai dentro de um `.rolo`.
6. **Não desfaça o CSS global com um curativo** (`.no.painel`, `td label`):
   escope a regra que está mordendo.
7. **Escreveu texto?** Ele tem teto: `max-width:var(--medida)`. Um parágrafo sem
   teto mede 5.040px num monitor duplo.
8. **Escreveu `<text>` dentro de um SVG?** Ou o desenho nasce na largura medida,
   ou a largura dele é fixa em px. `viewBox` esticado multiplica a fonte, e dois
   `<text>` do mesmo `<g>` se sobrepõem em silêncio — §6.1.
9. **Um par rótulo→valor** não estica: quando sobra largura, ele vira **coluna**
   (`columns:300px`), não uma linha de dois mil pixels.
10. **Nada centraliza acima de 2000px.** Num monitor duplo o meio da janela é a
    emenda física entre os dois — §4.1.

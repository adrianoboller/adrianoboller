# Multitela — abas vivas, regiões lado a lado e janelas

O console nascia com **uma tela por vez**: `folha(...)` trocava o `innerHTML`
do `#painel`, e abrir a próxima matava a anterior. Uma consulta com resultado,
uma grade rolada até a linha 800, a telemetria coletando — tudo se perdia ao
trocar de tela.

Este documento é o desenho do que substituiu isso, o que ele **não** faz, e o
que muda em cada navegador. O código está em `crates/phxsql-server/ui/`:
`multitela.js` (mecanismo) e `multitela.css` (folha própria).

## O pedido, e como ele mudou três vezes

O pedido chegou como *«arrastar uma tela para outro monitor, destacar uma aba
em janela independente, memorizar posição e monitor»* — no molde do WINDEV(R).
Depois vieram duas fotos: uma ultrawide com o WINDEV aberto em **três painéis
verticais lado a lado numa janela só**, e um monitor de 49" ligado por daisy
chain. E então a frase que fechou a questão:

> «É um site, então tem que esticar o navegador para todas as telas e dentro
> da página 1 ou índex distribuir as janelas dentro da mesma page.»

Isso inverteu a prioridade, e para melhor: **a resposta principal é layout, e
layout funciona em todo navegador**. A janela do sistema operacional continua
existindo, mas virou a segunda resposta — para quem prefere janelas soltas de
verdade.

Os três modos, todos dentro do mesmo `index.html`:

| modo | o que é | do que depende |
|---|---|---|
| **abas** | várias telas vivas na mesma região, uma na frente | nada |
| **regiões** | a área central dividida em 2, 3 ou 4 colunas, cada uma com a própria tira de abas | nada |
| **janelas soltas** | janelas flutuantes *dentro da página*, arrastáveis pelo cabeçalho e redimensionáveis pelo canto | nada |
| *(janela destacada)* | uma janela do sistema, fora da página | `window.open`; e a Window Management API só para abrir no monitor certo |

## As quatro telas nomeadas

O dono nomeou as quatro que importam, e são as que têm endereço próprio:

1. **Diagrama ER** — `?tela=diagrama&db=…`
2. **Telemetria** (o SQL Check) — `?tela=telemetria`
3. **Profiler** — `?tela=profiler`
4. **Query / Consulta** — `?tela=query`

Além delas: `painel`, `usuarios` e `tabela` (`?tela=tabela&db=…&tab=…`).

**O que ficou de fora do catálogo, e por quê:** toda tela que só faz sentido
com um formulário meio preenchido — o assistente de pivot, a junção, a união,
o copiar/colar de tabela, o restaurar de backup. Reabri-las por URL restauraria
a moldura e não o trabalho, e uma restauração que perde o trabalho e não avisa
é pior que não restaurar. Elas continuam funcionando normalmente em qualquer
aba, em qualquer região e em janela solta — o que não têm é **endereço** e,
portanto, não voltam sozinhas na abertura seguinte.

## As três decisões que sustentam o resto

### 1. Os ids moram na tela COM FOCO, e só nela

A página inteira fala por `$("#painel")`, `$("#titulo")`, `$("#abas")` —
centenas de lugares. Com quatro telas na tela ao mesmo tempo, quatro
`id="painel"` fariam `querySelector` devolver o primeiro em ordem de documento,
que quase nunca é o certo.

Então: **toda tela é desenhada por CLASSE** (`.painel`, `.abas`, `.cabecalho`),
e só a tela com foco ganha os **ids**. Trocar o foco move quatro atributos.
Nenhuma das centenas de chamadas mudou, e os módulos que buscam por id
(`telemetria.js`, `phx-grid`) continuam achando o que procuram.

Clicar em qualquer lugar de uma região dá o foco a ela — é o que decide qual
pane o menu e a barra de ferramentas comandam, como em qualquer editor com
painéis.

### 2. Aba escondida sai do documento

Aba que não está na frente tem o `.tela` **desanexado** — guardado em
JavaScript, com os `value` dos campos e os ouvintes intactos, mas fora do
`document`. Três consequências boas de graça:

- não há id repetido entre abas da mesma região;
- todo laço que já perguntava «ainda estou na tela?» para sozinho — o Profiler
  pergunta por `$("#pfCorpo")`, a telemetria por `$("#tlmBolhas")`;
- a aba escondida não consome desenho.

**A rolagem é o único estado que desanexar perde** — medido, zera —, e por
isso ela é salva antes e reposta depois, à mão. Vale para os três caminhos:
trocar de aba, soltar numa janela e acoplar de volta.

### 3. O `est` é da tela com foco, e a troca é explícita

`est` é um objeto só. Parte dele é **do servidor** (`sessao`, `usuario`,
`token`, `bancos`, `servidor`, `textos`, `rotulos`, a área de transferência) e
parte é **da tela** (`atual`, `aba`, `ordem`, `linhas`, `teto`,
`esquemaAtual`, `grade`, `painel`, `rascunho`, `pivot`, `maquina`,
`relogioMaquina`).

Duas abas de tabelas diferentes brigariam pelo mesmo `est.atual` — e esse
defeito só aparece com duas abertas, que é justamente o que ninguém testa. A
lista `DA_TELA` separa as duas metades; trocar de foco salva as chaves da tela
que sai e repõe as da que entra. O resto continua único, porque é mesmo único.

> **Um defeito real que isto já custou.** `focar()` *aplica* o estado guardado
> da tela. A tela com foco tem estado guardado **velho de propósito** — o vivo
> é o próprio `est`. Forçar `W.foco = null; focar(t)` para «reenfocar» a
> própria tela aplicava o velho por cima do vivo: soltar a grade de uma tabela
> numa janela devolvia `est.atual` nulo, e a janela não sabia mais o que
> mostrava. A regra é: **nunca reenfoque quem já tem o foco.**

## Quantas regiões cabem — o número é MEDIDO

Quem divide é a pessoa, com o botão. Divisão automática que muda sozinha
quando a janela redimensiona é desorientadora: a tela se reorganiza debaixo do
dedo de quem estava lendo. O que a largura faz é **apagar** o que não cabe —
e os itens do menu «Ver» acendem sozinhos quando a janela estica.

O teto é `largura útil ÷ 660 px`. Os 660 saíram de medição, não de palpite:
`testes-web/medir-regiao.mjs` estreita a região de 20 em 20 px e pergunta ao
navegador a partir de que largura o conteúdo passa a exigir rolagem lateral
(contando os `.rolo`, que rolam por dentro):

| tela | cabe até |
|---|---|
| Consulta (Query) | abaixo de 260 px |
| Telemetria | 600 px |
| Profiler | 600 px |
| **Diagrama ER** | **660 px** |
| Conteúdo de uma tabela de 7 colunas | 1160 px |

`MIN_REGIAO = 660` é o pior caso **das quatro telas nomeadas**. A grade ficou
de fora do critério de propósito: uma tabela larga rola de lado em qualquer
largura — é para isso que serve o `.rolo` —, e usar 1160 exigiria 4640 px
úteis para quatro regiões, o que estragaria o caso de uso principal por causa
de uma tela que já resolve o problema sozinha.

## O daisy chain: alinhar as calhas com as emendas físicas

Uma janela de 5120 px pode ter **dois monitores por baixo**. Uma região que
caia em cima da emenda é uma região partida ao meio.

Quando a `Window Management API` está disponível, `PhxTelas.emendas()` calcula
onde ficam as bordas de cada monitor **dentro** da área de trabalho da página,
e «Ver → Alinhar com as bordas dos monitores» põe uma calha em cada uma. É o
melhor uso dessa API neste console — mais útil que abrir janela em monitor.

A conta tem uma aproximação declarada: `screenX` aponta para a borda **externa**
da janela, e a moldura do navegador é estimada por
`(outerWidth − innerWidth) / 2`. Isso vale em todo navegador de mesa, onde a
moldura lateral é simétrica; num navegador com barra lateral vertical a emenda
sai deslocada da largura dessa barra. Não há API que dê o deslocamento exato.

Sem a API — Firefox, Safari, ou permissão negada — a divisão é em **partes
iguais**, e a tela diz isso em vez de fingir que é igual.

## O pino

Um só glifo, um só significado, e o mesmo do painel lateral esquerdo: **«fica
assim quando eu voltar»**. E o mesmo rodapé: *recolhido, pinado e largura ficam
neste navegador — não no servidor*.

Tudo em `localStorage`, na chave `phxsql-multitela`:

```json
{ "v": 1,
  "regioes": [ { "peso": 1.2, "abas": [ { "chave": "diagrama", "params": {"db":"loja"} } ] },
               { "peso": 0.8, "abas": [ { "chave": "telemetria", "params": {} } ] } ],
  "soltas":  [ { "chave": "profiler", "params": {}, "g": { "x": 40, "y": 30, "w": 880, "h": 560 } } ],
  "janelas": { "query": { "x": 2600, "y": 120, "w": 1100, "h": 760,
                          "monitor": "Direito", "mx": 2560, "my": 0, "dpr": 2 } } }
```

- **A divisão** (quantas regiões e o peso de cada uma) é lembrada sempre — é
  preferência, como a largura da lateral.
- **As abas** só voltam se estiverem **pinadas**. Quem nunca clicar no pino
  abre o console exatamente como antes deste modo existir. Guarda nova entra
  pedida, e aqui o pedido é o clique no pino.
- **Aba sem endereço não pina.** Uma folha avulsa (o Backup, a Junção) que
  «voltasse» traria a moldura sem o trabalho. Pinar essas seria mentir.
- **A geometria da janela solta é gravada a cada arrastar e redimensionar** —
  não há um segundo clique de confirmação, e a pessoa acabou de dizer onde ela
  quer a janela.

### Pixel CSS, e não pixel físico

`x`, `y`, `largura` e `altura` são guardados em **pixels CSS**. Três motivos, e
o terceiro é o que decide:

1. é a única unidade que `window.open` aceita, e a que `screenX`/`screenY`
   devolvem;
2. converter para físico exigiria saber o `devicePixelRatio` do monitor de
   destino **antes** de abrir a janela — e sem a Window Management API não se
   sabe;
3. o pixel CSS é o que decide **se a grade cabe**. Uma janela de 1100 px CSS
   mostra as mesmas colunas em 1× e em 2×; o que muda é o tamanho físico dela
   na mesa. Preservar as colunas vale mais que preservar centímetros.

O `dpr` do momento é guardado junto, mas só para o documento poder dizer isto.

### Monitor que sumiu, e janela que não cabe

**Janela destacada:** ao restaurar, o `label` do monitor é procurado na lista
de agora. Se ele não estiver mais lá, a janela abre no **principal**, presa
dentro da área disponível dele — e um aviso vermelho diz que caiu. Janela que
abre fora da vista é janela perdida: existe, consome sessão, e ninguém a vê
para fechar.

**Janela solta (dentro da página):** a mesma coisa, contra a área da página de
agora. `prender()` corta a posição e o tamanho para dentro do visível e avisa
que prendeu.

## O custo, medido

Números do caso `multitela` da bateria, num Chromium sem cabeça:

| situação | pedidos a `/api` |
|---|---|
| Telemetria **visível**, sozinha | 4 em 8 s |
| Telemetria **escondida** atrás da Consulta | **0** em 8 s |
| Telemetria aberta | 3 em 6 s |
| Telemetria **fechada** | **0** em 6 s |
| **Quatro telas visíveis** (ER + Telemetria + Profiler + Consulta), 3240 px | **15 em 10 s** (≈ 90/min) |
| Uma tela parada (Consulta) | 0 em 10 s |

Duas leituras honestas desses números:

- **Aba escondida custa zero**, e não «quase zero». Mas o mérito é dividido:
  desanexar do documento já faz a telemetria e o Profiler pararem sozinhos
  (eles perguntam pelo próprio nó antes de pedir). O par `PhxTelas.laco(...)`
  registrado por cada tela com relógio é o que garante o **outro lado** —
  religar ao voltar. Sem ele a aba volta morta, e foi assim que a prova real
  deste caso falhou quando o par foi retirado de propósito.
- **Quatro telas visíveis custam ≈ 90 pedidos/min**, e é o preço do modo que o
  dono pediu — no modo lado a lado ninguém está escondido, e pausar seria
  mentir sobre o que a tela mostra. Não há escalonamento dos relógios hoje:
  a Telemetria pergunta de 2 em 2 s e o Profiler de 1 em 1 s, e eles podem
  cair no mesmo instante. **Isso é uma pendência medida, não um mistério** —
  escalonar valeria uma rodada se o número incomodar, e o número está aqui
  para poder incomodar.

## O que este modo NÃO faz

**Não existe «arrastar a janela de volta para a barra de abas».** O navegador
não recebe evento nenhum quando uma janela do sistema passa por cima de outra —
não há `dragover` entre janelas, não há posição de ponteiro fora do documento.
O docking clássico do WINDEV(R) e do Visual Studio(R) **não é implementável
como arrasto** em navegador nenhum. O que dá, e está feito:

- **⤺ devolver**, na janela destacada, que manda a tela de volta pelo
  `BroadcastChannel` e fecha a janela;
- **⇤ acoplar**, na janela solta dentro da página;
- e o arrasto **dentro** da moldura, que o navegador enxerga porque começa
  dentro dele: reordenar abas, mudar uma aba de região, e arrastar uma aba
  para fora da área de regiões para virar janela.

**Não reabre sozinho as janelas destacadas ao carregar.** `window.open` sem
clique é bloqueio de popup em todo navegador. O arranjo fica guardado e volta
com um clique.

**Não guarda a sessão no disco do navegador.** Ver a seção seguinte.

**Não divide no celular.** Abaixo de 640 px as regiões empilham e as calhas
somem. Uma região de 190 px não é uma região, é uma coluna ilegível. O número
de regiões continua guardado — só não se desenha ali, e volta ao alargar.

**Não abre a mesma tela duas vezes.** Pedir uma tela já aberta traz a aba dela
para a frente. Duas cópias dividiriam os ids de dentro do painel (`#grade`,
`#pfCorpo`, `#cDb`) e a segunda roubaria os cliques da primeira. Pedir uma tela
já aberta **numa outra região** a move para lá — quem monta o arranjo está
dizendo onde cada tela vai.

## Como a sessão viaja (janela destacada)

Este era o ponto delicado quando a janela do sistema era a resposta principal.
Ele continua valendo, e o desenho é este:

1. a janela filha sobe em `?tela=…&destacada=1` e **não** mostra o formulário:
   ela publica `{t:"quero-sessao", de:<id>}` num `BroadcastChannel` da mesma
   origem;
2. a janela mãe responde `{t:"sessao", para:<id>, sessao, token, usuario, …}`;
3. a filha põe isso em memória (`est`) e chama `abrirApp()`;
4. **sem resposta em 2,5 s** — a mãe já fechou, ou está noutra origem —, a
   filha mostra o formulário de login com o recado *«a janela principal não
   respondeu»*.

**A ficha de sessão nunca encosta no `localStorage`.** O disco do navegador é
lido por qualquer outra aba da mesma origem e sobrevive ao fechamento do
navegador; a memória de uma aba, não. O caso `monitores` da bateria confere
isso literalmente: lê todas as chaves do `localStorage` da janela filha e
falha se o identificador de sessão aparecer em alguma.

`BroadcastChannel`, e não um barramento interno, **só porque as janelas são
documentos diferentes** — dentro da mesma página não haveria o que transportar.
No modo lado a lado, que é o principal, não há transporte nenhum: mesma página,
mesmo `est`, mesma sessão.

## O que muda em cada navegador

| | abas | regiões + calha | janela solta na página | `window.open` | abrir no monitor certo | alinhar com as emendas |
|---|---|---|---|---|---|---|
| Chrome / Edge 100+ | sim | sim | sim | sim | **sim**, com a permissão `window-management` | **sim** |
| Firefox | sim | sim | sim | sim | não — abre onde o navegador quiser | não, divide em partes iguais |
| Safari | sim | sim | sim | sim | não | não |

A `Window Management API` exige **contexto seguro**. `http://127.0.0.1` é
contexto seguro, então o console local se qualifica sem HTTPS. Num servidor
remoto por HTTP puro ela não existe — e aí Chrome se comporta como Firefox,
que é o degrau já previsto.

A tela **diz** em qual dos casos ela está, em «Ver → Sobre o modo multitela…».

## O que a bateria prova, e o que ela não prova

Dois casos novos em `testes-web/casos/`, ambos num tema só.

`12-multitela.mjs` — o comportamento **velho** (uma região, uma aba, um
`#painel`, nada gravado); estado por aba com duas tabelas; rolagem preservada
na troca de aba, ao soltar e ao acoplar; o mesmo nó do DOM viajando (marcado
com um `data-marca`); a aba escondida parando de pedir e a aba fechada
soltando o relógio, **contados**; duas regiões com a calha arrastada; uma aba
arrastada de uma região para outra; as quatro telas nomeadas vivas ao mesmo
tempo em 3240 px; a janela solta arrastada pelo cabeçalho e **não** arrastada
pelo corpo; o canto redimensionando; e o pino sobrevivendo a um recarregar.

`13-monitores.mjs` — a emenda entre dois monitores, o alinhamento das calhas,
o monitor pinado que sumiu, a janela presa dentro da vista, a janela destacada
pegando a sessão pelo canal, e a sessão **não** aparecendo no `localStorage`.

**O que fica sem prova real, dito sem maquiagem:** a `Window Management API`
não é exercitada de verdade. Ela existe no Chromium sem cabeça — o caso
registra isso —, mas pede a permissão `window-management`, que o Playwright
1.56 não sabe conceder (o nome não está na lista dele) e que o navegador sem
cabeça não concede sozinho: `getScreenDetails()` é chamada e **rejeita**.
Então ela é **dublada**, e o que se prova é o caminho *nosso* — achar a emenda,
alinhar as calhas, cair para o principal quando o monitor sumiu. O que fica
sem prova é a resposta do navegador; o que ela alimenta, não.

O **DPI diferente**, esse, se prova de verdade: o Playwright cria contexto com
`deviceScaleFactor`, e o caso carrega a página inteira num navegador de 2×. O
que se confere ali é a decisão de guardar pixel CSS — a mesma região mede
`1332 px` em 1× e em 2×, e cabem duas regiões nos dois.

**A troca de DPI em voo** (arrastar a janela de um monitor 1× para um 2×) tem
o ouvinte escrito — `matchMedia("(resolution: Xdppx)")`, cujo `change` dispara
quando a densidade muda —, mas **não é exercitada**: não há API no Playwright
para mudar a densidade de uma página já carregada. O que ele faz ao disparar é
redesenhar as regiões e chamar `atualizarVista()`.

## Onde este modo encosta em outro trabalho

O `#app` continua com a mesma grade e as mesmas faixas de largura (`@media` de
1024 e 640) do `index.html` — nada disso foi tocado. `multitela.css` só
acrescenta `main.corpo{position:relative}` (para a camada das janelas soltas
ter a que se prender) e veste `.corpo .painel` por classe, com os mesmos
valores que `#painel` já tinha, para as telas sem foco ficarem iguais.

Uma observação para quem cuida das faixas de largura: **com o navegador
esticado por vários monitores, a barra de 30 ferramentas e o menu de nove
títulos atravessam a tela inteira**. Aqui eles apenas envolvem/rolam e não
quebram nada, mas nada os impede de crescer. Se valer um teto de largura para
eles, é decisão do dono das faixas — não deste documento.

## Todo texto deste modo passa pela fábrica de idiomas

Sessenta e oito rótulos deste arquivo viraram setenta chaves da `FABRICA_TELA`
(`server/src/idiomas.rs`), com prefixo `tela.mt_`: as dicas da tira de abas, os
botões da janela solta, os dezesseis recados e a tela «Sobre o modo multitela».
É o primeiro arquivo de interface com **zero** texto cravado — o conferidor
(`cargo run --example textos-fora-da-fabrica -p phxsql-server -- --tudo`) não
o cita mais.

Três coisas deste módulo, em particular:

- **O `CATALOGO` usa o par `rot`/`txt`**, e não `txt(…)` direto, pelo mesmo
  motivo dos `MENUS` da página: ele nasce quando o módulo carrega, antes de a
  página ter pedido os textos ao servidor. Quem resolve é o `rotuloDe`. A
  entrada `tabela` **não tem** `txt` de propósito — o rótulo dela é o nome da
  tabela, que é dado, e dado não se traduz.
- **`PhxTelas.repintar()`** é o que o `aplicarIdioma` chama para o cromo daqui:
  a tira de abas e os botões da janela solta são pintados uma vez e ficariam
  na língua anterior. Ele repõe também o rótulo das abas cujo nome vem da
  fábrica — **por chave**, nunca comparando a frase com o rótulo antigo. O que
  ele não alcança é a aba de segundo plano cujo nome veio do título que a
  própria tela escreveu: repintá-la é repintar tela escondida, e isso
  contraria a decisão [2] deste módulo. Está nomeado no `docs/PENDENCIAS.md`.
- **A `telaAjuda` repõe o gancho `est.repintar`** que o `folha()` limpa: sem
  ele, trocar o idioma com esta tela aberta jogaria a pessoa no Painel.

E a lição de desenho que esta tela pagou, escrita por inteiro no
`docs/MENSAGENS.md`: a nota do alto era **uma frase picada pela marcação** —
`"funcionam em"` + `<b>qualquer navegador</b>` + `"— é layout…"` —, e pedaço
de frase é intraduzível por construção, porque a ordem das palavras muda de
língua para língua e em alemão o verbo vai para o fim. Hoje cada frase é uma
chave e a ênfase é uma **marca dentro do texto** (`**assim**`, e a palavra
entre crases); o corte em `<b>`/`<code>` acontece no `marcado()`, **depois**
da tradução, e o `marcado()` escapa tudo antes de marcar, porque o texto vem
de uma tabela que um administrador edita pela grade.

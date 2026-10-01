# Revisão medida da interface — 01/10/2026

Ordem do dono: «A interface deve ser revisada e qualificada.» Pedido ligado: **190** (bateria de todos os botões).
Papel E (designer). Nada aqui é de memória: todo número saiu de
`testes-web/revisao-da-interface.mjs` (o medidor, versionado) ou do conferidor
`textos-fora-da-fabrica`.

## 1. O que foi medido

**Inventário, saído do código.** O medidor lê `MENUS` e `FERRAMENTAS` da própria
página que o `http.rs` embute (`ui/index.html`), une os itens pelo texto da função
`faz` («Novo database…» mora em dois menus e é **uma** tela), e soma as cinco abas
da tabela e a tela de entrada:

| | n |
|---|---|
| itens de menu + barra, depois de unir os repetidos | 85 |
| telas medidas por combinação (inclui entrada e 5 abas) | 82–83 |
| fora, com motivo (Sair, Tema, Soltar numa janela) | 3 |
| cinza no estado (Server Mail, Blockchain, as regiões que não cabem na largura) — não medidas, como a pessoa não as alcança | 34 nas 6 combinações |
| combinações | 2 temas × 3 larguras (390, 1280, 1920) = **6** |
| telas × combinação medidas | **494** |

Fora do escopo, dito: o explorador da API (`ui/explorador.*`) é servido pelo
`rest.rs` noutra porta, não pelo `http.rs`.

**As réguas, por tela** (todas calculadas pelo navegador, não lidas do CSS):

1. erro — `pageerror`, `#aviso.mal`, `#painel .aviso.mal`;
2. contraste de **todo** texto visível contra o fundo efetivo (camadas
   semitransparentes compostas, opacidade da linhagem incluída), piso 4,5:1
   e 3:1 no texto grande; controle desabilitado isento (WCAG 1.4.3);
3. rolagem lateral da página;
4. botão de ação fora da convenção (fundo cheio em repouso, sem contorno, ou
   verbo de ação num `.botao` laranja cheio);
5. CSS global: `text-transform` sobre um **nome conhecido** do cenário
   (banco `revQualif`, tabelas, colunas, «Blumenau»…), caixa de marcar
   > 24 px, e a classe `.caixa` (o diálogo) fora de diálogo;
6. alvo de toque < 32 px — só em 390, e com **ponteiro de toque**
   (`isMobile`, `hasTouch`): a casa decidiu que o teste do dedo é
   `(pointer:coarse)`, não a largura, e medir 390 com mouse acusaria 1.897
   alvos que nenhum celular vê (medido no tema escuro: 1.897 com mouse, 1.158 com dedo).

## 2. Achados (antes) e o que aconteceu com cada um

Severidade: **ALTO** = dado errado, ilegível ou quebra; **MÉDIO**; **BAIXO**.
«n» = ocorrências somadas nas 6 combinações, quando não dito outro.

| # | tela | achado | n medido (antes) | sev. | depois |
|---|---|---|---|---|---|
| 1 | Configurações › Diretivas do banco | o **nome do banco** no título da seção sai em caixa alta («QUEM ALCANÇA REVQUALIF») — mentira sobre o nome gravado | 6 (todas as combinações) | ALTO | **0** — o nome vai num `<em>` sem transformação; o rótulo entrou na fábrica (`tela.dbd_quem_alcanca`) |
| 2 | Administração › Acessos | o **erro do servidor** (com nome de banco/tabela) num `.pino` em caixa alta | 1 (aparece quando há recusa no log) | ALTO | **0** — `.pino.crua` só quando é o texto do erro; o «negado» nosso continua rótulo |
| 3 | tela de entrada | a assinatura da marca *Built to store. Engineered to scale.* a **2,72:1** (claro) e 2,89:1 (escuro) — `opacity:.62` sobre `--texto-3` | 6 | ALTO (ilegível, < 3:1) | **0** |
| 4 | tela de entrada | «opcional» do campo Database a 2,62:1 / 2,78:1 (`opacity:.6`) | 6 | ALTO (< 3:1) | **0** |
| 5 | Nova tabela, Estrutura, LGPD | «coluna do motor» a 2,25:1 / 2,73:1 — a célula inteira a `opacity:.35` | 128 | ALTO (< 3:1) | **0** — só os controles apagam, a legenda não |
| 6 | Profiler | a caixa «só escrita» vestida de **cartão de diálogo flutuante** (sombra de 50 px, `width:100%`): o `label` usava a classe `.caixa`, que é a do diálogo | 1 por tema (achado na captura, provado pelo caso `css-global`) | MÉDIO | **0** — classe `de-marcar` |
| 7 | Nova tabela, Restaurar, Config. e diretivas, Gerais do servidor, Conteúdo | texto auxiliar (`.secao em`, `.criar label em`, `table.montar th em`, `.dica`) a 2,96–4,20:1 — `opacity` .70–.80 sobre `--texto-3` | 72 | MÉDIO | **0** |
| 8 | Telemetria | «pico …» e as quatro legendas de estado a 3,54–4,40:1 (`opacity` .75/.85) | 54 | MÉDIO | **0** |
| 9 | Gerir tabelas, Nova tabela, DbLink | **verbo de ação em `.botao` laranja cheio**: «Nova tabela…», «Criar tabela», «Nova ligação…» | 18 (3 × 6) | MÉDIO | **0** — verde contorno (inclui) |
| 9b | Copiar/colar, Editor de menu, Junção, União, Pivot, DbLink (gravar) | mesmo defeito, achado lendo o irmão: «Colar», «Aplicar», «Juntar», «Unir», «Montar a tabela dinâmica», «Gravar» | 6 botões | MÉDIO | verde (Colar; Gravar nova), amarelo (Aplicar; Gravar existente), azul (Juntar, Unir, Montar) |
| 10 | toda tela, 390 com dedo | alvo < 32 px: os botões do canto da tira (26 px de largura), pino/fechar da aba (28), ajuda e tema (24 de largura — o `flex` da barra os espremia) | 758 (379 por tema) | MÉDIO | **0** |
| 11 | toda grade (PhxGrid), 390 com dedo | filtro do cabeçalho 11×9, agregador 29×13, paginação/rodapé 26 de altura, campos de filtro 26 | 1.188 | MÉDIO | **0** de botão/campo; sobram 8 caixas de marcar da grade (ver #16) — piso só no `pointer:coarse` |
| 12 | Editor de menu, Nova tabela, Profiler, 390 com dedo | campos de formulário com 24–30 px de altura, botões de formulário (26–28), chips ativas/excluídas (22), trilha e legenda da telemetria (16–20) | 340 | MÉDIO | **0** |
| 13 | catraca dos textos | `TETO_ROTULOS_E_CRASE` em **880** com o conferidor medindo **879** — frouxa em 1 | 1 | MÉDIO (catraca frouxa não segura) | **871** (baixada ao medido depois das traduções) |
| 14 | — | 879 textos fora da fábrica (cobertura 72%) | 879 | BAIXO (dívida conhecida, catraca existe) | 871 |
| 15 | grades com dados (Conteúdo, Sessões) | nome de **coluna** em caixa alta no cabeçalho («CIDADE», «LIMITE») | 30 | BAIXO | fica — o `css-global` isenta `th` por decisão anterior (cabeçalho é rótulo); a coluna de nome misto («porNome») ficaria «PORNOME». Proposta: `.phx-th-titulo{text-transform:none}` precisa do dono da grade |
| 16 | 390 com dedo | caixa de marcar/radio com 13–15 px | 34 (+8 dentro da grade) | BAIXO | fica — o alvo é o `label` que a envolve, e o `css-global` reprova controle > 24 px |
| 17 | DbLink › Executar | «Executar» em laranja cheio | 1 | BAIXO | fica — ambíguo: a ligação pode escrever (`somente_leitura` desligado), azul «consulta» mentiria |
| 18 | PhxGrid agrupada | `.phx-gbox-bt` (.6), `.phx-gpill-dir` (.7), `.phx-grodape-rot` (.65) — mesma família do #7, não medidos (estado agrupado não aberto pela revisão) | — | BAIXO | fica, listado |
| 19 | Multitela 4 regiões | Telemetria cortando «0 B/s ler · 0» na borda (achado da captura de 79 telas) | **0 reproduzido**: regiões de 669 e 739 px (4 regiões a 2.960 e 3.240 px) e região única de 512 e 632 px, dois temas — nenhum filho de `.tlm-faixa` passa da borda; o valor quebra linha | — | causa provável: a captura do dossiê estava quebrada (2.800 px: cabem 3 regiões, o `dividir(4)` recusava). Consertada, ver §4 |

Sem achado, medido: **0** erro (`pageerror`/recado/painel) nas 494 telas;
**0** rolagem lateral; **0** botão de ação com fundo cheio em repouso; **0**
caixa de marcar esticada.

## 3. Antes × depois (as 6 combinações)

Mesma régua, mesmo inventário, mesmo cenário. Contraste, ação, CSS global, erro e
rolagem: 2 temas × 3 larguras = 494 telas. Toque: 390 com dedo, 2 temas = 164 telas.

| régua | antes | depois |
|---|---|---|
| erro (pageerror, recado, painel) | 0 | 0 |
| rolagem lateral | 0 | 0 |
| contraste < piso | **266** (em 12–13 telas por combinação) | **0** |
| botão de ação fora da convenção | **18** | **0** |
| `text-transform` sobre nome gravado — fora de cabeçalho de grade | **8** (título 6, pino 2) | **0** |
| `text-transform` sobre nome de coluna em `th` (BAIXO #15) | 30 | 30 |
| `.caixa` do diálogo fora de diálogo | 1 por tema (caso `css-global`) | **0** |
| alvo de toque < 32 px, 390 com dedo | **2.320** (cromo 758, grade 1.188, campos/botões 340, caixas de marcar 34) | **42** — todos caixa de marcar/radio (BAIXO #16) |
| textos fora da fábrica (conferidor) | 879, catraca 880 (frouxa) | **871**, catraca **871** |


## 4. O que mudou, e a prova

- `ui/index.html`, `ui/telemetria.css`: sem `opacity` em texto que já é
  `--texto-3` (oito regras); a `.lgpd-cel.fixa` apaga os controles e não a
  legenda.
- `ui/index.html`: título de Diretivas do banco e pino de erro de Acessos sem
  caixa alta no dado; nove botões com a cor da ação; `label.mini-campo.caixa`
  → `de-marcar`; bloco «dedo grosso» com piso de 32/40 px na tira, ajuda/tema
  e campos de formulário.
- `ui/grid/phx-grid.css` (+ CHANGELOG): piso de toque no `pointer:coarse`.
- `src/idiomas.rs`: 6 chaves novas nos seis idiomas; `src/conferidor.rs`:
  `TETO_ROTULOS_E_CRASE` 880 → **871**.
- `docs/dossie/capturar-dossie.mjs`: `cifra_fio:{exigir:false}` (a porta
  devolvia 403 desde o pedido 370) e a largura da multitela sai da página
  (4 × 660 + lateral + calhas = 2.948 px), recusando com o motivo se não
  couber. Rodada inteira: 20 capturas, multitela com 4 regiões.

**Prova real nos dois sentidos** (binário com o `ui/` do HEAD reposto, depois
com o conserto):

| caso | com o defeito | com o conserto |
|---|---|---|
| `09-cores` (régua nova: texto apagado por `opacity` + verbo de ação em laranja cheio, 8 telas) | FALHOU nos 2 temas: 4 botões e 17 textos (escuro 3,25–4,39:1; claro 2,96–4,40:1) | ok nos 2 temas |
| `06-css-global` (régua nova: nome conhecido em caixa alta em qualquer lugar; `.caixa` fora de diálogo; telas Diretivas do banco, Acessos e Profiler) | FALHOU nos 2 temas: «Quem alcança batCssE» (`h3.secao`), o erro `[SP000018]…` no `.pino` de Acessos, e `label.mini-campo caixa` no Profiler | ok nos 2 temas |

O `css-global` passou a isentar o `.pino` da regra antiga de «dado em célula»:
pino de célula é rótulo nosso por convenção («ok», «você»), e o pino que
carrega dado leva `.crua`; quem pega o pino que mente é a régua nova dos nomes.

## 5. O que ficou

Os BAIXOS 14–18 da tabela. Nenhum ALTO nem MÉDIO medido ficou aberto. O medidor fica em `testes-web/revisao-da-interface.mjs`
para a próxima rodada; ele **não reprova** — mede e grava JSON.

# Changelog — phx-grid

Formato baseado em *Keep a Changelog*. Versionamento semântico.

## [0.9.2] — 2026-09-02 · **O PAINEL VIVO, E A BUSCA QUE ENVELHECIA**

### Corrigido
- **`redesenhar()` não derrubava o índice da busca global.** A fonte local guarda esse índice em cache por conjunto de campos, o que é certo para uma grade estática e errado para um painel que se atualiza sozinho: depois da primeira volta do relógio, a busca respondia pelas linhas de dois segundos atrás. Resposta errada com a cara da certa é pior que busca lenta. Agora `redesenhar()` quer dizer «o dado mudou» e não «pinte de novo»: ele chama `fonte.invalidar()` antes de recarregar.

### Aprendido — a prova que passava por engano
- O caso `fonte_viva_redesenha_sem_perder_o_estado`, escrito na 0.9.1 para provar o padrão do painel vivo, **conferia o ESTADO e não o EFEITO**: afirmava que `estado().ordem` continuava `desc` depois de `redesenhar()`, e não que as linhas saíam ordenadas. Passava com uma `fonte` que ignora o `ordem` recebido — que é justamente o defeito que ele deveria pegar, e que só apareceu quando o gestor de threads da telemetria saiu torto na tela.
- A lição é a da casa, por outra porta: **com `fonte`, a ordenação é responsabilidade da FONTE** (o grid manda `{campo, dir, tipo}` e espera linhas ordenadas); com `dados`, é do grid. Painel vivo sobre um array em memória usa `dados` e muda o array NO LUGAR — nunca uma `fonte` caseira, que teria de reimplementar ordenar, filtrar e agrupar para não mentir.

## [0.9.1] — 2026-09-02 · **O ROTULO E O NOME SAO COISAS DIFERENTES**

### Corrigido
- **A grade quebrava o contrato que ela mesma documenta.** O LEIAME manda escrever coluna de ação como `{ campo: "acoes", titulo: "", ordenavel: false, formato: ... }`, e o código fazia `c.titulo || c.campo`: título declarado vazio caía no nome do campo, então a coluna de ação aparecia com **`__acao` escrito no cabeçalho** — e o CSV exportava uma coluna sem valor nenhum com esse mesmo nome interno por cabeçalho. Dois lugares, uma causa.
- A correção separa dois vocabulários que estavam no mesmo `||`: o **rótulo** é o que se pinta na cabeça da coluna (e aí título declarado manda, vazio inclusive); o **nome** é como se fala da coluna no seletor de colunas, no resumo de filtro e na pastilha de grupo (e aí uma caixa de marcar sem etiqueta seria pior que o nome do campo — esses três continuam caindo em `c.titulo || c.campo`, de propósito). O `rotulo()` usa `== null` e não `||`: com `||` a correção simplesmente não existe.

### Adicionado — a bancada que faltava
- **`testes-web/grade/bancada-grade.mjs`**: prova de contrato do componente, isolada, em Chromium de verdade. Carrega o `.js` e o `.css` **do disco** e monta grades em memória — sem servidor, sem login, e **sem a armadilha do binário velho**, porque não há binário no meio.
- Ela existe porque este defeito não tinha onde falhar: a bateria de `testes-web/` é de ponta a ponta, e um defeito do componente só aparecia através da tela que o usa, depois de recompilar o `phxsqld` que embute a página. Prova real nos dois sentidos: com o `||` reposto, **dois** dos cinco casos falham dizendo `__acao`; com o `rotulo()`, os cinco passam.
- Entre os cinco está o caso do comportamento **velho** (`sem_titulo_declarado_nada_muda`), que é o que mais importa numa mudança de significado: quem nunca declarou título continua vendo o nome do campo. Sem ele, «honrar o vazio» viraria «apagar o cabeçalho de quem não pediu».

## [0.9.0] — 2026-08-29 · **A GRADE QUE O CONSOLE LIGA** — a cerca, o congelar, a vista e a memória de layout

### Corrigido — o achado da rodada
- **O filtro Excel estava ilegível no console desde que a grade entrou.** Não era teoria: medido no Chrome, a caixinha de marcar da lista de valores tinha **204 px de largura** — o `input{width:100%}` da página — e empurrava o nome da cidade para fora do popover, então a lista aparecia como **oito quadrados sem texto nenhum**; o rádio E/OU dos Filtros de Número media **33,6 px**, a bolinha do tamanho da célula; e o `label{text-transform:uppercase}` mostrava «Blumenau» como «BLUMENAU», que é mentira sobre o dado. A cerca (`.phx-grid input[type=checkbox]`, `.phx-grid label`…) mora no `index.html`, e não aqui, porque o estrago vem de lá: esta folha é do ecossistema Phoenix e não tem por que conhecer o CSS do nosso console.
- **Seleção + agrupamento contavam cabeçalho de grupo como linha.** `atualizaMestre`, o "marcar todas" do cabeçalho e a faixa com Shift percorriam `ultimaCarga.linhas` inteiro — com grupos ligados, `__grupo` e `__rodape` caem no meio. O "marcar todas" nunca fechava (sempre menos marcadas que linhas) e a faixa punha no conjunto chaves que não existem no dado. Os três passaram a usar `eMarcador()`, e a **lista de marcadores é uma só** (`MARCADORES`), ao lado de quem os cria: peça nova no fim da lista quebra quem filtra pela primeira.

### Adicionado
- **Congelar coluna** (`api.congelar(campo, "esq"|null)` + alfinete ◧ no menu de Colunas). A coluna congelada **vai para a ponta da ordem**, porque o `sticky` gruda no lado do contêiner e não no lugar dela: congelar a quinta sem movê-la faria as quatro da esquerda passarem por baixo. O alfinete mora no menu de Colunas e não no cabeçalho pelo motivo prático de que a coluna que se quer congelar já saiu da tela quando dá vontade de congelá-la.
- **Exportar a vista** (`api.vistaAtual(cb)`, `api.csvDaVista(v)`, botão ⤓ no rodapé; `exportarVista:false` desliga). Não é exportar a tabela: sai **estas colunas, nesta ordem, com este filtro e esta ordenação**. Pede o conjunto **inteiro** à fonte e não a página — exportar a página 1 de 40 seria a mesma mentira do filtro truncado. CSV com `;` e BOM (o que o Excel em português abre sem perguntar) e **valor cru, não formatado**: "R$ 1.234,56" volta como texto e ninguém soma a coluna.
- **Layout lembrado** (`lembrar: "<chave>"`): largura, ordem, colunas escondidas, congeladas e itens por página, em `localStorage`, por grade. **Filtro e ordenação NÃO** — um filtro que volta sozinho ao reabrir a tela é a mesma mentira, com uma noite de intervalo. Layout é gosto; filtro é pergunta, e pergunta se refaz. Tudo em `try/catch`: em janela anônima o acesso **lança**, e uma grade que não abre por causa da memória de largura de coluna seria péssima troca. Coluna guardada que a tabela perdeu é ignorada, não ressuscitada. `api.esquecerLayout()` apaga.
- **Abrir a linha** (`aoAbrirLinha(linha, ix)`): **duplo** clique, porque o clique simples já é da seleção. A grade não sabe editar e não é ela que deve saber — quem recebe a linha abre a ficha, que carrega a versão do slot e recusa escrita concorrente.

### Versão — o que estava mentindo
O cabeçalho do `.js` e o do `.css` diziam **`v0.1.0 — Núcleo (S01)`** desde a S01, enquanto o `versao:` dizia `0.8.0` e o código já tinha **ordem por nível de grupo, rodapé de grupo e total geral** — que a 0.8.0 nem documenta. Três lugares, três respostas diferentes. Agora são conferidos entre si e contra o topo deste arquivo pelo teste **`grade_versao_nao_mente`** (`http.rs`): número visível ou sai de um gerador, ou está errado e ninguém percebeu ainda.

## [0.8.0] — 2026-08-26 · **S08 GROUP BY BOX** — o coração analítico abre (Fase 3 Pivot)

### Adicionado
- **Motor de agrupamento multi-nível**: `agrupa()` monta a árvore (grupos ordenados por chave tipada, nulos ao fim) com **contagens e agregados por grupo** (sum/avg/count/min/max — o ciclo da S02 finalmente consome); `achata()` lineariza pulando os **recolhidos**; **paginação corre sobre o achatado** (grupos contam como linha, como no DevExpress).
- **Group By Box** (`agrupavel: true`): faixa "Arraste uma coluna para cá" aceitando o **drop dos headers** (reuso do dragstart da S02), **pills** com × (desagrupa) e **drag entre pills reordena os níveis**; colunas agrupadas **somem do header** (inclusive das bandas — colspan desconta) e voltam ao desagrupar.
- **Render de grupo**: tr com colspan, caret ▸/▾, rótulo formatado pelo tipo (badge vira pill dentro do grupo), contagem "(N)" e **resumo de agregados inline** ("Valor: R$ 2,8M · Margem: 23,4%"); indentação por nível; clique alterna.
- **API**: `agrupar([campos])` · `grupos()` · `expandirGrupo(path, abrir)` · `expandirTodos(abrir)`; "Mostrando" vira "**N linhas em M níveis de grupo**"; contrato remoto ganha `grupos/recolhidos/aggCols/tiposCampos` — o pushdown de GROUP BY documentado. Log `phx.grid.group {campos, total, ms}`.
- Demo **`phx-grid-grupos.html`**: 1.500 pedidos já agrupados Região→Status (T3 viva).

### Corrigido (achado pela suíte)
- **A árvore de bandas ignorava o agrupamento**: `montaHeader`/`contaVisiveis` checavam só `ocultas` — coluna agrupada continuava no header. Ambos agora respeitam `agrupada()`; o repro isolado provou (`colunasVisiveis` certo, THs errados) antes do fix.

### Prova (bateria S08)
Suíte `grid-grupos` — **10 blocos; 8 suítes (81 blocos) verde 2×**: grupos == distintos ordenados com contagens == oráculo; **sort ativo ordena dentro dos grupos** (bloco do 1º grupo == oráculo); **agregados sum/avg == oráculo**; 2 níveis com **paths compostos** e contagens por sub == oráculo; **recolher esconde filhos e expandir volta bit-igual**; recolher pai esconde a subárvore; `expandirTodos(false)` = só pais; **× desagrupa e desagrupar tudo devolve o render pré-agrupamento bit-igual**; **reordenar pills inverte a árvore**; **drag header→box agrupa** com coluna sumindo do header e log; **paginação do achatado** (93 itens = 90+3 grupos).

### Telemetria (Chrome, medida — demo rebuildada)
**50.000 linhas · agrupar 2 níveis** (árvore + agregados + achatar + render): **13,1 ms** (mediana de 20) · recolher/expandir grupo 14,6 ms → **aceite <80 ms PASSOU com 6× de folga**.

Gates: `node --check` + acorn ES5 · single-file por grep.

## [0.7.0] — 2026-08-26 · **S07 BUSCA GLOBAL** — FullText com índice de engine e a tomada do Fx.ai (fecha a Fase 2)

### Adicionado
- **Busca global** (`buscaGlobal: true`): condição `busca` no campo virtual `"*"` do **mesmo store** (chip "Busca: \"termo\"" incluso, × remove) — termos separados por espaço em **E entre campos** ("silva pago" = silva em algum campo E pago em algum campo), sem acento nos dois lados; colunas buscáveis = textuais visíveis ou `cfg.buscaveis` explícito.
- **Ranking por relevância** quando não há sort: score = hits (termo×campo), estável; **sort do usuário sempre vence**. `api._scoreBusca(linha)` exposto.
- **Barra de busca** com debounce, **ESC limpa**, contador de resultados; API `buscar(termo, {modo})`; serialização remota `{campo:"*", termo, campos}` — o pushdown sabe **onde** buscar.
- **Gancho semântico** `cfg.buscaSemantica(termo, linhas, cb)`: modo `"semantica"` delega ordenação ao injetado (a tomada onde o Fx.ai pluga embeddings) — provado com stub; modo texto **não** o chama. Log `phx.grid.search {termo, modo, total, ms}`.
- Demo **`phx-grid-busca.html`**: 3.000 pedidos, instrução "tente \"silva pago\"".

### Otimizado (engine — a saga do aceite em 3 atos, tudo medido)
Aceite <60 ms em 100k abriu **falhando em 103,6 ms**:
1. **Passada única** busca+score no `fonteLocal` (decidir e pontuar juntos, cada campo normalizado 1× por linha): 103,6 → 77,6 ms. Cache de normalização testado e **revertido com dados** (campo único = miss permanente — só ruído).
2. **Índice de busca por campo, lazy na 1ª busca** (o mesmo desenho de Excel/DevExpress): buscas seguintes viram `indexOf` puro sobre colunas pré-normalizadas — **1ª busca 111,6 ms (constrói o índice, absorvida pelo debounce) · seguintes 18,4 ms → PASSOU**. Invalidação por chave dos campos; mapa sobra→índice global por ponteiro (válido porque o predicate ordering reordena condições, nunca linhas).

### Prova (bateria S07)
Suíte `grid-busca` — **9 blocos; 7 suítes (71 blocos) verde 2×**: conjunto == oráculo NFD (Silvânia via cidade incluída); **ranking 2-hits antes de 1-hit** e scores 2/1; **"silva pago" AND entre campos == oráculo (2 linhas)**; **sort vence ranking**; chip + coexistência com filtro de coluna == oráculo e × restaura; **contrato remoto espiado** (`campos:["a"]` só textuais); `buscaveis` restringe; **stub semântico reordena (reverse), modo texto não o chama**; debounce 1 aplicação, **ESC limpa input+filtro+contador**.

### Telemetria (Chrome, medida — demo rebuildada)
**100.000 linhas · 2 termos**: índice quente **18,4 ms** (mediana de 20) · 1ª busca 111,6 ms → **aceite <60 ms PASSOU**.

Gates: `node --check` + acorn ES5 · single-file por grep.

## [0.6.0] — 2026-08-26 · **S06 FILTER ROW + E/OU** — a T16 sob o header e os Filtros de Número da T15

### Adicionado
- **Condição `multi`**: N sub-expressões (`{op, valor}`) combinadas por **E/OU** no mesmo store — serialização estável, chip legível ("Valor > 1.000 E <= 5.000"), `passaExpr` extraída e reusada.
- **Painel "Filtros de Número"** no popup Excel (colunas numero/moeda/percentual): 2 condições com operadores por extenso ("é maior que"...), radios **E/OU**, **valores persistem ao reabrir** (filtro `expr` ou `multi` atual preenche os campos); preenchido, tem precedência sobre o checklist no OK — `filter.excel {numero:true, n, comb}`.
- **Filter row** (`filterRow: true`): linha de controles sob o header, um por tipo — texto→busca com **debounce** (`debounceMs`, default 300 ms), numérico→**op+valor** com debounce, data→**faixa do dia inteiro**, badge→**select de distintos** (50 primeiros, opção vazia remove), json/barra→vazias; células respeitam seleção e freeze. Row alimenta o **mesmo store**: chips e row nunca divergem — **× do chip e Limpar Todos zeram os controles** correspondentes.

### Otimizado (engine — achado pelo aceite que falhou)
A telemetria abriu em **84,4 ms** (limite 40). Decomposição honesta: o bench media **duas** aplicações e o texto pagava `semAcento`/`chaveOrd` **por linha**. Três correções de raiz:
1. **Memoização lazy nas condições** (`_q` do termo, `_kf` das expressões, `_kde/_kate` da faixa) — 1× por aplicação, não 100k×: 84,4 → 64,1 ms.
2. **Predicate ordering** no `aplicaFiltros`: condições baratas (valores/faixa/expr/multi) executam antes das caras (texto), cortando o dataset primeiro — princípio de query planner, o embrião do D5. AND comutativo ⇒ resultado idêntico (62 blocos verdes confirmam).
3. **Bench corrigido** para o que o usuário sente: **uma** aplicação com o combo no store.

### Prova (bateria S06)
Suíte `grid-frow` — **10 blocos; 6 suítes (62 blocos) verde 2×**: **tabela-verdade E/OU com 12 casos == oráculo** (inclui interseção vazia, união total, = e !=); **multi N=3**; chip multi; **popup numérico aplica e reabre preenchido**; **debounce cravado** ("3 inputs → 0 aplicações antes, exatamente 1 depois") e "acao" achando "Ação"; row numérica combinando com texto; select de badge aplica/remove; **date → dia inteiro == oráculo**; **× do chip limpa o input da row** e limparFiltros zera todos; **coexistência tripla** (row texto + checklist popup + multi) == oráculo.

### Telemetria (Chrome, medida — demo rebuildada)
**100.000 linhas · uma aplicação do combo multi(E,2)+texto**: **29,7 ms** (mediana de 20; 17.582 sobreviventes) → **aceite <40 ms PASSOU**. Decomposição: só multi 12,5 ms · só texto 29,7 ms → o ordering paga o combo quase ao preço do texto sozinho.

Gates: `node --check` + acorn ES5 · single-file por grep.

## [0.5.0] — 2026-08-26 · **S05 FILTRO EXCEL DA COLUNA** — a T15 completa, com a semântica que dá nome

### Adicionado
- **Popup de coluna** (funil ▾ no header, aceso quando há filtro): Classificar A–Z/Z–A, **Limpar Filtro**, **Pesquisar** (sem acento, cache por item), **(Selecionar Tudo) tri-state agindo só nos visíveis da busca**, checklist de distintos **formatados pelo tipo** (moeda ordena numérico e exibe "R$ 500,00"), **"Exibir itens sem valor"** quando há nulos (nasce **marcado** — default Excel), **truncamento em 500** com contador "mostrando X de N — refine", OK/Cancelar, fechamento por clique-fora/ESC.
- **Semântica Excel de verdade**: `valoresDistintos(campo)` calcula sobre o dataset filtrado pelos **demais** campos (**auto-exclusão do próprio**) — reabrir a coluna mostra a lista completa com só o subset marcado; **marcar tudo (nulos inclusos) remove o filtro**. Condição `valores` ganhou `incluiNulos`; contrato remoto: método opcional `fonte.distintos` documentado (sem ele, aviso honesto no popup).
- Demo **`phx-grid-excel.html`**: 800 pedidos com cidades/vendedores nulos — a T15 operável.

### Corrigido/Otimizado (achados pelas provas)
- **Checkboxes órfãos**: o re-render por clique deixava handlers em elementos desconectados (o jsdom expôs: `change` não dispara fora do DOM). Virou **delegação viva na lista** + atualização só do mestre — melhor produto (sem re-render por item, foco preservado) e teste determinístico.
- **Semântica do próprio teste corrigida**: "(sem valor)" nasce marcado como no Excel — o oráculo do teste é que assumia o contrário; [4] agora prova o toggle no sentido inverso (desmarcar exclui nulos).
- **`semAcento` por regex de classe** (uma passada) no lugar do char-a-char: abertura do popup no extremo de 50k distintos caiu **412,9 → 185,5 ms** (2,2×) — e todos os filtros de texto herdam o ganho.

### Prova (bateria S05)
Suíte `grid-excel` — **9 blocos; 5 suítes (52 blocos) verde 2×**: distintos de moeda ordenados numericamente e formatados; **distintos de Cidade sob Status=Pago == oráculo** e **auto-exclusão** (reabrir = lista completa, subset marcado); busca "goiania"→"Goiânia" com **Selecionar-Tudo só nos visíveis** e aplicação == oráculo; **nulos default-marcados** e toggle provado nos dois sentidos; **Cancelar = estado bit-igual**; **todos+nulos = filtro removido**; A–Z ordena e o **funil acende/apaga**; log `filter.excel {n}`; **trunca em 500 com contador "500 de 2.000"** e a busca refina a 1.
**Chrome real**: distintos de Cidade sob Pago = 7 + checkbox de sem-valor presente; popup da demo screenshotado.

### Telemetria (Chrome, medida — demo rebuildada)
**50.000 valores distintos**: abrir popup (distintos + cache + render 500) **185,5 ms** · **busca (filtrar 50k + render): 6,5 ms** (mediana de 20) → **aceite <30 ms PASSOU** com folga.

Gates: `node --check` + acorn ES5 · single-file por grep.

## [0.4.0] — 2026-08-26 · **S04 FILTROS + CHIPS** — o estado central que a Fase 2 inteira consome

### Adicionado
- **Store central de filtros** com 4 condições tipadas por campo: `valores` (lista aceita — a semente do Filtro Excel S05), `texto` (**busca sem acento PT-BR** por tabela própria ES5), `faixa` (de/até por chave tipada — números E datas) e `expr` (>, >=, <, <=, =, != — a semente do E/OU S06). Combinação **AND entre campos**; nulos tratados por condição.
- **Aplicação na fonte**: a `fonteLocal` filtra antes de ordenar/paginar e o `total` reflete o conjunto filtrado; o **contrato remoto ganha `filtros` serializados** (ordenação estável por campo + `tipoCol`) — exatamente o payload que o Query Planner/pushdown (C9) traduzirá para SQL.
- **Chips ativos** (T1): "Filtros Ativos (N)", resumo legível por tipo ("Status: Pago, Pendente +1" · `Valor: 1.000,00–5.000,00` · `Margem > 25`), **× remove individual**, "Limpar Todos"; a barra some com zero filtros.
- **API**: `filtrar(campo, condicao|null)` · `filtros()` · `limparFiltros()` · `estado().filtros`; filtrar volta à página 1. **Logs** `phx.grid.filter {campo, expr, total, ms}` e `filter.clear`.
- Demo **`phx-grid-filtros.html`**: painel de controles (multi-status, contém, faixa, slider de margem) alimentando os chips ao vivo — 500 pedidos.

### Prova (bateria S04)
Suíte `grid-filtros` — **9 blocos; 4 suítes (43 blocos) verde 2×**: **composição de 4 filtros linha-a-linha idêntica ao oráculo JS puro** (que usa `normalize("NFD")` — duas implementações independentes de acentos concordando); **remover 1 chip = oráculo dos 3 restantes; limpar tudo = dataset bit-igual na ordem natural**; chips com contagem/resumos/× individual/Limpar; "sao"↔"São"/"goiania"↔"Goiânia" nos dois sentidos; faixa aberta; **os 6 operadores de expr contra oráculo**; filtrar reseta p/ página 1 com Mostrando refletindo; **contrato remoto espiado** (campo+op+tipoCol); logs com expressão legível; **filtro→sort na ordem certa == oráculo**.
**Chrome real**: digitar "goiania" → 69 linhas, chip `Cliente: "goiania" ×`, primeira linha "Empório Goiânia".

### Telemetria (Chrome, medida — demo rebuildada antes, conforme o ritual)
**10k linhas · 3 filtros compostos re-aplicados** (aplicar + render, 3 chamadas encadeadas): **27,4 ms** (mediana de 20) → **aceite <40 ms PASSOU**.

Gates: `node --check` + acorn ES5 · single-file por grep (0 src, 0 http).

## [0.3.0] — 2026-08-26 · **S03 CÉLULAS RICAS** — a linha das telas T15/T17/T22, com seleção de verdade

### Adicionado
- **Seleção** (`selecao: true` + `chave`): checkbox por linha, **faixa com Shift-click** por âncora, **mestre tri-state** (checked/indeterminate) na página; **persistência entre páginas quando há chave** (sem chave, limpa ao paginar — documentado e provado); API `selecionar/selecionadas/limparSelecao`, `estado().selecao`, logs `phx.grid.select {n}`.
- **Novos renderers de célula**: `composta` (principal + sub com prefixo, ex. "ID: 10293"), `link` (com **sanitização de href** — `javascript:` vira `#`), `badge` (5 cores semânticas + mapa por valor; sem mapa = cinza — a coluna **Origem** da T22), `barra` (Margem % com fill proporcional + rótulo), `json` (**popover** escuro com o payload formatado — o layout das demais células fica **bit-intacto**, provado por snapshot).
- Delegação única de eventos no tbody (seleção/JSON) — zero listener por célula; `formata(c, v, linha, ix)` com acesso à linha; **telemetria interna** `api._ultimoRender()` (strMs/domMs/pagMs/fonteMs) nascida da investigação desta sprint.
- Demo **`phx-grid-celulas.html`**: 300 pedidos com ID fixo, compostas, links, Origem, Status colorido, barras e Detalhes(JSON) — T15+T22 vivas.

### Otimizado (raiz de produto)
- **Header só remonta em mudança estrutural** (init/sort/hide/reorder); troca de página, tamanho e `redesenhar` reconstroem apenas corpo+paginação — antes o header inteiro (com todos os listeners) renascia a cada página.

### Lição de processo (honesta, entra no ritual)
A telemetria acusou 47,7 ms e falhou o aceite; o raio-X interno mostrou str 0,4 + DOM ~12 ms... porque **a demo medida embutia o src antigo** — o `build.py` não tinha rodado após o patch. **Regra nova do ritual: rebuildar as demos antes de qualquer medição.** Rebuildada, a otimização do header já entregava o aceite.

### Prova (bateria S03)
Suíte `grid-celulas` — **10 blocos; 3 suítes (34 blocos) verde 2×**: composta com sub+prefixo; `javascript:` sanitizado; cores mapeadas + default cinza; barra `width:10.0%` + rótulo `10,0%`; **popover abre com o JSON formatado, tbody bit-igual, 2º clique fecha, log `expandjson`**; click marca por chave; **Shift-faixa = 5**; tri-state indeterminado → mestre marca a página; **seleção persiste entre páginas e os checks restauram**; sem chave limpa ao paginar; moeda intacta; `selecionar()` programática. **Chrome real**: popover abre/fecha com `offsetHeight` idêntico; **Shift-faixa 0..7 = 8 ✓**.

### Telemetria (Chrome, medida — demo rebuildada)
**100 linhas ricas × 9 colunas**: redesenhar **10,1 ms** · troca de página **11,4 ms** · seleção programática 20,2 ms → **aceite <25 ms PASSOU**.

Gates: `node --check` + acorn ES5 (módulo e demos) · single-file por grep.

## [0.2.0] — 2026-08-26 · **S02 BANDAS + FREEZE** — o header da T2 (anos × trimestres) com colunas congeladas

### Adicionado
- **Bandas multi-nível** via `cfg.bandas`: lista mista de campos soltos e bandas `{titulo, colunas|filhos}` normalizada em **árvore de profundidade arbitrária**; colspan = colunas visíveis descendentes (recalculado no hide), **rowspan automático** nas soltas, banda com zero visíveis some. A **ordem das colunas deriva do flatten da árvore** — bandas e ordem nunca divergem.
- **Reorder confinado ao pai**: arrastar dentro da banda funciona; entre bandas é **negado com log** `phx.grid.reorder-negado` (a banda nunca quebra).
- **Freeze esquerda/direita** (`fixa: "esq"|"dir"`): sticky com **offsets medidos** dos th publicados em regra CSS interna por coluna (th e td via `data-fx`); **sombra de rolagem** (`.phx-rolado`) com handler otimizado por flag (só toca o DOM na transição).
- **Agregador interativo**: o badge vira botão que **cicla sum→avg→count→min→max** com log `phx.grid.aggchange` — o estado que a S13 (totais) consumirá.
- Demo **`phx-grid-bandas.html`**: a tela T2 viva — 2024|2023 × (Trimestres Q1–Q4 | Acumulado), Categoria+Produto congeladas, Status à direita, 620 linhas LCG.

### Corrigido (pela raiz, achado pela prova no Chrome)
- **`border-collapse: collapse` quebra `position: sticky` em células no Chrome** — o freeze "funcionava" no jsdom e falhava no navegador real. Trocado por `separate + border-spacing: 0`; de bônus o init de 30 colunas caiu de 59,3 → 33,9 ms.
- Célula fixa com `background: inherit` vazava o conteúdo por baixo — regra dinâmica agora só posiciona; o fundo sólido é do CSS estático (hover incluído).

### Prova (bateria S02)
Suíte `grid-bandas` — **8 blocos, e as 2 suítes (24 blocos) verde 2×**: 3 linhas de header com colspan/rowspan exatos; flatten == ordem esperada; hide reduz colspan e **restaurar devolve o header bit-igual**; banda vazia some; reorder interno OK e **entre bandas negado sem mutação**; aggchange cicla com log; regras sticky publicadas com `left:0`/`right:0` e `data-fx` em todas as células fixas; **regressão S01 intacta** (grid sem bandas = 1 linha).
**Chrome real (a prova que importa)**: `scrollLeft=400` → **Categoria x=34 e Status x=1006 imóveis**, Vendedor rola (−113↔287), **td alinhado ao th** nas fixas, classe `phx-rolado` liga/desliga.

### Telemetria (Chrome, medida)
**30 colunas (6 bandas) × 300 linhas**: init **33,9 ms** · custo por mutação de scroll **3,5 ms** (dentro do orçamento de 16,6 ms/frame → **60 fps**).

Gates: `node --check` + acorn ES5 · demos single-file (0 CDN, 0 src — grep).

## [0.1.0] — 2026-08-26 · **S01 NÚCLEO** — o esqueleto que tudo usa (Onda 1 · Fase 1 do plano)

### Adicionado
- **`PhxGrid.criar(alvo, cfg)`** — colunas declarativas `{campo, titulo, tipo, largura, decimais, formato, agregador, dimensao, ordenavel, oculta}`; dados locais ou **fonte remota por contrato** `carregar({pagina, tamanho, ordem:{campo, dir, tipo}}, cb) → {linhas, total}`.
- **Header rico (T20)**: título + indicador de sort ▲▼, sub-rótulo de **dimensão** `(dim_tempo…)` e **badge do agregador** (SUM/AVG) — a semente do pivot; **resize** por arrasto (mín. 50px), **reorder** por drag&drop, **seletor "Colunas: N ▾"** com checkboxes.
- **Sort estável 3-state** (asc → desc → natural) com **nulos sempre ao fim**, chaves por tipo (numérico, texto case-insensitive, data por timestamp) e desempate pelo índice original.
- **Paginação completa**: « ‹ janela com elipses › », **ir para**, itens/página, "Página X de N (T registros)" e "Mostrando X–Y de N".
- **Formatação PT-BR própria**: moeda `R$ 1.234,50`, percentual `28,6%`, milhar `1.234.567`, dataHora `dd/mm/aaaa hh:mm:ss`.
- **Logs estruturados** `phx.grid.{init,sort,page,pagesize,resize,reorder,coluna,erro}` com duração (ms), bufferizados em `api.logs()`.
- API: `ordenar · pagina · tamanhoPagina · mostrarColuna · moverColuna · colunasVisiveis · linhas · estado · logs · redesenhar · destruir`.
- Demo **`demos/phx-grid-nucleo.html`** single-file (regra nº 1: 0 CDN, 0 src externo — verificado por grep) com 1.234 linhas LCG determinísticas no layout da tela T20.
- **Decisão D1 aplicada desde o nascimento**: fonte/estado separados do render (`fonteLocal` plugável) — o engine cognitivo cresce daqui sem retrabalho.

### Prova (bateria S01)
Suíte `grid-nucleo` — **16 blocos verde 2×**: sort estável **== oráculo decorate-sort** (asc e desc), numérico 9<10, dataHora por tempo, **nulos ao fim nas duas direções**, 3-state com **natural bit-igual ao snapshot inicial**, reorder refletido no DOM e no estado, show/hide sem resíduo, paginação sem vazamento de nós após 5 trocas, clamps (0→1, 9999→teto), recálculo de páginas, "Mostrando 481–500" na última parcial, formatos, **logs com ms**, estado serializável, **contrato remoto espiado** (params exatos), header rico renderizado.

### Telemetria (Chrome real, medida)
**10k linhas**: init **34,5 ms** · troca de página **10,0 ms** (mediana de 20) · sort completo **26,1 ms** → **aceite <50 ms PASSOU**.

Gates: `node --check` + **acorn ES5 estrito** (módulo e demo) · single-file provada.

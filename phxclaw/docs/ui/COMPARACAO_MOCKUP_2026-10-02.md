# SP000036 · Comparação da UI atual com o mockup do dono (02/10/2026)

Fase 1, só leitura e capturas. Ordem do dono: *«Melhore a interface atual, compare com esse que
enviei».* Mockup: `docs/ui/mockup-painel-2026-10-02.jpg` (1536×1024). UI medida: cópia de
`apps/phxclaw-ui` tirada às 04:09 de 02/10 (a W2 edita a original em paralelo; a cópia está em
`…/scratchpad/sp36/ui-copia`). Nada aqui foi editado na UI.

## 1. Capturas (exercitado, não lido)

Roteiro: `…/scratchpad/sp36/capturar.mjs` — o mesmo molde do `tests/desktop/ui_navegacao.mjs`
(`page.route` serve do disco, Tauri em stub, `/v1/tasks` e `/v1/config` falsos dos
`tests/desktop/dados/`), tema escolhido como o usuário escolhe (`phxclaw.tema` no `localStorage`
+ `colorScheme` do contexto), 1536×1024 (o tamanho do mockup). **Zero erros de console nos dois
temas; 7 telas por tema; fontes computadas: Exo 2 e IBM Plex Mono locais; 116 nós `data-txt*`.**

Pasta: `/tmp/claude-0/-home-user-adrianoboller/2069f8cc-5222-5b56-8192-d80cbf937721/scratchpad/sp36/`

| Tela | escuro | claro |
|---|---|---|
| Visão geral | `geral-escuro.png` | `geral-claro.png` |
| Agentes | `agentes-escuro.png` | `agentes-claro.png` |
| IDE (com bash aberto do stub) | `ide-escuro.png` | `ide-claro.png` |
| Ferramentas | `ferramentas-escuro.png` | `ferramentas-claro.png` |
| Absorção | `absorcao-escuro.png` | `absorcao-claro.png` |
| Tarefas | `tarefas-escuro.png` | `tarefas-claro.png` |
| Configuração | `config-escuro.png` | `config-claro.png` |
| Busca e comandos (Ctrl+K) aberta | `paleta-escuro.png` | `paleta-claro.png` |
| **Montagem** mockup · atual escuro · atual claro (3112×723) | `montagem-geral.png` | — |

Tokens lidos da página viva (não do fonte): escuro `--fundo #010418 --laranja #ff8a1c`; claro
`--fundo #f7f5f2 --laranja #c63c0a`. Os dois temas são desenhados, não invertidos (ver as duas
`geral-*.png`: o laranja escurece, o verde da absorção escurece, o papel é quente).

## 2. Bloco a bloco do mockup

Contei **22 blocos** na imagem. Duas contagens divergem do pedido: o menu lateral tem **19**
itens (não 17: Dashboard, Novo Projeto, Agentes & Swarm, Skills & Ferramentas, Modelos de IA,
Pesquisa & Conhecimento, Desenvolvimento, Git & Integração, Testes & Qualidade, Segurança,
Deploy & Infraestrutura, Monitoramento (SRE), Incidentes & Resiliência, Configuração, Plugins,
Documentação, Marketplace, Relatórios, Ajuda) e os cartões de ação são **6** (não 7: Pesquisar,
Analisar, Construir, Testar, Implantar, Entregar).

Critério: **existe** = o bloco está na UI com dado real, em qualquer tela; **parcial** = parte
dele; **não** = nada. Fonte de dado = o que alimentaria o bloco **hoje**, sem número digitado.
Rotas conferidas por `grep '\.route("'` em `crates/` e `apps/` (26 rotas, das quais em `/v1/`:
`config`, `config/perfis`, `events`, `events/publish|sse|ws`, `health`, `ide/completar|simbolos|terminal`,
`schedules`, `tasks` e 6 subrotas, `triggers/{nome}`).

| # | Bloco do mockup | Estado | Onde está hoje | Fonte de dado real |
|---|---|---|---|---|
| 1 | Logo + «AI-Native Engineering Platform / IDEA → … → IMPACT» | parcial | `index.html` `.brand-mini` («COMMAND CENTER • versão») | `host_status.version` (Tauri). A frase-tagline é texto de marca → fábrica (`splash.strap` já existe) |
| 2 | Barra de comando central «Digite um comando…» Ctrl K | existe (como diálogo, não barra) | `index.html` `#paleta`, `app.js:751` (`paleta`) — lupa no topo + Ctrl+K | `TELAS`, `equipe.json`, `ferramentas.json` (já faz: ir para tela, idioma, terminal, nova tarefa, agente/ferramenta pelo nome) |
| 3 | Seletor de workspace («ENGINE PRINT 3D») | não | — | `/v1/config` chave `agente.pasta` e escopo `pasta`/`projeto` (`config.js`); `/v1/config/perfis` (perfil ativo). Não há lista de workspaces → mostrar a pasta ativa, sem seletor |
| 4 | Sino de notificações com ponto | não | — | **sem fonte hoje**: 29 canais em `crates/phxclaw-agent/src/canais/` não expõem caixa de entrada; candidato `GET /v1/events` + `/v1/events/sse` (eventos do barramento, não mensagens) |
| 5 | Engrenagem (configurações) | existe | menu «Configuração», `tela-config` | `/v1/config` |
| 6 | Avatar + nome + papel («Adriano Boller · Administrador») + ponto verde | não | — | **sem fonte hoje**: o token (`phxclaw.token`) não carrega identidade; não há `/v1/me`. O ponto verde já existe como `#coreDot` (estado do host) |
| 7 | Menu lateral, 19 áreas, rótulo ao lado do ícone | parcial | `.sidebar nav` — 7 telas em trilho de 104 px com ícone sobre rótulo | Com tela real: Dashboard→geral, Agentes & Swarm→agentes, Skills & Ferramentas→ferramentas, Desenvolvimento→ide, Configuração→config; Testes & Qualidade→`#ideTestes` (dentro do IDE); Plugins→`#ferramentasPlugins` (dentro de Ferramentas). **12 das 19 não têm tela nem fonte** (Pesquisa & Conhecimento, Git & Integração, Segurança, Deploy & Infra, Monitoramento, Incidentes, Documentação, Marketplace, Relatórios, Ajuda, Modelos de IA, Novo Projeto) |
| 8 | Cartão lateral «PhxClaw v0.41 · Todos os sistemas operacionais · modular e extensível» + lema | parcial | `.security-card` «ZERO TRUST / Deny-by-default» (cravado, não traduzido) | versão: `host_status`; «todos operacionais»: `#coreStatus`/`#nativeBridgeStatus`/`#liveEventStatus` (já medidos no topo). O lema está no rodapé |
| 9 | Boas-vindas + «Nova Tarefa» (cheio) · Importar · Git Clone · Deploy · … | parcial | `.hero-row` (título + VER AGENTES / VER FERRAMENTAS); «Nova tarefa» na paleta e em `tela-tarefas` | `POST /v1/tasks`. Importar/Git Clone/Deploy: `ferramentas.json` tem `git write·read 3/3`, mas **não há rota** que a tela possa chamar sem passar por uma tarefa → viram «nova tarefa com objetivo pré-preenchido» ou ficam fora |
| 10 | 6 cartões de ação (Pesquisar, Analisar, Construir, Testar, Implantar, Entregar) com descrição | não | — | `ferramentas.json` (31 capacidades em grupos: `web`, `code`, `fs`, `git`, `shell`, `doc`…) — cada cartão é uma família de capacidade com a contagem concedida/total **medida** (como a «Malha de capacidades» já faz); clique = `POST /v1/tasks` com objetivo modelo. A descrição é rótulo → fábrica |
| 11 | Projetos Recentes (5, chip de estado, «há 2h») | não | — | **sem fonte hoje**: não há `/v1/projetos`. O mais próximo: `/v1/config/perfis` (lista de perfis, ativo) e `agente.pasta`. Estado «Em desenvolvimento/Planejado» é dado que ninguém grava |
| 12 | Agentes & Swarm (6 equipes, N agentes, Online/Em execução/Aguardando) | parcial | `#geralMacro` (11 macroáreas com total, de `equipe.json`); `tela-agentes` (grade phx-grid, 111) | `equipe.json`. Estado vivo por agente/equipe: **sem fonte** (`/v1/tasks` dá o que roda por tarefa, não por agente). Mostrar só o que `equipe.json` tem |
| 13 | Uso de Modelos: rosca, «1.248 requisições», 6 modelos em %, «US$ 12,46 ↓28%», mini-barras | não | — | `/v1/tasks[].model` existe hoje → **contagem de tarefas por modelo** é medível já. Requisições, tokens e **custo em US$: só com a SP000030** (duração/tokens por passo). Até lá: «NÃO MEDIDO» |
| 14 | Skills em Destaque (8 cartões com ícone colorido) | parcial | `tela-ferramentas` (72 ferramentas por capacidade, grade); `#geralCapacidades` (malha) | `ferramentas.json`. «Destaque» não tem critério medido (não há uso por ferramenta) → ordem alfabética ou por capacidade, nunca «destaque» inventado |
| 15 | Execuções Recentes (5, estado + «há N min») | existe (noutra tela) | `tela-tarefas` (grade com estado por cor de ação, objetivo, data) — não aparece na Visão geral | `GET /v1/tasks` (ordenada por `created_at`, as 5 mais novas); clique → `/v1/tasks/{id}` |
| 16 | Infraestrutura & Status (4 servidores, 2 links fibra, 1 Starlink, 99,8% uptime, «todos operacionais») | não | — | **sem rota hoje**: só a ferramenta `linux_system` (`crates/phxclaw-agent/src/sistema.rs:308`, `panel item=uptime|memory|disk|kernel`) chamada pelo agente, não pela tela. Servidores/fibra/Starlink: **sem fonte nenhuma** |
| 17 | Armazenamento (2,4 TB / 10 TB, 24 %, por categoria) | não | — | `linux_system panel item=disk` (mesma ressalva: sem rota). Categorias Projetos/Modelos/Logs: sem fonte |
| 18 | Tarefas Agendadas (5, «Diariamente HH:MM») | não | — | **`GET /v1/schedules` existe** (`api.rs:92`, `agenda.rs` `Schedule` com `next_fire` do cron). Falta só a tela |
| 19 | Notificações (5, com ícone por tipo) | não | — | mesmo que o #4: `/v1/events` (eventos do barramento) é o único; «Licitação encontrada», «Novo lead» são dados de canal que a API não expõe |
| 20 | Assistente em painel lateral (abas Chat/Agentes/Skills/Arquivos, 6 atalhos, caixa com anexo/web/@) | não | — | A conversa **é a tarefa**: `POST /v1/tasks` → `/v1/tasks/{id}` (plano, passos, `question`) → `/answer` e `/approve`. Abas: `equipe.json`, `ferramentas.json`, `/v1/tasks/{id}/artifacts/`. Anexo/web/@: sem rota de upload |
| 21 | Citação «Automação inteligente para um futuro maior» | existe (no rodapé) | `.footerbar b` «Juntos somos infinitos.» (`rodape.lema`) | fábrica |
| 22 | Rodapé: versão · nome · PESQUISAR CONSTRUIR ENTREGAR EVOLUIR · workspace · «Curitiba – PR – Brasil» · bandeira | parcial | `.footerbar` (5 palavras + lema, tudo pela fábrica) | versão: `host_status`; workspace: `agente.pasta`; localidade: `navigator.language` + idioma da fábrica (a bandeira é do idioma escolhido, como no login do PhxSql) — nunca uma cidade digitada |

**Contagem: existe 4 · parcial 7 · não 11** (de 22). Dos 11 «não», **6 têm fonte real hoje**
(#3, #10, #13-parcial por modelo, #15-no-painel, #18, #20) e **5 não têm fonte nenhuma** (#4/#19
caixa de canais, #6 identidade, #11 projetos, #16/#17 infra sem rota).

Achado de passagem (não é deste SP, vai para a W2): a UI já pede `/v1/ide/testes` e
`/v1/plugins/catalogo` (`index.html` `data-fonte`), e **nenhum `.route(` em Rust os declara** —
só o `tests/desktop/qualificacao/servidor.mjs` os simula. Se a W2 não os entregar, os painéis
«Explorador de testes» e «Loja de plugins» ficam em 404 no agente real.

## 3. Crítica de design pelo olhar da marca

### O que adotar

- **O molde do painel**: linha de ações no alto, grade de cartões de 3 colunas no meio, faixa de
  4 blocos operacionais embaixo. A Visão geral atual é mais vertical (4 métricas → 2 painéis →
  malha → host) e exige rolagem a 1024 px de altura (ver `geral-escuro.png`: o painel do host
  fica cortado). O mockup cabe inteiro.
- **Hierarquia por cabeçalho de cartão**: título + «Ver todos ›» à direita. Já é o padrão do
  `.panel-head` atual (eyebrow + h2 + botão de contorno) — mantém-se, com o eyebrow dizendo a
  fonte (`EQUIPE / EQUIPE.JSON`), que o mockup não tem e a casa exige.
- **Densidade**: 12 blocos visíveis sem rolar, listas de 5 linhas. Adotar o teto de 5 com «ver
  todos» abrindo a grade phx-grid da tela própria.
- **Barra de comando visível no topo** no lugar da lupa: o `#paleta` já faz tudo o que a barra
  promete; muda só a afordância (campo largo com `Ctrl K` desenhado). Zero lógica nova.
- **Menu com rótulo ao lado do ícone**, agrupado por área, em vez do trilho de 104 px com rótulo
  embaixo (que quebra «Configuração» em duas linhas a 1536 px). Só com as áreas que **têm tela**.
- **Painel do assistente** à direita, recolhível: é a tarefa (plano → aprovar → responder) vista
  como conversa — a API já tem os quatro verbos.
- **Rodapé com versão, pasta e idioma** — todos com fonte real.

### O que NÃO adotar, com o número

1. **Paleta azul.** O mockup é azul `#0077f3` sobre `#021019`; a marca é laranja `#ff8a1c`
   sobre `#010418`. Lado a lado, sobre o fundo de cada um:

   | Token | mockup | marca escuro | marca claro |
   |---|---|---|---|
   | fundo | `#021019` | `#010418` | `#f7f5f2` |
   | painel | `#051520` | `#0a1122` | `#ffffff` |
   | cor primária | `#0077f3` (azul) | `#ff8a1c` (laranja) | `#c63c0a` |
   | contraste primária/fundo | 5,84:1 (`#0077f3` sobre `#010418`) | **8,63:1** | 4,76:1 |
   | texto 1 | `#f8f9fb` | `#dde2eb` | `#1a1210` |
   | texto 2 | `#9ba4b9` | `#a8b0c0` | `#4a3f3a` |
   | estado ok | `#03b174` | `#6cc98c` | `#2f7a3e` |
   | link/consulta | `#349ae0` | `#5fa6e8` | `#1f5c93` |

   O azul do mockup fica na marca como `--acao-consultar`/`--info` (`#5fa6e8`), que é **estado e
   consulta**, não identidade. A pétrea «a marca manda sobre qualquer paleta inventada» decide
   sem discussão; o número só mostra que não se perde nada: o laranja contrasta 48 % mais que
   o azul sobre o fundo da marca.

2. **Fundo cheio nos botões primários e no item ativo do menu.** Medido no mockup (percentil 97
   da cor do texto sobre a cor mais frequente do fundo): «Nova Tarefa» `#e7f3ff` sobre `#0077f3`
   = **3,78:1**; «Dashboard» ativo `#eff5fe` sobre `#0072f6` = **4,04:1**. São os **únicos 2 pares
   de texto reprovados em 24 medidos** (22/24 ≥ 4,5:1 — o resto do mockup é bem contrastado).
   A regra «contorno, nunca fundo cheio» já existe por este motivo exato, e o roteiro
   `ui_navegacao.mjs` reprova `.acao` com fundo fora do hover.

3. **Ícones cheios, um de cada cor.** Seis quadrados de 48 px em `#006ffc`, `#13c05c`, `#4d3ff2`,
   `#006efa`, laranja-vermelho e `#016872`, com o glifo branco por cima: o verde «Analisar» mede
   **2,41:1** (abaixo dos 3:1 de gráfico), e seis cores que não significam ação diferente são o
   «arco-íris de 27 ícones» que o `STYLE_PHOENIX_PADRAO.md` já listou como defeito #3 do PhxSql.
   A casa usa traço único (`--traco: 1.75`, Lucide-like) e cor **só** nas cinco ações. Os
   cartões de ação entram com ícone de traço e borda da cor de ação quando houver ação
   (Construir = incluir/verde, Testar = consulta/azul, Implantar = alterar/amarelo…), ou neutros.

4. **Números ilustrativos.** `1.248 requisições`, `US$ 12,46 ↓28%`, `99,8% uptime`, `2,4 TB / 10
   TB`, `4 servidores`, `6 equipes ativas`, `v0.41`: **17 números** no mockup sem gerador. A
   `ui_navegacao.mjs` já reprova número fora de `[data-fonte]` na Visão geral; cada cartão sem
   fonte sai «NÃO MEDIDO» com o nome do gerador que faltaria (como o dossiê de testes faz).

5. **Rosca de uso por modelo.** Além de não ter fonte (SP000030), a rosca com 6 fatias em 6 cores
   é a mesma lição do item 3; a casa já resolveu «três estados» com forma (cheio / hachurado /
   tracejado, `#geralAbsorcaoLista`). Quando houver tokens por modelo, barras horizontais com o
   número ao lado, não rosca.

6. **Dado estilizado.** Chips «Em desenvolvimento», «Planejado», «Online» em caixa normal estão
   certos; mas o rodapé «PESQUISAR CONSTRUIR ENTREGAR EVOLUIR» em caixa alta é rótulo (ok). O que
   não pode entrar: o nome do workspace «ENGINE PRINT 3D» em caixa alta no seletor — é dado
   (`agente.pasta`), e `text-transform` sobre ele é o «BLUMENAU».

Contraste do mockup, 24 pares medidos por pixel (texto = percentil 97 de contraste sobre a cor
mais frequente da região):

| Par | fundo | texto | razão |
|---|---|---|---|
| Título «Bem-vindo» | `#000218` | `#ffffff` | 20,55 |
| Subtítulo | `#00101c` | `#9ba4b9` | 7,70 |
| **Botão «Nova Tarefa» (cheio)** | `#0077f3` | `#e7f3ff` | **3,78** |
| **Menu ativo «Dashboard» (cheio)** | `#0072f6` | `#eff5fe` | **4,04** |
| Item de menu | `#04121c` | `#c9d2e0` | 12,43 |
| Rótulo do cartão | `#061520` | `#f8f9fb` | 17,55 |
| Descrição do cartão | `#051520` | `#b1bed1` | 9,83 |
| Chip «Em desenvolvimento» | `#052c2d` | `#50cdbf` | 7,71 |
| Chip «Planejado» | `#282c20` | `#cbb462` | 6,96 |
| «Online» | `#04131e` | `#03b174` | 6,75 |
| «2h atrás» | `#04141f` | `#7598be` | 6,21 |
| Legenda da rosca | `#06141f` | `#c0c7cd` | 10,90 |
| Rodapé (palavras) | `#041019` | `#7b91af` | 5,96 |
| Balão do assistente | `#081a28` | `#b8ccda` | 10,67 |
| Rodapé (localidade) | `#041019` | `#798da9` | 5,67 |
| Placeholder da barra | `#06101a` | `#6b82ad` | 4,94 |
| Notificação | `#031520` | `#adb0bc` | 8,58 |
| Agenda | `#02131e` | `#a6b9c9` | 9,34 |
| Nome do projeto | `#051420` | `#eef0f9` | 16,38 |
| Cabeçalho de seção | `#000314` | `#ffffff` | 20,53 |
| Link «Ver todos» | `#071927` | `#349ae0` | 5,81 |
| Custo | `#02040a` | `#ffffff` | 20,50 |
| Número da rosca | `#05111a` | `#fdfeff` | 18,88 |
| Rótulo da infra | `#03151f` | `#bcc9d2` | 10,99 |

Ícone branco sobre o fundo cheio dos cartões: Pesquisar `#006ffc` 4,49 · Analisar `#13c05c`
**2,41** · Construir `#4d3ff2` 6,35 · Testar `#006efa` 4,55 · Entregar `#016872` 6,51 (Implantar
é degradê, não medido).

## 4. Plano da fase 2, em lotes exercitáveis

Fonte de risco com a W2, medida em `git diff --stat HEAD -- apps/phxclaw-ui` às 04:09:
`app.css +32`, `app.js −292` (movido para `paineis.js`/`ide.js`), `index.html +47` (seções IDE,
Ferramentas, Configuração), `textos.json +57`, `sw.js`. Os lotes abaixo tocam **cabeçalho,
lateral, rodapé e `tela-geral`** — hunks diferentes dos da W2, exceto `textos.json`, onde os
dois acrescentam no fim. Mitigação: as chaves novas entram com prefixo próprio (`casca.`,
`painel.`) num bloco só, e o `textos_fora_da_fabrica.mjs` (teto 0) decide quem esqueceu.

| Lote | Entrega | Arquivos | Roteiro de prova | Textos novos | Choque com a W2 |
|---|---|---|---|---|---|
| **L1 casca** | menu com rótulo ao lado, agrupado em 4 áreas (Painel; Trabalho: Tarefas, IDE; Capacidades: Agentes, Ferramentas, Absorção; Sistema: Configuração); barra de comando visível no topo que abre o `#paleta`; rodapé com versão (`host_status`), pasta (`/v1/config` `agente.pasta`) e idioma/bandeira; `<aside>` do assistente **vazio** e recolhido, só com o título e «sem conversa» | `index.html` (header, sidebar, footer, aside: ~70 linhas), `app.css` (≈90 linhas: `.sidebar` largura 104→224 px e `@media(max-width:1300px)` volta ao trilho, `.cmd-bar`, `.assistente`, `.footerbar`), `app.js` (≈25 linhas: rodapé lê host/config; grupos do menu), `textos.json` | `ui_navegacao.mjs` (a checagem «menu tem as sete telas» na mesma ordem continua valendo), `qualificar.mjs` T4 (4 larguras) e T1 (temas), `textos_fora_da_fabrica.mjs` | **11** chaves (4 grupos do menu, 1 placeholder da barra, 2 do assistente, 3 do rodapé, 1 do «NÃO MEDIDO») × 2 idiomas | baixo: `index.html` fora das seções da W2; `app.js` fora da paleta (`:751+`), que fica igual; `textos.json` no fim (conflito trivial de merge) |
| **L2 ações + execuções** | 6 cartões de ação com contagem concedida/total por família de capacidade (`ferramentas.json`) e clique → nova tarefa com objetivo modelo; «Execuções recentes» = 5 tarefas mais novas de `/v1/tasks` na Visão geral, estado pela cor de ação, «ver todas» → Tarefas | `index.html` (`tela-geral`), `app.js` (≈80), `tarefas.js` (reusar o formatador de estado), `app.css` (≈40), `textos.json` | `ui_navegacao.mjs` (número fora de `[data-fonte]` reprova; nova checagem: 5 linhas = as 5 mais novas do stub) | 6 nomes + 6 descrições + 6 objetivos-modelo + 2 títulos = **20** | baixo (`tela-geral` e `app.js` topo) |
| **L3 workspace + agenda** | cartão «Pasta / perfil ativo» (`/v1/config`, `/v1/config/perfis`) no lugar de «Projetos recentes»; «Tarefas agendadas» de `GET /v1/schedules` (`next_fire`) — 5 linhas, grade phx-grid na tela Tarefas | `index.html`, `app.js` (≈60), `tarefas.js` (grade da agenda ≈50), `servidor.mjs`/`ui_navegacao.mjs` (stub de `/v1/schedules`), `textos.json` | `ui_navegacao.mjs` + `ui_config.mjs` (perfil ativo) | **9** | médio: `tarefas.js` e `config.js` são da W2 (perfis) — coordenar o hunk |
| **L4 agentes & skills** | «Agentes» = macroáreas (já existe) compactado em 5 + «ver todos»; «Skills» = 8 ferramentas por capacidade, ordem fixa (alfabética), ícone de traço, sem «destaque» | `index.html`, `app.js` (≈40), `app.css` (≈30) | `ui_navegacao.mjs` (macro e famílias = JSON) | **4** | baixo |
| **L5 modelos** | só o que se mede: contagem de tarefas por `model` de `/v1/tasks` em barras; tokens/custo ficam «NÃO MEDIDO — SP000030» até a SP000030 entregar | `app.js` (≈40), `app.css` | `ui_navegacao.mjs` (barras = contagem do stub por modelo) | **4** | nenhum (depende da SP000030 para a 2ª metade) |
| **L6 notificações** | caixa de eventos de `/v1/events` + `/v1/events/sse` (o que o barramento publica), 5 mais novos, por tipo com forma, não só cor; sino com contagem dos não vistos (local) | `app.js` (≈70), `index.html`, `app.css`, stub em `servidor.mjs` | `ui_paineis.mjs` (SSE no stub) | **6** | baixo; **sem fonte** para mensagens de canal — fora até a API expor a caixa |

Total estimado de chaves novas: **54** (×2 idiomas). Cada lote fecha com `textos_fora_da_fabrica`
em 0, `qualificar.mjs` nos dois temas e as capturas regravadas em `tests/desktop/out/`.

**Tamanho do L1**: ~185 linhas (70 HTML + 90 CSS + 25 JS) + 11 chaves; 4 arquivos; zero rota nova.

## Resposta curta

- **Blocos: 22 — existe 4 · parcial 7 · não 11** (6 dos «não» com fonte real hoje, 5 sem fonte).
- **5 maiores ganhos**: (1) menu com rótulo ao lado e por áreas, (2) barra de comando visível
  reaproveitando o `#paleta`, (3) execuções recentes e cartões de ação na Visão geral com dado
  de `/v1/tasks` e `ferramentas.json`, (4) agenda de `/v1/schedules` (rota pronta, tela zero),
  (5) painel do assistente sobre os 4 verbos de tarefa que já existem.
- **3 «não adotar»**: paleta azul (`#0077f3` 5,84:1 × laranja `#ff8a1c` **8,63:1** sobre o fundo
  da marca); fundo cheio nos primários (**3,78:1** e **4,04:1**, os 2 únicos reprovados de 24);
  ícones cheios coloridos (verde «Analisar» **2,41:1**, 6 cores sem significado de ação).
- **L1**: ~185 linhas em 4 arquivos, 11 chaves novas, zero rota nova, risco baixo com a W2.

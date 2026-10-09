# Segurança e DBA — revisão da onda 3 do motor de fluxo (09/10/2026, sobre `211b9cd6`)

- **M1 — pin só na execução manual.** O `ARQ.pins.json` valia em gatilho, agenda, API e sub-fluxo:
  quem o escrevesse trocava a saída de qualquer passo de produção. Agora só `fluxo rodar --pins`
  aplica; `esperar` pinado é recusado na leitura.
- **M5 — duas esperas de webhook:** o segredo de A podia entregar em B (dois POST juntos). A espera
  é a que o segredo alcança, e a entrega confere que ela continua aberta.
- **M6 — código do formulário próprio** (`segredo_formulario`): o `_segredo` da página não vale
  como credencial JSON/HMAC do gatilho, e o segredo do gatilho não vale no campo.
- **Repetição do gatilho:** a requisição assinada reenviada na janela de 300 s disparava o fluxo
  de novo; agora volta o id do primeiro (também por `Idempotency-Key`). Em memória.
- **Entrada com credencial** (corpo do webhook, arquivo do gatilho, sub-fluxo, resposta de espera)
  chegava a `{{entrada}}` e ao `task.json`: recusada pelo mesmo `fluxos::conferir_entrada` em toda
  porta. **Mudança de comportamento:** na entrada que vem de fora conta só a FORMA da
  credencial — chave de provedor com corpo (`ghp_…`, `sk-…`, `xoxb-…`, `AKIA…`, inclusive colada
  depois de `:` ou `=`), JWT, bloco PEM, URL com senha e cabeçalho `Basic`/`Bearer` com valor —,
  pelo motor único `phxclaw_types::segredo`: a mesma lista de prefixos, o mesmo separador de
  palavras e a mesma tarja de URL com senha que o broker usa para redigir gravação e memória, e a
  lista que o `config.json` usa (antes eram duas listas, de 11 e 20, e dois separadores; cinco
  formas recusadas na entrada passavam em claro na tarja).
  Nome de campo e entropia não contam ali: medido em 09/10, a guarda por nome recusava com 400
  eventos do Jira e do S3 (`key`) e paginação (`next_page_token`), e o prefixo sem corpo recusava
  o locale `sk-SK`. O nome continua valendo para o que o operador escreve (variável, pin, campo de
  formulário, `config.json`).
- **B2–B5:** binário conferido só em `binarios/`, arquivo regular (FIFO prendia a retomada para
  sempre), até 64 MiB, nome com o sha256 inteiro, `binarios/` link recusado; página do formulário
  com CSP por hash, `frame-ancestors 'none'`, `no-store`, `no-referrer`, `autocomplete="off"`;
  `/v1/flows/{tarefa}/resume` confere a credencial antes do disco (401 para tarefa inexistente
  sem token).
- **Pânico:** `esperar.ms` enorme derrubava o laço das esperas na soma de data; teto de 366 dias
  na leitura.
- **DBA:** `entregar` recusa formato futuro; o laço retoma a entrega gravada sem retomada; pins
  antes do fluxo no importar; poda por lápide; `fsync` das pastas criadas; detector de segredo
  numa lista só (`phxclaw_types::segredo`), usada também pelo `config.json` (`carga::nome_de_segredo`
  delega a ela). Por isso a chave `rollback_authorization` de `config/updater-policy.v027.json`
  virou `rollback_authorization_mode`: o valor é o modo da política, não um segredo, e nenhum
  código lia a chave.
- **Google Chat (mudança de comportamento):** sem `PHXCLAW_GOOGLECHAT_AUDIENCIA` o canal não sobe,
  e a mensagem diz o que falta. Recusa é o padrão por orientação da Microsoft e do Google; quem
  tinha o canal ligado sem a variável precisa configurá-la.
- Provas: `tests/fluxo_onda3b.rs` (20 testes), `apps/phxclaw/tests/fluxo_cli.rs`; RED medido em
  20. Detalhe em `docs/N8N.md` §8f.

# Segurança — hotfix do IDE no navegador (09/10/2026, sobre `211b9cd6`)

- **ALTO (revisão SEC): `GET /v1/ide/arquivo` servia a chave-mestra do broker.** A rota confinava à
  raiz do projeto, e no layout padrão a pasta do agente (`var/agente`) mora dentro dela:
  `?caminho=var/agente/segredos/master.key` devolvia a chave em texto, e o cofre, o `api.token` e
  um symlink do projeto saíam igual. Conserto na porta única (`tarefa::confine`): a área do agente
  (`--pasta`/`PHXCLAW_HOME`/`var/agente`, exceto a pasta da própria tarefa) e qualquer broker
  reconhecido pela forma (`segredos/` com `master.key`) são recusados — vale também para o
  `read_file` e as outras ferramentas que aceitam absoluto numa raiz do workspace. A rota responde
  403 sem dizer o motivo.
- **B1 da mesma revisão:** a rota abre o arquivo uma vez, sem esperar escritor (FIFO não prende
  thread), reconfere o caminho real pelo descritor aberto (`/proc/self/fd`) e lê dentro de
  `spawn_blocking`.
- Provas em `tests/ide_web.rs`: 8 portas que vazavam (RED medido: 8/8 com 200 e o segredo no
  corpo) agora 403; FIFO e symlink para FIFO dão 422 sem travar; esparso de 8 GiB dá 413 em < 2 s
  (sem o `take`, 19,0 s). O minimapa no navegador passa a conferir a faixa pelos pixels
  (`tests/desktop/ide_minimapa.mjs`).
- **A mesma chave, pela porta de processo.** O terminal Helix (`/v1/ide/terminal`) rodava sem
  sandbox na pasta do projeto (`:open var/agente/segredos/master.key` desenhava a chave na grade),
  e todo processo do bwrap com o projeto em `/work` — explorador de testes, servidores de
  linguagem, túnel, git, hooks — via `var/agente` inteira. Conserto num lugar só: toda montagem
  do bwrap que **contém** a pasta do agente ganha um tmpfs vazio e só leitura por cima dela
  (`phxclaw_sandbox::mascaras`, registrada por `processo::mascarar_a_pasta_do_agente` no
  `achar_bwrap`; a pasta é a mesma do `confine`, `tarefa::pasta_do_agente`, e a pasta da tarefa
  em `<agente>/tasks/<id>/work` segue gravável). O Helix passa a subir no mesmo bwrap
  (`processo::terminal_no_bwrap`): sem `--new-session` (com ele o redimensionar não chega,
  medido), ambiente herdado apagado por `--unsetenv` (o Bearer da completação nunca vai ao argv).
  **Sem bwrap, o terminal recusa:** fora do sandbox o hx abre qualquer caminho absoluto.
- Provas: `tests/sandbox_agente.rs` (3) e `tests/ide_web.rs` (`o_helix_do_ide_nao_abre_a_chave…`,
  `os_testes_do_ide_nao_leem…`); RED medido nas 5 com a máscara retirada, e no Helix também com
  o terminal fora do bwrap. A guarda de processos (`tests/guardas.rs`) passa a ver o
  `Terminal::abrir`. Limites: as raízes do workspace entram graváveis no terminal; o pyright do
  `~/.local/bin` não está no `PATH` do sandbox (o rust-analyzer entra pelo toolchain do `lsp.rs`);
  broker achado só pela forma, fora da pasta do agente, não é mascarado (o `confine` o recusa).

# v0.70 — PHX Flow Engine onda 1, sala XMPP, casca do painel (02/10/2026, commit 4802e21b)

- `fluxos.rs` vira motor de dados: saída por itens JSON, nós `se`/`juntar`/`lote`/`parar_com_erro`,
  `ao_errar` por passo, `fluxo_de_erro`, `teto_ms` por passo e por fluxo, expressões por caminho
  JSON (sem JS). Fluxo antigo roda igual (teste do comportamento velho). 10 testes em
  `tests/fluxo_motor.rs`; prova real reposta um a um fica para a onda 2. n8n 40,7% → 55,9% no
  agente (`gerar_absorcao.py`). Formato e divergências: `docs/N8N.md` §8.
- Canal XMPP com sala multiusuário (XEP-0045): chaves `SALAS`/`APELIDO`, eco e histórico com
  `<delay/>` ignorados, `groupchat` para a sala e `chat` para privada de ocupante, erro de presença
  (409 etc.) legível (`docs/GUIA_DO_OPERADOR.md`, «Canal XMPP»).
- Segurança do XMPP (revisão SEC, retomada 06/10): a sala em `PERMITIDOS` deixa o agente ouvir e
  falar nela, mas só comanda o ocupante cujo JID real (`<item jid>`) está na lista; resposta a
  pergunta pendente só de quem abriu a tarefa; `CONFIAR_NO_NICK` (padrão `false`) para sala
  anônima; atributo injetado, estrofe acima de 256 KiB, fila cheia, texto de quem não é permitido
  no disco e eco com nick trocado consertados — cada um com teste que reprova com o defeito reposto.
- Motor de fluxo, onda 2: tipos de passo skill/mcp/comando/comportamento pelo mesmo portão,
  ferramenta `fluxo` (sub-fluxo once/each, profundidade 8, ciclo recusado), `rodar --ate`, gatilho
  que dispara fluxo, `variaveis` com segredo recusado; hash canônico da definição, `formato` no
  relatório, uma cópia só do dado por passo, teto de itens e de chamadas simultâneas por item;
  passadas boas ficam quando uma falha. 17 testes em `tests/fluxo_onda2.rs`; n8n 55,9% → 62,7% no
  agente (`gerar_absorcao.py`).
- Painel L2 (cartões de ação e execuções recentes) e checagem de número digitado no topo e no
  rodapé da tela.
- Casca do painel (SP000036 L1): menu em 4 áreas com rótulo ao lado, barra de comando que abre a
  paleta, rodapé lido (versão, pasta, idioma), assistente recolhível; 13 chaves `casca.*`; ordem
  do DOM do menu = ordem visual (`docs/ui/COMPARACAO_MOCKUP_2026-10-02.md` §5).
- Portões (parecer do integrador): fmt ok, clippy 0, 450 testes verdes / 0 falhas / 1 ignorado,
  ui_navegacao 58/58, ui_config 21/21, qualificar 18/18, textos cravados 0.
- Documentação: nasce `docs/TECNOLOGIAS.md` com extrator `tools/gerar_tecnologias.py`.
- Sem entradas v0.68 e v0.69 neste arquivo (não escritas na época; não inventadas agora).

# v0.67 — BPM Visual Editor + Crash-safe Replay

- F13 source gap closed;
- bpmn-js 18.30.1 editor;
- token lease/fencing;
- append-only BPM events;
- checkpoint/replay.

# v0.66 — Persistent Research + Hypothesis

- F01/F02 source gaps closed;
- PostgreSQL persistence + RLS;
- evidence provenance + graph links;
- hypothesis transition guard + append-only decisions;
- experiment run/result/evidence ledger.

# v0.65 — Installer Backup/Restore + Disaster Recovery

- F03 source gap closed;
- SHA-256 content-addressed backups;
- verify-before-restore + staging/rollback;
- transactional upgrade hooks;
- pg_dump/pg_restore no-shell integration;
- DR/RPO/RTO evidence schema.

# v0.64.0 — Managed MCP/LSP Runtime

- F18 promoted to source_ready/static_verified.
- Real managed child-process MCP/LSP stdio sessions.
- MCP Streamable HTTP request transport with SSE response parsing.
- Cancellation/timeouts/reconnect policy and strict execution/network allowlists.
- Evidence/Event integration and PostgreSQL append-only session audit.
- Real fixture process smoke: 7/7 PASS.
- Cargo + independent-server E2E remain release gates.

# v0.63 — Native Repo Intelligence / Tree-sitter

- F19 source gap closed;
- pinned Tree-sitter parser/grammar dependencies;
- AST symbols/calls/dependencies;
- PageRank + hotspot scoring;
- PostgreSQL append-only repo intelligence evidence.

# v0.62 — Canonical Project State + Knowledge Promotion Gate

- canonical project state;
- synchronized sprint descriptors;
- evidence-bound knowledge promotion;
- human-gated governance/revocation;
- v0.61 candidate promotion DB hardening;
- tenant setting compatibility function.

# Changelog — PhxClaw v0.20

## Added

- Unified `phxclaw-core-runtime` facade.
- Single native `phxclaw` application binary.
- Managed PostgreSQL bootstrap crate and installer.
- Windows portable EDB PostgreSQL 18.6 download with SHA-256 verification.
- PGDG/Homebrew installation paths for supported Unix platforms.
- Runtime configuration referencing database password by Secret Broker UUID.
- `phx install`, `phx db`, `phx core` commands.
- Colored sprint status: green/yellow/red.
- Migration `0020_core_runtime_installation.sql`.

## Changed

- RustClaw/openclaw-rs compatibility code is treated as internal implementation/provenance, not a separate product surface.
- PostgreSQL 19 is explicitly preview-only until stable.
- Legacy `apps/phxclaw-cli` is no longer an active workspace member; `apps/phxclaw` is the product binary.

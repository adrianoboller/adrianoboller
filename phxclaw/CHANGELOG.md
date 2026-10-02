# v0.70 — PHX Flow Engine onda 1, sala XMPP, casca do painel (02/10/2026, commit 4802e21b)

- `fluxos.rs` vira motor de dados: saída por itens JSON, nós `se`/`juntar`/`lote`/`parar_com_erro`,
  `ao_errar` por passo, `fluxo_de_erro`, `teto_ms` por passo e por fluxo, expressões por caminho
  JSON (sem JS). Fluxo antigo roda igual (teste do comportamento velho). 10 testes em
  `tests/fluxo_motor.rs`; prova real reposta um a um fica para a onda 2. n8n 40,7% → 55,9% no
  agente (`gerar_absorcao.py`). Formato e divergências: `docs/N8N.md` §8.
- Canal XMPP com sala multiusuário (XEP-0045): chaves `SALAS`/`APELIDO`, eco e histórico com
  `<delay/>` ignorados, `groupchat` para a sala e `chat` para privada de ocupante, erro de presença
  (409 etc.) legível. Sala em `PERMITIDOS` = qualquer ocupante comanda o agente
  (`docs/GUIA_DO_OPERADOR.md`, «Canal XMPP»).
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

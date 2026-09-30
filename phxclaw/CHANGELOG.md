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

# PhxClaw v0.40 — Unified Configuration

`config/phxclaw.config.json` is the single canonical configuration document.

## Rules
- `ui/config.html` edits the canonical document through `GET/PUT /v1/config` when hosted by PhxClaw.
- Standalone browser mode supports import/validate/export because ordinary web pages cannot write arbitrary local files.
- Writes use optimistic concurrency (`revision`) and atomic rename.
- Previous revisions are retained in `config/history/`.
- Plaintext passwords, API keys, bearer tokens, private keys and client secrets are forbidden.
- Configuration stores only Secret Broker UUID references.
- Legacy versioned policy JSON files become generated compatibility views and are not authoritative.
- Hot reload is allowed only for explicitly listed sections; database/security/control-plane changes require restart/reconciliation.

## Software Factory
The same configuration drives the v0.40 autonomous factory pipeline:
`intake -> decompose -> research -> architecture -> implement -> integrate -> QA -> security -> docs -> RC -> approval -> delivery`.

## Local CLI
`tools/configctl.py` validates, hashes, reads and atomically changes the same canonical JSON with optimistic revision checks.

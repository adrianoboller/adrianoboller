# PhxClaw v0.40 — Build Report

## Unified configuration
- Canonical source: `config/phxclaw.config.json`.
- Visual editor: `ui/config.html`.
- JSON Schema, revision/hash, atomic save, history and optimistic concurrency.
- Secrets remain in Secret Broker; only UUID references are allowed in config.
- Plugin/policy/feature-flag configuration is inline in the same canonical JSON.
- Legacy policy JSON files are compatibility views, not authoritative.

## Autonomous Software Factory
`intake -> decompose -> research -> architecture -> implement -> integrate -> QA -> security -> documentation -> RC -> approval -> delivery`.

## Validation
- Static: 25 PASS / 0 FAIL
- Local/application: 25 PASS / 0 FAIL
- Package/security: 13 PASS / 0 FAIL

## Native gates
Not release-ready on this host. Cargo/Rust/PostgreSQL native gates remain external.

# PhxClaw v0.24 — Release Hardening

State: **hardening / review**.

All F00–F25 have source-level implementation, but no sprint becomes green merely because source exists.

## Hardening delivered

- evidence-driven native Release Gate;
- proof bound to exact workspace SHA-256;
- `UNAVAILABLE` never becomes `PASS`;
- F24 → F25 skill-release lineage bridge;
- composite tenant foreign keys across F22/F24/F25;
- `FORCE ROW LEVEL SECURITY`;
- append-only release evidence/attestations and audit facts;
- PostgreSQL cross-tenant E2E script;
- static preflight + native preflight runner;
- safe repair for malformed workspace `members` array created by older overlay composition.

## Native gates still required on release host

`cargo fmt --check`, `cargo check --workspace`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`, PostgreSQL migration + RLS E2E,
Mission E2E, Tauri native E2E and provider/model E2E.

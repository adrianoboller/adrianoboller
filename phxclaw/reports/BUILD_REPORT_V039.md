# PhxClaw v0.39 — Build Report

## Delivered

- `phxclaw-swarm-merge-intelligence` crate.
- Typed change intents and contract surfaces.
- Semantic conflict detection for symbols, contracts, schema/database, behavior and security policy.
- Deterministic topological merge ordering with cycle fail-closed.
- Integration replay bound to exact base/source and candidate hashes.
- Mutation gate with minimum score, minimum mutant count and critical-survivor block.
- Trusted Ed25519 conflict-resolution verification.
- Evidence-over-votes consensus rule.
- PostgreSQL append-only evidence with tenant+swarm composite integrity and FORCE RLS.
- Hash-guarded repair for the known malformed v0.38 RLS migration snapshot.

## Verification executed here

- Static verifier: **54 PASS / 0 FAIL**.
- Local/application: **12 PASS / 0 FAIL**.
- Package/security: **28 PASS / 0 FAIL**.

## Native gates

`cargo`, `rustc`, `psql` and `postgres` are unavailable on this host. Native Rust compilation, PostgreSQL execution, real mutation testing and real Git integration replay remain unproven and are not marked PASS.

## Capability projection

698 + 52 = **750 capabilities**.

## Sprint colors

🟢 0 / 🟡 26 / 🔴 0.

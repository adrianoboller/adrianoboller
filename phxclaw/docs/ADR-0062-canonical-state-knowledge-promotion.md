# ADR-0062 — Canonical Project State + Knowledge Promotion Gate

## Decision

1. `config/project-state-canonical.v062.json` is the only current machine-readable project/sprint status source.
2. Historical `PROJECT_STATUS_*` documents remain evidence, not current truth.
3. Implementation state and verification state are separate dimensions.
4. Knowledge promotion is evidence-bound and append-only: `raw_observation -> unverified -> accepted -> governed`.
5. `governed` and revocation require human authority by default.
6. Active refutation, stale evidence, source-state mismatch or unresolved contradiction blocks promotion.
7. v0.61 candidate promotion is hardened by a v0.62 database trigger requiring a gated receipt.
8. Tenant context is normalized through `phxclaw.current_tenant_uuid()`, accepting `phxclaw.tenant_uuid` and the historical `phxclaw.tenant_id` alias.

## Non-goals

This version does not claim native PostgreSQL, Cargo or Tauri E2E proof on the generation host.

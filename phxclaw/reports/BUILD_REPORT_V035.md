# PhxClaw v0.35 — Build Report

## Scope

AI SRE & Cost/Capacity Autopilot over v0.34.

- SLO/error-budget evaluation
- demand/token/cost forecasts with signed freshness policy
- Ed25519 provider rate-limit evidence and decision-time TTL/reset revalidation
- queue admission/backpressure with priority aging
- incident evidence deduplication + contradiction rejection
- bounded, signed autopilot plans inside the v0.34 portfolio
- provider-level and model-level membership guards
- cost-increase guard based on budget limit; zero budget is fail-closed
- cost+incident evidence binding
- execution expiry + v0.30 leader/fencing authorization
- PostgreSQL FORCE RLS, tenant-composite FKs, append-only records and idempotent execution
- non-vacuous cross-tenant RLS E2E script with separate non-superuser/NOBYPASSRLS connection

## Verified in this host

- Static verifier: **101 PASS / 0 FAIL**
- Local/application/security: **45 PASS / 0 FAIL**
- Migration quoting: **1 PASS / 0 FAIL**
- Package checks: **40 PASS / 0 FAIL**

## Native gate status

`cargo`, `rustc`, `psql`, and `postgres` are unavailable in this execution host. Native Rust/PostgreSQL/provider gates are therefore **not executed and not marked PASS**.

## Status

- `source_ready = true`
- `static_verified = true`
- `runtime_verified = partial`
- `e2e_verified = false`
- `release_ready = false`
- capabilities: **488 + 42 = 530 projected**
- sprint colors: **0 green / 26 yellow / 0 red**

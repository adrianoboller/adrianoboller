# PhxClaw v0.33 — Continuous Model Arena — Build Report

## Delivered

- `phxclaw-model-arena`
- `phxclaw-model-drift`
- signed and verified Arena specifications (Ed25519)
- immutable Arena specification/challenger registration with append-only lifecycle events
- deterministic Shadow / Canary / Paired assignments
- production candidates restricted to models already eligible under v0.31 and backed by promoted v0.32 profiles
- paired observation windows and challenger verdicts
- fixture/production evidence boundary
- promotion recommendation requiring the v0.32 Promotion Gate
- champion drift detection and fallback to the v0.31 base router
- PostgreSQL RLS/FORCE RLS and append-only evidence tables

## Verified in this host

- static verifier: **95 PASS / 0 FAIL**
- local Arena/drift/application suite: **27 PASS / 0 FAIL**

## Not proven in this host

`cargo`, `rustc`, `psql`, and `postgres` are unavailable. Native Rust compilation, PostgreSQL E2E, RLS cross-tenant and live-provider Arena tests remain release gates. No synthetic fixture is reported as a production-model benchmark.

## Status

`source_ready=true`, `static_verified=true`, `runtime_verified=partial`, `e2e_verified=false`, `release_ready=false`.

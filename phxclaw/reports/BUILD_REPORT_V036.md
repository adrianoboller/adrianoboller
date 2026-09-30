# PhxClaw v0.36 — Build Report

## Delivered

- governed Skill Router with 20 audited external skill adapters;
- four-stage task workflow: research/validate -> understand/improve -> create/explain -> refine/deliver;
- Incident Commander contracts with signed policy, approval and v0.30 fencing;
- Chaos/Resilience Lab with bounded blast radius, abort conditions, recovery verification and postmortems;
- PostgreSQL tenant RLS, composite FKs and append-only evidence/event tables;
- 66 capabilities, projected total 596.

## Verification executed

- static verifier: **102 PASS / 0 FAIL**;
- local behavior/application: **14 PASS / 0 FAIL**;
- package/security checks: **11 PASS / 0 FAIL**.

## Licensing/provenance

No upstream repository is vendored in this overlay. The 20 projects are represented as adapter/provenance records. Video ShotCraft media/assets are excluded; QMD uses canonical upstream `tobi/qmd`; Graphify stays adapter-only because the repository exposes both Apache-2.0 and MIT notices.

## Native gates

`cargo`, `rustc`, `psql` and `postgres` are unavailable on this host, so Rust compile/test/clippy, PostgreSQL RLS E2E, real external-adapter E2E, incident recovery drills and chaos experiments remain unproven. No mock is substituted for these gates.

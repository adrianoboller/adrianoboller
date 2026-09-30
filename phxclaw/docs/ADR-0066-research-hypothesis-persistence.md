# ADR-0066 — Persistent Research + Hypothesis Experiment Ledger

F01/F02 move from in-memory models to tenant-isolated PostgreSQL persistence. Research evidence keeps immutable URI/type/time/SHA-256 provenance and can be linked to Knowledge Graph nodes. Hypothesis identity/plan is persisted, decisions are append-only, transitions are guarded, and experiment runs/evidence are durable.

Native PostgreSQL E2E remains a release gate; source/static completion does not claim database execution on this host.

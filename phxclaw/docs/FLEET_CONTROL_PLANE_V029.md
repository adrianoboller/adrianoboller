# PhxClaw v0.29 — Fleet Control Plane + Remote Device Orchestrator

## Boundary

`F22 Device Nodes` owns node identity, Ed25519 envelope validation, replay reservation, declared capabilities, command authorization, approvals, Secret Broker handles and device-session fencing.

`v0.28 Fleet Updater` owns signed channel rollout, deterministic cohort selection, health gates, anti-rollback update sequence, coordinated core/plugin/database update and rollback authorization.

`v0.29 Fleet Control Plane` composes these systems. It does **not** add arbitrary shell execution and does not bypass either authorization boundary.

## Flow

```text
F22 Device Nodes ── identity/capabilities/heartbeat ──┐
                                                     ├─> Inventory Projection
v0.28 Fleet Nodes ─ version/sequence/known-good ─────┘          │
                                                                ▼
                                                    Groups / Tags / Region
                                                                │
                                                    Frozen Selector Snapshot
                                                       │                │
                                                       ▼                ▼
                                              Remote Command Plan   Rollout Scope
                                                       │                │
                                                F22 authorize       v0.28 cohort
                                                       │                │
                                                       └───────┬────────┘
                                                               ▼
                                                   PostgreSQL fencing
                                                               │
                                                       Event + Evidence
```

## Safety invariants

- membership is frozen before side effects; selector drift rejects the operation;
- arbitrary shell is disabled;
- raw credentials are rejected; F22 Secret Broker handles remain mandatory;
- high/critical commands retain F22 approval requirements;
- controller lease and command-plan transitions use PostgreSQL fencing CAS;
- offline rejoin cannot reduce update sequence or reuse stale fencing;
- rollout by region/group/tag is intersected with, never substituted for, the v0.28 deterministic cohort;
- tenant FKs are composite and all new tables use FORCE RLS;
- selector snapshots, recovery evidence and control events are append-only.

# PhxClaw v0.35 — AI SRE & Cost/Capacity Autopilot

## Purpose

Turn v0.31-v0.34 AI routing into an operationally governed service plane without granting a new authority to promote models or bypass signed portfolio membership.

## Flow

```text
Aggregated telemetry (no raw prompt/output)
        ↓
SLO + error budget
Demand / token / cost forecasts
Signed rate-limit evidence
Queue/backpressure
        ↓
Incident detection
        ↓
Bounded Autopilot plan
        ↓
Signed-plan verification
        ↓
v0.30 leader lease + fencing
        ↓
Reversible action inside v0.34 portfolio
```

## Automatic vs approval-required

Automatic actions must be reversible, remain inside the signed v0.34 portfolio, remain within the signed v0.35 operational bounds and preserve v0.31 privacy/budget constraints.

External capacity purchase is disabled by default. A cost-increasing action becomes automatic only if the signed SRE policy explicitly permits it and the projected cost remains inside the pre-authorized headroom. Otherwise the system emits a plan for approval.

## Evidence discipline

- SRE policy: Ed25519 signed and immutable.
- Provider rate-limit snapshot: Ed25519 signed, TTL bound and revalidated at admission time (including reset timestamp).
- SLO, demand and cost records: evidence hashes only; stale/future telemetry is rejected using the signed policy freshness limit.
- Incidents: append-only events; identical evidence deduplicates, contradictory semantics for the same evidence hash fail closed.
- Autopilot plan: Ed25519 signed and bound to source-state, portfolio, SRE policy, cost forecast and incident-detection evidence; execution rechecks expiry.
- Execution: requires v0.30 leader lease and fencing epoch.

No raw prompts or raw outputs are persisted.


## Native RLS proof

Migration may use an administrative `DATABASE_URL`, but the cross-tenant RLS test must use a separate `PHXCLAW_RLS_DATABASE_URL`. The runner refuses superuser/BYPASSRLS roles. The SQL test also refuses to run vacuously: tenant A must already have a real v0.34 portfolio fixture, and the inserted SRE policy is asserted before switching to tenant B.

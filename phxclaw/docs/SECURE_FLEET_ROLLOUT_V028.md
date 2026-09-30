# PhxClaw v0.28 — Secure Auto-Updater + Fleet Rollout

v0.28 adds a fail-closed staged updater control plane. A GA/update manifest remains signed by the release authority from v0.27. Fleet operations use a separate `config/trusted-fleet-signers.v028.json` trust store with explicit purposes (`fleet.rollout`, `fleet.health`, `fleet.state`, `fleet.rollback`). Release authority therefore does not automatically become fleet authority.

Rollouts are deterministic by `SHA256(node_uuid || rollout_uuid)` bucket and support Canary/Beta/Stable stages. A stage advances only after signed, fresh health evidence satisfies sample, soak and threshold gates. Health failure pauses automatically; critical failure requests rollback. Every state transition increments generation and fencing token.

Node preflight verifies the v0.27 update manifest, exact plan identity, cohort membership and monotonic sequence before installation is permitted. Core, plugins and database are coordinated by a separately signed `fleet.component_plan` expand/contract DAG bound to the component policy hash. `database_contract` is prohibited until 100% of the fleet is compatible, the signed fleet state is `completed`, and a short-lived `fleet.database_contract` approval is bound to the exact component plan and fleet state. After the irreversible fence, automatic rollback is prohibited; recovery becomes an explicit restore operation.

The migration uses tenant-composite foreign keys, FORCE RLS and append-only health/event evidence. This source package is not evidence of a real production fleet rollout; native devices, installers, telemetry, migrations and rollback drills remain E2E gates.

PostgreSQL exposes `phoenix_fleet_transition(...)`, which performs compare-and-swap on the expected fencing token and rejects stale controllers.

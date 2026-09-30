# PhxClaw Mission Runtime v0.11

The v0.11 Mission Runtime closes the first deterministic coding loop without silently replacing missing dependencies with mocks.

## Flow

`MissionSpec -> AgentCatalog -> permission gate -> CodeWorkspace -> typed action -> LiveEventHub -> EvidenceLedger -> MissionReport`

## Workspace isolation

By default a mission starts from a clean Git repository and creates an isolated worktree/branch under `agent/mission/<mission-id>`. The main working tree is not edited directly.

## Typed actions

- `snapshot`
- `write_file`
- `command_gate`
- `diff`
- `model_synthesis`
- `extension_call`

`model_synthesis` and `extension_call` are fail-closed until real executors are attached. No test double is selected by the runtime.

## Security

The Code Workspace uses direct process execution only. Shell composition is not used. Programs are controlled by an exact allowlist, file writes are confined to the selected worktree, `..`/absolute path escapes are rejected, Git mutation is separately permissioned, command time is bounded and output size is capped.

Every mission step is assigned to an Agent Registry role and the role must also hold the system permission corresponding to the action (`workspace.snapshot`, `workspace.file.write`, `workspace.gate.run`, `workspace.diff`, etc.).

## Build/test gates

The reference Rust validation mission requires:

1. `cargo fmt --check`
2. `cargo check --workspace`
3. `cargo test --workspace`
4. objective-proof diff collection

If `cargo` is absent, the mission fails. That is intentional.

## Persistence

Migration `0014_mission_runtime.sql` adds `phoenix_missions` and `phoenix_mission_steps`. `PostgresMissionJournal::persist_report` persists completed mission reports when PostgreSQL is available.

## Current verification boundary

The host workflow smoke test proves Git initialization, clean-start detection, worktree creation, direct process gate execution, diff capture and worktree removal on this host. Rust compilation of this crate remains unverified here because `cargo/rustc` are not installed in the execution environment.

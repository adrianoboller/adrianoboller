# PhxClaw v0.16 — Claw Code Source Integration

## Upstream

Canonical source: `https://github.com/ultraworkers/claw-code`

- license: MIT;
- canonical runtime tree: `rust/`;
- upstream CLI binary: `claw`;
- PhxClaw policy: private-only integration.

## What was integrated

### 1. MCP Runtime v2

`crates/phxclaw-mcp-lsp-runtime` now models both MCP eras:

- legacy: 2024-11-05, 2025-03-26, 2025-06-18, 2025-11-25;
- modern: 2026-07-28.

The runtime adds:

- typed JSON-RPC ids and errors;
- Content-Length framing accepting CRLF and LF header termination;
- notification classification instead of assuming the next frame is always the response;
- protocol-era classification and legacy version validation;
- legacy `notifications/initialized` generation;
- modern `server/discover` request generation;
- lifecycle phase validation;
- degraded-server/tool report;
- stable MCP tool qualification `mcp__server__tool`;
- explicit transport model: stdio / Streamable HTTP / SSE / WebSocket / in-process / managed proxy.

Only the framing/lifecycle/version contract is source-ready in v0.16. Live remote transports still require E2E implementation and therefore remain deny-by-default/review.

### 2. Claw Code private bridge

New crates/apps:

- `crates/phxclaw-claw-code-bridge`;
- `apps/phxclaw-claw-code-plugin`.

Capabilities:

- `claw.health`;
- `claw.doctor`;
- `claw.status`;
- `claw.sandbox.status`;
- `claw.mcp.status`;
- `claw.skills.list`;
- `claw.agents.list`;
- `claw.prompt`.

`claw.prompt` sends the prompt via stdin rather than placing the prompt body in process arguments.
Workspace access is confined to `PHXCLAW_CLAW_WORKSPACE`.

### 3. Reproducible vendoring

Scripts:

- `scripts/vendor_claw_code.sh`;
- `scripts/vendor_claw_code.ps1`.

They clone the canonical upstream repository into `private/vendor/claw-code/upstream`, resolve the exact commit and write `UPSTREAM.lock.json`.

## Environment limitation during this build

The build container could resolve the upstream repository through the web research surface but its shell environment could not resolve `github.com`, so a full `git clone` could not be completed here.

For that reason this package does **not** claim that the entire upstream checkout is embedded. The vendoring scripts and integration code are present, and the actual clone must run on the private build host with network access.

## License handling

The upstream MIT license is preserved under:

`private/vendor/claw-code/LICENSE.MIT`

No PhxClaw repository is made public by this integration.

## Remaining gates

- run vendoring script on networked private build host;
- record `UPSTREAM.lock.json` with exact commit;
- `cargo fmt --check`;
- `cargo check --workspace`;
- `cargo test --workspace`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- Claw bridge E2E with a real built `claw` binary;
- MCP stdio E2E against a real server that emits notifications;
- MCP 2026-07-28 Streamable HTTP E2E;
- remote transport auth through F23 Secret Broker.

# ADR-0064 — Managed MCP/LSP Runtime

## Decision
F18 uses a managed, deny-by-default runtime.

- MCP standard transports: stdio and Streamable HTTP.
- LSP process transport: stdio with `Content-Length` framing.
- Executables require canonical absolute-path allowlisting.
- Environment variables are allowlist-only.
- Remote MCP origins are explicit allowlist; HTTPS is mandatory except loopback HTTP.
- Requests have timeout and cancellation.
- Unexpected disconnects use bounded exponential reconnect policy.
- Session/request activity emits Event Bus envelopes and Evidence Ledger records.
- WebSocket remains a PhxClaw extension transport, not a standard MCP release requirement.

## Why
Running arbitrary MCP/LSP commands is equivalent to granting local process authority. A protocol adapter without executable, cwd, env, network, timeout and evidence policy would violate the PhxClaw fail-closed constitution.

## Release gate
Source-ready is not release-ready. Cargo compilation plus independent MCP/LSP server E2E remain mandatory.

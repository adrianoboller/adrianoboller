# PhxClaw v0.64 — MCP + LSP Managed Runtime

## Implemented
- real child-process spawn for MCP/LSP stdio;
- newline JSON MCP framing;
- LSP `Content-Length` framing;
- legacy MCP initialize + initialized;
- modern MCP server/discover;
- LSP initialize/initialized/shutdown/exit;
- JSON-RPC response correlation;
- notification collection limits;
- timeout and cancellation;
- LSP `$/cancelRequest`;
- MCP `notifications/cancelled` on shared stdio transports;
- Streamable HTTP request transport with JSON or SSE responses;
- modern MCP standard headers;
- bounded reconnect policy;
- executable/cwd/env/origin allowlists;
- Evidence Ledger + EventEnvelope audit output;
- append-only PostgreSQL session/request/event evidence.

## Deliberate boundary
WebSocket is retained only as a non-standard extension identifier. The standard remote MCP implementation target is Streamable HTTP.

## Still required for release
- `cargo check/test/clippy` on the Rust implementation;
- E2E against independent MCP 2026 and legacy servers;
- E2E against rust-analyzer/pyright or equivalent LSP servers;
- network disconnect/reconnect/timeout/cancellation chaos tests.

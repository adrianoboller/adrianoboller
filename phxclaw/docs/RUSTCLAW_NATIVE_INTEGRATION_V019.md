# PhxClaw v0.19 — RustClaw native MIT integration

## License decision

The user-supplied `RustClaw-main(1).zip` contains `LICENSE.md` with a complete MIT grant.
PhxClaw therefore moved RustClaw from quarantine to verified vendoring.

- Upstream: `https://github.com/Adaimade/RustClaw`
- Supplied version: `0.5.0`
- License: MIT
- Vendored snapshot: `private/vendor/rustclaw/upstream`
- Lock/provenance: `private/vendor/rustclaw/UPSTREAM.lock.json` and `PROVENANCE.json`

The PhxClaw repository stays private. MIT notices remain preserved.

## What was adopted natively

`phxclaw-rustclaw-native` adapts selected MIT concepts instead of linking the entire upstream crate:

1. Gateway request/response frame model.
2. Challenge/auth lifecycle, changed to opaque Secret Broker lease references instead of plaintext tokens.
3. Session/message model, changed to UUIDv7 and SHA-256 message evidence.
4. Conversation user-scope derivation, normalized to `channel:<channel>:user:<id>`.
5. Scheduled job/cron model with a PhxClaw minimum recurring interval of 60 seconds.
6. MCP newline-delimited JSON compatibility from RustClaw plus PhxClaw Content-Length framing.
7. Canonical `mcp__server__tool` naming alongside the RustClaw legacy `mcp_server_tool` form.

## What was intentionally NOT adopted

- RustClaw API keys embedded in `AgentConfig`.
- Plaintext gateway tokens in connection frames.
- SQLite as the official PhxClaw state database.
- Shell commands assembled from arbitrary untrusted input.
- Direct GitHub/email/channel secrets in configuration.
- `rustmem` git dependency as an implicit PhxClaw memory backend.

PhxClaw keeps Secret Broker F23, PostgreSQL, Capability Broker, Event Bus and Evidence Ledger as the governing layers.

## Persistence

Migration `0019_rustclaw_native_session_cron.sql` maps the useful session/cron model into PostgreSQL.
The upstream SQLite store remains only in the vendored snapshot for provenance/reference.

## MCP interoperability

The native compatibility layer accepts both:

- `Content-Length` framed JSON-RPC used by PhxClaw/Claw Code compatibility.
- NDJSON used by the supplied RustClaw MCP client.

This is a compatibility codec, not a claim that every remote MCP transport is already E2E-verified.

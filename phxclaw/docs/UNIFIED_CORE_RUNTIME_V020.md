# PhxClaw v0.20 — Unified Core Runtime

## Goal

PhxClaw is presented as one product. MIT-derived RustClaw/openclaw-rs code is retained only as vendored provenance; user-facing APIs are Phoenix-native.

## User-facing layers

```text
phx / PhxClaw Desktop
        │
        ▼
phxclaw (single Rust binary)
        │
        ▼
phxclaw-core-runtime
        │
 ┌──────┼────────┬────────┬────────┐
 ▼      ▼        ▼        ▼        ▼
Mission Team   Channel   MCP     Model
Runtime Runtime Gateway  Runtime  Gateway
        │
        ├─ Secret Broker
        ├─ PostgreSQL
        ├─ Evidence Ledger
        └─ Plugin/Extension Host
```

Compatibility crates remain implementation details. New code must depend on Phoenix-native contracts instead of calling upstream crates directly.

## Compatibility policy

- `private/vendor/rustclaw/upstream`: MIT source snapshot and license.
- `private/vendor/openclaw-rs/upstream`: MIT source snapshot and license.
- `phxclaw-rustclaw-native`: adaptation layer, not a user-facing subsystem.
- `phxclaw-core-runtime`: public facade used by the product binary.
- legacy bridge crates remain for migration compatibility only and should not appear in normal UX.

## Core rules

- UUIDv7 for new durable identities.
- PostgreSQL is authoritative persistent state.
- Secret Broker supplies credentials by handle/lease.
- Event Bus + Evidence Ledger record side effects.
- Capability/permission checks happen before actions.
- No mock fallback in production.

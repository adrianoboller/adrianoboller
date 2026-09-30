# PhxClaw v0.17 — Audit and Integration of Uploaded Rust Sources

## Scope

User supplied two source archives:

- `openclaw-rs-main.zip`
- `RustClaw-main.zip`

The goal is to extract reusable architecture without weakening PhxClaw rules: private-only repositories, UUIDv7 persistent identity, deny-by-default capabilities, evidence, sandboxing, provenance, rollback and no silent mock fallback.

## openclaw-rs

### License and provenance

The uploaded source contains a root `LICENSE` with the MIT License. The workspace declares repository `https://github.com/neul-labs/openclaw-rs`, version `0.1.0`, Rust `1.85`, edition `2024`.

The full user-supplied snapshot is preserved under:

`private/vendor/openclaw-rs/upstream`

with archive SHA-256 and provenance in `private/vendor/openclaw-rs/UPSTREAM.lock.json`.

### High-value components

1. `openclaw-channels`
   - channel capability description
   - provider registry
   - allowlist
   - deterministic agent routing
   - probe/health model

2. `openclaw-providers`
   - common provider trait
   - structured completion/message/tool types
   - Anthropic/OpenAI adapters
   - token usage normalization

3. `openclaw-plugins`
   - plugin lifecycle hooks
   - native and WASM plugin paths
   - hook model: before/after message, before/after tool, session start/end, response/error

4. `openclaw-core`
   - encrypted credential store
   - redacted API-key wrapper
   - append-only session events
   - CRDT-style session projection
   - path/message validation
   - structured session keys

5. `openclaw-ipc`
   - typed IPC messages and transport abstraction

6. `openclaw-gateway`
   - RPC/gateway separation
   - middleware and event interfaces

### Adopted into PhxClaw v0.17

- `ChannelCapabilities`, `ChannelProbe`, allowlist and priority agent routing concepts added to `phxclaw-channel-gateway`.
- plugin lifecycle hook taxonomy added to `phxclaw-plugin-sdk`.
- source kept vendored with MIT notice.
- process bridge `phxclaw-openclaw-rs-bridge` added so a separately built `openclaw` binary can be used through capabilities instead of bypassing policy.
- knowledge source `openclaw-rs-user-source` added for Research Agent and Documentador.

### Explicit non-adoptions

- Native FFI plugin loading is **not copied into PhxClaw core**. The upstream implementation requires `unsafe`; PhxClaw keeps process/WASM isolation as the preferred model.
- upstream UUID/session identities are not substituted for PhxClaw UUIDv7 identity.
- upstream credential storage is not yet adopted as F23 implementation; it is a reviewed source for the future Secret & Credential Broker.

## RustClaw

### License status

The uploaded README contains an MIT badge and names `https://github.com/Adaimade/RustClaw` as the clone URL, but the uploaded ZIP contains **no LICENSE file**. The archive therefore remains quarantined at:

`private/vendor-quarantine/rustclaw/upstream`

and is **not linked or copied into PhxClaw core**.

`private/vendor-quarantine/rustclaw/LICENSE_STATUS.json` records this state.

### High-value architecture found

- lightweight Axum/WebSocket gateway
- fail-closed non-loopback gateway without token
- streaming agent runner with tool iteration
- Telegram, Discord and WebChat channels
- MCP manager
- cron scheduler
- filesystem/exec/search/GitHub/email/system tools
- session memory/store separation

### Adopted safely

- external process bridge `phxclaw-rustclaw-bridge`.
- read-only health/status/GitHub-scan capabilities are mapped behind Phoenix capabilities.
- `rustclaw.agent.prompt` is deny-by-default because the upstream CLI accepts the prompt as a positional argument, which exposes it in process argv on many operating systems. PhxClaw refuses this capability unless explicitly enabled with `PHXCLAW_RUSTCLAW_ALLOW_PROMPT_ARGV=true`.
- source available to Research Agent/Documentador as a **non-authoritative quarantined reference**.

### Not adopted

- shell/system command implementation is not copied because PhxClaw already has a typed, policy-gated system automation layer.
- UUIDv4 use is not adopted; PhxClaw remains UUIDv7.
- GitHub auto-fix/PR actions are not exposed by default.
- source code is not linked until license text is verified and preserved.

## Integration architecture

```text
Uploaded source
   |
   +-- openclaw-rs (MIT verified)
   |      +-- vendored snapshot
   |      +-- channel concepts -> F21
   |      +-- hooks -> Plugin SDK
   |      +-- provider/secrets/events -> future adoption
   |      +-- process bridge -> Phoenix capabilities
   |
   +-- RustClaw (license text missing in ZIP)
          +-- quarantine snapshot
          +-- architecture reference
          +-- process bridge only
          +-- no source linkage

All invocations
   -> Capability Broker
   -> permission policy
   -> Extension Host / process bridge
   -> Event Bus
   -> Evidence Ledger
```

## Next adoption targets

1. F23 Secret & Credential Broker: compare OpenClaw-rs AES-GCM/redaction design against Phoenix requirements.
2. F21 real channel providers: use the new capability/probe/allowlist contracts for Telegram/Discord/WhatsApp plugins.
3. Model Gateway adapters: normalize Anthropic/OpenAI-compatible provider execution using the reviewed provider trait model.
4. F22 Device/IPC: evaluate openclaw-rs typed IPC as an input for node transport while retaining Phoenix pairing/capability tokens.

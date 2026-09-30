# F21 Channel Providers — v0.68

Provider adapters implemented behind the existing provider-neutral Channel Gateway:

- Telegram Bot API
- Discord REST API
- Slack Web API (`chat.postMessage`, `auth.test`)
- WhatsApp Cloud API (Graph endpoint version is configuration, never hard-coded globally)
- Microsoft Teams via Microsoft Graph (chat or channel route)

## Security invariants

- Tokens are resolved through short-lived Secret Broker leases.
- Only allowlisted HTTPS origins are accepted by default.
- Tokens are scrubbed from request errors and response bodies before propagation.
- Normal Teams send uses delegated permissions; application-only migration semantics are not treated as normal messaging.
- Provider delivery remains fail-closed; no mock fallback exists in release paths.

Credentialed provider E2E is a native release gate, not a source implementation gap.

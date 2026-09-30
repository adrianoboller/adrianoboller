# F21 Channel Gateway — PhxClaw v0.15

State: **review**.

## Implemented in source
- provider-neutral channel contract;
- external identity -> Phoenix principal binding;
- identity states: pending / active / blocked;
- stable session reuse by principal + channel + account + conversation;
- external-message de-duplication;
- outbound preparation;
- provider injection through `ChannelProvider`;
- Event Bus hooks;
- Evidence Ledger hooks;
- PostgreSQL migration `0017_channel_gateway.sql`;
- deny-by-default channel capabilities.

## Security model
Channel providers are plugins. The gateway does not embed Telegram/WhatsApp/Slack credentials and does not bypass the future F23 Secret Broker. An inbound identity must be explicitly bound and active. Blocked or pending identities are rejected.

## Still required before DONE
- `cargo check/test/clippy`;
- PostgreSQL concurrency / de-dup E2E;
- real private channel provider plugin;
- webhook authenticity verification per provider;
- rate limiting and abuse controls;
- F23 secret integration;
- restart/recovery tests.

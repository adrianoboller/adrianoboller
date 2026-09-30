# F23 — Secret & Credential Broker v0.18

Status: **review**.

## Goals

PhxClaw must never place API keys, bot tokens, passwords or refresh tokens in prompts, manifests, process argv or evidence payloads. F23 centralizes encrypted storage and ephemeral access.

## Architecture

```text
Secret Value
  ↓
SecretBroker::store
  ↓ AES-256-GCM
Encrypted .phxsecret
  ↓
metadata only → PostgreSQL / Event Bus / Evidence Ledger
  ↓
lease UUIDv7 + scope + TTL + consumer
  ↓
SecretBroker::resolve
  ↓
SecretValue([REDACTED])
  ↓
provider API call
  ↓
lease revoke / expire
```

### Implemented

- AES-256-GCM at-rest encryption.
- UUIDv7 descriptors and leases.
- secret SHA-256 fingerprint without storing plaintext.
- scope validation.
- lease TTL 1..3600 seconds.
- version-bound leases: rotation invalidates older leases.
- secret revocation and lease revocation.
- Event Bus + Evidence Ledger metadata only.
- redacted `Display` / `Debug`.
- CLI bootstrap storage and doctor.

### Master key

`FileMasterKeyProvider` is a bootstrap/private-host provider. It creates a 32-byte random key with restrictive permissions where the OS supports them. Production release requires a platform key provider (OS keyring/HSM/KMS or equivalent) and remains a release gate.

### CLI

```bash
phx secrets init
printf '%s\n' "$TOKEN" | phx secrets store telegram-bot --namespace channels \
  --scope channel:telegram:send --scope channel:telegram:probe
phx secrets list
phx secrets show <uuid>
phx secrets doctor <uuid>
printf '%s\n' "$NEW_TOKEN" | phx secrets rotate <uuid>
phx secrets lease <uuid> --consumer channel.provider.telegram \
  --scope channel:telegram:send --ttl 30
phx secrets revoke <uuid>
```

Values should be supplied over stdin instead of command-line arguments.

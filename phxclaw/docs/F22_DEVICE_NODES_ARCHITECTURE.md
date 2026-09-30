# F22 — Device Nodes architecture (v0.21)

## Fluxo

```text
Device/Edge Node
  -> enrollment token (one-time, hash only at rest)
  -> Ed25519 node identity
  -> signed envelope + sequence + nonce
  -> replay reservation (PostgreSQL unique constraints)
  -> capability + permission policy
  -> command approval gate
  -> fencing token
  -> execution
  -> Event Bus / Evidence Ledger / command event journal
```

## Regras
- UUIDv7 para identidades novas.
- PostgreSQL é a verdade autoritativa.
- Chaves privadas de nó nunca ficam no servidor.
- Segredos de APIs/serviços não entram em `arguments`; somente `secret_uuid`/`lease_uuid`.
- Nós `quarantined`/`revoked` falham fechados.
- Replays são bloqueados por `(tenant,node,session,sequence)`, nonce e `message_uuid`.
- Comandos `high`/`critical` ou capabilities protegidas exigem approval.
- Fencing token impede ACK/execução de lease antigo.
- Sem fallback mock em release.

## Transporte
Este overlay define o **envelope de dispositivo**, não redefine o Process Protocol v1 do Extension Host. A ligação ao `phxclaw-plugin-sdk` deve reutilizar o contrato exato já presente na base v0.20 durante o merge completo.

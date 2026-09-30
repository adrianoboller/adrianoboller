# PhxClaw Evidence Ledger v0.6

O ledger fornece rastreabilidade local imediata para as ações executadas pelo Desktop Host.

## Invariantes

1. `uuid` e `action_uuid` usam UUIDv7.
2. cada registro contém `previous_hash`.
3. `record_hash = SHA256(canonical_payload)`.
4. o payload canônico inclui o hash anterior.
5. append usa flush + `sync_data()`.
6. o ledger é validado ao abrir; cadeia inválida falha fechada.
7. texto digitado pelo usuário é registrado como quantidade de caracteres, não conteúdo.
8. stdout/stderr e resultados WebView são limitados antes de entrar na evidência.

## Fluxo

```text
Action UUIDv7
   |
   v
Policy Gate
   |
   +--> denied --------+
   |                   |
   v                   v
Execute              Evidence
   |                   ^
   v                   |
Result ----------------+
   |
   v
Live Event: evidence.ledger/recorded
```

## Persistência

- primeira camada: JSONL append-only local para registrar ações mesmo antes do PostgreSQL estar disponível;
- camada oficial: `phoenix_evidence_records` no PostgreSQL via migration 0009;
- sincronização JSONL → PostgreSQL entra na próxima fase de persistência do Desktop Host.

# PhxClaw Process Protocol v1

## Objetivo

Padronizar IPC entre runtime e plugins `entrypoint.type=process` sem acoplar a implementação a Python, Rust, Go, Node ou outra linguagem.

## Frame

Um JSON por linha (`JSONL`). Máximo: 4 MiB.

### Request

```json
{
  "protocol": "phxclaw-process-v1",
  "message_uuid": "<uuidv7>",
  "correlation_uuid": null,
  "kind": "execute",
  "sent_at": "2026-09-27T19:45:00Z",
  "payload": {}
}
```

### Reply

```json
{
  "protocol": "phxclaw-process-v1",
  "message_uuid": "<uuidv7>",
  "correlation_uuid": "<request message_uuid>",
  "status": "ok",
  "emitted_at": "2026-09-27T19:45:00Z",
  "payload": {},
  "error": null
}
```

## Regras

- exatamente um reply para uma chamada request/response;
- correlation obrigatória no reply;
- UUID persistente no padrão UUIDv7;
- erros possuem `code`, `message`, `retryable`;
- protocolo desconhecido deve ser `rejected`;
- `health`, `execute`, `cancel` e `shutdown` são os kinds base.

## Implementação

Crate: `phxclaw-process-protocol`.

`ProcessRunner` usa o `SandboxPlan`; portanto o IPC não contorna a política de sandbox.

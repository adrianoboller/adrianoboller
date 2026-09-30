# PhxClaw Live Event API v0.6

## Endpoints

```text
GET  /v1/health
GET  /v1/events?limit=100&after=<uuidv7>
GET  /v1/events/sse
GET  /v1/events/ws
POST /v1/events/publish
```

`/v1/health` não entrega segredos. Os demais endpoints exigem:

```http
Authorization: Bearer <token>
```

## EventEnvelope

```json
{
  "uuid": "<uuidv7>",
  "topic": "desktop.action",
  "event_type": "succeeded",
  "aggregate_uuid": null,
  "correlation_uuid": "<uuidv7-or-null>",
  "causation_uuid": "<uuidv7-or-null>",
  "payload": {},
  "occurred_at": "2026-09-27T20:00:00Z",
  "schema_version": 1
}
```

## Delivery planes

```text
PostgreSQL Outbox  = durabilidade/publicação oficial
LiveEventHub       = fan-out de baixa latência dentro do host
Tauri event        = Rust -> Command Center
SSE                = cliente externo unidirecional
WebSocket          = cliente externo persistente
```

O LiveEventHub mantém um replay ring limitado; ele não substitui o Outbox.

## Backpressure

O hub usa `tokio::sync::broadcast`. Consumidores atrasados recebem um sinal `stream_lagged` com a quantidade de frames perdidos e devem recuperar o estado pelo snapshot/replay ou PostgreSQL.

## Host-control

Tópicos como `desktop.*`, `system.command*`, `system.input*`, `screen.*` e `webview.action*` são negados no publish HTTP por padrão. A UI nativa e agentes internos continuam usando o Event Bus interno sob capability policy.

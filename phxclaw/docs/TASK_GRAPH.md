# F06 — Task Graph

## Contrato

Cada `TaskSpec` possui:

- UUIDv7;
- nome;
- capability;
- payload;
- dependencies;
- requested permissions;
- retry policy;
- approval gate opcional;
- idempotency key;
- priority.

## Validações

`TaskGraph::new()` rejeita:

- UUID que não seja v7;
- UUID duplicado;
- idempotency key duplicado;
- dependência inexistente;
- auto-dependência;
- ciclos;
- retry policy inválida.

## Scheduler

`TaskScheduler` recalcula readiness em ordem topológica.

Tarefas elegíveis são ordenadas por:

1. priority descendente;
2. UUID ascendente.

Retries usam backoff exponencial determinístico:

`min(base_delay * 2^(attempt-1), max_delay)`

Sem jitter para preservar reprodutibilidade do core. Um layer externo poderá adicionar jitter apenas se a política permitir.

## Approval

Tarefas com `ApprovalGate.required=true` ficam em `waiting_approval` depois que as dependências terminam. Somente um approval explícito libera `ready`.

## PostgreSQL

Migration: `0004_task_graph.sql`.

O journal de eventos é append-only por UUID. O worker distribuído futuro deverá reivindicar linhas com `FOR UPDATE SKIP LOCKED` + fencing token.

# Agent Runtime — F05 / v0.3

## Princípio

Agente é plugin. Nome, personalidade, domínio e modelo não dão privilégios. O runtime confia apenas em manifesto validado, capability declarada, permissions aprovadas e sandbox.

## Manifest profile

Um plugin `kind=agent` precisa adicionar:

```json
{
  "agent": {
    "display_name": "Oráculo",
    "role": "Decisão técnica baseada em evidências",
    "priority": 90,
    "accepts": ["application/json"],
    "emits": ["application/json"]
  }
}
```

## Routing

A chamada abaixo é resolvida apenas entre agentes `ready` que declaram a capability:

```rust
let request = AgentRequest::new(
    "decision.technical",
    serde_json::json!({"subject": "plugin-integrity-gate"}),
);
let decision = runtime.route(&request)?;
```

A ordenação é estável: prioridade descendente e UUID ascendente.

## Permission gate

O request pode declarar claims. Se uma claim não estiver presente no manifesto do agente, a chamada falha antes do dispatch.

```rust
request.requested_permissions.push(PermissionClaim {
    name: "filesystem.write".into(),
    scope: "/etc/passwd".into(),
});
```

Esse exemplo é negado porque nenhum agente recebe esse privilégio.

## Process execution

Em v0.3, o backend Bubblewrap executa um lifecycle smoke do processo. O transporte do payload por stdin/stdout com envelopes versionados entra no F06 junto com Task Graph e idempotência.

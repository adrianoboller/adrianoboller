# Agent Catalog + Logical Agent Runtime v0.8

## Motivo

PhxClaw possui 110 agentes/subagentes declarados na planilha oficial. Criar 110 processos independentes seria desperdício e confundiria identidade organizacional com execução física.

A v0.8 separa:

```text
Logical Agent
= papel, missão, capabilities, permissions, knowledge ACL, model policy

Process Plugin / Tool Provider
= executor concreto, sandbox, processo, serviço, API ou modelo
```

## Fonte declarativa

Runtime:

```text
config/agents/001-...agent.json
...
config/agents/110-...agent.json
```

Índice:

`config/agents/registry.index.json`

Cada manifesto preserva o UUIDv7 gerado a partir da planilha oficial.

## AgentCatalog

Responsabilidades:

- carregar diretório;
- validar versão do manifesto;
- validar UUIDv7;
- rejeitar duplicidade de UUID/nome;
- exigir pelo menos uma capability;
- lookup por nome/UUID;
- filtrar candidatos por capability;
- verificar source ACL;
- verificar permission/scope.

## LogicalAgentRuntime

Estados:

```text
registered -> ready -> stopped
```

Roteamento:

```text
AgentRequest capability
        |
        v
ready agents
        |
        v
manifest supports capability?
        |
        v
permission/scope authorized?
        |
        v
deterministic ordering
        |
        v
LogicalRouteDecision
```

O Research Pipeline usa `route_named("Research Agent", ...)` porque o workflow exige esse papel específico. Outros workflows podem usar `route(capability)` para seleção por capability.

## Segurança

O catálogo não executa conteúdo da planilha/JSON. Ele somente interpreta campos de contrato.

Nenhum comando de shell, URL ou script vira execução automática por existir no manifesto. Toda execução real continua passando por Capability Broker/policy e pelo provider correspondente.

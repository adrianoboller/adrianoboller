# F15 — Skill Runtime + Memory/Context Engine

## Regra estrutural

PhxClaw separa deliberadamente **Skill**, **Memory** e **Context Pack**:

```text
Memory = fato/estado durável
Skill  = procedimento versionado
Context Pack = seleção mínima e temporária para uma execução
```

Nenhum agente recebe a memória inteira. O Context Compiler aplica escopo,
classificação, namespace, limite de itens e limite de bytes antes de montar o
contexto de uma tarefa.

## Skill lifecycle

```text
candidate
   │
   ▼
validated  -- evidence required
   │
   ▼
promoted   -- Objective Proof + QA evidence required
   │
   ├── disabled
   └── rejected
```

Promotion é um gate explícito. Uma experiência bem-sucedida não vira Skill
oficial automaticamente.

## Memory

Cada `MemoryRecord` possui:

- UUIDv7;
- namespace + key;
- scope: session / agent / project / organization;
- classification: public / internal / confidential / restricted;
- evidence refs;
- confidence 0..1000;
- expiry opcional;
- SHA-256 do registro.

## Context Compiler

O compilador é determinístico e, por padrão:

- exclui memória expirada;
- exclui dados acima da classificação permitida;
- filtra namespaces quando configurado;
- limita itens;
- limita bytes;
- produz `ContextPack` com UUIDv7 e correlation UUID.

## Event Bus

Eventos previstos:

```text
skill.registered
skill.validated
skill.promoted
skill.disabled
memory.recorded
memory.expired
context.compiled
context.rejected
```

Cada side effect deve produzir Evidence Ledger correlacionado.

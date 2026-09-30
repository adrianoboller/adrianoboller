# Research Context Pipeline v0.8

## Objetivo

Conectar os componentes que já existiam isoladamente para que uma pesquisa técnica possua identidade, roteamento, skill, fonte, contexto, eventos e evidência de ponta a ponta.

## Contrato principal

Entrada:

```rust
pub struct ResearchTask {
    pub uuid: Uuid,
    pub correlation_uuid: Uuid,
    pub query: String,
    pub project: String,
    pub preferred_agent: String,
    pub source_name: String,
    pub skill_name: String,
    pub skill_version_requirement: String,
    pub freshness: SourceFreshness,
    pub max_source_hits: usize,
    pub context_policy: ContextPolicy,
    pub scope_filter: ContextScopeFilter,
}
```

Saída:

```rust
pub struct PreparedResearchTask {
    pub task_uuid: Uuid,
    pub correlation_uuid: Uuid,
    pub agent: PreparedAgent,
    pub skill: SkillResolution,
    pub source: PreparedSource,
    pub source_hits: Vec<OfflineSearchHit>,
    pub context: ContextBundle,
    pub research_record: ResearchRecord,
    pub evidence: Vec<EvidenceRef>,
    pub model_context: Value,
    pub prepared_at: DateTime<Utc>,
}
```

## Fluxo

```text
1. ResearchTask
2. research.pipeline/started
3. Agent Runtime route_named()
4. agent.runtime/routed
5. LazySkillResolver.resolve()
6. skill.runtime/resolved
7. SourceRegistry.resolve_for_query()
8. offline search OU online official plan
9. knowledge.source/*
10. ContextCompiler.compile_for_query()
11. context.compiler/compiled
12. EvidenceLedger.append()
13. evidence.ledger/recorded
14. ResearchRecord + model_context
15. research.pipeline/ready
```

## Roteamento de agente

O pipeline não codifica as permissões do Research Agent em um segundo cadastro paralelo. Ele carrega o manifesto oficial de `config/agents/` via `AgentCatalog`, e o `LogicalAgentRuntime` autoriza a tarefa conforme capabilities, permissions/scopes e knowledge-source ACL.

Exemplo do request:

```rust
let mut request = AgentRequest::new(
    "research.collect",
    json!({
        "task_uuid": task.uuid,
        "query": &task.query,
        "source": &task.source_name,
    }),
);

request.requested_permissions = vec![
    PermissionClaim {
        name: "research.collect".into(),
        scope: "project".into(),
    },
    PermissionClaim {
        name: "knowledge.rust.read".into(),
        scope: task.source_name.clone(),
    },
    PermissionClaim {
        name: "context.compile".into(),
        scope: "project".into(),
    },
    PermissionClaim {
        name: "skill.read".into(),
        scope: "project".into(),
    },
];
```

## Skill lazy-loading

O runtime lê primeiro `registry.index.json`, que é pequeno. Só depois de escolher a skill ele abre o manifesto completo.

Gates:

```text
name/version
+ required capabilities
+ knowledge source
+ state policy
+ canonical SHA-256
+ root/path containment
= skill carregável
```

Perfis:

```text
development: validated ou promoted
production:  somente promoted
```

## Source Registry

Para `SourceFreshness::Stable`:

```text
snapshot/index offline disponível?
       |
      sim ----> Offline
       |
      não ----> endpoint oficial online
```

Para `Current`/`Latest`, o endpoint oficial online recebe preferência.

A resolução online não é o mesmo que efetuar uma requisição. O v0.8 produz um `PreparedSource::Online { retrieval_required: true }`; o executor HTTP/Browser deve fazer a recuperação sob Egress Broker.

Essa separação impede que um simples ato de montar contexto abra rede implicitamente.

## Busca offline

O índice `documents.jsonl` contém registros do tipo:

```json
{
  "path": "book/ch09-02-recoverable-errors-with-result.html",
  "title": "Recoverable Errors with Result",
  "sha256": "...",
  "text": "..."
}
```

O ranking usa termos da consulta com pesos maiores para título/path e frequência limitada no texto. O resultado carrega:

- source;
- URI interna `phxclaw://knowledge/...`;
- path;
- title;
- SHA-256;
- score;
- excerpt limitado.

## Context Compiler

O Memory/Context Engine agora recebe a consulta e calcula relevância antes de materializar valores.

Ordem determinística:

```text
relevance_score DESC
confidence DESC
namespace ASC
key ASC
UUID ASC
```

Gates antes de inserir no pack:

- expiração;
- classificação máxima;
- namespace;
- scope;
- `max_items`;
- `max_bytes`.

O `ContextBundle` também armazena `query_sha256`.

## Event Bus

Tópicos utilizados:

```text
research.pipeline
agent.runtime
skill.runtime
knowledge.source
context.compiler
evidence.ledger
```

Cada evento possui:

- UUIDv7 do evento;
- correlation UUID;
- causation UUID;
- payload estruturado;
- timestamp.

A causação permite reconstruir a sequência da pesquisa.

## Evidence Ledger

Ao final da preparação, o pipeline registra uma evidência contendo:

```text
action_uuid = task.uuid
correlation_uuid = task.correlation_uuid
actor = agente roteado
capability = research.pipeline.prepare
action = resolve_agent_skill_source_context
query armazenada como SHA-256 no summary
source/skill/context IDs
artifact URIs das fontes
previous_hash + record_hash
```

Os hits offline também geram `EvidenceRef` com hash do documento indexado.

## Pesquisa Rust

Factory:

```rust
let task = ResearchTask::rust(
    "How do I propagate Result errors in Rust?",
    "phxclaw",
);
```

Defaults:

```text
agent: Research Agent
source: rust-official
skill: rust.research.official
freshness: stable
source hits: 8
context items: 24
context bytes: 96 KiB
```

## Fixture de integração

`tests/fixtures/rust-offline-sample/` possui conteúdo sintético mínimo para validar o fluxo sem rede/rustup. Não é apresentado como documentação Rust oficial.

O teste Rust preparado verifica, quando `cargo test` estiver disponível:

- 110 manifests carregáveis;
- Research Agent roteado;
- skill Rust resolvida;
- fonte offline resolvida;
- hit `Result` encontrado;
- memória relevante selecionada;
- Evidence Ledger válido;
- `research.pipeline/ready` publicado;
- correlation UUID igual em todos os eventos do teste.

No host atual, esse teste Rust não foi executado porque `cargo/rustc` não estão instalados. A mesma estrutura é verificada por testes estáticos Python e fixture determinístico.

## Limite atual e NEXT

O v0.8 fecha **preparação + pesquisa offline + contexto + evidência**.

Para online, falta o executor governado:

```text
PreparedSource::Online
       |
       v
HTTP/Browser Executor
       |
       v
Egress Broker exact-origin
       |
       v
content hash + EvidenceRef
       |
       v
Model Gateway
       |
       v
synthesis
       |
       v
Objective Proof / QA
```

Esse executor deve ser o próximo bloco, não um `reqwest` escondido dentro do Source Registry.

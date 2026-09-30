# F25 — Knowledge / Evidence Graph — PhxClaw v0.23

State: **review**. Release-ready remains false until native Rust/PostgreSQL/E2E gates pass.

## Purpose

F25 turns PhxClaw's evidence, hypotheses, decisions, requirements, tasks, agents, skills, releases and tests into a governed trace graph without treating memory or model output as truth.

## Epistemic boundary

```text
RAW SOURCE / OBSERVATION
        ↓
UNVERIFIED CLAIM / HYPOTHESIS
        ↓ evidence bound to exact source-state hash
ACCEPTED KNOWLEDGE
        ↓ explicit governance / approval
GOVERNED KNOWLEDGE
```

Important non-equivalences:

```text
memory != truth
model confidence != proof
repetition != proof
graph centrality != truth
accepted knowledge != policy
```

## Core invariants

1. Raw sources and evidence nodes are immutable.
2. Interpretations change through new nodes and `supersedes` edges, not destructive overwrite.
3. Every durable identity is UUIDv7.
4. Every claim/evidence artifact carries SHA-256 and source-state SHA-256.
5. Evidence for promotion must bind to the exact claim source state.
6. Active refuting evidence blocks promotion.
7. Unresolved contradictions block promotion.
8. Learning authority can create only raw/unverified non-policy nodes.
9. Learning cannot create Policy, Constraint or Decision nodes.
10. Learning cannot mark knowledge Accepted or Governed.
11. Governed knowledge requires human approval by default.
12. Cross-tenant edges/bindings are rejected.
13. Traversal is bounded to prevent unbounded graph expansion.
14. Graph snapshots have deterministic SHA-256 roots for the same graph state.

## Node classes

- raw_source
- artifact
- claim
- evidence
- hypothesis
- decision
- requirement
- task
- agent
- skill
- release
- test
- policy
- constraint
- external_fact

## Edge classes

- supports / refutes
- derived_from / produced_by
- validates / tests
- depends_on / implements
- supersedes / contradicts
- approved_by / promoted_from
- caused_by / relates_to

## Contradictions

Claims with the same subject + predicate and different value hashes are contradiction candidates when both are Accepted/Governed. A contradiction is stored as its own durable object. Resolution is explicit and can link to a resolution node; it is never silently decided from confidence alone.

## PostgreSQL

Migration `0023_knowledge_evidence_graph.sql` adds:

- `knowledge_nodes`
- `knowledge_edges`
- `knowledge_evidence_bindings`
- `knowledge_contradictions`
- `knowledge_contradiction_resolutions`
- `knowledge_snapshots`
- `knowledge_graph_events`

All tables are tenant-isolated by RLS. Graph facts, edges, evidence bindings, contradictions and contradiction resolutions are append-only; new interpretations use new nodes plus `supersedes` edges.

## Native gates still required

- cargo fmt --check
- cargo check --workspace
- cargo test --workspace
- cargo clippy --workspace --all-targets -- -D warnings
- PostgreSQL migration/E2E with separate tenants and roles
- concurrency/idempotency tests
- Event Bus + Evidence Ledger integration E2E
- F24 skill-promotion -> F25 lineage trace E2E
- Mission/Team runtime -> F25 provenance trace E2E

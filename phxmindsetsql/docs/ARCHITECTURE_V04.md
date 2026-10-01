# Arquitetura — PhxMindSetSQL v0.4

## Objetivo

Garantir isolamento rígido entre **linguagem SQL**, **modelo semântico**, **projeções de grafo** e **renderização**.

```text
┌──────────────────────────────────────────────────────────────┐
│ SQL source                                                   │
└──────────────────────────────┬───────────────────────────────┘
                               ▼
                     ┌─────────────────┐
                     │ DialectDetector │
                     └────────┬────────┘
                              ▼
          ┌────────────────────────────────────────┐
          │ Parser Adapter Registry                │
          │ PostgreSQL | MySQL | SQLite | SQLServer│
          └────────────────────┬───────────────────┘
                               ▼
                     ┌─────────────────┐
                     │ UnifiedSqlModel │
                     │ contract 1.0    │
                     └────────┬────────┘
                              ▼
                    ┌───────────────────┐
                    │ Graph Projections │
                    └─────────┬─────────┘
                              ▼
        ┌────────────┬────────────┬────────────┬────────────┐
        ▼            ▼            ▼            ▼
     MindSet        DER        Obsidian       Hybrid
       SVG        ELK/SVG     Sigma/WebGL      SVG
```

## Dependências permitidas

| Camada | Pode conhecer | Não pode conhecer |
|---|---|---|
| Detector | texto SQL, signatures | DOM, renderer |
| Parser adapter | SQL do dialeto, `UnifiedSqlModel` | DOM, Canvas, SVG, Sigma, ELK |
| Unified model | entidades SQL neutras | dialect grammar, UI |
| Projection | `UnifiedSqlModel` | parser, regex SQL, UI framework |
| Renderer | model/projection | SQL bruto, parser, dialect rules |
| App/Composition | services + renderer | gramática específica |

## Web

- `src/analysis-service.js`: porta usada pela aplicação;
- `src/sql-worker.js`: composition root de parsing no worker;
- `src/parsers/detector.js`: detector puro;
- `src/parsers/registry.js`: registro de adapters;
- `src/parsers/postgresql.js`: adapter PostgreSQL puro;
- `src/model/unified-model.js`: contrato/validação;
- `src/projections/graph-projection.js`: projeções neutras;
- `src/enhancer.js`: renderer avançado, sem parsing;
- `app.js`: UI/orquestração, sem gramática SQL.

## Rust

### `phx-sql-model`
Modelo intermediário neutro. Não depende de parser nem renderização.

### `phx-sql-parser-api`
Define `SqlDialect`, `DialectDetection`, `ParseError` e `SqlDialectParser`.

### `phx-parser-postgresql`
Implementa PostgreSQL e retorna `SqlModel`.

### `phx-sql-projection`
Converte `SqlModel` em `GraphProjection`, sem conhecer o dialeto de origem.

### `phx-sql-core`
Composition root/WASM facade. Faz dispatch do adapter e serializa o contrato Web. Não contém gramática PostgreSQL.

## Contrato estável

O renderer recebe estruturas como:

```json
{
  "contract_version": "1.0",
  "dialect": "postgresql",
  "tables": [],
  "relationships": [],
  "objects": [],
  "stats": {}
}
```

`dialect` é metadado; o renderer não toma decisões gráficas baseadas em sintaxe do dialeto.

## Extensão

Um novo parser precisa apenas satisfazer o contrato. O mesmo DER, MindSet, Obsidian e Hybrid deve renderizar o novo modelo sem mudanças.

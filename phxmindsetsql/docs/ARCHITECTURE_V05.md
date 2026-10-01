# Arquitetura — PhxMindSetSQL v0.5

## Princípio

**O parser não conhece UI. O renderer não conhece SQL.**

```text
Dialect Detector
      │
      ▼
Parser Registry
 ├─ PostgreSQL
 ├─ MySQL/MariaDB
 ├─ SQLite
 └─ SQL Server
      │
      ▼
UnifiedSqlModel 1.0
      │
      ▼
Graph Projection
 ├─ MindSet
 ├─ Relacional/DER
 ├─ Obsidian
 └─ Hybrid
```

A camada `Graph Projection` também não conhece dialeto. Ela recebe somente `SqlModel` e produz nós/arestas.

Novo dialeto deve ser implementado como adapter e registrado sem alterar os renderers.

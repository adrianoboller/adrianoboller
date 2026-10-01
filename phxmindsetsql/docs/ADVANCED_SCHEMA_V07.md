# Schema avançado — v0.7

## Recursos normalizados

### CHECK constraints

Representados por `CheckConstraint { name, expression, column, source_line }`. A expressão é preservada para Inspector e diff.

### Generated / computed columns

Cada `Column` pode carregar:

- `generated_expression` — expressão completa;
- `generated_kind` — por exemplo `stored`, `virtual` ou `computed/persisted` conforme normalização do adapter.

### Sequences

Objeto independente `sequence`. Suportado no parser PostgreSQL/SQL Server e nos respectivos catálogos vivos.

### Materialized views

Objeto `materialized_view`. PostgreSQL é o dialeto suportado nesta versão.

### Partitioning

A tabela recebe `partitioning { kind, expression, parent, bound }`. Partições também podem aparecer como objetos `partition` para navegação gráfica.

### Synonyms SQL Server

Objeto `synonym`, com target/base object quando disponível.

### Events MySQL

Objeto `event`, com metadados de schedule quando recuperáveis.

### Extensions PostgreSQL

Objeto `extension`, incluindo extensões instaladas detectadas em DDL ou catálogo.

### Estatísticas

`TableStatistics` suporta:

- `estimated_rows`;
- `data_bytes`;
- `index_bytes`;
- `total_bytes`.

Esses valores são apresentados como estimativas/estado operacional e **não fazem parte do fingerprint de schema**.

## Baseline PhxClaw

No FULL INSTALL v0.60 desta entrega:

- 669 CHECK constraints;
- 2 generated columns;
- `pgcrypto` como extension;
- não há sequence/materialized view/partition no baseline detectado.

Fixtures específicas cobrem os demais recursos em cada dialeto.

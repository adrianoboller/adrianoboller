# Live Database — v0.6

## PostgreSQL

Fontes de catálogo:

- `information_schema.columns`;
- `information_schema.table_constraints`;
- `information_schema.key_column_usage`;
- `information_schema.referential_constraints`;
- `pg_indexes`;
- `information_schema.routines`;
- `information_schema.triggers`;
- `pg_policies` quando habilitado.

Documentação oficial:

- https://www.postgresql.org/docs/current/infoschema-key-column-usage.html
- https://www.postgresql.org/docs/current/infoschema-referential-constraints.html

## MySQL / MariaDB

Fontes:

- `INFORMATION_SCHEMA.COLUMNS`;
- `TABLE_CONSTRAINTS`;
- `KEY_COLUMN_USAGE`;
- `REFERENTIAL_CONSTRAINTS`;
- `STATISTICS`;
- `VIEWS`, `ROUTINES`, `TRIGGERS`.

Documentação MySQL:

- https://dev.mysql.com/doc/refman/8.4/en/information-schema-key-column-usage-table.html
- https://dev.mysql.com/doc/refman/8.4/en/information-schema-table-constraints-table.html
- https://dev.mysql.com/doc/refman/8.4/en/information-schema-referential-constraints-table.html

## SQLite

Fontes:

- `sqlite_schema` para objetos e SQL original;
- `PRAGMA table_xinfo` para colunas, incluindo colunas geradas/ocultas;
- `PRAGMA foreign_key_list` para FKs;
- `PRAGMA index_list` + `PRAGMA index_info` para unicidade.

Documentação:

- https://sqlite.org/pragma.html

## SQL Server

Fontes:

- `sys.tables`;
- `sys.schemas`;
- `sys.columns`;
- `sys.types`;
- `sys.indexes` + `sys.index_columns`;
- `sys.foreign_keys` + `sys.foreign_key_columns`;
- `sys.views`;
- `sys.objects`;
- `sys.triggers`.

Documentação:

- https://learn.microsoft.com/sql/relational-databases/system-catalog-views/sys-foreign-key-columns-transact-sql

Driver Rust: Tiberius/TDS. TLS é exigido para `prefer/require` nesta primeira implementação do adapter SQL Server; `disable` precisa ser explicitamente solicitado.

## O que entra no modelo

v0.6 prioriza estrutura necessária para os gráficos:

- schemas;
- tabelas;
- colunas;
- PKs;
- FKs compostas e simples;
- unique de uma coluna;
- views;
- indexes;
- funções/rotinas;
- triggers;
- policies PostgreSQL opcionais.

## Próximas extensões naturais

- CHECK constraints;
- computed/generated columns com expressão completa;
- sequences;
- materialized views;
- partitioning;
- synonyms SQL Server;
- events MySQL;
- extensões PostgreSQL;
- cardinalidade estimada e tamanho de tabela;
- diff de schema vivo vs arquivo SQL.

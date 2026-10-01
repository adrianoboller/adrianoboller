# Changelog

## 0.7.0

- sobe o contrato neutro para `UnifiedSqlModel 1.1`;
- adiciona CHECK constraints e expressão completa de generated/computed columns;
- adiciona sequences, materialized views, partitioning, synonyms SQL Server, events MySQL e extensions PostgreSQL;
- adiciona cardinalidade estimada e tamanhos no catálogo vivo quando suportado;
- adiciona `SchemaDiff` puro entre dois modelos normalizados;
- diff compara tabelas, colunas, constraints, partitioning, objetos e FKs, ignorando estatísticas operacionais;
- amplia os quatro introspectores com metadados avançados;
- amplia parsers Web e fontes Rust para manter paridade de recursos;
- adiciona painel `Schema Diff` e Inspector com CHECK/generated/partitioning/estatísticas;
- mantém parsing, introspecção, diff e renderização em camadas separadas;
- baseline: 294 tabelas / 2.837 campos / 254 FKs / 669 CHECKs / 2 generated / 1 extensão.

## 0.6.0

- adiciona **Conectar DB** ao PWA;
- adiciona Gateway Rust local-first (`phx-sql-gateway`);
- adiciona contrato `DatabaseIntrospector` independente de parser e UI;
- adiciona introspectores PostgreSQL, MySQL/MariaDB, SQLite e SQL Server;
- adiciona `CatalogSnapshot -> UnifiedSqlModel`;
- extrai serialização JSON para `phx-sql-contract`;
- adiciona profiles seguros com senha via variável de ambiente;
- conexão efêmera desabilitada por padrão;
- Service Worker passa a excluir `/api/*` do cache;
- adiciona testes de contrato de live database e isolamento de arquitetura;
- preserva sem alterações o núcleo dos quatro renderers da v0.5;
- mantém baseline PhxClaw em 294 tabelas / 2.837 campos / 254 FKs.

## 0.5.0

- parsers independentes para PostgreSQL, MySQL/MariaDB, SQLite e SQL Server;
- paridade de projeção entre dialetos;
- isolamento parser/renderer protegido por testes.

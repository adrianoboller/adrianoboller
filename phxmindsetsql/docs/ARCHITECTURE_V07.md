# Arquitetura v0.7 — modelo 1.1 e schema avançado

## Princípio obrigatório

Nenhuma regra de dialeto pode entrar em MindSet, DER, Obsidian ou Hybrid. Nenhum parser ou introspector pode depender de DOM, SVG, Canvas, WebGL, Sigma ou ELK.

## Fluxos de entrada

```text
SQL text -> DialectDetector -> ParserAdapter -----------+
                                                       |
Live DB  -> Gateway -> IntrospectorAdapter -------------+-> UnifiedSqlModel 1.1
                                                                  |
                                +---------------------------------+------------------+
                                |                                 |                  |
                         GraphProjection                     SchemaDiff         Inspector data
                                |                                 |
                 +--------------+-------------+                   |
                 |              |             |                   |
              MindSet          DER         Obsidian/Hybrid     Diff UI
```

## UnifiedSqlModel 1.1

O contrato 1.1 adiciona campos opcionais, preservando consumidores do 1.0:

- `Table.check_constraints[]`;
- `Table.partitioning`;
- `Table.statistics`;
- `Column.generated_expression`;
- `Column.generated_kind`;
- novos `ObjectKind`: `materialized_view`, `sequence`, `synonym`, `event`, `extension`, `partition`;
- `SqlObject.metadata` neutro para metadados específicos que ainda não justificam um tipo forte.

## SchemaDiff

`src/schema-diff.js` depende somente do `UnifiedSqlModel`. Ele não recebe SQL bruto, conexão ou dialeto. Assim, é possível comparar PostgreSQL escrito em arquivo com, por exemplo, um banco PostgreSQL vivo sem ensinar o diff a interpretar PostgreSQL.

## Introspecção

Cada SGBD possui um adapter independente e devolve `CatalogSnapshot`, convertido para `SqlModel` pela API neutra de introspecção.

- PostgreSQL: objetos avançados, partitions, estimates e sizes;
- MySQL/MariaDB: generated, CHECK, partitions, events, table estimates/sizes;
- SQLite: `table_xinfo`/schema SQL para generated e CHECK; estatística de tamanho por tabela não é exposta como capacidade portátil nesta versão;
- SQL Server: computed/check, partitions/row-size estimates, synonyms e sequences.

## Invariantes verificadas

- parser -> model, nunca parser -> renderer;
- introspector -> catalog/model, nunca introspector -> renderer;
- schema diff -> model, nunca schema diff -> parser;
- renderer -> model/projection;
- UI recebe um único contrato normalizado independente da origem.

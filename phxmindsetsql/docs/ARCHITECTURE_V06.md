# Arquitetura v0.6 — duas origens, um modelo, quatro renderers

## Princípio

Há duas maneiras de obter estrutura SQL:

1. texto DDL;
2. catálogo de um banco em execução.

Nenhuma delas pode contaminar a camada gráfica.

```text
                    +-------------------+
SQL text ---------->| ParserAdapter     |
                    +---------+---------+
                              |
                              v
                      +---------------+
                      | UnifiedSqlModel|
                      +-------+-------+
                              ^
                              |
                    +---------+-----------+
Live database ----->| IntrospectorAdapter |
                    +---------------------+
                              |
                              v
                      GraphProjection
                              |
          +-------------------+--------------------+
          |                   |                    |
       MindSet               DER              Obsidian/Hybrid
```

## Dependências permitidas

```text
phx-parser-* --------> phx-sql-parser-api -----> phx-sql-model
phx-introspector-* --> phx-sql-introspection-api -> phx-sql-model
phx-sql-contract ----> phx-sql-model
phx-sql-projection --> phx-sql-model
phx-sql-gateway -----> phx-introspector-* + phx-sql-contract
Web UI -------------> UnifiedSqlModel JSON + GraphProjection JS
```

## Dependências proibidas

- parser -> renderer/UI;
- introspector -> renderer/UI;
- renderer -> parser;
- renderer -> driver de banco;
- projection -> parser/introspector;
- `app.js` -> `information_schema`, `sys.*`, PRAGMA ou driver;
- credenciais -> `UnifiedSqlModel`.

## Contrato

`phx-sql-contract` é uma nova fronteira. Ele recebe `SqlModel` neutro e produz o JSON `UnifiedSqlModel 1.0`. O produtor é apenas metadado:

```json
{
  "contract_version": "1.0",
  "dialect": "postgresql",
  "producer": "postgresql-catalog-introspector",
  "source_kind": "live_database"
}
```

Os renderers não consultam `producer` para decidir lógica gráfica.

## Caminho de arquivo

```text
.sql -> Web Worker -> Rust/WASM ou parser JS -> UnifiedSqlModel -> projeção -> renderer
```

## Caminho live

```text
PWA -> /api/v1/introspect -> Gateway Rust -> IntrospectorAdapter
    -> CatalogSnapshot -> SqlModel -> phx-sql-contract -> UnifiedSqlModel
    -> projeção -> renderer
```

## Gateway

O Gateway serve o PWA e a API na mesma origem por padrão. Isso evita CORS desnecessário e permite um único endereço local.

Default:

```text
127.0.0.1:8787
```

Para exposição remota devem ser adicionados reverse proxy HTTPS, autenticação, ACL e política de secrets.

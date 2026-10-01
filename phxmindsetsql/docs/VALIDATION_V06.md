# Validação v0.6

## Executado neste ambiente

`npm test`:

- `architecture-check`: OK;
- baseline: **294 tabelas / 2.837 colunas / 254 FKs**;
- detector: PostgreSQL / MySQL / SQLite / SQL Server: OK;
- parser adapters: 4: OK;
- projection parity: OK;
- renderer core imutável contra v0.5: OK;
- live gateway contract: OK;
- introspection architecture isolation: OK.

## Arquitetura validada

O teste impede:

- introspectores importarem parsers concretos;
- introspectores importarem projection/UI;
- API de introspecção depender de driver;
- live Web service importar parser SQL;
- `app.js` conter SQL de catálogo/driver;
- projection depender de introspection.

## Não executado neste ambiente

Não há `cargo`, `rustc`, `wasm-pack` nem servidores de banco acessíveis. Portanto não foi possível executar:

```text
cargo check --workspace
cargo test --workspace
cargo run -p phx-sql-gateway
```

nem testes de integração contra PostgreSQL/MySQL/SQLite/SQL Server reais.

Os fontes Rust foram submetidos a validação estática de estrutura/TOML e os contratos Web foram testados com um Gateway simulado.

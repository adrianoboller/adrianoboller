# API de Parser/Dialeto — v0.5

JavaScript adapters implementam `{ dialect, id, version, parse(sql, source) }` e retornam `UnifiedSqlModel`.

Rust adapters implementam:

```rust
pub trait SqlDialectParser: Send + Sync {
    fn dialect(&self) -> SqlDialect;
    fn parse(&self, source_name: &str, sql: &str) -> Result<SqlModel, ParseError>;
}
```

Para adicionar Oracle/Firebird/DB2/HFSQL/etc.: crie adapter, registre, adicione fixture de paridade e execute testes. Não altere os renderers.

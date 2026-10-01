use phx_sql_core::{
    parse_mysql_model, parse_postgresql_model, parse_sqlite_model, parse_sqlserver_model,
    ObjectKind,
};

#[test]
fn postgresql_advanced_schema_features() {
    let sql = r#"
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE SEQUENCE public.invoice_seq START 100;
CREATE TABLE public.orders (
  id bigint PRIMARY KEY,
  qty integer CHECK (qty > 0),
  total numeric GENERATED ALWAYS AS (qty * 10) STORED
) PARTITION BY RANGE (id);
CREATE TABLE public.orders_2026 PARTITION OF public.orders FOR VALUES FROM (1) TO (1000);
CREATE MATERIALIZED VIEW public.order_totals AS SELECT id, total FROM public.orders;
"#;
    let m = parse_postgresql_model("pg.sql", sql).expect("postgresql advanced parse");
    assert_eq!(m.stats.check_constraints, 1);
    assert_eq!(m.stats.generated_columns, 1);
    assert_eq!(m.stats.sequences, 1);
    assert_eq!(m.stats.materialized_views, 1);
    assert_eq!(m.stats.extensions, 1);
    assert_eq!(m.stats.partitions, 1);
    let c = m.tables[0].columns.iter().find(|c| c.name == "total").unwrap();
    assert!(c.generated_expression.as_deref().unwrap_or("").contains("qty * 10"));
}

#[test]
fn mysql_advanced_schema_features() {
    let sql = r#"
CREATE TABLE sales (
 id bigint PRIMARY KEY,
 qty int CHECK (qty > 0),
 gross decimal(10,2) GENERATED ALWAYS AS (qty * 9.99) STORED
) PARTITION BY HASH(id) PARTITIONS 4;
CREATE EVENT ev_cleanup ON SCHEDULE EVERY 1 DAY DO DELETE FROM sales WHERE id < 0;
"#;
    let m = parse_mysql_model("mysql.sql", sql).expect("mysql advanced parse");
    assert_eq!(m.stats.check_constraints, 1);
    assert_eq!(m.stats.generated_columns, 1);
    assert_eq!(m.stats.events, 1);
    assert!(m.tables[0].partitioning.is_some());
}

#[test]
fn sqlite_advanced_schema_features() {
    let sql = r#"
CREATE TABLE calc (
 a INTEGER,
 b INTEGER CHECK (b >= 0),
 c INTEGER GENERATED ALWAYS AS (a + b) STORED
);
"#;
    let m = parse_sqlite_model("sqlite.sql", sql).expect("sqlite advanced parse");
    assert_eq!(m.stats.check_constraints, 1);
    assert_eq!(m.stats.generated_columns, 1);
}

#[test]
fn sqlserver_advanced_schema_features() {
    let sql = r#"
CREATE SEQUENCE dbo.InvoiceSeq START WITH 1 INCREMENT BY 1;
CREATE TABLE dbo.calc (
 id bigint PRIMARY KEY,
 qty int CONSTRAINT CK_calc_qty CHECK (qty > 0),
 total AS (qty * 10) PERSISTED
);
CREATE SYNONYM dbo.calc_alias FOR dbo.calc;
GO
"#;
    let m = parse_sqlserver_model("sqlserver.sql", sql).expect("sqlserver advanced parse");
    assert_eq!(m.stats.check_constraints, 1);
    assert_eq!(m.stats.generated_columns, 1);
    assert_eq!(m.stats.sequences, 1);
    assert_eq!(m.stats.synonyms, 1);
    assert!(m.objects.iter().any(|o| o.kind == ObjectKind::Synonym));
}

use phx_sql_core::{parse_auto_model, parse_to_ui_json, SqlDialect};

#[test]
fn parses_phxclaw_v060_baseline() {
    let sql = include_str!("../../../assets/PhxClaw_PostgreSQL_FULL_INSTALL_v0.60.sql");
    let (detection, model) = parse_auto_model("baseline.sql", sql).expect("parse baseline");
    assert_eq!(detection.dialect, SqlDialect::PostgreSql);
    assert_eq!(model.stats.tables, 294, "contagem de tabelas mudou");
    assert_eq!(model.stats.migrations, 55, "contagem de migrations mudou");
    assert_eq!(model.stats.columns, 2837, "contagem de colunas mudou");
    assert_eq!(model.stats.relationships, 254, "contagem de FKs mudou");
    assert_eq!(model.stats.views, 3, "contagem de views mudou");
    assert_eq!(model.stats.functions, 46, "contagem de funções mudou");
    assert_eq!(model.stats.triggers, 58, "contagem de triggers mudou");
    assert_eq!(model.stats.indexes, 76, "contagem de índices mudou");
    assert_eq!(model.stats.policies, 68, "contagem de policies mudou");
    assert_eq!(model.stats.check_constraints, 669, "contagem de CHECKs mudou");
    assert_eq!(model.stats.generated_columns, 2, "contagem de generated columns mudou");
    assert_eq!(model.stats.extensions, 1, "contagem de extensões mudou");
}

#[test]
fn contract_is_dialect_neutral() {
    let sql = include_str!("../../../assets/PhxClaw_PostgreSQL_FULL_INSTALL_v0.60.sql");
    let json = parse_to_ui_json("baseline.sql", sql).expect("contract json");
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(value["contract_version"], "1.1");
    assert_eq!(value["dialect"], "postgresql");
    assert_eq!(value["stats"]["tables"], 294);
    assert_eq!(value["stats"]["columns"], 2837);
    assert_eq!(value["stats"]["foreign_keys"], 254);
    assert_eq!(value["stats"]["check_constraints"], 669);
    assert_eq!(value["stats"]["generated_columns"], 2);
    assert_eq!(value["stats"]["extensions"], 1);
    assert!(value["tables"][0]["ddl"].is_string());
}

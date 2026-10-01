use phx_sql_model::{canonical_name, ForeignKey, ObjectKind, SqlModel};
use phx_sql_parser_api::SqlDialect;
use serde_json::{json, Value};
use std::collections::HashMap;

pub const CONTRACT_VERSION: &str = "1.1";

fn kind_name(kind: ObjectKind) -> &'static str {
    match kind {
        ObjectKind::Table => "table",
        ObjectKind::View => "view",
        ObjectKind::MaterializedView => "materialized_view",
        ObjectKind::Sequence => "sequence",
        ObjectKind::Function => "function",
        ObjectKind::Trigger => "trigger",
        ObjectKind::Index => "index",
        ObjectKind::Policy => "policy",
        ObjectKind::Schema => "schema",
        ObjectKind::Synonym => "synonym",
        ObjectKind::Event => "event",
        ObjectKind::Extension => "extension",
        ObjectKind::Partition => "partition",
    }
}

pub fn model_to_contract_value(model: &SqlModel, dialect: SqlDialect, producer: &str, source_kind: &str) -> Value {
    let mut degree: HashMap<String, usize> = HashMap::new();
    for fk in &model.relationships {
        *degree.entry(canonical_name(&fk.from_table)).or_default() += 1;
        *degree.entry(canonical_name(&fk.to_table)).or_default() += 1;
    }
    let fk_json = |fk: &ForeignKey| json!({
        "constraint": fk.constraint_name,
        "from_table": fk.from_table,
        "from_columns": fk.from_columns,
        "to_table": fk.to_table,
        "to_columns": fk.to_columns,
        "on_delete": fk.on_delete,
        "line": fk.source_line
    });
    let tables: Vec<Value> = model.tables.iter().map(|t| {
        let columns: Vec<Value> = t.columns.iter().map(|c| json!({
            "name": c.name,
            "data_type": c.data_type,
            "nullable": c.nullable,
            "default": c.default_expr,
            "primary_key": c.primary_key,
            "unique": c.unique,
            "generated_expression": c.generated_expression,
            "generated_kind": c.generated_kind
        })).collect();
        let checks: Vec<Value> = t.check_constraints.iter().map(|c| json!({
            "name": c.name,
            "expression": c.expression,
            "column": c.column,
            "line": c.source_line
        })).collect();
        json!({
            "name": t.name,
            "line": t.line,
            "end_line": t.end_line,
            "migration": t.migration,
            "columns": columns,
            "primary_key": t.primary_key,
            "foreign_keys": t.foreign_keys.iter().map(&fk_json).collect::<Vec<_>>(),
            "check_constraints": checks,
            "partitioning": t.partitioning,
            "statistics": t.statistics,
            "ddl": t.raw_sql,
            "degree": degree.get(&canonical_name(&t.name)).copied().unwrap_or(0)
        })
    }).collect();
    let objects: Vec<Value> = model.objects.iter().map(|o| json!({
        "kind": kind_name(o.kind),
        "name": o.name,
        "line": o.line,
        "migration": o.migration,
        "target": o.target,
        "ddl": o.raw_sql,
        "metadata": o.metadata
    })).collect();

    let functions = dialect != SqlDialect::Sqlite;
    let schemas = dialect != SqlDialect::Sqlite;
    json!({
        "contract_version": CONTRACT_VERSION,
        "dialect": dialect.as_str(),
        "parser": producer,
        "producer": producer,
        "source_kind": source_kind,
        "source": model.source_name,
        "capabilities": {
            "tables": true,
            "columns": true,
            "foreign_keys": true,
            "check_constraints": true,
            "generated_columns": true,
            "views": true,
            "materialized_views": dialect == SqlDialect::PostgreSql,
            "sequences": matches!(dialect, SqlDialect::PostgreSql | SqlDialect::SqlServer),
            "functions": functions,
            "triggers": true,
            "indexes": true,
            "policies": dialect == SqlDialect::PostgreSql,
            "schemas": schemas,
            "partitioning": matches!(dialect, SqlDialect::PostgreSql | SqlDialect::MySql | SqlDialect::SqlServer),
            "synonyms": dialect == SqlDialect::SqlServer,
            "events": dialect == SqlDialect::MySql,
            "extensions": dialect == SqlDialect::PostgreSql,
            "table_statistics": source_kind == "live_database" && dialect != SqlDialect::Sqlite,
            "schema_diff": true
        },
        "stats": {
            "lines": model.stats.lines,
            "bytes": model.stats.bytes,
            "tables": model.stats.tables,
            "columns": model.stats.columns,
            "foreign_keys": model.stats.relationships,
            "check_constraints": model.stats.check_constraints,
            "generated_columns": model.stats.generated_columns,
            "views": model.stats.views,
            "materialized_views": model.stats.materialized_views,
            "sequences": model.stats.sequences,
            "functions": model.stats.functions,
            "triggers": model.stats.triggers,
            "indexes": model.stats.indexes,
            "policies": model.stats.policies,
            "schemas": model.stats.schemas,
            "synonyms": model.stats.synonyms,
            "events": model.stats.events,
            "extensions": model.stats.extensions,
            "partitions": model.stats.partitions,
            "migrations": model.stats.migrations
        },
        "migrations": model.migrations,
        "tables": tables,
        "relationships": model.relationships.iter().map(fk_json).collect::<Vec<_>>(),
        "objects": objects
    })
}

pub fn model_to_contract_json(model: &SqlModel, dialect: SqlDialect, producer: &str, source_kind: &str) -> Result<String, serde_json::Error> {
    serde_json::to_string(&model_to_contract_value(model, dialect, producer, source_kind))
}

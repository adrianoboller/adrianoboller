use async_trait::async_trait;
use phx_sql_model::{
    CheckConstraint, Column, ForeignKey, Migration, ObjectKind, Partitioning, SqlModel, SqlObject,
    SqlStats, Table, TableStatistics,
};
use phx_sql_parser_api::SqlDialect;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionMode { Profile, Ephemeral }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TlsMode { Disable, Prefer, Require }
impl Default for TlsMode { fn default() -> Self { Self::Prefer } }

#[derive(Clone, Serialize, Deserialize)]
pub struct ConnectionSpec {
    pub dialect: SqlDialect,
    #[serde(default = "default_host")]
    pub host: String,
    pub port: Option<u16>,
    pub database: String,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default)]
    pub tls: TlsMode,
    /// SQLite file path or URL. Ignored by server databases.
    pub path: Option<String>,
}
fn default_host() -> String { "127.0.0.1".to_owned() }

impl fmt::Debug for ConnectionSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectionSpec")
            .field("dialect", &self.dialect)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "***REDACTED***"))
            .field("tls", &self.tls)
            .field("path", &self.path)
            .finish()
    }
}

impl ConnectionSpec {
    pub fn safe_source_name(&self) -> String {
        match self.dialect {
            SqlDialect::Sqlite => format!("live:sqlite:{}", self.path.as_deref().unwrap_or(&self.database)),
            d => format!("live:{}://{}:{}/{}", d.as_str(), self.host, self.port.unwrap_or(default_port(d)), self.database),
        }
    }
}

pub fn default_port(dialect: SqlDialect) -> u16 {
    match dialect { SqlDialect::PostgreSql => 5432, SqlDialect::MySql => 3306, SqlDialect::SqlServer => 1433, _ => 0 }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntrospectionOptions {
    #[serde(default)] pub schemas: Vec<String>,
    #[serde(default)] pub include_system: bool,
    #[serde(default = "yes")] pub include_views: bool,
    #[serde(default = "yes")] pub include_indexes: bool,
    #[serde(default = "yes")] pub include_triggers: bool,
    #[serde(default = "yes")] pub include_routines: bool,
    #[serde(default)] pub include_policies: bool,
    #[serde(default = "yes")] pub include_advanced_objects: bool,
    #[serde(default = "yes")] pub include_statistics: bool,
    #[serde(default = "default_max_objects")] pub max_objects: usize,
}
fn yes() -> bool { true }
fn default_max_objects() -> usize { 20_000 }
impl Default for IntrospectionOptions {
    fn default() -> Self { Self { schemas: vec![], include_system: false, include_views: true, include_indexes: true, include_triggers: true, include_routines: true, include_policies: false, include_advanced_objects: true, include_statistics: true, max_objects: default_max_objects() } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogColumn {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default_expr: Option<String>,
    pub primary_key: bool,
    pub unique: bool,
    pub ordinal: usize,
    #[serde(default)] pub generated_expression: Option<String>,
    #[serde(default)] pub generated_kind: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CatalogCheckConstraint {
    pub name: Option<String>,
    pub expression: String,
    pub column: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogForeignKey {
    pub constraint_name: Option<String>,
    pub from_schema: String,
    pub from_table: String,
    pub from_columns: Vec<String>,
    pub to_schema: String,
    pub to_table: String,
    pub to_columns: Vec<String>,
    pub on_delete: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogTable {
    pub schema: String,
    pub name: String,
    #[serde(default)] pub columns: Vec<CatalogColumn>,
    #[serde(default)] pub primary_key: Vec<String>,
    #[serde(default)] pub foreign_keys: Vec<CatalogForeignKey>,
    #[serde(default)] pub check_constraints: Vec<CatalogCheckConstraint>,
    #[serde(default)] pub partitioning: Option<Partitioning>,
    #[serde(default)] pub statistics: Option<TableStatistics>,
    pub definition: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogObject {
    pub kind: ObjectKind,
    pub schema: Option<String>,
    pub name: String,
    pub target: Option<String>,
    pub definition: Option<String>,
    #[serde(default)] pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogSnapshot {
    pub dialect: SqlDialect,
    pub source_name: String,
    pub database_name: String,
    #[serde(default)] pub tables: Vec<CatalogTable>,
    #[serde(default)] pub objects: Vec<CatalogObject>,
}

#[derive(Debug, Error)]
pub enum IntrospectionError {
    #[error("dialeto não suportado pelo introspector: {0}")]
    UnsupportedDialect(String),
    #[error("falha de conexão: {0}")]
    Connection(String),
    #[error("falha de catálogo: {0}")]
    Catalog(String),
    #[error("limite de objetos excedido: {0}")]
    ObjectLimit(usize),
    #[error("entrada inválida: {0}")]
    InvalidInput(String),
}

#[async_trait]
pub trait DatabaseIntrospector: Send + Sync {
    fn dialect(&self) -> SqlDialect;
    async fn introspect(&self, spec: &ConnectionSpec, options: &IntrospectionOptions) -> Result<CatalogSnapshot, IntrospectionError>;
}

pub fn snapshot_to_model(snapshot: CatalogSnapshot) -> Result<SqlModel, IntrospectionError> {
    let mut relationships = Vec::new();
    let mut tables = Vec::with_capacity(snapshot.tables.len());
    let mut objects = Vec::new();
    let mut schemas = BTreeSet::new();
    let mut object_id = 0usize;

    for ct in snapshot.tables {
        schemas.insert(ct.schema.clone());
        let full_name = qualified(&ct.schema, &ct.name);
        let mut raw_sql = ct.definition.clone().unwrap_or_else(|| synthesize_table_ddl(&ct.schema, &ct.name, &ct.columns, &ct.primary_key, &ct.check_constraints));
        if raw_sql.trim().is_empty() { raw_sql = format!("-- definição não retornada pelo catálogo para {full_name}"); }
        let fks: Vec<ForeignKey> = ct.foreign_keys.into_iter().map(|fk| ForeignKey {
            constraint_name: fk.constraint_name,
            from_table: qualified(&fk.from_schema, &fk.from_table),
            from_columns: fk.from_columns,
            to_table: qualified(&fk.to_schema, &fk.to_table),
            to_columns: fk.to_columns,
            on_delete: fk.on_delete,
            source_line: 0,
        }).collect();
        relationships.extend(fks.iter().cloned());
        let columns = ct.columns.into_iter().map(|c| Column {
            name: c.name,
            data_type: c.data_type,
            nullable: c.nullable,
            default_expr: c.default_expr,
            primary_key: c.primary_key,
            unique: c.unique,
            generated_expression: c.generated_expression,
            generated_kind: c.generated_kind,
        }).collect::<Vec<_>>();
        let checks = ct.check_constraints.into_iter().map(|c| CheckConstraint { name:c.name, expression:c.expression, column:c.column, source_line:0 }).collect();
        tables.push(Table { name: full_name.clone(), line: 0, end_line: 0, migration: None, columns, primary_key: ct.primary_key, foreign_keys: fks, check_constraints:checks, partitioning:ct.partitioning, statistics:ct.statistics, raw_sql: raw_sql.clone() });
        objects.push(SqlObject { id: object_id, kind: ObjectKind::Table, name: full_name, line: 0, end_line: 0, migration: None, target: None, raw_sql, metadata:BTreeMap::new() });
        object_id += 1;
    }

    for obj in snapshot.objects {
        if obj.kind == ObjectKind::Schema { schemas.insert(obj.name.clone()); }
        let name = match obj.schema.as_deref() { Some(s) if !s.is_empty() => qualified(s, &obj.name), _ => obj.name };
        objects.push(SqlObject { id: object_id, kind: obj.kind, name, line: 0, end_line: 0, migration: None, target: obj.target, raw_sql: obj.definition.unwrap_or_default(), metadata:obj.metadata });
        object_id += 1;
    }

    for schema in schemas.iter() {
        if !objects.iter().any(|o| o.kind == ObjectKind::Schema && o.name.eq_ignore_ascii_case(schema)) {
            objects.push(SqlObject { id: object_id, kind: ObjectKind::Schema, name: schema.clone(), line: 0, end_line: 0, migration: None, target: None, raw_sql: String::new(), metadata:BTreeMap::new() });
            object_id += 1;
        }
    }

    let mut stats = SqlStats::default();
    stats.tables = tables.len();
    stats.columns = tables.iter().map(|t| t.columns.len()).sum();
    stats.relationships = relationships.len();
    stats.check_constraints = tables.iter().map(|t| t.check_constraints.len()).sum();
    stats.generated_columns = tables.iter().flat_map(|t| t.columns.iter()).filter(|c| c.generated_expression.is_some()).count();
    stats.views = objects.iter().filter(|o| o.kind == ObjectKind::View).count();
    stats.materialized_views = objects.iter().filter(|o| o.kind == ObjectKind::MaterializedView).count();
    stats.sequences = objects.iter().filter(|o| o.kind == ObjectKind::Sequence).count();
    stats.functions = objects.iter().filter(|o| o.kind == ObjectKind::Function).count();
    stats.triggers = objects.iter().filter(|o| o.kind == ObjectKind::Trigger).count();
    stats.indexes = objects.iter().filter(|o| o.kind == ObjectKind::Index).count();
    stats.policies = objects.iter().filter(|o| o.kind == ObjectKind::Policy).count();
    stats.schemas = schemas.len();
    stats.synonyms = objects.iter().filter(|o| o.kind == ObjectKind::Synonym).count();
    stats.events = objects.iter().filter(|o| o.kind == ObjectKind::Event).count();
    stats.extensions = objects.iter().filter(|o| o.kind == ObjectKind::Extension).count();
    stats.partitions = tables.iter().filter(|t| t.partitioning.is_some()).count() + objects.iter().filter(|o| o.kind == ObjectKind::Partition).count();

    Ok(SqlModel { source_name: snapshot.source_name, sql: String::new(), migrations: Vec::<Migration>::new(), tables, relationships, objects, stats })
}

fn qualified(schema: &str, name: &str) -> String {
    if schema.trim().is_empty() { name.to_owned() } else { format!("{}.{}", schema.trim(), name.trim()) }
}

fn synthesize_table_ddl(schema: &str, table: &str, cols: &[CatalogColumn], pk: &[String], checks:&[CatalogCheckConstraint]) -> String {
    let mut lines = Vec::new();
    for c in cols {
        let mut line = format!("  {} {}", c.name, if c.data_type.trim().is_empty() { "TEXT" } else { &c.data_type });
        if let Some(expr)=&c.generated_expression { line.push_str(" GENERATED ALWAYS AS ("); line.push_str(expr); line.push(')'); if let Some(k)=&c.generated_kind { line.push(' '); line.push_str(&k.to_ascii_uppercase()); } }
        if !c.nullable { line.push_str(" NOT NULL"); }
        if let Some(d) = &c.default_expr { line.push_str(" DEFAULT "); line.push_str(d); }
        lines.push(line);
    }
    if !pk.is_empty() { lines.push(format!("  PRIMARY KEY ({})", pk.join(", "))); }
    for c in checks { let prefix=c.name.as_ref().map(|n|format!("CONSTRAINT {n} ")).unwrap_or_default(); lines.push(format!("  {prefix}CHECK ({})",c.expression)); }
    format!("CREATE TABLE {}.{} (\n{}\n);\n-- reconstruído a partir de metadados de catálogo", schema, table, lines.join(",\n"))
}

/// Helper used by catalog adapters to mark PK/unique flags after constraint scans.
pub fn apply_key_flags(tables: &mut [CatalogTable], primary_keys: &HashMap<(String,String), Vec<String>>, unique_columns: &HashMap<(String,String), BTreeSet<String>>) {
    for table in tables {
        let key = (table.schema.to_ascii_lowercase(), table.name.to_ascii_lowercase());
        if let Some(pk) = primary_keys.get(&key) { table.primary_key = pk.clone(); }
        let pk_set: BTreeSet<String> = table.primary_key.iter().map(|x| x.to_ascii_lowercase()).collect();
        let uq = unique_columns.get(&key);
        for c in &mut table.columns {
            c.primary_key = pk_set.contains(&c.name.to_ascii_lowercase());
            c.unique = uq.map(|s| s.contains(&c.name.to_ascii_lowercase())).unwrap_or(false);
        }
    }
}

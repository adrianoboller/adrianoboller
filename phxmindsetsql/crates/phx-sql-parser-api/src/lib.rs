use phx_sql_model::SqlModel;
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SqlDialect {
    PostgreSql,
    MySql,
    Sqlite,
    SqlServer,
    Unknown,
}

impl SqlDialect {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PostgreSql => "postgresql",
            Self::MySql => "mysql",
            Self::Sqlite => "sqlite",
            Self::SqlServer => "sqlserver",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialectDetection {
    pub dialect: SqlDialect,
    pub confidence: f32,
}

#[derive(Clone, Debug)]
pub struct ParseError {
    pub dialect: SqlDialect,
    pub message: String,
}

impl ParseError {
    pub fn new(dialect: SqlDialect, message: impl Into<String>) -> Self {
        Self { dialect, message: message.into() }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} parser: {}", self.dialect.as_str(), self.message)
    }
}
impl Error for ParseError {}

/// Dialect parser contract. Parsers depend only on the neutral model contract.
/// No UI/rendering type is allowed in this API.
pub trait SqlDialectParser: Send + Sync {
    fn dialect(&self) -> SqlDialect;
    fn parse(&self, source_name: &str, sql: &str) -> Result<SqlModel, ParseError>;
}

pub fn detect_dialect(sql: &str) -> DialectDetection {
    let upper = sql.to_ascii_uppercase();
    let mut pg = 0i32;
    let mut mysql = 0i32;
    let mut sqlite = 0i32;
    let mut mssql = 0i32;

    for (needle, weight) in [
        ("LANGUAGE PLPGSQL", 7), ("CREATE POLICY", 6), ("JSONB", 4),
        ("CREATE EXTENSION", 5), ("SET SEARCH_PATH", 5), (" ILIKE ", 3),
    ] { if upper.contains(needle) { pg += weight; } }
    if sql.contains("::") { pg += 3; }
    if sql.contains("$$") { pg += 2; }

    for (needle, weight) in [
        ("ENGINE=INNODB", 7), ("ENGINE = INNODB", 7), ("AUTO_INCREMENT", 6),
        (" UNSIGNED", 4), ("DELIMITER ", 5), ("TINYINT(1)", 3),
    ] { if upper.contains(needle) { mysql += weight; } }
    if sql.contains('`') { mysql += 2; }

    for (needle, weight) in [
        ("PRAGMA ", 7), ("AUTOINCREMENT", 5), ("WITHOUT ROWID", 7),
        ("SQLITE_MASTER", 6), ("SQLITE_SCHEMA", 6),
    ] { if upper.contains(needle) { sqlite += weight; } }

    for (needle, weight) in [
        ("IDENTITY(", 6), ("NVARCHAR", 3), ("CLUSTERED", 4),
        ("CREATE OR ALTER", 5), ("UNIQUEIDENTIFIER", 5), ("GETDATE(", 3),
    ] { if upper.contains(needle) { mssql += weight; } }

    let ranked = [
        (SqlDialect::PostgreSql, pg), (SqlDialect::MySql, mysql),
        (SqlDialect::Sqlite, sqlite), (SqlDialect::SqlServer, mssql),
    ];
    let (dialect, best) = ranked.into_iter().max_by_key(|(_, score)| *score).unwrap();
    if best == 0 {
        if upper.contains("CREATE TABLE") || upper.contains("ALTER TABLE") {
            return DialectDetection { dialect: SqlDialect::PostgreSql, confidence: 0.35 };
        }
        return DialectDetection { dialect: SqlDialect::Unknown, confidence: 0.0 };
    }
    let total = (pg + mysql + sqlite + mssql).max(1) as f32;
    DialectDetection { dialect, confidence: ((best as f32 / total).max(0.35)).min(0.99) }
}

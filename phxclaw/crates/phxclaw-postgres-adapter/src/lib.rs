use postgres::{Client, NoTls, types::ToSql};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    pub dsn: String,
    pub application_name: String,
    pub statement_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlCommand {
    pub sql: String,
    pub params: Vec<SqlParam>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum SqlParam {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    Text(String),
    Json(Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlExecution {
    pub affected_rows: u64,
}

#[derive(Debug, Error)]
pub enum PostgresAdapterError {
    #[error("PostgreSQL error: {0}")]
    Postgres(#[from] postgres::Error),
    #[error("query cannot be empty")]
    EmptyQuery,
    #[error("statement_timeout_ms is too large")]
    TimeoutRange,
}

pub struct PostgresAdapter {
    client: Client,
}

impl PostgresAdapter {
    pub fn connect(config: &PostgresConfig) -> Result<Self, PostgresAdapterError> {
        if config.statement_timeout_ms > i32::MAX as u64 {
            return Err(PostgresAdapterError::TimeoutRange);
        }
        let mut client = Client::connect(&config.dsn, NoTls)?;
        client.execute(
            "SELECT set_config('application_name', $1, false)",
            &[&config.application_name],
        )?;
        client.execute(
            "SELECT set_config('statement_timeout', $1, false)",
            &[&config.statement_timeout_ms.to_string()],
        )?;
        Ok(Self { client })
    }

    pub fn batch_execute(&mut self, sql: &str) -> Result<(), PostgresAdapterError> {
        ensure_sql(sql)?;
        self.client.batch_execute(sql)?;
        Ok(())
    }

    pub fn execute(&mut self, command: &SqlCommand) -> Result<SqlExecution, PostgresAdapterError> {
        ensure_sql(&command.sql)?;
        let owned = bind_params(&command.params);
        let refs = as_refs(&owned);
        let affected_rows = self.client.execute(&command.sql, &refs)?;
        Ok(SqlExecution { affected_rows })
    }

    /// Returns every row as PostgreSQL row_to_json(), preserving native JSON types.
    pub fn query_json(&mut self, command: &SqlCommand) -> Result<Vec<Value>, PostgresAdapterError> {
        ensure_sql(&command.sql)?;
        let wrapped = format!(
            "SELECT row_to_json(_phoenix_row) FROM ({}) AS _phoenix_row",
            command.sql
        );
        let owned = bind_params(&command.params);
        let refs = as_refs(&owned);
        let rows = self.client.query(&wrapped, &refs)?;
        Ok(rows.into_iter().map(|row| row.get::<_, Value>(0)).collect())
    }

    /// Executes a deterministic list of statements in a single transaction.
    pub fn transaction(
        &mut self,
        commands: &[SqlCommand],
    ) -> Result<Vec<SqlExecution>, PostgresAdapterError> {
        let mut tx = self.client.transaction()?;
        let mut out = Vec::with_capacity(commands.len());
        for command in commands {
            ensure_sql(&command.sql)?;
            let owned = bind_params(&command.params);
            let refs = as_refs(&owned);
            let affected_rows = tx.execute(&command.sql, &refs)?;
            out.push(SqlExecution { affected_rows });
        }
        tx.commit()?;
        Ok(out)
    }

    pub fn ping(&mut self) -> Result<(), PostgresAdapterError> {
        self.client.simple_query("SELECT 1")?;
        Ok(())
    }
}

fn ensure_sql(sql: &str) -> Result<(), PostgresAdapterError> {
    if sql.trim().is_empty() {
        Err(PostgresAdapterError::EmptyQuery)
    } else {
        Ok(())
    }
}

enum BoundParam {
    Null(Option<String>),
    Bool(bool),
    I64(i64),
    F64(f64),
    Text(String),
    Json(Value),
}

fn bind_params(values: &[SqlParam]) -> Vec<BoundParam> {
    values
        .iter()
        .map(|value| match value {
            SqlParam::Null => BoundParam::Null(None),
            SqlParam::Bool(v) => BoundParam::Bool(*v),
            SqlParam::I64(v) => BoundParam::I64(*v),
            SqlParam::F64(v) => BoundParam::F64(*v),
            SqlParam::Text(v) => BoundParam::Text(v.clone()),
            SqlParam::Json(v) => BoundParam::Json(v.clone()),
        })
        .collect()
}

fn as_refs(values: &[BoundParam]) -> Vec<&(dyn ToSql + Sync)> {
    values
        .iter()
        .map(|value| match value {
            BoundParam::Null(v) => v as &(dyn ToSql + Sync),
            BoundParam::Bool(v) => v as &(dyn ToSql + Sync),
            BoundParam::I64(v) => v as &(dyn ToSql + Sync),
            BoundParam::F64(v) => v as &(dyn ToSql + Sync),
            BoundParam::Text(v) => v as &(dyn ToSql + Sync),
            BoundParam::Json(v) => v as &(dyn ToSql + Sync),
        })
        .collect()
}

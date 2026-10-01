pub use phx_sql_model::*;
pub use phx_sql_parser_api::{detect_dialect,DialectDetection,ParseError,SqlDialect,SqlDialectParser};
pub use phx_sql_projection::{project_table_graph,GraphEdge,GraphNode,GraphProjection};
pub use phx_sql_contract::{model_to_contract_json,model_to_contract_value,CONTRACT_VERSION};
use phx_parser_postgresql::PostgreSqlParser;
use phx_parser_mysql::MySqlParser;
use phx_parser_sqlite::SqliteParser;
use phx_parser_sqlserver::SqlServerParser;

pub fn parse_postgresql_model(s:&str,sql:&str)->Result<SqlModel,ParseError>{PostgreSqlParser.parse(s,sql)}
pub fn parse_mysql_model(s:&str,sql:&str)->Result<SqlModel,ParseError>{MySqlParser.parse(s,sql)}
pub fn parse_sqlite_model(s:&str,sql:&str)->Result<SqlModel,ParseError>{SqliteParser.parse(s,sql)}
pub fn parse_sqlserver_model(s:&str,sql:&str)->Result<SqlModel,ParseError>{SqlServerParser.parse(s,sql)}

pub fn parse_dialect_model(d:SqlDialect,s:&str,sql:&str)->Result<SqlModel,ParseError>{
    match d{
        SqlDialect::PostgreSql=>PostgreSqlParser.parse(s,sql),
        SqlDialect::MySql=>MySqlParser.parse(s,sql),
        SqlDialect::Sqlite=>SqliteParser.parse(s,sql),
        SqlDialect::SqlServer=>SqlServerParser.parse(s,sql),
        SqlDialect::Unknown=>Err(ParseError::new(d,"dialeto SQL não reconhecido")),
    }
}

pub fn parse_auto_model(s:&str,sql:&str)->Result<(DialectDetection,SqlModel),ParseError>{
    let d=detect_dialect(sql);let m=parse_dialect_model(d.dialect,s,sql)?;Ok((d,m))
}
fn parser_id(d:SqlDialect)->&'static str{match d{SqlDialect::PostgreSql=>"postgresql-rust",SqlDialect::MySql=>"mysql-rust",SqlDialect::Sqlite=>"sqlite-rust",SqlDialect::SqlServer=>"sqlserver-rust",SqlDialect::Unknown=>"unknown-rust"}}

pub fn parse_dialect_to_ui_json(d:SqlDialect,s:&str,sql:&str)->Result<String,Box<dyn std::error::Error>>{
    let m=parse_dialect_model(d,s,sql)?;Ok(model_to_contract_json(&m,d,parser_id(d),"sql_text")?)
}
pub fn parse_to_ui_json(s:&str,sql:&str)->Result<String,Box<dyn std::error::Error>>{parse_dialect_to_ui_json(SqlDialect::PostgreSql,s,sql)}
pub fn parse_auto_to_ui_json(s:&str,sql:&str)->Result<String,Box<dyn std::error::Error>>{let(d,m)=parse_auto_model(s,sql)?;Ok(model_to_contract_json(&m,d.dialect,parser_id(d.dialect),"sql_text")?)}

#[cfg(feature="wasm")] use wasm_bindgen::prelude::*;
#[cfg(feature="wasm")]
#[wasm_bindgen] pub fn parse_postgresql(s:String,sql:String)->Result<String,JsValue>{parse_dialect_to_ui_json(SqlDialect::PostgreSql,&s,&sql).map_err(|e|JsValue::from_str(&e.to_string()))}
#[cfg(feature="wasm")]
#[wasm_bindgen] pub fn parse_mysql(s:String,sql:String)->Result<String,JsValue>{parse_dialect_to_ui_json(SqlDialect::MySql,&s,&sql).map_err(|e|JsValue::from_str(&e.to_string()))}
#[cfg(feature="wasm")]
#[wasm_bindgen] pub fn parse_sqlite(s:String,sql:String)->Result<String,JsValue>{parse_dialect_to_ui_json(SqlDialect::Sqlite,&s,&sql).map_err(|e|JsValue::from_str(&e.to_string()))}
#[cfg(feature="wasm")]
#[wasm_bindgen] pub fn parse_sqlserver(s:String,sql:String)->Result<String,JsValue>{parse_dialect_to_ui_json(SqlDialect::SqlServer,&s,&sql).map_err(|e|JsValue::from_str(&e.to_string()))}
#[cfg(feature="wasm")]
#[wasm_bindgen] pub fn parse_auto(s:String,sql:String)->Result<String,JsValue>{parse_auto_to_ui_json(&s,&sql).map_err(|e|JsValue::from_str(&e.to_string()))}
#[cfg(feature="wasm")]
#[wasm_bindgen] pub fn detect_sql_dialect(sql:String)->String{detect_dialect(&sql).dialect.as_str().to_owned()}

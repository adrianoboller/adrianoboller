use phx_parser_common::{identifier_pattern,parse_generic,GenericDialectConfig,ObjectSpec};
use phx_sql_model::{ObjectKind,SqlModel};
use phx_sql_parser_api::{ParseError,SqlDialect,SqlDialectParser};
pub struct SqlServerParser;
impl SqlDialectParser for SqlServerParser{fn dialect(&self)->SqlDialect{SqlDialect::SqlServer}fn parse(&self,s:&str,sql:&str)->Result<SqlModel,ParseError>{parse_sql(s,sql).map_err(|e|ParseError::new(SqlDialect::SqlServer,e))}}
pub fn parse_sql(s:&str,sql:&str)->Result<SqlModel,String>{let id=identifier_pattern();let cfg=GenericDialectConfig{dialect:"sqlserver",
 create_table_pattern:format!(r"(?im)^\s*CREATE\s+TABLE\s+(?P<name>{id})\s*\("),
 alter_fk_pattern:Some(format!(r"(?is)ALTER\s+TABLE\s+(?P<from>{id})\s+(?:WITH\s+(?:CHECK|NOCHECK)\s+)?ADD\s+(?:CONSTRAINT\s+(?P<constraint>{id})\s+)?FOREIGN\s+KEY\s*\((?P<from_cols>[^)]*)\)\s+REFERENCES\s+(?P<to>{id})\s*\((?P<to_cols>[^)]*)\)(?P<tail>[\s\S]*?)(?=;|\r?\n\s*GO\b|$)")),
 constraint_keywords:vec!["NOT NULL","NULL","DEFAULT","PRIMARY KEY","UNIQUE","REFERENCES","CHECK","CONSTRAINT","COLLATE","IDENTITY","GENERATED","AS","PERSISTED"],
 object_specs:vec![
  ObjectSpec{kind:ObjectKind::View,pattern:format!(r"(?im)^\s*CREATE\s+(?:OR\s+ALTER\s+)?VIEW\s+(?P<name>{id})"),target_pattern:None},
  ObjectSpec{kind:ObjectKind::Function,pattern:format!(r"(?im)^\s*CREATE\s+(?:OR\s+ALTER\s+)?(?:FUNCTION|PROCEDURE|PROC)\s+(?P<name>{id})"),target_pattern:None},
  ObjectSpec{kind:ObjectKind::Trigger,pattern:format!(r"(?im)^\s*CREATE\s+(?:OR\s+ALTER\s+)?TRIGGER\s+(?P<name>{id})"),target_pattern:Some(format!(r"(?i)\bON\s+(?P<target>{id})"))},
  ObjectSpec{kind:ObjectKind::Index,pattern:format!(r"(?im)^\s*CREATE\s+(?:UNIQUE\s+)?(?:(?:CLUSTERED|NONCLUSTERED)\s+)?INDEX\s+(?P<name>{id})"),target_pattern:Some(format!(r"(?i)\bON\s+(?P<target>{id})"))},
  ObjectSpec{kind:ObjectKind::Schema,pattern:format!(r"(?im)^\s*CREATE\s+SCHEMA\s+(?P<name>{id})"),target_pattern:None},
  ObjectSpec{kind:ObjectKind::Sequence,pattern:format!(r"(?im)^\s*CREATE\s+SEQUENCE\s+(?P<name>{id})"),target_pattern:None},
  ObjectSpec{kind:ObjectKind::Synonym,pattern:format!(r"(?im)^\s*CREATE\s+SYNONYM\s+(?P<name>{id})"),target_pattern:Some(format!(r"(?i)\bFOR\s+(?P<target>{id})"))}
 ],go_batch:true};parse_generic(s,sql,&cfg)}

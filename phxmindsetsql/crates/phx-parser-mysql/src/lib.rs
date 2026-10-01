use phx_parser_common::{identifier_pattern,parse_generic,GenericDialectConfig,ObjectSpec};
use phx_sql_model::{ObjectKind,SqlModel};
use phx_sql_parser_api::{ParseError,SqlDialect,SqlDialectParser};
pub struct MySqlParser;
impl SqlDialectParser for MySqlParser{fn dialect(&self)->SqlDialect{SqlDialect::MySql}fn parse(&self,s:&str,sql:&str)->Result<SqlModel,ParseError>{parse_sql(s,sql).map_err(|e|ParseError::new(SqlDialect::MySql,e))}}
pub fn parse_sql(s:&str,sql:&str)->Result<SqlModel,String>{let id=identifier_pattern();let cfg=GenericDialectConfig{dialect:"mysql",
 create_table_pattern:format!(r"(?im)^\s*CREATE\s+(?:TEMPORARY\s+)?TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(?P<name>{id})\s*\("),
 alter_fk_pattern:Some(format!(r"(?is)ALTER\s+TABLE\s+(?P<from>{id})\s+ADD\s+(?:CONSTRAINT\s+(?P<constraint>{id})\s+)?FOREIGN\s+KEY\s*\((?P<from_cols>[^)]*)\)\s+REFERENCES\s+(?P<to>{id})\s*\((?P<to_cols>[^)]*)\)(?P<tail>[^;]*)")),
 constraint_keywords:vec!["NOT NULL","NULL","DEFAULT","PRIMARY KEY","UNIQUE","REFERENCES","CHECK","CONSTRAINT","COLLATE","AUTO_INCREMENT","COMMENT","GENERATED","AS","ON UPDATE"],
 object_specs:vec![
  ObjectSpec{kind:ObjectKind::View,pattern:format!(r"(?im)^\s*CREATE\s+(?:OR\s+REPLACE\s+)?VIEW\s+(?P<name>{id})"),target_pattern:None},
  ObjectSpec{kind:ObjectKind::Function,pattern:format!(r"(?im)^\s*CREATE\s+(?:FUNCTION|PROCEDURE)\s+(?P<name>{id})"),target_pattern:None},
  ObjectSpec{kind:ObjectKind::Trigger,pattern:format!(r"(?im)^\s*CREATE\s+TRIGGER\s+(?P<name>{id})"),target_pattern:Some(format!(r"(?i)\bON\s+(?P<target>{id})"))},
  ObjectSpec{kind:ObjectKind::Index,pattern:format!(r"(?im)^\s*CREATE\s+(?:(?:UNIQUE|FULLTEXT|SPATIAL)\s+)?INDEX\s+(?P<name>{id})"),target_pattern:Some(format!(r"(?i)\bON\s+(?P<target>{id})"))},
  ObjectSpec{kind:ObjectKind::Schema,pattern:format!(r"(?im)^\s*CREATE\s+(?:DATABASE|SCHEMA)\s+(?:IF\s+NOT\s+EXISTS\s+)?(?P<name>{id})"),target_pattern:None},
  ObjectSpec{kind:ObjectKind::Event,pattern:format!(r"(?im)^\s*CREATE\s+(?:(?:DEFINER\s*=\s*\S+)\s+)?EVENT\s+(?:IF\s+NOT\s+EXISTS\s+)?(?P<name>{id})"),target_pattern:None}
 ],go_batch:false};parse_generic(s,sql,&cfg)}

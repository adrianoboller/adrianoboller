use phx_sql_introspection_api::*;
use phx_sql_parser_api::SqlDialect;

#[test]
fn credentials_are_redacted_from_debug_and_source(){
    let c=ConnectionSpec{dialect:SqlDialect::PostgreSql,host:"db.example".into(),port:Some(5432),database:"app".into(),username:Some("reader".into()),password:Some("super-secret".into()),tls:TlsMode::Require,path:None};
    let dbg=format!("{c:?}");assert!(!dbg.contains("super-secret"));assert!(dbg.contains("REDACTED"));let source=c.safe_source_name();assert!(!source.contains("reader"));assert!(!source.contains("super-secret"));
}

#[test]
fn catalog_snapshot_converges_to_neutral_model(){
    let snap=CatalogSnapshot{dialect:SqlDialect::PostgreSql,source_name:"live:postgresql://db/app".into(),database_name:"app".into(),tables:vec![
        CatalogTable{schema:"public".into(),name:"users".into(),columns:vec![CatalogColumn{name:"id".into(),data_type:"uuid".into(),nullable:false,default_expr:None,primary_key:true,unique:false,ordinal:1,generated_expression:None,generated_kind:None}],primary_key:vec!["id".into()],foreign_keys:vec![],check_constraints:vec![],partitioning:None,statistics:None,definition:None},
        CatalogTable{schema:"public".into(),name:"orders".into(),columns:vec![CatalogColumn{name:"user_id".into(),data_type:"uuid".into(),nullable:false,default_expr:None,primary_key:false,unique:false,ordinal:1,generated_expression:None,generated_kind:None}],primary_key:vec![],foreign_keys:vec![CatalogForeignKey{constraint_name:Some("fk_orders_users".into()),from_schema:"public".into(),from_table:"orders".into(),from_columns:vec!["user_id".into()],to_schema:"public".into(),to_table:"users".into(),to_columns:vec!["id".into()],on_delete:Some("CASCADE".into())}],check_constraints:vec![],partitioning:None,statistics:None,definition:None}
    ],objects:vec![]};
    let m=snapshot_to_model(snap).unwrap();assert_eq!(m.stats.tables,2);assert_eq!(m.stats.columns,2);assert_eq!(m.stats.relationships,1);assert_eq!(m.relationships[0].from_table,"public.orders");assert_eq!(m.relationships[0].to_table,"public.users");
}

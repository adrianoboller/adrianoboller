use axum::{extract::State,http::StatusCode,response::IntoResponse,routing::get,Json,Router};
use phx_introspector_mysql::MySqlIntrospector;
use phx_introspector_postgresql::PostgreSqlIntrospector;
use phx_introspector_sqlite::SqliteIntrospector;
use phx_introspector_sqlserver::SqlServerIntrospector;
use phx_sql_contract::model_to_contract_value;
use phx_sql_introspection_api::{snapshot_to_model,ConnectionSpec,DatabaseIntrospector,IntrospectionOptions,TlsMode};
use phx_sql_parser_api::SqlDialect;
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use std::{collections::HashMap,env,net::SocketAddr,path::PathBuf,sync::Arc};
use tower_http::{services::ServeDir,trace::TraceLayer};

#[derive(Clone)]struct AppState{profiles:Arc<HashMap<String,ResolvedProfile>>,allow_ephemeral:bool}
#[derive(Clone)]struct ResolvedProfile{id:String,label:String,connection:ConnectionSpec}

#[derive(Debug,Deserialize)]struct ProfileFile{profiles:Vec<ProfileConfig>}
#[derive(Debug,Deserialize)]struct ProfileConfig{id:String,label:Option<String>,dialect:SqlDialect,host:Option<String>,port:Option<u16>,database:String,username:Option<String>,password_env:Option<String>,tls:Option<TlsMode>,path:Option<String>}

#[derive(Debug,Deserialize)]struct IntrospectRequest{profile_id:Option<String>,connection:Option<ConnectionSpec>,#[serde(default)]options:IntrospectionOptions}
#[derive(Debug,Serialize)]struct PublicProfile{id:String,label:String,dialect:String,host:String,port:Option<u16>,database:String}

#[tokio::main]
async fn main(){
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let profiles=Arc::new(load_profiles().unwrap_or_else(|e|{tracing::warn!("profiles não carregados: {e}");HashMap::new()}));
    let state=AppState{profiles,allow_ephemeral:env::var("PHX_ALLOW_EPHEMERAL").ok().as_deref()==Some("1")};
    let web_root=env::var_os("PHX_WEB_ROOT").map(PathBuf::from).unwrap_or_else(||PathBuf::from("."));
    let app=Router::new().route("/api/v1/health",get(health)).route("/api/v1/profiles",get(profiles)).route("/api/v1/introspect",axum::routing::post(introspect)).fallback_service(ServeDir::new(web_root).append_index_html_on_directories(true)).layer(TraceLayer::new_for_http()).with_state(state);
    let addr:SocketAddr=env::var("PHX_BIND").unwrap_or_else(|_|"127.0.0.1:8787".into()).parse().expect("PHX_BIND inválido");
    tracing::info!(%addr,"PhxMindSetSQL gateway");
    let listener=tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener,app).with_graceful_shutdown(shutdown()).await.expect("server");
}

async fn shutdown(){let _=tokio::signal::ctrl_c().await;}

async fn health(State(st):State<AppState>)->Json<Value>{Json(json!({"ok":true,"service":"phx-sql-gateway","version":"0.7.0","profiles":st.profiles.len(),"ephemeral":st.allow_ephemeral,"dialects":["postgresql","mysql","sqlite","sqlserver"]}))}
async fn profiles(State(st):State<AppState>)->Json<Vec<PublicProfile>>{let mut out=st.profiles.values().map(|p|PublicProfile{id:p.id.clone(),label:p.label.clone(),dialect:p.connection.dialect.as_str().into(),host:p.connection.host.clone(),port:p.connection.port,database:p.connection.database.clone()}).collect::<Vec<_>>();out.sort_by(|a,b|a.label.cmp(&b.label));Json(out)}

async fn introspect(State(st):State<AppState>,Json(req):Json<IntrospectRequest>)->impl IntoResponse{
    match resolve_connection(&st,&req){
        Ok(spec)=>match run_introspection(&spec,&req.options).await{
            Ok(v)=>(StatusCode::OK,Json(v)),
            Err(e)=>{tracing::warn!(error=%e,dialect=spec.dialect.as_str(),"introspection failed");(StatusCode::BAD_GATEWAY,Json(json!({"ok":false,"error":e.to_string()})))},
        },
        Err(e)=>(StatusCode::BAD_REQUEST,Json(json!({"ok":false,"error":e}))),
    }
}

fn resolve_connection(st:&AppState,req:&IntrospectRequest)->Result<ConnectionSpec,String>{
    if let Some(id)=&req.profile_id{return st.profiles.get(id).map(|p|p.connection.clone()).ok_or_else(||"profile_id não encontrado".into());}
    if let Some(c)=&req.connection{if !st.allow_ephemeral{return Err("conexões efêmeras estão desabilitadas; use profile_id ou PHX_ALLOW_EPHEMERAL=1".into())}return Ok(c.clone())}
    Err("informe profile_id ou connection".into())
}

async fn run_introspection(spec:&ConnectionSpec,opt:&IntrospectionOptions)->Result<Value,Box<dyn std::error::Error>>{
    let snapshot=match spec.dialect{
        SqlDialect::PostgreSql=>PostgreSqlIntrospector.introspect(spec,opt).await?,
        SqlDialect::MySql=>MySqlIntrospector.introspect(spec,opt).await?,
        SqlDialect::Sqlite=>SqliteIntrospector.introspect(spec,opt).await?,
        SqlDialect::SqlServer=>SqlServerIntrospector.introspect(spec,opt).await?,
        SqlDialect::Unknown=>return Err("dialeto desconhecido".into()),
    };
    let dialect=snapshot.dialect;let model=snapshot_to_model(snapshot)?;
    Ok(model_to_contract_value(&model,dialect,&format!("{}-catalog-introspector",dialect.as_str()),"live_database"))
}

fn load_profiles()->Result<HashMap<String,ResolvedProfile>,Box<dyn std::error::Error>>{
    let Some(path)=env::var_os("PHX_PROFILES_FILE") else{return Ok(HashMap::new())};
    let parsed:ProfileFile=serde_json::from_slice(&std::fs::read(path)?)?;let mut out=HashMap::new();
    for p in parsed.profiles{
        let password=p.password_env.as_ref().and_then(|name|env::var(name).ok());
        let c=ConnectionSpec{dialect:p.dialect,host:p.host.unwrap_or_else(||"127.0.0.1".into()),port:p.port,database:p.database,username:p.username,password,tls:p.tls.unwrap_or_default(),path:p.path};
        out.insert(p.id.clone(),ResolvedProfile{id:p.id,label:p.label.unwrap_or_else(||c.safe_source_name()),connection:c});
    }
    Ok(out)
}

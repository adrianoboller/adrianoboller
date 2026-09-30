//! PhxClaw v0.59 — Historical Backfill & Knowledge Migration.
//! Control-plane contracts for resumable, idempotent migration into the v0.58 Project Trace Tree.
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use uuid::Uuid;

#[derive(Debug,thiserror::Error)]
pub enum BackfillError {
    #[error("registry source table is invalid")] InvalidTable,
    #[error("batch size must be between 1 and 10000")] BatchSize,
    #[error("registry hash does not match resumed run")] RegistryDrift,
    #[error("project could not be resolved")] ProjectUnresolved,
    #[error("raw secrets are forbidden in trace metadata")] Secret,
}

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="snake_case")]
pub enum ScopePolicy { DirectProject, ReferenceRequired, PortfolioScope, TenantScope }

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct BackfillSource {
    pub priority:u16,
    pub source_version:String,
    pub source_table:String,
    pub domain:String,
    pub node_kind:String,
    pub scope_policy:ScopePolicy,
    pub parent_ref_columns:Vec<String>,
    pub link_ref_columns:Vec<String>,
    pub source_state_columns:Vec<String>,
    pub evidence_columns:Vec<String>,
    pub enabled:bool,
}
impl BackfillSource {
    pub fn validate(&self)->Result<(),BackfillError>{
        if self.source_table.is_empty() || !self.source_table.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'_') { return Err(BackfillError::InvalidTable); }
        Ok(())
    }
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct BackfillRun {
    pub run_uuid:Uuid, pub tenant_uuid:Uuid, pub source_state_sha256:String,
    pub registry_sha256:String, pub batch_size:u32,
}
impl BackfillRun { pub fn validate(&self)->Result<(),BackfillError>{ if self.batch_size==0||self.batch_size>10_000 {Err(BackfillError::BatchSize)} else {Ok(())} } }

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct BackfillCheckpoint {
    pub run_uuid:Uuid, pub source_table:String, pub checkpoint_seq:u64,
    pub offset_rows:u64, pub processed:u64, pub inserted:u64, pub skipped:u64, pub orphaned:u64,
    pub cursor:serde_json::Value, pub evidence_sha256:String,
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct CoverageSnapshot {
    pub run_uuid:Uuid, pub source_table:String, pub source_rows:u64,
    pub processed_rows:u64, pub bound_rows:u64, pub orphan_rows:u64, pub skipped_rows:u64,
}
impl CoverageSnapshot {
    pub fn ratio(&self)->f64 { if self.source_rows==0 {1.0} else {self.bound_rows as f64/self.source_rows as f64} }
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct OrphanRecord {
    pub orphan_uuid:Uuid, pub run_uuid:Uuid, pub source_table:String,
    pub source_key_sha256:String, pub row_sha256:String, pub reason_code:String,
    pub candidate_refs:serde_json::Value,
}

pub fn canonical_sha256(v:&serde_json::Value)->String {
    let bytes=serde_json::to_vec(v).expect("serializable json");
    format!("{:x}",Sha256::digest(bytes))
}

pub fn contains_secret_fields(v:&serde_json::Value)->bool {
    match v {
        serde_json::Value::Object(m)=>m.iter().any(|(k,v)|{
            let k=k.to_ascii_lowercase();
            matches!(k.as_str(),"password"|"token"|"api_key"|"apikey"|"secret"|"private_key"|"access_token"|"refresh_token") || contains_secret_fields(v)
        }),
        serde_json::Value::Array(a)=>a.iter().any(contains_secret_fields),
        _=>false,
    }
}

pub fn source_key_hash(table:&str, pk:&serde_json::Value)->String {
    canonical_sha256(&serde_json::json!({"table":table,"pk":pk}))
}

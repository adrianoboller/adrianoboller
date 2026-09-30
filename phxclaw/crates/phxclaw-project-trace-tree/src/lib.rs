//! PhxClaw v0.58 — Project Trace Tree
//! Hierarchical project-control index over specialized domain tables.
use serde::{Deserialize, Serialize};
use sha2::{Digest,Sha256};
use uuid::Uuid;

#[derive(Debug,thiserror::Error)]
pub enum TraceError {
    #[error("depth exceeds configured maximum")]
    Depth,
    #[error("path must end with the node uuid")]
    Path,
    #[error("source sha256 must be lowercase hex")]
    SourceHash,
    #[error("raw secret content is not allowed in trace metadata")]
    Secret,
}

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="snake_case")]
pub enum NodeKind {
    Project, Portfolio, Domain, Module, Sprint, Task, Run, Source, Artifact, Log,
    Evidence, KnowledgeFruitful, KnowledgeUnfruitful, ContextFingerprint,
    Hypothesis, Experiment, Prompt, Skill, Workflow, Agent, Model, Decision,
    Contradiction, Promotion, Version, Backup, SelfImprovement, Release,
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct TraceNode {
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub node_uuid: Uuid,
    pub parent_uuid: Option<Uuid>,
    pub root_uuid: Uuid,
    pub depth: u16,
    pub path_ids: Vec<Uuid>,
    pub kind: NodeKind,
    pub logical_key: String,
    pub title: String,
    pub source_state_sha256: String,
    pub object_type: Option<String>,
    pub object_uuid: Option<Uuid>,
    pub metadata: serde_json::Value,
}
impl TraceNode {
    pub fn validate(&self,max_depth:u16)->Result<(),TraceError>{
        if self.depth>max_depth { return Err(TraceError::Depth); }
        if self.path_ids.last().copied()!=Some(self.node_uuid) { return Err(TraceError::Path); }
        if contains_secret_fields(&self.metadata) { return Err(TraceError::Secret); }
        Ok(())
    }
}

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="snake_case")]
pub enum LinkKind { Supports, Refutes, DerivedFrom, CausedBy, Supersedes, Tests, Implements, Uses, ProducedBy, DecidedBy, DependsOn, RelatedTo }

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct TraceLink {
    pub link_uuid:Uuid, pub from_node_uuid:Uuid, pub to_node_uuid:Uuid,
    pub kind:LinkKind, pub evidence_sha256:Option<String>, pub source_state_sha256:String,
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct SourceDocument {
    pub source_uuid:Uuid, pub project_uuid:Uuid, pub node_uuid:Uuid,
    pub source_kind:String, pub uri:String, pub content_sha256:String,
    pub repository:Option<String>, pub commit_sha:Option<String>,
    pub line_start:Option<u32>, pub line_end:Option<u32>, pub provenance:serde_json::Value,
}
impl SourceDocument {
    pub fn validate(&self)->Result<(),TraceError>{
        if self.content_sha256.len()!=64 || !self.content_sha256.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) { return Err(TraceError::SourceHash); }
        if contains_secret_fields(&self.provenance) { return Err(TraceError::Secret); }
        Ok(())
    }
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct DecisionContext {
    pub decision_node:Uuid, pub ancestors:Vec<Uuid>, pub supporting:Vec<Uuid>,
    pub refuting:Vec<Uuid>, pub caused_by:Vec<Uuid>, pub sources:Vec<Uuid>,
    pub source_state_sha256:String,
}

pub fn canonical_sha256(v:&serde_json::Value)->String {
    let bytes=serde_json::to_vec(v).expect("json serializable");
    format!("{:x}",Sha256::digest(bytes))
}
fn contains_secret_fields(v:&serde_json::Value)->bool{
    match v {
        serde_json::Value::Object(m)=>m.iter().any(|(k,v)|{
            let k=k.to_ascii_lowercase();
            matches!(k.as_str(),"password"|"token"|"api_key"|"apikey"|"secret"|"private_key") || contains_secret_fields(v)
        }),
        serde_json::Value::Array(a)=>a.iter().any(contains_secret_fields),
        _=>false,
    }
}

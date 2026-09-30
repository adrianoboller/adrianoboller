use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all="snake_case")]
pub enum ContractKind { Api, Database, Schema, Behavior, SecurityPolicy, Event, Config }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all="snake_case")]
pub enum ConflictClass {
    SymbolCollision, ContractViolation, DependencyOrder, SchemaMigrationConflict,
    BehavioralDivergence, SecurityPolicyConflict, TestExpectationConflict, EvidenceConflict,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all="snake_case")]
pub enum Severity { Info, Low, Medium, High, Critical }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum ReplayStatus { Passed, Failed, Diverged, SideEffectMismatch }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractSurface {
    pub contract_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub kind: ContractKind,
    pub name: String,
    pub version: String,
    pub schema_sha256: String,
    pub breaking: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeIntent {
    pub intent_uuid: Uuid,
    pub swarm_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub team: String,
    pub actor_uuid: Uuid,
    pub base_source_sha256: String,
    pub artifact_sha256: String,
    pub changed_paths: BTreeSet<String>,
    pub changed_symbols: BTreeSet<String>,
    pub contract_changes: BTreeSet<Uuid>,
    pub depends_on_intents: BTreeSet<Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticConflict {
    pub conflict_uuid: Uuid,
    pub swarm_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub class: ConflictClass,
    pub severity: Severity,
    pub intent_uuids: BTreeSet<Uuid>,
    pub subject: String,
    pub evidence_sha256: String,
    pub blocking: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MergeOrderPlan {
    pub plan_uuid: Uuid,
    pub swarm_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub base_source_sha256: String,
    pub ordered_intents: Vec<Uuid>,
    pub plan_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntegrationReplayEvidence {
    pub replay_uuid: Uuid,
    pub swarm_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub base_source_sha256: String,
    pub merge_plan_sha256: String,
    pub candidate_artifacts: BTreeMap<Uuid,String>,
    pub replay_tree_sha256: String,
    pub test_suite_sha256: String,
    pub status: ReplayStatus,
    pub side_effects_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MutationEvidence {
    pub evidence_uuid: Uuid,
    pub swarm_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub source_state_sha256: String,
    pub total_mutants: u32,
    pub killed_mutants: u32,
    pub critical_survivors: u32,
    pub report_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConflictResolutionPayload {
    pub resolution_uuid: Uuid,
    pub conflict_uuid: Uuid,
    pub swarm_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub conflict_evidence_sha256: String,
    pub chosen_resolution_sha256: String,
    pub rationale_sha256: String,
    pub approved: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedResolutionDocument {
    pub payload: ConflictResolutionPayload,
    pub signer_key_id: String,
    pub signer_public_key_base64: String,
    pub signature_base64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifiedResolution {
    pub payload: ConflictResolutionPayload,
    pub signer_key_id: String,
    pub document_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsensusEvidence {
    pub source_state_sha256: String,
    pub qa_passed: bool,
    pub security_passed: bool,
    pub architecture_passed: bool,
    pub contract_gate_passed: bool,
    pub replay_passed: bool,
    pub mutation_passed: bool,
    pub unresolved_conflicts: BTreeSet<Uuid>,
    pub highest_security_severity: Severity,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum MergeIntelligenceError {
    #[error("invalid sha256")] InvalidHash,
    #[error("source state mismatch")] SourceStateMismatch,
    #[error("dependency cycle")] DependencyCycle,
    #[error("missing dependency")] MissingDependency,
    #[error("blocking semantic conflict")] BlockingConflict,
    #[error("contract gate failed")] ContractGateFailed,
    #[error("integration replay failed")] ReplayFailed,
    #[error("mutation gate failed")] MutationGateFailed,
    #[error("evidence gate failed")] EvidenceGateFailed,
    #[error("security gate failed")] SecurityGateFailed,
    #[error("untrusted signer")] UntrustedSigner,
    #[error("invalid signature")] InvalidSignature,
    #[error("resolution mismatch")] ResolutionMismatch,
}

fn valid_hash(v: &str) -> bool { v.len()==64 && v.bytes().all(|b| b.is_ascii_hexdigit()) }
fn sha_bytes(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

pub fn intent_hash(i: &ChangeIntent) -> Result<String, MergeIntelligenceError> {
    if !valid_hash(&i.base_source_sha256) || !valid_hash(&i.artifact_sha256) { return Err(MergeIntelligenceError::InvalidHash); }
    Ok(sha_bytes(&serde_json::to_vec(i).expect("serializable")))
}

pub fn detect_semantic_conflicts(intents: &[ChangeIntent], contracts: &BTreeMap<Uuid,ContractSurface>) -> Result<Vec<SemanticConflict>, MergeIntelligenceError> {
    let mut out=Vec::new();
    for i in intents {
        if !valid_hash(&i.base_source_sha256) || !valid_hash(&i.artifact_sha256) { return Err(MergeIntelligenceError::InvalidHash); }
        for cid in &i.contract_changes { if !contracts.contains_key(cid) { return Err(MergeIntelligenceError::MissingDependency); } }
    }
    for a in 0..intents.len() { for b in a+1..intents.len() {
        let ia=&intents[a]; let ib=&intents[b];
        if ia.tenant_uuid!=ib.tenant_uuid || ia.swarm_uuid!=ib.swarm_uuid || ia.base_source_sha256!=ib.base_source_sha256 { return Err(MergeIntelligenceError::SourceStateMismatch); }
        let symbols: Vec<_>=ia.changed_symbols.intersection(&ib.changed_symbols).cloned().collect();
        if !symbols.is_empty() {
            let ev=sha_bytes(format!("symbols:{:?}",symbols).as_bytes());
            out.push(SemanticConflict{ conflict_uuid:Uuid::now_v7(), swarm_uuid:ia.swarm_uuid, tenant_uuid:ia.tenant_uuid, class:ConflictClass::SymbolCollision, severity:Severity::Medium, intent_uuids:[ia.intent_uuid,ib.intent_uuid].into_iter().collect(), subject:symbols.join(","), evidence_sha256:ev, blocking:false });
        }
        let shared_contracts: Vec<_>=ia.contract_changes.intersection(&ib.contract_changes).copied().collect();
        for cid in shared_contracts {
            let c=&contracts[&cid];
            let (class,severity,blocking)=match c.kind {
                ContractKind::SecurityPolicy => (ConflictClass::SecurityPolicyConflict,Severity::Critical,true),
                ContractKind::Database|ContractKind::Schema => (ConflictClass::SchemaMigrationConflict,Severity::High,true),
                ContractKind::Behavior => (ConflictClass::BehavioralDivergence,Severity::High,true),
                _ => (ConflictClass::ContractViolation,Severity::High,true),
            };
            let ev=sha_bytes(format!("contract:{}:{}",cid,c.schema_sha256).as_bytes());
            out.push(SemanticConflict{ conflict_uuid:Uuid::now_v7(), swarm_uuid:ia.swarm_uuid, tenant_uuid:ia.tenant_uuid, class, severity, intent_uuids:[ia.intent_uuid,ib.intent_uuid].into_iter().collect(), subject:c.name.clone(), evidence_sha256:ev, blocking });
        }
    }}
    out.sort_by(|a,b| (a.class,a.severity,&a.subject).cmp(&(b.class,b.severity,&b.subject)));
    Ok(out)
}

pub fn build_merge_order(swarm_uuid:Uuid, tenant_uuid:Uuid, base_source_sha256:&str, intents:&[ChangeIntent]) -> Result<MergeOrderPlan,MergeIntelligenceError> {
    if !valid_hash(base_source_sha256) { return Err(MergeIntelligenceError::InvalidHash); }
    let ids:BTreeSet<_>=intents.iter().map(|i|i.intent_uuid).collect();
    let mut indegree:BTreeMap<Uuid,usize>=ids.iter().map(|id|(*id,0)).collect();
    let mut edges:BTreeMap<Uuid,BTreeSet<Uuid>>=BTreeMap::new();
    for i in intents {
        if i.swarm_uuid!=swarm_uuid || i.tenant_uuid!=tenant_uuid || i.base_source_sha256!=base_source_sha256 { return Err(MergeIntelligenceError::SourceStateMismatch); }
        for dep in &i.depends_on_intents {
            if !ids.contains(dep) { return Err(MergeIntelligenceError::MissingDependency); }
            if edges.entry(*dep).or_default().insert(i.intent_uuid) { *indegree.get_mut(&i.intent_uuid).unwrap()+=1; }
        }
    }
    let mut ready:BTreeSet<_>=indegree.iter().filter_map(|(k,v)| if *v==0 {Some(*k)} else {None}).collect();
    let mut ordered=Vec::with_capacity(ids.len());
    while let Some(id)=ready.iter().next().copied() {
        ready.remove(&id); ordered.push(id);
        for nxt in edges.get(&id).cloned().unwrap_or_default() {
            let d=indegree.get_mut(&nxt).unwrap(); *d-=1; if *d==0 { ready.insert(nxt); }
        }
    }
    if ordered.len()!=ids.len() { return Err(MergeIntelligenceError::DependencyCycle); }
    let raw=serde_json::to_vec(&(swarm_uuid,tenant_uuid,base_source_sha256,&ordered)).expect("serializable");
    Ok(MergeOrderPlan{plan_uuid:Uuid::now_v7(),swarm_uuid,tenant_uuid,base_source_sha256:base_source_sha256.into(),ordered_intents:ordered,plan_sha256:sha_bytes(&raw)})
}

pub fn contract_gate(conflicts:&[SemanticConflict], verified_resolutions:&BTreeMap<Uuid,VerifiedResolution>) -> Result<(),MergeIntelligenceError> {
    for c in conflicts {
        if c.blocking && !verified_resolutions.contains_key(&c.conflict_uuid) { return Err(MergeIntelligenceError::BlockingConflict); }
    }
    Ok(())
}

pub fn replay_gate(plan:&MergeOrderPlan, replay:&IntegrationReplayEvidence, expected_artifacts:&BTreeMap<Uuid,String>) -> Result<(),MergeIntelligenceError> {
    if replay.swarm_uuid!=plan.swarm_uuid || replay.tenant_uuid!=plan.tenant_uuid || replay.base_source_sha256!=plan.base_source_sha256 || replay.merge_plan_sha256!=plan.plan_sha256 { return Err(MergeIntelligenceError::SourceStateMismatch); }
    for h in [&replay.replay_tree_sha256,&replay.test_suite_sha256,&replay.side_effects_sha256] { if !valid_hash(h) { return Err(MergeIntelligenceError::InvalidHash); } }
    if &replay.candidate_artifacts!=expected_artifacts || replay.status!=ReplayStatus::Passed { return Err(MergeIntelligenceError::ReplayFailed); }
    Ok(())
}

pub fn mutation_gate(ev:&MutationEvidence, minimum_score_bps:u16, minimum_mutants:u32) -> Result<u16,MergeIntelligenceError> {
    if !valid_hash(&ev.source_state_sha256) || !valid_hash(&ev.report_sha256) { return Err(MergeIntelligenceError::InvalidHash); }
    if ev.total_mutants<minimum_mutants || ev.critical_survivors>0 || ev.killed_mutants>ev.total_mutants { return Err(MergeIntelligenceError::MutationGateFailed); }
    let score=((ev.killed_mutants as u64)*10_000/(ev.total_mutants.max(1) as u64)) as u16;
    if score<minimum_score_bps { return Err(MergeIntelligenceError::MutationGateFailed); }
    Ok(score)
}

pub fn verify_signed_resolution(doc:&SignedResolutionDocument, trusted_keys:&BTreeMap<String,String>, expected_conflict:&SemanticConflict) -> Result<VerifiedResolution,MergeIntelligenceError> {
    if doc.payload.conflict_uuid!=expected_conflict.conflict_uuid || doc.payload.swarm_uuid!=expected_conflict.swarm_uuid || doc.payload.tenant_uuid!=expected_conflict.tenant_uuid || doc.payload.conflict_evidence_sha256!=expected_conflict.evidence_sha256 || !doc.payload.approved { return Err(MergeIntelligenceError::ResolutionMismatch); }
    for h in [&doc.payload.conflict_evidence_sha256,&doc.payload.chosen_resolution_sha256,&doc.payload.rationale_sha256] { if !valid_hash(h) { return Err(MergeIntelligenceError::InvalidHash); } }
    let trusted=trusted_keys.get(&doc.signer_key_id).ok_or(MergeIntelligenceError::UntrustedSigner)?;
    if trusted!=&doc.signer_public_key_base64 { return Err(MergeIntelligenceError::UntrustedSigner); }
    let pk=base64::engine::general_purpose::STANDARD.decode(&doc.signer_public_key_base64).map_err(|_|MergeIntelligenceError::InvalidSignature)?;
    let sig=base64::engine::general_purpose::STANDARD.decode(&doc.signature_base64).map_err(|_|MergeIntelligenceError::InvalidSignature)?;
    let key=VerifyingKey::from_bytes(pk.as_slice().try_into().map_err(|_|MergeIntelligenceError::InvalidSignature)?).map_err(|_|MergeIntelligenceError::InvalidSignature)?;
    let signature=Signature::from_slice(&sig).map_err(|_|MergeIntelligenceError::InvalidSignature)?;
    let payload=serde_json::to_vec(&doc.payload).expect("serializable");
    key.verify(&payload,&signature).map_err(|_|MergeIntelligenceError::InvalidSignature)?;
    Ok(VerifiedResolution{payload:doc.payload.clone(),signer_key_id:doc.signer_key_id.clone(),document_sha256:sha_bytes(&serde_json::to_vec(doc).expect("serializable"))})
}

pub fn consensus_gate(ev:&ConsensusEvidence) -> Result<(),MergeIntelligenceError> {
    if !valid_hash(&ev.source_state_sha256) { return Err(MergeIntelligenceError::InvalidHash); }
    if !ev.unresolved_conflicts.is_empty() || !ev.qa_passed || !ev.architecture_passed || !ev.contract_gate_passed || !ev.replay_passed || !ev.mutation_passed { return Err(MergeIntelligenceError::EvidenceGateFailed); }
    if !ev.security_passed || ev.highest_security_severity>=Severity::High { return Err(MergeIntelligenceError::SecurityGateFailed); }
    Ok(())
}

pub fn merge_bundle_hash(plan:&MergeOrderPlan, replay:&IntegrationReplayEvidence, mutation:&MutationEvidence, resolutions:&BTreeMap<Uuid,VerifiedResolution>) -> Result<String,MergeIntelligenceError> {
    if plan.base_source_sha256!=replay.base_source_sha256 || plan.base_source_sha256!=mutation.source_state_sha256 { return Err(MergeIntelligenceError::SourceStateMismatch); }
    let canonical=serde_json::to_vec(&(plan,replay,mutation,resolutions)).expect("serializable");
    Ok(sha_bytes(&canonical))
}

pub fn no_majority_override(conflicts:&[SemanticConflict], resolutions:&BTreeMap<Uuid,VerifiedResolution>) -> Result<(),MergeIntelligenceError> {
    // Evidence is authoritative. Counts/votes are intentionally ignored.
    contract_gate(conflicts,resolutions)
}

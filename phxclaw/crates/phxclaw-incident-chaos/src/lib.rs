use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all="snake_case")]
pub enum Severity { Low, Medium, High, Critical }
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum IncidentState { Detected, Declared, Triaged, Mitigating, Recovering, Verifying, Resolved, Postmortem }
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum Environment { Sandbox, Test, Staging, Production }
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum FaultKind { ProviderUnavailable, AddedLatency, RateLimitPressure, QueueSaturation, ModelError, DatabaseReadOnly, NetworkPartitionSimulated }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncidentPolicy {
    pub tenant_uuid: Uuid, pub policy_uuid: Uuid, pub source_state_sha256: String, pub starts_at_unix:i64, pub expires_at_unix:i64,
    pub approval_required_at_or_above: Severity, pub leader_fence_required_at_or_above: Severity,
    pub allowed_action_capabilities:BTreeSet<String>, pub forbidden_action_capabilities:BTreeSet<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedIncidentPolicy { pub signer_id:String, pub signature_hex:String, pub policy:IncidentPolicy }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifiedIncidentPolicy { pub signer_id:String, pub policy:IncidentPolicy, pub policy_sha256:String }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncidentRecord { pub incident_uuid:Uuid, pub tenant_uuid:Uuid, pub severity:Severity, pub title:String, pub detection_evidence_sha256:String, pub state:IncidentState }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncidentActionPlan { pub action_uuid:Uuid, pub incident_uuid:Uuid, pub capability:String, pub reversible:bool, pub approval_uuid:Option<Uuid>, pub leader_epoch:Option<u64>, pub fencing_token:Option<u64>, pub input_sha256:String }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChaosPolicy {
    pub tenant_uuid:Uuid, pub policy_uuid:Uuid, pub source_state_sha256:String, pub starts_at_unix:i64, pub expires_at_unix:i64,
    pub allowed_environments:BTreeSet<Environment>, pub production_human_approval_required:bool, pub production_leader_fence_required:bool,
    pub max_production_blast_radius_basis_points:u16, pub allowed_faults:BTreeSet<FaultKind>, pub forbidden_capabilities:BTreeSet<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedChaosPolicy { pub signer_id:String, pub signature_hex:String, pub policy:ChaosPolicy }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifiedChaosPolicy { pub signer_id:String, pub policy:ChaosPolicy, pub policy_sha256:String }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChaosExperimentPlan {
    pub experiment_uuid:Uuid, pub tenant_uuid:Uuid, pub environment:Environment, pub fault:FaultKind,
    pub blast_radius_basis_points:u16, pub steady_state_sha256:String, pub abort_conditions_sha256:String,
    pub approval_uuid:Option<Uuid>, pub leader_epoch:Option<u64>, pub fencing_token:Option<u64>, pub recovery_runbook_uuid:Uuid,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryEvidence { pub experiment_uuid:Uuid, pub before_sha256:String, pub after_sha256:String, pub steady_state_restored:bool, pub evidence_sha256:String }

#[derive(Error, Debug, PartialEq, Eq)]
pub enum GuardError {
    #[error("untrusted signer")] UntrustedSigner, #[error("invalid signature")] InvalidSignature, #[error("inactive policy")] InactivePolicy,
    #[error("action capability is denied")] CapabilityDenied, #[error("approval is required")] ApprovalRequired, #[error("leader fencing is required")] FenceRequired,
    #[error("environment is not allowed")] EnvironmentDenied, #[error("fault is not allowed")] FaultDenied, #[error("blast radius exceeds policy")] BlastRadiusExceeded,
    #[error("recovery evidence did not restore steady state")] RecoveryNotVerified, #[error("invalid hash")] InvalidHash,
}
fn hex64(s:&str)->bool{s.len()==64 && s.bytes().all(|b|b.is_ascii_hexdigit())}
fn canonical_hash<T:Serialize>(value:&T)->String{format!("{:x}",Sha256::digest(serde_json::to_vec(value).expect("serializable")))}
fn verify_sig<T:Serialize>(signer_id:&str, signature_hex:&str, value:&T, trusted:&BTreeMap<String,VerifyingKey>)->Result<String,GuardError>{
    let key=trusted.get(signer_id).ok_or(GuardError::UntrustedSigner)?; let bytes=serde_json::to_vec(value).expect("serializable");
    let raw=hex::decode(signature_hex).map_err(|_|GuardError::InvalidSignature)?; let sig=Signature::from_slice(&raw).map_err(|_|GuardError::InvalidSignature)?;
    key.verify(&bytes,&sig).map_err(|_|GuardError::InvalidSignature)?; Ok(format!("{:x}",Sha256::digest(bytes)))
}
pub fn verify_incident_policy(doc:&SignedIncidentPolicy,trusted:&BTreeMap<String,VerifyingKey>,now:i64)->Result<VerifiedIncidentPolicy,GuardError>{
    if now<doc.policy.starts_at_unix || now>doc.policy.expires_at_unix {return Err(GuardError::InactivePolicy)}
    if !hex64(&doc.policy.source_state_sha256){return Err(GuardError::InvalidHash)}
    let h=verify_sig(&doc.signer_id,&doc.signature_hex,&doc.policy,trusted)?; Ok(VerifiedIncidentPolicy{signer_id:doc.signer_id.clone(),policy:doc.policy.clone(),policy_sha256:h})
}
pub fn authorize_incident_action(policy:&VerifiedIncidentPolicy,incident:&IncidentRecord,action:&IncidentActionPlan)->Result<(),GuardError>{
    if incident.tenant_uuid!=policy.policy.tenant_uuid || action.incident_uuid!=incident.incident_uuid{return Err(GuardError::CapabilityDenied)}
    if policy.policy.forbidden_action_capabilities.contains(&action.capability) || !policy.policy.allowed_action_capabilities.contains(&action.capability){return Err(GuardError::CapabilityDenied)}
    if incident.severity>=policy.policy.approval_required_at_or_above && action.approval_uuid.is_none(){return Err(GuardError::ApprovalRequired)}
    if incident.severity>=policy.policy.leader_fence_required_at_or_above && (action.leader_epoch.is_none() || action.fencing_token.is_none()){return Err(GuardError::FenceRequired)}
    if !hex64(&action.input_sha256){return Err(GuardError::InvalidHash)}
    Ok(())
}
pub fn verify_chaos_policy(doc:&SignedChaosPolicy,trusted:&BTreeMap<String,VerifyingKey>,now:i64)->Result<VerifiedChaosPolicy,GuardError>{
    if now<doc.policy.starts_at_unix || now>doc.policy.expires_at_unix {return Err(GuardError::InactivePolicy)}
    if !hex64(&doc.policy.source_state_sha256){return Err(GuardError::InvalidHash)}
    let h=verify_sig(&doc.signer_id,&doc.signature_hex,&doc.policy,trusted)?; Ok(VerifiedChaosPolicy{signer_id:doc.signer_id.clone(),policy:doc.policy.clone(),policy_sha256:h})
}
pub fn authorize_chaos(policy:&VerifiedChaosPolicy,plan:&ChaosExperimentPlan)->Result<(),GuardError>{
    if plan.tenant_uuid!=policy.policy.tenant_uuid || !policy.policy.allowed_environments.contains(&plan.environment){return Err(GuardError::EnvironmentDenied)}
    if !policy.policy.allowed_faults.contains(&plan.fault){return Err(GuardError::FaultDenied)}
    if matches!(plan.environment,Environment::Production){
        if plan.blast_radius_basis_points>policy.policy.max_production_blast_radius_basis_points{return Err(GuardError::BlastRadiusExceeded)}
        if policy.policy.production_human_approval_required && plan.approval_uuid.is_none(){return Err(GuardError::ApprovalRequired)}
        if policy.policy.production_leader_fence_required && (plan.leader_epoch.is_none()||plan.fencing_token.is_none()){return Err(GuardError::FenceRequired)}
    }
    if !hex64(&plan.steady_state_sha256)||!hex64(&plan.abort_conditions_sha256){return Err(GuardError::InvalidHash)}
    Ok(())
}
pub fn verify_recovery(e:&RecoveryEvidence)->Result<String,GuardError>{
    if !e.steady_state_restored{return Err(GuardError::RecoveryNotVerified)}
    if !hex64(&e.before_sha256)||!hex64(&e.after_sha256)||!hex64(&e.evidence_sha256){return Err(GuardError::InvalidHash)}
    Ok(canonical_hash(e))
}

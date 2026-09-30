use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use phxclaw_ai_portfolio::VerifiedPortfolioDocument;
use phxclaw_ai_sre::{BudgetState, CostForecast, IncidentDetection, IncidentSeverity, VerifiedSrePolicyDocument};
use phxclaw_ha_control_plane::{authorize_mutation, LeaderLease, OperationFence};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutopilotActionKind { Throttle, DrainProvider, ResumeProvider, DecreaseConcurrency, IncreaseConcurrency, RebalanceShare, RequestCapacityIncrease }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutopilotAction {
    pub kind: AutopilotActionKind,
    pub slot_uuid: Option<Uuid>,
    pub provider_uuid: Uuid,
    pub model_id: Option<String>,
    pub to_provider_uuid: Option<Uuid>,
    pub to_model_id: Option<String>,
    pub magnitude_basis_points: u16,
    pub projected_cost_delta_micro_usd: i64,
    pub reversible: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutopilotPlanSpec {
    pub tenant_uuid: Uuid,
    pub plan_uuid: Uuid,
    pub sre_policy_document_sha256: String,
    pub portfolio_document_sha256: String,
    pub source_state_sha256: String,
    pub trigger_evidence_hashes: Vec<String>,
    pub created_at_unix: i64,
    pub expires_at_unix: i64,
    pub automatic_allowed: bool,
    pub requires_human_approval: bool,
    pub actions: Vec<AutopilotAction>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedAutopilotPlanDocument { pub document_sha256: String, pub signer_id: String, pub signature_hex: String, pub spec: AutopilotPlanSpec }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifiedAutopilotPlanDocument { pub document_sha256: String, pub signer_id: String, pub spec: AutopilotPlanSpec }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutopilotExecution { pub execution_uuid: Uuid, pub plan_sha256: String, pub controller_uuid: Uuid, pub leader_epoch: u64, pub source_state_sha256: String, pub result_state_sha256: String, pub executed_at_unix: i64, pub execution_sha256: String }

#[derive(Error, Debug, PartialEq, Eq)]
pub enum AutopilotError {
    #[error("invalid autopilot plan")] InvalidPlan,
    #[error("invalid hash")] InvalidHash,
    #[error("signer is not trusted")] UntrustedSigner,
    #[error("invalid signature")] InvalidSignature,
    #[error("plan references a target outside signed portfolio")] TargetOutsidePortfolio,
    #[error("automatic action exceeds signed SRE bounds")] AutomaticBoundsExceeded,
    #[error("automatic action would increase cost without healthy headroom")] CostHeadroom,
    #[error("human approval is required")] HumanApprovalRequired,
    #[error("HA mutation authorization failed")] MutationUnauthorized,
    #[error("autopilot plan expired")] PlanExpired,
}

fn valid_sha(s: &str) -> bool { s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()) }
fn sha(bytes: &[u8]) -> String { hex::encode(Sha256::digest(bytes)) }
fn canonical_spec(spec: &AutopilotPlanSpec) -> Vec<u8> { let mut x = spec.clone(); x.trigger_evidence_hashes.sort(); serde_json::to_vec(&x).expect("autopilot plan serialization") }
fn portfolio_members(portfolio: &VerifiedPortfolioDocument) -> BTreeSet<(Uuid, String)> { portfolio.spec.slots.iter().flat_map(|s| s.members.iter().map(|m| (m.candidate.provider_uuid, m.candidate.model_id.clone()))).collect() }
fn portfolio_providers(portfolio: &VerifiedPortfolioDocument) -> BTreeSet<Uuid> { portfolio.spec.slots.iter().flat_map(|s| s.members.iter().map(|m| m.candidate.provider_uuid)).collect() }
fn slot_has(portfolio: &VerifiedPortfolioDocument, slot_uuid: Uuid, p: Uuid, m: &str) -> bool { portfolio.spec.slots.iter().find(|s| s.slot_uuid == slot_uuid).map(|s| s.members.iter().any(|x| x.candidate.provider_uuid == p && x.candidate.model_id == m)).unwrap_or(false) }

fn projected_cost_delta_bps(cost: &CostForecast, delta: i64) -> u16 {
    if delta <= 0 { return 0; }
    if cost.budget_limit_micro_usd == 0 { return 10_000; }
    ((delta as u128 * 10_000) / cost.budget_limit_micro_usd as u128).min(10_000) as u16
}

pub fn recompute_automatic(policy: &VerifiedSrePolicyDocument, portfolio: &VerifiedPortfolioDocument, cost: &CostForecast, incident: &IncidentDetection, actions: &[AutopilotAction]) -> Result<bool, AutopilotError> {
    if policy.spec.portfolio_document_sha256 != portfolio.document_sha256 || cost.account_uuid != policy.spec.cost.budget_account_uuid || !valid_sha(&cost.forecast_sha256) || !valid_sha(&incident.detection_sha256) { return Err(AutopilotError::InvalidPlan); }
    let members = portfolio_members(portfolio);
    let providers = portfolio_providers(portfolio);
    if actions.is_empty() { return Err(AutopilotError::InvalidPlan); }
    let mut automatic = true;
    for a in actions {
        if a.magnitude_basis_points > 10_000 || !providers.contains(&a.provider_uuid) { return Err(AutopilotError::TargetOutsidePortfolio); }
        if let Some(model) = &a.model_id { if !members.contains(&(a.provider_uuid, model.clone())) { return Err(AutopilotError::TargetOutsidePortfolio); } }
        if let Some(tp) = a.to_provider_uuid { if !providers.contains(&tp) { return Err(AutopilotError::TargetOutsidePortfolio); } }
        match a.kind {
            AutopilotActionKind::Throttle | AutopilotActionKind::DrainProvider | AutopilotActionKind::ResumeProvider => { if !a.reversible { automatic = false; } }
            AutopilotActionKind::DecreaseConcurrency => { if !a.reversible || a.magnitude_basis_points > policy.spec.autopilot.max_concurrency_decrease_basis_points { automatic = false; } }
            AutopilotActionKind::IncreaseConcurrency => {
                if !a.reversible || a.magnitude_basis_points > policy.spec.autopilot.max_concurrency_increase_basis_points { automatic = false; }
                if a.projected_cost_delta_micro_usd > 0 && (cost.state != BudgetState::Healthy || projected_cost_delta_bps(cost, a.projected_cost_delta_micro_usd) > policy.spec.autopilot.max_auto_projected_cost_increase_basis_points) { automatic = false; }
            }
            AutopilotActionKind::RebalanceShare => {
                let slot = a.slot_uuid.ok_or(AutopilotError::InvalidPlan)?;
                let fm = a.model_id.as_deref().ok_or(AutopilotError::InvalidPlan)?;
                let tp = a.to_provider_uuid.ok_or(AutopilotError::InvalidPlan)?;
                let tm = a.to_model_id.as_deref().ok_or(AutopilotError::InvalidPlan)?;
                if !slot_has(portfolio, slot, a.provider_uuid, fm) || !slot_has(portfolio, slot, tp, tm) { return Err(AutopilotError::TargetOutsidePortfolio); }
                if !a.reversible || a.magnitude_basis_points > policy.spec.autopilot.max_share_move_basis_points { automatic = false; }
            }
            AutopilotActionKind::RequestCapacityIncrease => {
                if !policy.spec.autopilot.allow_auto_capacity_purchase || !a.reversible || cost.state != BudgetState::Healthy || projected_cost_delta_bps(cost, a.projected_cost_delta_micro_usd) > policy.spec.autopilot.max_auto_projected_cost_increase_basis_points { automatic = false; }
            }
        }
    }
    if incident.severity == IncidentSeverity::Info && actions.iter().any(|a| matches!(a.kind, AutopilotActionKind::DrainProvider | AutopilotActionKind::RequestCapacityIncrease)) { automatic = false; }
    Ok(automatic)
}

pub fn verify_signed_plan(now_unix: i64, doc: &SignedAutopilotPlanDocument, trusted_keys: &BTreeMap<String, String>, policy: &VerifiedSrePolicyDocument, portfolio: &VerifiedPortfolioDocument, cost: &CostForecast, incident: &IncidentDetection) -> Result<VerifiedAutopilotPlanDocument, AutopilotError> {
    let s = &doc.spec;
    if now_unix < policy.spec.starts_at_unix || now_unix > policy.spec.expires_at_unix { return Err(AutopilotError::InvalidPlan); }
    if s.tenant_uuid != policy.spec.tenant_uuid || s.sre_policy_document_sha256 != policy.document_sha256 || s.portfolio_document_sha256 != portfolio.document_sha256 || !valid_sha(&s.source_state_sha256) || s.created_at_unix > s.expires_at_unix || now_unix < s.created_at_unix || now_unix > s.expires_at_unix || s.trigger_evidence_hashes.is_empty() || s.trigger_evidence_hashes.iter().any(|h| !valid_sha(h)) { return Err(AutopilotError::InvalidPlan); }
    if !s.trigger_evidence_hashes.contains(&cost.forecast_sha256) || !s.trigger_evidence_hashes.contains(&incident.detection_sha256) { return Err(AutopilotError::InvalidPlan); }
    let automatic = recompute_automatic(policy, portfolio, cost, incident, &s.actions)?;
    if automatic != s.automatic_allowed || s.requires_human_approval == automatic { return Err(AutopilotError::AutomaticBoundsExceeded); }
    let body = canonical_spec(s);
    if sha(&body) != doc.document_sha256 || !valid_sha(&doc.document_sha256) { return Err(AutopilotError::InvalidHash); }
    let key_hex = trusted_keys.get(&doc.signer_id).ok_or(AutopilotError::UntrustedSigner)?;
    let kb = hex::decode(key_hex).map_err(|_| AutopilotError::InvalidSignature)?;
    let ka: [u8; 32] = kb.try_into().map_err(|_| AutopilotError::InvalidSignature)?;
    let vk = VerifyingKey::from_bytes(&ka).map_err(|_| AutopilotError::InvalidSignature)?;
    let sb = hex::decode(&doc.signature_hex).map_err(|_| AutopilotError::InvalidSignature)?;
    let sa: [u8; 64] = sb.try_into().map_err(|_| AutopilotError::InvalidSignature)?;
    vk.verify(&body, &Signature::from_bytes(&sa)).map_err(|_| AutopilotError::InvalidSignature)?;
    Ok(VerifiedAutopilotPlanDocument { document_sha256: doc.document_sha256.clone(), signer_id: doc.signer_id.clone(), spec: s.clone() })
}

pub fn authorize_automatic_execution(plan: &VerifiedAutopilotPlanDocument, lease: &LeaderLease, fence: &OperationFence, current_state_sha256: &str, result_state_sha256: &str, db_now: DateTime<Utc>) -> Result<AutopilotExecution, AutopilotError> {
    if !plan.spec.automatic_allowed || plan.spec.requires_human_approval { return Err(AutopilotError::HumanApprovalRequired); }
    if db_now.timestamp() > plan.spec.expires_at_unix { return Err(AutopilotError::PlanExpired); }
    if plan.spec.source_state_sha256 != current_state_sha256 || !valid_sha(result_state_sha256) { return Err(AutopilotError::InvalidHash); }
    authorize_mutation(lease, fence, current_state_sha256, db_now).map_err(|_| AutopilotError::MutationUnauthorized)?;
    let execution_uuid = Uuid::now_v7();
    let body = serde_json::to_vec(&(execution_uuid, &plan.document_sha256, lease.holder_uuid, lease.epoch, current_state_sha256, result_state_sha256, db_now.timestamp())).expect("autopilot execution serialization");
    Ok(AutopilotExecution { execution_uuid, plan_sha256: plan.document_sha256.clone(), controller_uuid: lease.holder_uuid, leader_epoch: lease.epoch, source_state_sha256: current_state_sha256.into(), result_state_sha256: result_state_sha256.into(), executed_at_unix: db_now.timestamp(), execution_sha256: sha(&body) })
}

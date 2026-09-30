use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum EngineeringStage {
    Intake,
    Research,
    CodeMap,
    Hypothesis,
    Plan,
    Implement,
    Test,
    Review,
    Explain,
    Deliver,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskClass {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StageOutcome {
    Completed,
    Failed,
    RolledBack,
    Skipped,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowSpec {
    pub workflow_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub objective: String,
    pub source_state_sha256: String,
    pub policy_sha256: String,
    pub skill_catalog_sha256: String,
    pub risk: RiskClass,
    pub max_attempts_per_stage: u8,
    pub allowed_skill_ids: BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageEvidence {
    pub evidence_uuid: Uuid,
    pub workflow_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub stage: EngineeringStage,
    pub attempt: u8,
    pub source_state_sha256: String,
    pub input_sha256: String,
    pub output_sha256: String,
    pub evidence_refs: Vec<String>,
    pub actor_uuid: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Checkpoint {
    pub checkpoint_uuid: Uuid,
    pub workflow_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub stage: EngineeringStage,
    pub source_state_sha256: String,
    pub workspace_state_sha256: String,
    pub git_commit: Option<String>,
    pub fencing_token: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransitionEvent {
    pub event_uuid: Uuid,
    pub workflow_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub stage: EngineeringStage,
    pub outcome: StageOutcome,
    pub attempt: u8,
    pub evidence_uuid: Option<Uuid>,
    pub checkpoint_uuid: Option<Uuid>,
    pub idempotency_key: String,
    pub fencing_token: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalProof {
    pub approval_uuid: Uuid,
    pub workflow_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub stage: EngineeringStage,
    pub approver_uuid: Uuid,
    pub actor_uuid: Uuid,
    pub plan_sha256: String,
    pub approved: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureAction {
    Retry,
    RollbackThenRetry,
    Stop,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum WorkflowError {
    #[error("invalid sha256")]
    InvalidHash,
    #[error("source-state mismatch")]
    SourceStateMismatch,
    #[error("stage order violation")]
    StageOrder,
    #[error("evidence missing")]
    MissingEvidence,
    #[error("approval required")]
    ApprovalRequired,
    #[error("self approval forbidden")]
    SelfApprovalForbidden,
    #[error("checkpoint required")]
    CheckpointRequired,
    #[error("stale fencing token")]
    StaleFencing,
    #[error("attempt limit exceeded")]
    AttemptLimit,
    #[error("skill not allowed")]
    SkillNotAllowed,
    #[error("external delivery side effect not approved")]
    DeliveryNotApproved,
}

fn valid_hash(v: &str) -> bool {
    v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn stage_order() -> &'static [EngineeringStage] {
    &[
        EngineeringStage::Intake,
        EngineeringStage::Research,
        EngineeringStage::CodeMap,
        EngineeringStage::Hypothesis,
        EngineeringStage::Plan,
        EngineeringStage::Implement,
        EngineeringStage::Test,
        EngineeringStage::Review,
        EngineeringStage::Explain,
        EngineeringStage::Deliver,
    ]
}

pub fn workflow_hash(spec: &WorkflowSpec) -> Result<String, WorkflowError> {
    if !valid_hash(&spec.source_state_sha256)
        || !valid_hash(&spec.policy_sha256)
        || !valid_hash(&spec.skill_catalog_sha256)
    {
        return Err(WorkflowError::InvalidHash);
    }
    let b = serde_json::to_vec(spec).expect("serializable");
    Ok(format!("{:x}", Sha256::digest(b)))
}

pub fn verify_evidence(spec: &WorkflowSpec, ev: &StageEvidence) -> Result<(), WorkflowError> {
    if ev.workflow_uuid != spec.workflow_uuid
        || ev.tenant_uuid != spec.tenant_uuid
        || ev.source_state_sha256 != spec.source_state_sha256
    {
        return Err(WorkflowError::SourceStateMismatch);
    }
    if !valid_hash(&ev.input_sha256) || !valid_hash(&ev.output_sha256) {
        return Err(WorkflowError::InvalidHash);
    }
    if ev.evidence_refs.is_empty() && !matches!(ev.stage, EngineeringStage::Intake) {
        return Err(WorkflowError::MissingEvidence);
    }
    if ev.attempt == 0 || ev.attempt > spec.max_attempts_per_stage {
        return Err(WorkflowError::AttemptLimit);
    }
    Ok(())
}

pub fn next_stage(completed: &BTreeSet<EngineeringStage>) -> Option<EngineeringStage> {
    stage_order()
        .iter()
        .copied()
        .find(|s| !completed.contains(s))
}

pub fn can_enter(
    stage: EngineeringStage,
    completed: &BTreeSet<EngineeringStage>,
) -> Result<(), WorkflowError> {
    let idx = stage_order()
        .iter()
        .position(|x| *x == stage)
        .expect("known stage");
    if stage_order()[..idx].iter().any(|s| !completed.contains(s)) {
        return Err(WorkflowError::StageOrder);
    }
    Ok(())
}

pub fn stage_requires_checkpoint(stage: EngineeringStage) -> bool {
    matches!(stage, EngineeringStage::Implement)
}

pub fn approval_required(
    stage: EngineeringStage,
    risk: RiskClass,
    external_side_effect: bool,
) -> bool {
    if external_side_effect {
        return true;
    }
    matches!(
        risk,
        RiskClass::Medium | RiskClass::High | RiskClass::Critical
    ) && matches!(
        stage,
        EngineeringStage::Implement | EngineeringStage::Deliver
    )
}

pub fn verify_approval(
    p: &ApprovalProof,
    stage: EngineeringStage,
    critical_independent_review: bool,
) -> Result<(), WorkflowError> {
    if !p.approved || p.stage != stage || !valid_hash(&p.plan_sha256) {
        return Err(WorkflowError::ApprovalRequired);
    }
    if critical_independent_review && p.approver_uuid == p.actor_uuid {
        return Err(WorkflowError::SelfApprovalForbidden);
    }
    Ok(())
}

pub fn verify_transition_fencing(
    current: u64,
    event: &TransitionEvent,
) -> Result<(), WorkflowError> {
    if event.fencing_token < current {
        return Err(WorkflowError::StaleFencing);
    }
    Ok(())
}

pub fn failure_action(
    stage: EngineeringStage,
    attempt: u8,
    max_attempts: u8,
    reversible: bool,
    checkpoint_verified: bool,
) -> FailureAction {
    if attempt >= max_attempts {
        return FailureAction::Stop;
    }
    if matches!(stage, EngineeringStage::Implement) {
        if reversible && checkpoint_verified {
            FailureAction::RollbackThenRetry
        } else {
            FailureAction::Stop
        }
    } else if reversible {
        FailureAction::Retry
    } else {
        FailureAction::Stop
    }
}

pub fn validate_skill_selection(
    spec: &WorkflowSpec,
    selected: &[String],
) -> Result<(), WorkflowError> {
    if selected.iter().any(|s| !spec.allowed_skill_ids.contains(s)) {
        return Err(WorkflowError::SkillNotAllowed);
    }
    Ok(())
}

pub fn deterministic_stage_digest(
    stage: EngineeringStage,
    input: &BTreeMap<String, String>,
    evidence: &[String],
) -> String {
    let b = serde_json::to_vec(&(stage, input, evidence)).expect("serializable");
    format!("{:x}", Sha256::digest(b))
}

pub fn validate_delivery(
    external_publish: bool,
    approval: Option<&ApprovalProof>,
) -> Result<(), WorkflowError> {
    if external_publish && approval.is_none() {
        return Err(WorkflowError::DeliveryNotApproved);
    }
    Ok(())
}

pub fn stages_for_practical_change() -> Vec<EngineeringStage> {
    stage_order().to_vec()
}

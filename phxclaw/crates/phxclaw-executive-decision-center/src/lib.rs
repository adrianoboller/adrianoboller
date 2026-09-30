//! PhxClaw v0.49 — Executive Decision Center.
//!
//! Produces governed executive decisions from v0.48 Control Tower signals and v0.46
//! predictive intelligence. It is simulation/decision support, not a mutation bypass.
//! Any operational execution is delegated to v0.47 Autonomous Project Supervisor.

use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use thiserror::Error;
use uuid::Uuid;

pub const DECISION_RULE: &str =
    "observe -> simulate -> compare -> approve -> delegate to supervisor";
pub const HARD_GATE_RULE: &str =
    "never override security/privacy/release/knowledge/fencing/hard-budget gates";
pub const EXECUTION_RULE: &str = "decision-center does not mutate project state directly";

#[derive(Debug, Error)]
pub enum DecisionError {
    #[error("stale source or forecast")]
    Stale,
    #[error("source-state mismatch")]
    SourceStateMismatch,
    #[error("tenant/project mismatch")]
    ScopeMismatch,
    #[error("hard gate violation")]
    HardGate,
    #[error("approval required")]
    ApprovalRequired,
    #[error("approval signer not trusted")]
    UntrustedSigner,
    #[error("approval signature invalid")]
    InvalidSignature,
    #[error("operational execution must be delegated to v0.47 supervisor")]
    DelegateRequired,
    #[error("scenario invalid: {0}")]
    InvalidScenario(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    Reprioritize,
    ReassignAgent,
    ReduceWip,
    ResequenceTasks,
    ModelRouteChange,
    ResourceLeveling,
    ScopeChange,
    BaselineChange,
    DeadlineChange,
    BudgetIncrease,
    ProductionChange,
    DestructiveChange,
}

impl DecisionKind {
    pub fn requires_explicit_approval(self) -> bool {
        matches!(
            self,
            Self::ScopeChange
                | Self::BaselineChange
                | Self::DeadlineChange
                | Self::BudgetIncrease
                | Self::ProductionChange
                | Self::DestructiveChange
        )
    }
    pub fn reversible_by_default(self) -> bool {
        matches!(
            self,
            Self::Reprioritize
                | Self::ReassignAgent
                | Self::ReduceWip
                | Self::ResequenceTasks
                | Self::ModelRouteChange
                | Self::ResourceLeveling
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionCase {
    pub case_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub source_state_sha256: String,
    pub portfolio_snapshot_sha256: String,
    pub trigger_kind: String,
    pub trigger_evidence_sha256: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactVector {
    /// Negative means faster, positive means later.
    pub deadline_delta_days: f64,
    /// Negative means savings, positive means more cost.
    pub cost_delta_usd: f64,
    /// Negative means less risk, positive means more risk.
    pub risk_delta: f64,
    /// Positive means better quality.
    pub quality_delta: f64,
    /// Positive means more available capacity.
    pub capacity_delta: f64,
    /// 0..1 probability that predicted effect materializes.
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatIfScenario {
    pub scenario_uuid: Uuid,
    pub case_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub source_state_sha256: String,
    pub kind: DecisionKind,
    pub title: String,
    pub parameters: serde_json::Value,
    pub impact: ImpactVector,
    pub predicted_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub model_evidence_sha256: String,
    pub scenario_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectiveWeights {
    pub deadline: f64,
    pub cost: f64,
    pub risk: f64,
    pub quality: f64,
    pub capacity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionConstraints {
    pub max_budget_increase_usd: f64,
    pub max_deadline_slip_days: f64,
    pub max_risk_increase: f64,
    pub min_quality_delta: f64,
    pub security_gate_passed: bool,
    pub privacy_gate_passed: bool,
    pub release_gate_passed: bool,
    pub knowledge_gate_passed: bool,
    pub fencing_valid: bool,
    pub hard_budget_ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioEvaluation {
    pub scenario_uuid: Uuid,
    pub feasible: bool,
    pub approval_required: bool,
    pub utility_score: f64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitivityPoint {
    pub variable: String,
    pub delta_pct: f64,
    pub resulting_utility: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedApproval {
    pub approval_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub case_uuid: Uuid,
    pub scenario_uuid: Uuid,
    pub source_state_sha256: String,
    pub signer_uuid: Uuid,
    pub public_key_hex: String,
    pub signature_hex: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedApproval {
    pub document: SignedApproval,
    pub approval_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionExecutionEnvelope {
    pub execution_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub case_uuid: Uuid,
    pub scenario_uuid: Uuid,
    pub supervisor_plan_uuid: Uuid,
    pub source_state_sha256: String,
    pub scenario_sha256: String,
    pub approval_sha256: Option<String>,
    pub controller_epoch: u64,
    pub fencing_token: u64,
    pub evidence_sha256: String,
}

pub fn scenario_hash(s: &WhatIfScenario) -> String {
    let canonical = serde_json::json!({
        "scenario_uuid":s.scenario_uuid,
        "case_uuid":s.case_uuid,
        "tenant_uuid":s.tenant_uuid,
        "project_uuid":s.project_uuid,
        "source_state_sha256":s.source_state_sha256,
        "kind":s.kind,
        "title":s.title,
        "parameters":s.parameters,
        "impact":s.impact,
        "predicted_at":s.predicted_at,
        "expires_at":s.expires_at,
        "model_evidence_sha256":s.model_evidence_sha256,
    });
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&canonical).expect("scenario json"))
    )
}

pub fn validate_case(case: &DecisionCase, now: DateTime<Utc>) -> Result<(), DecisionError> {
    if case.expires_at < now {
        return Err(DecisionError::Stale);
    }
    if case.source_state_sha256.len() != 64 || case.trigger_evidence_sha256.len() != 64 {
        return Err(DecisionError::InvalidScenario(
            "invalid evidence hash".into(),
        ));
    }
    Ok(())
}

pub fn evaluate(
    s: &WhatIfScenario,
    w: &ObjectiveWeights,
    c: &DecisionConstraints,
    now: DateTime<Utc>,
) -> Result<ScenarioEvaluation, DecisionError> {
    if s.expires_at < now {
        return Err(DecisionError::Stale);
    }
    if scenario_hash(s) != s.scenario_sha256 {
        return Err(DecisionError::InvalidScenario(
            "scenario hash mismatch".into(),
        ));
    }
    let hard_ok = c.security_gate_passed
        && c.privacy_gate_passed
        && c.release_gate_passed
        && c.knowledge_gate_passed
        && c.fencing_valid
        && c.hard_budget_ok;
    if !hard_ok {
        return Err(DecisionError::HardGate);
    }
    let mut reasons = Vec::new();
    if s.impact.cost_delta_usd > c.max_budget_increase_usd {
        reasons.push("budget_constraint".into());
    }
    if s.impact.deadline_delta_days > c.max_deadline_slip_days {
        reasons.push("deadline_constraint".into());
    }
    if s.impact.risk_delta > c.max_risk_increase {
        reasons.push("risk_constraint".into());
    }
    if s.impact.quality_delta < c.min_quality_delta {
        reasons.push("quality_constraint".into());
    }
    let feasible = reasons.is_empty();
    let utility = (-w.deadline * s.impact.deadline_delta_days)
        + (-w.cost * s.impact.cost_delta_usd / 1000.0)
        + (-w.risk * s.impact.risk_delta * 100.0)
        + (w.quality * s.impact.quality_delta * 100.0)
        + (w.capacity * s.impact.capacity_delta * 100.0);
    Ok(ScenarioEvaluation {
        scenario_uuid: s.scenario_uuid,
        feasible,
        approval_required: s.kind.requires_explicit_approval(),
        utility_score: utility * s.impact.confidence.clamp(0.0, 1.0),
        reasons,
    })
}

pub fn compare(
    scenarios: &[WhatIfScenario],
    w: &ObjectiveWeights,
    c: &DecisionConstraints,
    now: DateTime<Utc>,
) -> Result<Vec<ScenarioEvaluation>, DecisionError> {
    let mut out = Vec::new();
    for s in scenarios {
        out.push(evaluate(s, w, c, now)?);
    }
    out.sort_by(|a, b| {
        b.feasible
            .cmp(&a.feasible)
            .then_with(|| {
                b.utility_score
                    .partial_cmp(&a.utility_score)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| a.scenario_uuid.cmp(&b.scenario_uuid))
    });
    Ok(out)
}

pub fn sensitivity(s: &WhatIfScenario, w: &ObjectiveWeights) -> Vec<SensitivityPoint> {
    let base = (-w.deadline * s.impact.deadline_delta_days)
        + (-w.cost * s.impact.cost_delta_usd / 1000.0)
        + (-w.risk * s.impact.risk_delta * 100.0)
        + (w.quality * s.impact.quality_delta * 100.0)
        + (w.capacity * s.impact.capacity_delta * 100.0);
    [-0.20, -0.10, 0.10, 0.20]
        .iter()
        .flat_map(|d| {
            ["confidence", "cost", "deadline"]
                .iter()
                .map(move |v| SensitivityPoint {
                    variable: (*v).into(),
                    delta_pct: *d,
                    resulting_utility: match *v {
                        "confidence" => base * (s.impact.confidence * (1.0 + d)).clamp(0.0, 1.0),
                        "cost" => base - w.cost * (s.impact.cost_delta_usd * d) / 1000.0,
                        _ => base - w.deadline * (s.impact.deadline_delta_days * d),
                    },
                })
        })
        .collect()
}

pub fn verify_approval(
    a: SignedApproval,
    trusted_signers: &[(Uuid, VerifyingKey)],
    now: DateTime<Utc>,
) -> Result<VerifiedApproval, DecisionError> {
    if a.expires_at < now || a.issued_at > now + Duration::minutes(5) {
        return Err(DecisionError::Stale);
    }
    let key = trusted_signers
        .iter()
        .find(|(id, _)| *id == a.signer_uuid)
        .map(|(_, k)| k)
        .ok_or(DecisionError::UntrustedSigner)?;
    if hex::encode(key.as_bytes()) != a.public_key_hex.to_lowercase() {
        return Err(DecisionError::UntrustedSigner);
    }
    let payload = serde_json::json!({
        "approval_uuid":a.approval_uuid,"tenant_uuid":a.tenant_uuid,"project_uuid":a.project_uuid,
        "case_uuid":a.case_uuid,"scenario_uuid":a.scenario_uuid,"source_state_sha256":a.source_state_sha256,
        "signer_uuid":a.signer_uuid,"issued_at":a.issued_at,"expires_at":a.expires_at
    });
    let bytes = serde_json::to_vec(&payload).expect("approval json");
    let sig_bytes = hex::decode(&a.signature_hex).map_err(|_| DecisionError::InvalidSignature)?;
    let sig = Signature::from_slice(&sig_bytes).map_err(|_| DecisionError::InvalidSignature)?;
    key.verify(&bytes, &sig)
        .map_err(|_| DecisionError::InvalidSignature)?;
    let approval_sha256 = format!("{:x}", Sha256::digest(bytes));
    Ok(VerifiedApproval {
        document: a,
        approval_sha256,
    })
}

pub fn build_execution_envelope(
    case: &DecisionCase,
    scenario: &WhatIfScenario,
    supervisor_plan_uuid: Uuid,
    approval: Option<&VerifiedApproval>,
    controller_epoch: u64,
    fencing_token: u64,
) -> Result<DecisionExecutionEnvelope, DecisionError> {
    if case.tenant_uuid != scenario.tenant_uuid
        || case.project_uuid != scenario.project_uuid
        || case.case_uuid != scenario.case_uuid
    {
        return Err(DecisionError::ScopeMismatch);
    }
    if case.source_state_sha256 != scenario.source_state_sha256 {
        return Err(DecisionError::SourceStateMismatch);
    }
    if scenario.kind.requires_explicit_approval() && approval.is_none() {
        return Err(DecisionError::ApprovalRequired);
    }
    if let Some(a) = approval {
        if a.document.tenant_uuid != case.tenant_uuid
            || a.document.project_uuid != case.project_uuid
            || a.document.case_uuid != case.case_uuid
            || a.document.scenario_uuid != scenario.scenario_uuid
            || a.document.source_state_sha256 != case.source_state_sha256
        {
            return Err(DecisionError::ScopeMismatch);
        }
    }
    if controller_epoch == 0 || fencing_token == 0 {
        return Err(DecisionError::HardGate);
    }
    let evidence = serde_json::json!({"case":case.case_uuid,"scenario":scenario.scenario_uuid,"source":case.source_state_sha256,"scenario_sha256":scenario.scenario_sha256,"approval":approval.map(|a|&a.approval_sha256),"controller_epoch":controller_epoch,"fencing_token":fencing_token});
    let evidence_sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&evidence).expect("execution evidence"))
    );
    Ok(DecisionExecutionEnvelope {
        execution_uuid: Uuid::now_v7(),
        tenant_uuid: case.tenant_uuid,
        project_uuid: case.project_uuid,
        case_uuid: case.case_uuid,
        scenario_uuid: scenario.scenario_uuid,
        supervisor_plan_uuid,
        source_state_sha256: case.source_state_sha256.clone(),
        scenario_sha256: scenario.scenario_sha256.clone(),
        approval_sha256: approval.map(|a| a.approval_sha256.clone()),
        controller_epoch,
        fencing_token,
        evidence_sha256,
    })
}

pub fn direct_operational_mutation() -> Result<(), DecisionError> {
    Err(DecisionError::DelegateRequired)
}

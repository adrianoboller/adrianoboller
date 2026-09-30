//! PhxClaw v0.53 — Adaptive Portfolio Execution Controller.
//! Observes an approved v0.52 roadmap, detects persistent drift, builds shadow replans,
//! and delegates governed decisions to v0.49. Direct project mutation is forbidden.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

pub const DIRECT_MUTATION_ALLOWED: bool = false;
pub const DELEGATE_TO: &str = "executive_decision_center";
pub const REPLAN_PIPELINE: [&str; 3] = [
    "portfolio_digital_twin",
    "portfolio_optimizer",
    "autonomous_portfolio_planner",
];
pub const OLLAMA_POLICY: &str =
    "Ollama local-first when eligible; cloud escalation only when evidence justifies it";

#[derive(Debug, Error)]
pub enum ControllerError {
    #[error("source state mismatch")]
    SourceStateMismatch,
    #[error("telemetry is stale")]
    StaleTelemetry,
    #[error("invalid thresholds or policy")]
    InvalidPolicy,
    #[error("cooldown active")]
    Cooldown,
    #[error("replan rate limit exceeded")]
    RateLimit,
    #[error("hard gate violation")]
    HardGate,
    #[error("approval required")]
    ApprovalRequired,
    #[error("direct mutation forbidden")]
    DirectMutationForbidden,
    #[error("stale fencing token")]
    StaleFencing,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd)]
pub enum DriftKind {
    Schedule,
    Cost,
    Capacity,
    AgentUnavailable,
    ModelUnavailable,
    Dependency,
    Quality,
    Budget,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    High,
    Critical,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriftThresholds {
    pub schedule_drift_days: f64,
    pub cost_variance_ratio: f64,
    pub capacity_utilization_ratio: f64,
    pub dependency_slip_days: f64,
    pub quality_drop_ratio: f64,
    pub budget_remaining_ratio_warning: f64,
}
impl DriftThresholds {
    pub fn validate(&self) -> Result<(), ControllerError> {
        let vals = [
            self.schedule_drift_days,
            self.cost_variance_ratio,
            self.capacity_utilization_ratio,
            self.dependency_slip_days,
            self.quality_drop_ratio,
            self.budget_remaining_ratio_warning,
        ];
        if vals.iter().any(|x| !x.is_finite() || *x < 0.0)
            || self.capacity_utilization_ratio > 1.0
            || self.cost_variance_ratio > 1.0
            || self.quality_drop_ratio > 1.0
            || self.budget_remaining_ratio_warning > 1.0
        {
            return Err(ControllerError::InvalidPolicy);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControllerPolicy {
    pub telemetry_ttl_seconds: i64,
    pub confirmation_window_seconds: i64,
    pub critical_confirmation_window_seconds: i64,
    pub cooldown_seconds: i64,
    pub max_replans_per_hour: u32,
    pub shadow_first: bool,
    pub compare_current_vs_candidate: bool,
    pub auto_execute_reversible_preapproved: bool,
}
impl ControllerPolicy {
    pub fn validate(&self) -> Result<(), ControllerError> {
        if self.telemetry_ttl_seconds <= 0
            || self.confirmation_window_seconds < 0
            || self.critical_confirmation_window_seconds < 0
            || self.cooldown_seconds < 0
            || self.max_replans_per_hour == 0
            || !self.shadow_first
            || !self.compare_current_vs_candidate
            || self.auto_execute_reversible_preapproved
        {
            return Err(ControllerError::InvalidPolicy);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivePlanRef {
    pub tenant_uuid: Uuid,
    pub portfolio_uuid: Uuid,
    pub plan_uuid: Uuid,
    pub plan_sha256: String,
    pub source_state_sha256: String,
    pub revision_no: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionTelemetry {
    pub project_uuid: Uuid,
    pub observed_at_epoch: i64,
    pub source_state_sha256: String,
    pub schedule_drift_days: f64,
    pub cost_variance_ratio: f64,
    pub capacity_utilization_ratio: f64,
    pub dependency_slip_days: f64,
    pub quality_drop_ratio: f64,
    pub budget_remaining_ratio: f64,
    pub agent_unavailable: bool,
    pub model_unavailable: bool,
    pub evidence_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriftObservation {
    pub drift_uuid: Uuid,
    pub project_uuid: Uuid,
    pub kind: DriftKind,
    pub severity: Severity,
    pub magnitude: f64,
    pub first_seen_epoch: i64,
    pub last_seen_epoch: i64,
    pub evidence_sha256: String,
}

pub fn detect_drifts(
    now: i64,
    source_state: &str,
    t: &ExecutionTelemetry,
    th: &DriftThresholds,
    policy: &ControllerPolicy,
) -> Result<Vec<DriftObservation>, ControllerError> {
    th.validate()?;
    policy.validate()?;
    if t.source_state_sha256 != source_state {
        return Err(ControllerError::SourceStateMismatch);
    };
    if now - t.observed_at_epoch > policy.telemetry_ttl_seconds {
        return Err(ControllerError::StaleTelemetry);
    }
    let mut out = Vec::new();
    let mut push = |kind: DriftKind, sev: Severity, mag: f64| {
        out.push(DriftObservation {
            drift_uuid: Uuid::now_v7(),
            project_uuid: t.project_uuid,
            kind,
            severity: sev,
            magnitude: mag,
            first_seen_epoch: t.observed_at_epoch,
            last_seen_epoch: now,
            evidence_sha256: t.evidence_sha256.clone(),
        })
    };
    if t.schedule_drift_days >= th.schedule_drift_days {
        push(
            DriftKind::Schedule,
            if t.schedule_drift_days >= th.schedule_drift_days * 2.0 {
                Severity::High
            } else {
                Severity::Warning
            },
            t.schedule_drift_days,
        )
    }
    if t.cost_variance_ratio >= th.cost_variance_ratio {
        push(
            DriftKind::Cost,
            if t.cost_variance_ratio >= th.cost_variance_ratio * 2.0 {
                Severity::High
            } else {
                Severity::Warning
            },
            t.cost_variance_ratio,
        )
    }
    if t.capacity_utilization_ratio >= th.capacity_utilization_ratio {
        push(
            DriftKind::Capacity,
            Severity::High,
            t.capacity_utilization_ratio,
        )
    }
    if t.dependency_slip_days >= th.dependency_slip_days {
        push(
            DriftKind::Dependency,
            Severity::High,
            t.dependency_slip_days,
        )
    }
    if t.quality_drop_ratio >= th.quality_drop_ratio {
        push(DriftKind::Quality, Severity::High, t.quality_drop_ratio)
    }
    if t.budget_remaining_ratio <= th.budget_remaining_ratio_warning {
        push(
            DriftKind::Budget,
            Severity::Critical,
            t.budget_remaining_ratio,
        )
    }
    if t.agent_unavailable {
        push(DriftKind::AgentUnavailable, Severity::Critical, 1.0)
    }
    if t.model_unavailable {
        push(DriftKind::ModelUnavailable, Severity::Critical, 1.0)
    }
    Ok(out)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriftConfirmation {
    pub kind: DriftKind,
    pub confirmed: bool,
    pub confirmation_age_seconds: i64,
    pub confirmations: u32,
    pub evidence_sha256: String,
}
pub fn confirmed_drifts(
    obs: &[DriftObservation],
    confirmations: &BTreeMap<DriftKind, DriftConfirmation>,
    policy: &ControllerPolicy,
) -> Vec<DriftObservation> {
    obs.iter()
        .filter(|o| {
            let Some(c) = confirmations.get(&o.kind) else {
                return matches!(o.severity, Severity::Critical)
                    && policy.critical_confirmation_window_seconds == 0;
            };
            let need = if matches!(o.severity, Severity::Critical) {
                policy.critical_confirmation_window_seconds
            } else {
                policy.confirmation_window_seconds
            };
            c.confirmed && c.confirmations > 0 && c.confirmation_age_seconds >= need
        })
        .cloned()
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplanHistory {
    pub last_replan_epoch: Option<i64>,
    pub replans_last_hour: u32,
    pub last_controller_epoch: u64,
    pub last_fencing_token: u64,
}
pub fn authorize_replan(
    now: i64,
    policy: &ControllerPolicy,
    h: &ReplanHistory,
    controller_epoch: u64,
    fencing_token: u64,
) -> Result<(), ControllerError> {
    policy.validate()?;
    if controller_epoch < h.last_controller_epoch || fencing_token <= h.last_fencing_token {
        return Err(ControllerError::StaleFencing);
    };
    if let Some(last) = h.last_replan_epoch {
        if now - last < policy.cooldown_seconds {
            return Err(ControllerError::Cooldown);
        }
    };
    if h.replans_last_hour >= policy.max_replans_per_hour {
        return Err(ControllerError::RateLimit);
    };
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplanRequest {
    pub request_uuid: Uuid,
    pub portfolio_uuid: Uuid,
    pub base_plan_uuid: Uuid,
    pub base_plan_sha256: String,
    pub base_revision_no: u64,
    pub source_state_sha256: String,
    pub confirmed_drifts: Vec<DriftObservation>,
    pub pipeline: Vec<String>,
    pub shadow_only: bool,
    pub request_sha256: String,
}
pub fn build_replan_request(
    plan: &ActivePlanRef,
    drifts: Vec<DriftObservation>,
) -> Result<ReplanRequest, ControllerError> {
    if drifts.is_empty() {
        return Err(ControllerError::HardGate);
    };
    let request_uuid = Uuid::now_v7();
    let pipeline = REPLAN_PIPELINE
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>();
    #[derive(Serialize)]
    struct H<'a> {
        request_uuid: Uuid,
        portfolio_uuid: Uuid,
        base_plan_uuid: Uuid,
        base_plan_sha256: &'a str,
        revision: u64,
        source_state: &'a str,
        drifts: &'a [DriftObservation],
        pipeline: &'a [String],
    }
    let request_sha256 = canonical_sha256(&H {
        request_uuid,
        portfolio_uuid: plan.portfolio_uuid,
        base_plan_uuid: plan.plan_uuid,
        base_plan_sha256: &plan.plan_sha256,
        revision: plan.revision_no,
        source_state: &plan.source_state_sha256,
        drifts: &drifts,
        pipeline: &pipeline,
    });
    Ok(ReplanRequest {
        request_uuid,
        portfolio_uuid: plan.portfolio_uuid,
        base_plan_uuid: plan.plan_uuid,
        base_plan_sha256: plan.plan_sha256.clone(),
        base_revision_no: plan.revision_no,
        source_state_sha256: plan.source_state_sha256.clone(),
        confirmed_drifts: drifts,
        pipeline,
        shadow_only: true,
        request_sha256,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanRevisionProposal {
    pub revision_uuid: Uuid,
    pub base_plan_uuid: Uuid,
    pub base_plan_sha256: String,
    pub base_revision_no: u64,
    pub candidate_plan_sha256: String,
    pub source_state_sha256: String,
    pub delta_sha256: String,
    pub expected_cost_delta: f64,
    pub expected_finish_delta_days: f64,
    pub expected_risk_delta: f64,
    pub reversible: bool,
    pub requires_approval: bool,
    pub proposal_sha256: String,
}
pub fn compare_revision(
    base: &ActivePlanRef,
    candidate_plan_sha256: String,
    delta_sha256: String,
    cost_delta: f64,
    finish_delta: f64,
    risk_delta: f64,
    reversible: bool,
) -> PlanRevisionProposal {
    let revision_uuid = Uuid::now_v7();
    let requires_approval = true;
    #[derive(Serialize)]
    struct H<'a> {
        revision_uuid: Uuid,
        base: &'a str,
        candidate: &'a str,
        delta: &'a str,
        cost: f64,
        finish: f64,
        risk: f64,
        reversible: bool,
    }
    let proposal_sha256 = canonical_sha256(&H {
        revision_uuid,
        base: &base.plan_sha256,
        candidate: &candidate_plan_sha256,
        delta: &delta_sha256,
        cost: cost_delta,
        finish: finish_delta,
        risk: risk_delta,
        reversible,
    });
    PlanRevisionProposal {
        revision_uuid,
        base_plan_uuid: base.plan_uuid,
        base_plan_sha256: base.plan_sha256.clone(),
        base_revision_no: base.revision_no,
        candidate_plan_sha256,
        source_state_sha256: base.source_state_sha256.clone(),
        delta_sha256,
        expected_cost_delta: cost_delta,
        expected_finish_delta_days: finish_delta,
        expected_risk_delta: risk_delta,
        reversible,
        requires_approval,
        proposal_sha256,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecisionDelegation {
    pub revision_uuid: Uuid,
    pub proposal_sha256: String,
    pub source_state_sha256: String,
    pub delegate_to: String,
    pub direct_mutation: bool,
    pub required_approvals: Vec<String>,
    pub controller_epoch: u64,
    pub fencing_token: u64,
    pub evidence_sha256: String,
}
pub fn delegate_revision(
    p: &PlanRevisionProposal,
    controller_epoch: u64,
    fencing_token: u64,
) -> Result<DecisionDelegation, ControllerError> {
    if controller_epoch == 0 || fencing_token == 0 {
        return Err(ControllerError::StaleFencing);
    };
    let required_approvals = vec!["portfolio_replan".into(), "plan_revision".into()];
    #[derive(Serialize)]
    struct H<'a> {
        revision: Uuid,
        proposal: &'a str,
        state: &'a str,
        epoch: u64,
        fence: u64,
        approvals: &'a [String],
    }
    let evidence_sha256 = canonical_sha256(&H {
        revision: p.revision_uuid,
        proposal: &p.proposal_sha256,
        state: &p.source_state_sha256,
        epoch: controller_epoch,
        fence: fencing_token,
        approvals: &required_approvals,
    });
    Ok(DecisionDelegation {
        revision_uuid: p.revision_uuid,
        proposal_sha256: p.proposal_sha256.clone(),
        source_state_sha256: p.source_state_sha256.clone(),
        delegate_to: DELEGATE_TO.into(),
        direct_mutation: false,
        required_approvals,
        controller_epoch,
        fencing_token,
        evidence_sha256,
    })
}

pub fn assert_no_direct_mutation() -> Result<(), ControllerError> {
    if DIRECT_MUTATION_ALLOWED {
        Err(ControllerError::DirectMutationForbidden)
    } else {
        Ok(())
    }
}
pub fn canonical_sha256<T: Serialize>(x: &T) -> String {
    let v = serde_json::to_value(x).unwrap();
    let b = serde_json::to_vec(&sort_json(v)).unwrap();
    let mut h = Sha256::new();
    h.update(b);
    format!("{:x}", h.finalize())
}
fn sort_json(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(m) => {
            let mut b = BTreeMap::new();
            for (k, v) in m {
                b.insert(k, sort_json(v));
            }
            serde_json::Value::Object(b.into_iter().collect())
        }
        serde_json::Value::Array(a) => {
            serde_json::Value::Array(a.into_iter().map(sort_json).collect())
        }
        x => x,
    }
}

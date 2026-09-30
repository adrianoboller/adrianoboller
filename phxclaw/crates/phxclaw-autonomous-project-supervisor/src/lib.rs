//! PhxClaw v0.47 — Autonomous Project Supervisor.
//!
//! The supervisor consumes v0.45 runtime state and v0.46 predictive evidence to propose
//! or execute only policy-authorized project interventions. It cannot relax security,
//! privacy, knowledge-promotion, release, fencing or hard-budget gates.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

pub const SUPERVISOR_RULE: &str = "predict -> guard -> intervene -> check -> learn";
pub const COST_RULE: &str = "Ollama local-first when eligible; cloud escalation requires evidence";

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("active project mismatch")]
    ProjectMismatch,
    #[error("source state mismatch")]
    SourceStateMismatch,
    #[error("health snapshot is stale")]
    StaleHealth,
    #[error("hard gate cannot be overridden")]
    HardGate,
    #[error("approval required")]
    ApprovalRequired,
    #[error("intervention exceeds cycle action limit")]
    TooManyActions,
    #[error("invalid intervention evidence")]
    InvalidEvidence,
    #[error("pdca check is required before close")]
    PdcaCheckRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorPolicy {
    pub health_ttl_seconds: i64,
    pub deadline_risk_intervene: f64,
    pub budget_overrun_intervene: f64,
    pub rework_risk_intervene: f64,
    pub quality_floor: f64,
    pub wip_saturation: f64,
    pub max_actions_per_cycle: usize,
    pub autonomous_actions: BTreeSet<InterventionKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectHealthSnapshot {
    pub snapshot_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub source_state_sha256: String,
    pub generated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub deadline_risk: f64,
    pub budget_overrun_risk: f64,
    pub rework_risk: f64,
    pub predicted_quality: f64,
    pub wip_ratio: f64,
    pub queue_depth: u32,
    pub blocked_tasks: u32,
    pub available_agents: u32,
    pub expected_remaining_cost_usd: f64,
    pub remaining_budget_usd: f64,
    pub expected_remaining_duration_ms: u64,
    pub millis_to_deadline: u64,
    pub evidence_sha256: String,
    pub snapshot_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterventionKind {
    Reprioritize,
    ReassignWithinApprovedPool,
    ReduceWip,
    RelieveBottleneck,
    RescheduleSprint,
    RescheduleGantt,
    RecomputeCriticalPath,
    RebalanceResources,
    ReforecastBudget,
    CloudThrottle,
    OpenCorrectiveAction,
    ScopeChange,
    BaselineChange,
    DeadlineChange,
    BudgetIncrease,
    ProductionChange,
    DestructiveChange,
    EscalateHuman,
}

impl InterventionKind {
    pub fn always_requires_approval(self) -> bool {
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterventionAction {
    pub action_uuid: Uuid,
    pub kind: InterventionKind,
    pub target_uuid: Option<Uuid>,
    pub reason: String,
    pub expected_effect: String,
    pub predicted_cost_delta_usd: f64,
    pub predicted_duration_delta_ms: i64,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterventionPlan {
    pub plan_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub health_snapshot_uuid: Uuid,
    pub source_state_sha256: String,
    pub policy_sha256: String,
    pub actions: Vec<InterventionAction>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub evidence_sha256: String,
    pub plan_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PdcaPhase {
    Plan,
    Do,
    Check,
    Act,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorCycle {
    pub cycle_uuid: Uuid,
    pub project_uuid: Uuid,
    pub source_state_sha256: String,
    pub phase: PdcaPhase,
    pub plan_uuid: Option<Uuid>,
    pub check_evidence_sha256: Option<String>,
    pub fruitful_candidates: Vec<Uuid>,
    pub unfruitful_candidates: Vec<Uuid>,
}

pub fn health_score(h: &ProjectHealthSnapshot) -> f64 {
    let risk = 0.30 * h.deadline_risk
        + 0.25 * h.budget_overrun_risk
        + 0.20 * h.rework_risk
        + 0.15 * h.wip_ratio
        + 0.10 * (1.0 - h.predicted_quality);
    (1.0 - risk).clamp(0.0, 1.0)
}

pub fn validate_health(
    h: &ProjectHealthSnapshot,
    project_uuid: Uuid,
    source_state_sha256: &str,
    now: DateTime<Utc>,
) -> Result<(), SupervisorError> {
    if h.project_uuid != project_uuid {
        return Err(SupervisorError::ProjectMismatch);
    }
    if h.source_state_sha256 != source_state_sha256 {
        return Err(SupervisorError::SourceStateMismatch);
    }
    if h.expires_at < now {
        return Err(SupervisorError::StaleHealth);
    }
    Ok(())
}

pub fn propose_interventions(
    h: &ProjectHealthSnapshot,
    policy: &SupervisorPolicy,
    policy_sha256: &str,
    now: DateTime<Utc>,
) -> Result<InterventionPlan, SupervisorError> {
    if h.expires_at < now {
        return Err(SupervisorError::StaleHealth);
    }
    let mut actions = Vec::new();
    let mut push = |kind: InterventionKind, reason: &str, effect: &str, cost: f64, dur: i64| {
        actions.push(InterventionAction {
            action_uuid: Uuid::now_v7(),
            kind,
            target_uuid: None,
            reason: reason.into(),
            expected_effect: effect.into(),
            predicted_cost_delta_usd: cost,
            predicted_duration_delta_ms: dur,
            requires_approval: kind.always_requires_approval()
                || !policy.autonomous_actions.contains(&kind),
        });
    };
    if h.deadline_risk >= policy.deadline_risk_intervene {
        push(
            InterventionKind::Reprioritize,
            "deadline risk above intervention threshold",
            "move critical-path work earlier",
            0.0,
            -(h.expected_remaining_duration_ms as i64 / 10),
        );
        push(
            InterventionKind::RebalanceResources,
            "deadline risk requires capacity rebalance",
            "shift approved agents toward critical-path work",
            0.0,
            -(h.expected_remaining_duration_ms as i64 / 20),
        );
        push(
            InterventionKind::RecomputeCriticalPath,
            "forecast changed critical timing",
            "refresh dependency/critical-path evidence",
            0.0,
            0,
        );
    }
    if h.wip_ratio >= policy.wip_saturation {
        push(
            InterventionKind::ReduceWip,
            "WIP saturation exceeds policy",
            "stop starting and finish blocked/in-progress work",
            0.0,
            0,
        );
        push(
            InterventionKind::RelieveBottleneck,
            "flow bottleneck detected",
            "move capacity to bottleneck stage",
            0.0,
            -60000,
        );
    }
    if h.budget_overrun_risk >= policy.budget_overrun_intervene {
        push(
            InterventionKind::CloudThrottle,
            "budget overrun risk above threshold",
            COST_RULE,
            -(h.expected_remaining_cost_usd * 0.08),
            0,
        );
        push(
            InterventionKind::ReforecastBudget,
            "cost trajectory changed",
            "refresh EAC/ETC/VAC and route budgets",
            0.0,
            0,
        );
    }
    if h.rework_risk >= policy.rework_risk_intervene || h.predicted_quality < policy.quality_floor {
        push(
            InterventionKind::OpenCorrectiveAction,
            "rework/quality risk requires PDCA correction",
            "open RCA and corrective action before more parallel work",
            0.0,
            0,
        );
    }
    if actions.len() > policy.max_actions_per_cycle {
        return Err(SupervisorError::TooManyActions);
    }
    let plan_uuid = Uuid::now_v7();
    let expires_at = now + Duration::seconds(policy.health_ttl_seconds);
    let canonical = serde_json::json!({"plan_uuid":plan_uuid,"project_uuid":h.project_uuid,"health_snapshot_uuid":h.snapshot_uuid,"source_state_sha256":h.source_state_sha256,"policy_sha256":policy_sha256,"actions":actions});
    let evidence_sha256 = format!(
        "{:x}",
        Sha256::digest(format!("{}:{}", h.evidence_sha256, h.snapshot_sha256).as_bytes())
    );
    let plan_sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&canonical).expect("plan json"))
    );
    Ok(InterventionPlan {
        plan_uuid,
        tenant_uuid: h.tenant_uuid,
        project_uuid: h.project_uuid,
        health_snapshot_uuid: h.snapshot_uuid,
        source_state_sha256: h.source_state_sha256.clone(),
        policy_sha256: policy_sha256.into(),
        actions,
        created_at: now,
        expires_at,
        evidence_sha256,
        plan_sha256,
    })
}

pub fn can_execute(
    action: &InterventionAction,
    approved: bool,
    hard_gates: BTreeMap<String, bool>,
) -> Result<(), SupervisorError> {
    if hard_gates.values().any(|v| !*v) {
        return Err(SupervisorError::HardGate);
    }
    if action.requires_approval && !approved {
        return Err(SupervisorError::ApprovalRequired);
    }
    Ok(())
}

pub fn advance_cycle(
    c: &mut SupervisorCycle,
    check_evidence: Option<String>,
) -> Result<(), SupervisorError> {
    c.phase = match c.phase {
        PdcaPhase::Plan => PdcaPhase::Do,
        PdcaPhase::Do => PdcaPhase::Check,
        PdcaPhase::Check => {
            let ev = check_evidence.ok_or(SupervisorError::PdcaCheckRequired)?;
            c.check_evidence_sha256 = Some(ev);
            PdcaPhase::Act
        }
        PdcaPhase::Act => {
            if c.check_evidence_sha256.is_none() {
                return Err(SupervisorError::PdcaCheckRequired);
            }
            PdcaPhase::Closed
        }
        PdcaPhase::Closed => PdcaPhase::Closed,
    };
    Ok(())
}

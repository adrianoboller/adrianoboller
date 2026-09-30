//! PhxClaw v0.45 — Active Project Execution Scheduler & Cost-Aware Agent Runtime.
//!
//! This crate intentionally does not own model eligibility or learning promotion.
//! It composes v0.44 routing/PDCA primitives into active-project scheduling.

use chrono::{DateTime, Duration, Utc};
use phxclaw_agent_control_plane::{
    classify_learning, knowledge_guard, route_model, AgentRuntimeProfile, Complexity, ControlError,
    DataClass, EitherKnowledge, ExecutionOutcome, FruitfulKnowledge, ModelCandidate, PdcaCycle,
    PdcaPhase, RoutingDecision, TaskRoutingRequest, UnfruitfulKnowledge,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

/// Human-readable policy marker; eligibility still comes from the governed v0.44 router.
pub const LOCAL_FIRST_POLICY: &str = "Ollama local-first when eligible";

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("project is not active")]
    ProjectNotActive,
    #[error("tenant/project mismatch")]
    ProjectMismatch,
    #[error("task dependencies are not satisfied")]
    DependenciesBlocked,
    #[error("task is not dispatchable from its current state")]
    InvalidTaskState,
    #[error("agent capacity exhausted")]
    AgentCapacityExhausted,
    #[error("project concurrency exhausted")]
    ProjectConcurrencyExhausted,
    #[error("stale or invalid fencing token")]
    InvalidFencing,
    #[error("execution lease expired")]
    LeaseExpired,
    #[error("project budget would be exceeded")]
    BudgetExceeded,
    #[error("promoted unfruitful knowledge blocks this route")]
    KnownFailedRoute,
    #[error("retry requires a different routing decision")]
    RetryDecisionRequired,
    #[error("max attempts reached")]
    AttemptsExhausted,
    #[error("PDCA/evidence requirements not satisfied")]
    PdcaEvidenceMissing,
    #[error(transparent)]
    Control(#[from] ControlError),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Queued,
    Ready,
    Leased,
    Running,
    Checking,
    Acting,
    Completed,
    Failed,
    Cancelled,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveProject {
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub source_state_sha256: String,
    pub active: bool,
    pub paused: bool,
    pub max_concurrency: u32,
    pub max_budget_usd: f64,
    pub reserved_budget_usd: f64,
    pub actual_cost_usd: f64,
}

impl ActiveProject {
    pub fn remaining_budget(&self) -> f64 {
        (self.max_budget_usd - self.reserved_budget_usd - self.actual_cost_usd).max(0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTask {
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub task_uuid: Uuid,
    pub task_class: String,
    pub capability: String,
    pub complexity: Complexity,
    pub data_class: DataClass,
    pub priority: i32,
    pub dependencies: BTreeSet<Uuid>,
    pub state: TaskState,
    pub attempts: u32,
    pub max_attempts: u32,
    pub context_fingerprint: String,
    pub source_state_sha256: String,
    pub required_agent_uuid: Option<Uuid>,
    pub quality_floor: f64,
    pub max_estimated_cost_usd: f64,
    pub max_p95_latency_ms: Option<u64>,
}

impl ProjectTask {
    pub fn ready(&self, completed: &BTreeSet<Uuid>) -> bool {
        self.state == TaskState::Queued && self.dependencies.iter().all(|d| completed.contains(d))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapacity {
    pub agent_uuid: Uuid,
    pub max_concurrency: u32,
    pub active_runs: u32,
    pub healthy: bool,
    pub heartbeat_at: DateTime<Utc>,
}

impl AgentCapacity {
    pub fn available(&self, now: DateTime<Utc>, heartbeat_ttl_seconds: i64) -> bool {
        self.healthy
            && self.active_runs < self.max_concurrency
            && now.signed_duration_since(self.heartbeat_at)
                <= Duration::seconds(heartbeat_ttl_seconds)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchLease {
    pub lease_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub task_uuid: Uuid,
    pub agent_uuid: Uuid,
    pub run_uuid: Uuid,
    pub fencing_token: u64,
    pub routing_decision: RoutingDecision,
    pub estimated_cost_usd: f64,
    pub expires_at: DateTime<Utc>,
    pub pdca_cycle_uuid: Uuid,
    pub lease_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DispatchInput<'a> {
    pub project: &'a ActiveProject,
    pub task: &'a ProjectTask,
    pub agents: &'a [AgentRuntimeProfile],
    pub capacities: &'a [AgentCapacity],
    pub candidates: &'a [ModelCandidate],
    pub fruitful: &'a [FruitfulKnowledge],
    pub unfruitful: &'a [UnfruitfulKnowledge],
    pub completed_tasks: &'a BTreeSet<Uuid>,
    pub project_active_runs: u32,
    pub next_fencing_token: u64,
    pub heartbeat_ttl_seconds: i64,
    pub lease_seconds: i64,
    pub now: DateTime<Utc>,
    pub previous_failed_decision_sha256: Option<&'a str>,
}

fn choose_agent<'a>(
    task: &ProjectTask,
    agents: &'a [AgentRuntimeProfile],
    capacities: &[AgentCapacity],
    now: DateTime<Utc>,
    ttl: i64,
) -> Result<&'a AgentRuntimeProfile, RuntimeError> {
    let mut eligible: Vec<&AgentRuntimeProfile> = agents
        .iter()
        .filter(|a| {
            a.active
                && a.project_uuid == task.project_uuid
                && a.capability == task.capability
                && task
                    .required_agent_uuid
                    .map(|x| x == a.agent_uuid)
                    .unwrap_or(true)
                && capacities
                    .iter()
                    .find(|c| c.agent_uuid == a.agent_uuid)
                    .map(|c| c.available(now, ttl))
                    .unwrap_or(false)
        })
        .collect();
    eligible.sort_by_key(|a| a.agent_uuid);
    eligible
        .into_iter()
        .next()
        .ok_or(RuntimeError::AgentCapacityExhausted)
}

fn promoted_failed_route_matches(
    task: &ProjectTask,
    unfruitful: &[UnfruitfulKnowledge],
    agent_uuid: Uuid,
    profile_uuid: Uuid,
) -> bool {
    unfruitful.iter().any(|k| {
        k.project_uuid == task.project_uuid
            && k.task_class == task.task_class
            && k.context_fingerprint == task.context_fingerprint
            && k.agent_uuid == agent_uuid
            && k.model_profile_uuid == profile_uuid
            && k.promotion_state != "candidate"
            && k.promotion_state != "expired"
            && k.promotion_state != "superseded"
    })
}

pub fn plan_dispatch(input: DispatchInput<'_>) -> Result<(DispatchLease, PdcaCycle), RuntimeError> {
    let p = input.project;
    let t = input.task;
    if !p.active || p.paused {
        return Err(RuntimeError::ProjectNotActive);
    }
    if p.tenant_uuid != t.tenant_uuid || p.project_uuid != t.project_uuid {
        return Err(RuntimeError::ProjectMismatch);
    }
    if t.state != TaskState::Queued && t.state != TaskState::Ready {
        return Err(RuntimeError::InvalidTaskState);
    }
    if !t
        .dependencies
        .iter()
        .all(|d| input.completed_tasks.contains(d))
    {
        return Err(RuntimeError::DependenciesBlocked);
    }
    if t.attempts >= t.max_attempts {
        return Err(RuntimeError::AttemptsExhausted);
    }
    if input.project_active_runs >= p.max_concurrency {
        return Err(RuntimeError::ProjectConcurrencyExhausted);
    }

    // PLAN consults both successful and failed governed knowledge before execution.
    let guard = knowledge_guard(
        &t.task_class,
        &t.context_fingerprint,
        input.fruitful,
        input.unfruitful,
    );
    let agent = choose_agent(
        t,
        input.agents,
        input.capacities,
        input.now,
        input.heartbeat_ttl_seconds,
    )?;
    let req = TaskRoutingRequest {
        tenant_uuid: t.tenant_uuid,
        project_uuid: t.project_uuid,
        task_uuid: t.task_uuid,
        agent_uuid: agent.agent_uuid,
        capability: t.capability.clone(),
        complexity: t.complexity,
        data_class: t.data_class,
        quality_floor: t.quality_floor,
        max_estimated_cost_usd: t.max_estimated_cost_usd.min(p.remaining_budget()),
        max_p95_latency_ms: t.max_p95_latency_ms,
        require_independent_review: t.complexity == Complexity::Extreme,
        source_state_sha256: t.source_state_sha256.clone(),
    };
    let decision = route_model(agent, &req, input.candidates)?;

    // Promoted failed route = hard guard for exact context. Candidate failures never block.
    if promoted_failed_route_matches(
        t,
        input.unfruitful,
        agent.agent_uuid,
        decision.selected_profile_uuid,
    ) {
        return Err(RuntimeError::KnownFailedRoute);
    }
    if input.previous_failed_decision_sha256 == Some(decision.decision_sha256.as_str()) {
        return Err(RuntimeError::RetryDecisionRequired);
    }
    if decision.estimated_cost_usd > p.remaining_budget() {
        return Err(RuntimeError::BudgetExceeded);
    }

    let expected = vec![
        "acceptance".into(),
        "qa".into(),
        "security".into(),
        "cost_settlement".into(),
    ];
    let pdca = PdcaCycle::new(
        &req,
        format!("execute {} using governed route", t.task_class),
        expected,
    );
    let lease_uuid = Uuid::now_v7();
    let run_uuid = Uuid::now_v7();
    let expires_at = input.now + Duration::seconds(input.lease_seconds);
    let canonical = serde_json::json!({
        "lease_uuid": lease_uuid, "project_uuid": p.project_uuid, "task_uuid": t.task_uuid,
        "agent_uuid": agent.agent_uuid, "run_uuid": run_uuid, "fencing_token": input.next_fencing_token,
        "decision_sha256": decision.decision_sha256, "estimated_cost_usd": decision.estimated_cost_usd,
        "expires_at": expires_at, "source_state_sha256": t.source_state_sha256,
        "knowledge_guard": {"reused": guard.reused_successes, "failed": guard.matched_failures}
    });
    let lease_sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&canonical).expect("canonical lease"))
    );
    Ok((
        DispatchLease {
            lease_uuid,
            tenant_uuid: p.tenant_uuid,
            project_uuid: p.project_uuid,
            task_uuid: t.task_uuid,
            agent_uuid: agent.agent_uuid,
            run_uuid,
            fencing_token: input.next_fencing_token,
            routing_decision: decision.clone(),
            estimated_cost_usd: decision.estimated_cost_usd,
            expires_at,
            pdca_cycle_uuid: pdca.cycle_uuid,
            lease_sha256,
        },
        pdca,
    ))
}

pub fn verify_execution_fence(
    lease: &DispatchLease,
    expected_fencing_token: u64,
    now: DateTime<Utc>,
) -> Result<(), RuntimeError> {
    if lease.fencing_token != expected_fencing_token {
        return Err(RuntimeError::InvalidFencing);
    }
    if now > lease.expires_at {
        return Err(RuntimeError::LeaseExpired);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckEvidence {
    pub acceptance_passed: bool,
    pub qa_passed: bool,
    pub security_passed: bool,
    pub quality_score: f64,
    pub actual_cost_usd: f64,
    pub evidence_refs: Vec<String>,
    pub failure_signature: Option<String>,
    pub root_cause: Option<String>,
    pub remediation: Option<String>,
}

pub fn close_pdca_and_classify(
    task: &ProjectTask,
    lease: &DispatchLease,
    mut pdca: PdcaCycle,
    check: CheckEvidence,
) -> Result<(PdcaCycle, EitherKnowledge), RuntimeError> {
    if check.evidence_refs.is_empty() {
        return Err(RuntimeError::PdcaEvidenceMissing);
    }
    if pdca.phase == PdcaPhase::Plan {
        pdca.advance(PdcaPhase::Do)?;
    }
    if pdca.phase == PdcaPhase::Do {
        pdca.advance(PdcaPhase::Check)?;
    }
    if pdca.phase == PdcaPhase::Check {
        pdca.advance(PdcaPhase::Act)?;
    }
    let outcome = ExecutionOutcome {
        run_uuid: lease.run_uuid,
        tenant_uuid: lease.tenant_uuid,
        project_uuid: lease.project_uuid,
        task_uuid: task.task_uuid,
        agent_uuid: lease.agent_uuid,
        model_profile_uuid: lease.routing_decision.selected_profile_uuid,
        task_class: task.task_class.clone(),
        context_fingerprint: task.context_fingerprint.clone(),
        source_state_sha256: task.source_state_sha256.clone(),
        acceptance_passed: check.acceptance_passed,
        qa_passed: check.qa_passed,
        security_passed: check.security_passed,
        quality_score: check.quality_score,
        actual_cost_usd: check.actual_cost_usd,
        evidence_refs: check.evidence_refs,
        failure_signature: check.failure_signature,
        root_cause: check.root_cause,
        remediation: check.remediation,
    };
    let learning = classify_learning(&outcome)?;
    pdca.advance(PdcaPhase::Closed)?;
    Ok((pdca, learning))
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectExecutionSummary {
    pub queued: u64,
    pub running: u64,
    pub completed: u64,
    pub failed: u64,
    pub blocked: u64,
    pub estimated_reserved_usd: f64,
    pub actual_usd: f64,
    pub by_model_profile: BTreeMap<Uuid, f64>,
    pub by_agent: BTreeMap<Uuid, f64>,
}

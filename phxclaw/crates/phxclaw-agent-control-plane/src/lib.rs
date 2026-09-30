//! PhxClaw v0.44 — Agent & Subagent Control Plane + PDCA Learning.
//!
//! Core invariants:
//! - assignments are scoped to the active project;
//! - agent UUIDs come from the governed registry;
//! - the spreadsheet "suggested model" is a preference hint, never authority;
//! - routing is local-first and cost-aware, but never relaxes privacy/quality gates;
//! - only promoted model profiles may execute governed production tasks;
//! - every governed run closes a PDCA cycle;
//! - successful and failed learning records remain separate;
//! - failed patterns are consulted before repeating equivalent work;
//! - raw prompts, API keys and plaintext secrets are not persisted here.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

/// Human-readable local provider preference. Execution still depends on governed eligibility.
pub const LOCAL_PROVIDER_HINT: &str = "Ollama";

#[derive(Debug, Error)]
pub enum ControlError {
    #[error("no eligible model satisfies all policy gates")]
    NoEligibleModel,
    #[error("agent is not active for this project")]
    AgentNotActive,
    #[error("project mismatch")]
    ProjectMismatch,
    #[error("model profile is not promoted")]
    ModelNotPromoted,
    #[error("knowledge record has incompatible source state")]
    IncompatibleKnowledge,
    #[error("PDCA transition is invalid")]
    InvalidPdcaTransition,
    #[error("critical task requires independent reviewer")]
    IndependentReviewRequired,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Complexity {
    Low,
    Medium,
    High,
    Extreme,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataClass {
    Public,
    Internal,
    Confidential,
    Restricted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRuntimeProfile {
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub agent_uuid: Uuid,
    pub name: String,
    pub capability: String,
    pub default_complexity: Complexity,
    pub model_hint: Option<String>,
    pub local_first: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCandidate {
    pub profile_uuid: Uuid,
    pub provider: String,
    pub model: String,
    pub is_local: bool,
    pub is_deterministic: bool,
    pub promoted: bool,
    pub healthy: bool,
    pub supported_capabilities: BTreeSet<String>,
    pub max_complexity: Complexity,
    pub quality_score: f64,
    pub estimated_cost_usd: f64,
    pub p95_latency_ms: u64,
    pub confidential_allowed: bool,
    pub restricted_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRoutingRequest {
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub task_uuid: Uuid,
    pub agent_uuid: Uuid,
    pub capability: String,
    pub complexity: Complexity,
    pub data_class: DataClass,
    pub quality_floor: f64,
    pub max_estimated_cost_usd: f64,
    pub max_p95_latency_ms: Option<u64>,
    pub require_independent_review: bool,
    pub source_state_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub decision_uuid: Uuid,
    pub task_uuid: Uuid,
    pub agent_uuid: Uuid,
    pub selected_profile_uuid: Uuid,
    pub provider: String,
    pub model: String,
    pub local: bool,
    pub estimated_cost_usd: f64,
    pub reason: Vec<String>,
    pub decision_sha256: String,
}

fn rank_complexity(c: Complexity) -> u8 {
    match c {
        Complexity::Low => 1,
        Complexity::Medium => 2,
        Complexity::High => 3,
        Complexity::Extreme => 4,
    }
}

fn candidate_allowed(req: &TaskRoutingRequest, c: &ModelCandidate) -> bool {
    if !c.promoted || !c.healthy { return false; }
    if !c.supported_capabilities.contains(&req.capability) { return false; }
    if rank_complexity(c.max_complexity) < rank_complexity(req.complexity) { return false; }
    if c.quality_score < req.quality_floor { return false; }
    if c.estimated_cost_usd > req.max_estimated_cost_usd { return false; }
    if let Some(max_latency) = req.max_p95_latency_ms {
        if c.p95_latency_ms > max_latency { return false; }
    }
    match req.data_class {
        DataClass::Restricted => c.is_local && c.restricted_allowed,
        DataClass::Confidential => c.is_local || c.confidential_allowed,
        DataClass::Public | DataClass::Internal => true,
    }
}

/// Chooses the lowest-cost eligible model, while preferring local execution when
/// quality/capability/privacy requirements are already satisfied.
pub fn route_model(
    agent: &AgentRuntimeProfile,
    req: &TaskRoutingRequest,
    candidates: &[ModelCandidate],
) -> Result<RoutingDecision, ControlError> {
    if !agent.active || agent.project_uuid != req.project_uuid || agent.agent_uuid != req.agent_uuid {
        return Err(ControlError::AgentNotActive);
    }
    if req.complexity == Complexity::Extreme && !req.require_independent_review {
        return Err(ControlError::IndependentReviewRequired);
    }

    let mut eligible: Vec<&ModelCandidate> = candidates.iter()
        .filter(|c| candidate_allowed(req, c))
        .collect();
    if eligible.is_empty() { return Err(ControlError::NoEligibleModel); }

    eligible.sort_by(|a, b| {
        // local-first only after all hard gates pass
        let local_a = if agent.local_first && a.is_local { 0 } else { 1 };
        let local_b = if agent.local_first && b.is_local { 0 } else { 1 };
        local_a.cmp(&local_b)
            .then_with(|| a.estimated_cost_usd.total_cmp(&b.estimated_cost_usd))
            .then_with(|| b.quality_score.total_cmp(&a.quality_score))
            .then_with(|| a.p95_latency_ms.cmp(&b.p95_latency_ms))
            .then_with(|| a.profile_uuid.cmp(&b.profile_uuid))
    });

    let selected = eligible[0];
    let mut reason = vec![
        "privacy/capability/complexity/quality/budget gates satisfied".to_string(),
        if selected.is_local { "local-first eligible".to_string() } else { "cloud fallback required".to_string() },
        "lowest governed cost among preferred candidates".to_string(),
    ];
    if let Some(hint) = &agent.model_hint {
        reason.push(format!("agent model hint retained as non-authoritative preference: {hint}"));
    }

    let decision_uuid = Uuid::now_v7();
    let canonical = serde_json::json!({
        "decision_uuid": decision_uuid,
        "task_uuid": req.task_uuid,
        "agent_uuid": req.agent_uuid,
        "selected_profile_uuid": selected.profile_uuid,
        "source_state_sha256": req.source_state_sha256,
        "estimated_cost_usd": selected.estimated_cost_usd,
    });
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&canonical).unwrap()));

    Ok(RoutingDecision {
        decision_uuid,
        task_uuid: req.task_uuid,
        agent_uuid: req.agent_uuid,
        selected_profile_uuid: selected.profile_uuid,
        provider: selected.provider.clone(),
        model: selected.model.clone(),
        local: selected.is_local,
        estimated_cost_usd: selected.estimated_cost_usd,
        reason,
        decision_sha256: digest,
    })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PdcaPhase { Plan, Do, Check, Act, Closed }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdcaCycle {
    pub cycle_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub task_uuid: Uuid,
    pub phase: PdcaPhase,
    pub objective: String,
    pub expected_evidence: Vec<String>,
    pub source_state_sha256: String,
    pub created_at: DateTime<Utc>,
}

impl PdcaCycle {
    pub fn new(req: &TaskRoutingRequest, objective: String, expected_evidence: Vec<String>) -> Self {
        Self {
            cycle_uuid: Uuid::now_v7(), tenant_uuid: req.tenant_uuid, project_uuid: req.project_uuid,
            task_uuid: req.task_uuid, phase: PdcaPhase::Plan, objective, expected_evidence,
            source_state_sha256: req.source_state_sha256.clone(), created_at: Utc::now(),
        }
    }
    pub fn advance(&mut self, next: PdcaPhase) -> Result<(), ControlError> {
        let ok = matches!((self.phase, next),
            (PdcaPhase::Plan, PdcaPhase::Do) |
            (PdcaPhase::Do, PdcaPhase::Check) |
            (PdcaPhase::Check, PdcaPhase::Act) |
            (PdcaPhase::Act, PdcaPhase::Closed));
        if !ok { return Err(ControlError::InvalidPdcaTransition); }
        self.phase = next;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionOutcome {
    pub run_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub task_uuid: Uuid,
    pub agent_uuid: Uuid,
    pub model_profile_uuid: Uuid,
    pub task_class: String,
    pub context_fingerprint: String,
    pub source_state_sha256: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FruitfulKnowledge {
    pub knowledge_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub task_class: String,
    pub context_fingerprint: String,
    pub source_state_sha256: String,
    pub pattern_summary: String,
    pub agent_uuid: Uuid,
    pub model_profile_uuid: Uuid,
    pub quality_score: f64,
    pub actual_cost_usd: f64,
    pub evidence_refs: Vec<String>,
    pub confidence: f64,
    pub reuse_count: u64,
    pub promotion_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnfruitfulKnowledge {
    pub knowledge_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub task_class: String,
    pub context_fingerprint: String,
    pub source_state_sha256: String,
    pub failure_signature: String,
    pub root_cause: String,
    pub remediation: Option<String>,
    pub avoidance_rule: String,
    pub agent_uuid: Uuid,
    pub model_profile_uuid: Uuid,
    pub evidence_refs: Vec<String>,
    pub occurrence_count: u64,
    pub promotion_state: String,
}

pub fn classify_learning(outcome: &ExecutionOutcome) -> Result<EitherKnowledge, ControlError> {
    let success = outcome.acceptance_passed && outcome.qa_passed && outcome.security_passed;
    if success {
        Ok(EitherKnowledge::Fruitful(FruitfulKnowledge {
            knowledge_uuid: Uuid::now_v7(), tenant_uuid: outcome.tenant_uuid, project_uuid: outcome.project_uuid,
            task_class: outcome.task_class.clone(), context_fingerprint: outcome.context_fingerprint.clone(),
            source_state_sha256: outcome.source_state_sha256.clone(),
            pattern_summary: "candidate successful execution pattern; requires F24/F25 promotion".into(),
            agent_uuid: outcome.agent_uuid, model_profile_uuid: outcome.model_profile_uuid,
            quality_score: outcome.quality_score, actual_cost_usd: outcome.actual_cost_usd,
            evidence_refs: outcome.evidence_refs.clone(), confidence: outcome.quality_score,
            reuse_count: 0, promotion_state: "candidate".into(),
        }))
    } else {
        let sig = outcome.failure_signature.clone().unwrap_or_else(|| "unspecified_failure".into());
        let cause = outcome.root_cause.clone().unwrap_or_else(|| "root cause pending".into());
        Ok(EitherKnowledge::Unfruitful(UnfruitfulKnowledge {
            knowledge_uuid: Uuid::now_v7(), tenant_uuid: outcome.tenant_uuid, project_uuid: outcome.project_uuid,
            task_class: outcome.task_class.clone(), context_fingerprint: outcome.context_fingerprint.clone(),
            source_state_sha256: outcome.source_state_sha256.clone(), failure_signature: sig.clone(),
            root_cause: cause, remediation: outcome.remediation.clone(),
            avoidance_rule: format!("before repeating {sig}, require context match + remediation evidence"),
            agent_uuid: outcome.agent_uuid, model_profile_uuid: outcome.model_profile_uuid,
            evidence_refs: outcome.evidence_refs.clone(), occurrence_count: 1, promotion_state: "candidate".into(),
        }))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EitherKnowledge { Fruitful(FruitfulKnowledge), Unfruitful(UnfruitfulKnowledge) }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreTaskKnowledgeGuard {
    pub reused_successes: Vec<Uuid>,
    pub matched_failures: Vec<Uuid>,
    pub warnings: Vec<String>,
    pub block: bool,
}

/// Exact context matches can block known failed paths. Non-exact/context-unknown
/// failures only warn; they are not generalized into universal truth.
pub fn knowledge_guard(
    task_class: &str,
    context_fingerprint: &str,
    fruitful: &[FruitfulKnowledge],
    unfruitful: &[UnfruitfulKnowledge],
) -> PreTaskKnowledgeGuard {
    let reused_successes = fruitful.iter()
        .filter(|k| k.task_class == task_class && k.context_fingerprint == context_fingerprint && k.promotion_state != "candidate")
        .map(|k| k.knowledge_uuid).collect::<Vec<_>>();
    let matched = unfruitful.iter()
        .filter(|k| k.task_class == task_class && k.context_fingerprint == context_fingerprint && k.promotion_state != "candidate")
        .collect::<Vec<_>>();
    let block = matched.iter().any(|k| !k.avoidance_rule.is_empty());
    let warnings = matched.iter().map(|k| format!("known failed pattern: {} — {}", k.failure_signature, k.avoidance_rule)).collect();
    PreTaskKnowledgeGuard { reused_successes, matched_failures: matched.iter().map(|k| k.knowledge_uuid).collect(), warnings, block }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostSummary {
    pub estimated_usd: f64,
    pub actual_usd: f64,
    pub by_provider: BTreeMap<String, f64>,
    pub by_agent: BTreeMap<Uuid, f64>,
}

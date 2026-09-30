//! PhxClaw v0.52 — Autonomous Portfolio Planner.
//! Converts an explicitly selected v0.51 Pareto candidate into a governed roadmap.
//! It never mutates projects directly; execution is delegated to v0.49/v0.47.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;
use uuid::Uuid;

pub const DIRECT_MUTATION_ALLOWED: bool = false;
pub const DELEGATE_TO: &str = "executive_decision_center";
pub const DEFAULT_STARVATION_GUARD_DAYS: u32 = 14;

#[derive(Debug, Error)]
pub enum PlannerError {
    #[error("source state mismatch")]
    SourceStateMismatch,
    #[error("selected Pareto candidate required")]
    SelectedCandidateRequired,
    #[error("selected candidate is not in Pareto frontier")]
    CandidateNotPareto,
    #[error("dependency cycle detected")]
    DependencyCycle,
    #[error("hard gate violation")]
    HardGate,
    #[error("capacity infeasible")]
    Capacity,
    #[error("budget infeasible")]
    Budget,
    #[error("approval required")]
    ApprovalRequired,
    #[error("direct mutation forbidden")]
    DirectMutationForbidden,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DependencyType {
    FS,
    SS,
    FF,
    SF,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectDemand {
    pub project_uuid: Uuid,
    pub priority: i32,
    pub locked: bool,
    pub mandatory_start_day: Option<i32>,
    pub mandatory_finish_day: Option<i32>,
    pub duration_days: u32,
    pub budget_required: f64,
    pub agent_hours_per_day: BTreeMap<Uuid, f64>,
    pub model_quota_per_day: BTreeMap<String, f64>,
    pub dependencies: Vec<ProjectDependency>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectDependency {
    pub predecessor_uuid: Uuid,
    pub dependency_type: DependencyType,
    pub lag_days: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapacitySnapshot {
    pub agent_hours_per_day: BTreeMap<Uuid, f64>,
    pub model_quota_per_day: BTreeMap<String, f64>,
    pub portfolio_budget_limit: f64,
    pub source_state_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OptimizerSelection {
    pub search_uuid: Uuid,
    pub selected_candidate_hash: String,
    pub pareto_hashes: BTreeSet<String>,
    pub selected_strategy: String,
    pub agent_capacity_multiplier: f64,
    pub model_quota_multiplier: f64,
    pub cloud_share_cap: f64,
    pub source_state_sha256: String,
    pub optimizer_evidence_sha256: String,
    pub explicitly_selected: bool,
    pub preapproved_weighted_winner: bool,
}
impl OptimizerSelection {
    pub fn validate(&self, source_state: &str) -> Result<(), PlannerError> {
        if self.source_state_sha256 != source_state {
            return Err(PlannerError::SourceStateMismatch);
        }
        if !self.explicitly_selected && !self.preapproved_weighted_winner {
            return Err(PlannerError::SelectedCandidateRequired);
        }
        if !self.pareto_hashes.contains(&self.selected_candidate_hash) {
            return Err(PlannerError::CandidateNotPareto);
        }
        if !(self.agent_capacity_multiplier.is_finite()
            && self.agent_capacity_multiplier > 0.0
            && self.model_quota_multiplier.is_finite()
            && self.model_quota_multiplier > 0.0
            && self.cloud_share_cap.is_finite()
            && (0.0..=1.0).contains(&self.cloud_share_cap))
        {
            return Err(PlannerError::HardGate);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlannerPolicy {
    pub planning_horizon_days: u32,
    pub minimum_service_share: f64,
    pub starvation_guard_days: u32,
    pub allow_pause_recommendations: bool,
    pub allow_start_recommendations: bool,
    pub allow_budget_increase_without_approval: bool,
    pub allow_scope_change: bool,
    pub allow_baseline_change: bool,
    pub auto_execute: bool,
}
impl PlannerPolicy {
    pub fn validate(&self) -> Result<(), PlannerError> {
        if self.planning_horizon_days == 0 || self.planning_horizon_days > 3650 {
            return Err(PlannerError::HardGate);
        }
        if !(0.0..=1.0).contains(&self.minimum_service_share) {
            return Err(PlannerError::HardGate);
        }
        if self.allow_budget_increase_without_approval
            || self.allow_scope_change
            || self.allow_baseline_change
            || self.auto_execute
        {
            return Err(PlannerError::HardGate);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RecommendationKind {
    Start,
    Hold,
    Pause,
    Resume,
    Continue,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectRoadmapItem {
    pub project_uuid: Uuid,
    pub sequence: u32,
    pub planned_start_day: i32,
    pub planned_finish_day: i32,
    pub recommendation: RecommendationKind,
    pub agent_reservations: BTreeMap<Uuid, f64>,
    pub model_quota_reservations: BTreeMap<String, f64>,
    pub budget_reservation: f64,
    pub rationale: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortfolioPlan {
    pub plan_uuid: Uuid,
    pub source_state_sha256: String,
    pub selected_candidate_hash: String,
    pub items: Vec<ProjectRoadmapItem>,
    pub total_budget_reserved: f64,
    pub cloud_share_cap: f64,
    pub optimizer_evidence_sha256: String,
    pub plan_sha256: String,
    pub requires_approval: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecisionExecutionEnvelope {
    pub plan_uuid: Uuid,
    pub plan_sha256: String,
    pub source_state_sha256: String,
    pub delegate_to: String,
    pub direct_mutation: bool,
    pub required_approvals: Vec<String>,
    pub evidence_sha256: String,
}

pub fn dependency_order(projects: &[ProjectDemand]) -> Result<Vec<Uuid>, PlannerError> {
    let ids: BTreeSet<_> = projects.iter().map(|p| p.project_uuid).collect();
    let mut indeg: BTreeMap<Uuid, usize> = ids.iter().map(|x| (*x, 0usize)).collect();
    let mut out: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    for p in projects {
        for d in &p.dependencies {
            if ids.contains(&d.predecessor_uuid) {
                *indeg.get_mut(&p.project_uuid).unwrap() += 1;
                out.entry(d.predecessor_uuid)
                    .or_default()
                    .push(p.project_uuid)
            }
        }
    }
    let by_id: BTreeMap<Uuid, &ProjectDemand> =
        projects.iter().map(|p| (p.project_uuid, p)).collect();
    let mut q: Vec<_> = indeg
        .iter()
        .filter(|(_, v)| **v == 0)
        .map(|(k, _)| *k)
        .collect();
    q.sort_by(|a, b| {
        by_id[b]
            .priority
            .cmp(&by_id[a].priority)
            .then_with(|| a.cmp(b))
    });
    let mut dq: VecDeque<_> = q.into();
    let mut result = Vec::new();
    while let Some(x) = dq.pop_front() {
        result.push(x);
        if let Some(ns) = out.get(&x) {
            let mut ready = Vec::new();
            for n in ns {
                let e = indeg.get_mut(n).unwrap();
                *e -= 1;
                if *e == 0 {
                    ready.push(*n)
                }
            }
            ready.sort_by(|a, b| {
                by_id[b]
                    .priority
                    .cmp(&by_id[a].priority)
                    .then_with(|| a.cmp(b))
            });
            for n in ready {
                dq.push_back(n)
            }
        }
    }
    if result.len() != projects.len() {
        return Err(PlannerError::DependencyCycle);
    }
    Ok(result)
}

fn dependency_bounds(p: &ProjectDemand, placed: &BTreeMap<Uuid, (i32, i32)>) -> (i32, Option<i32>) {
    let mut min_start = 0i32;
    let mut min_finish = None;
    for d in &p.dependencies {
        if let Some((ps, pf)) = placed.get(&d.predecessor_uuid) {
            match d.dependency_type {
                DependencyType::FS => min_start = min_start.max(*pf + d.lag_days),
                DependencyType::SS => min_start = min_start.max(*ps + d.lag_days),
                DependencyType::FF => {
                    min_finish = Some(min_finish.unwrap_or(i32::MIN).max(*pf + d.lag_days))
                }
                DependencyType::SF => {
                    min_finish = Some(min_finish.unwrap_or(i32::MIN).max(*ps + d.lag_days))
                }
            }
        }
    }
    (min_start, min_finish)
}

fn window_feasible(
    start: i32,
    duration: u32,
    p: &ProjectDemand,
    selection: &OptimizerSelection,
    cap: &CapacitySnapshot,
    agent_use: &BTreeMap<(Uuid, i32), f64>,
    model_use: &BTreeMap<(String, i32), f64>,
) -> bool {
    let finish = start + duration as i32;
    if let Some(s) = p.mandatory_start_day {
        if start > s {
            return false;
        }
    }
    if let Some(f) = p.mandatory_finish_day {
        if finish > f {
            return false;
        }
    }
    for day in start..finish {
        for (a, h) in &p.agent_hours_per_day {
            let limit = cap.agent_hours_per_day.get(a).copied().unwrap_or(0.0)
                * selection.agent_capacity_multiplier;
            let used = agent_use.get(&(*a, day)).copied().unwrap_or(0.0);
            if used + *h > limit + 1e-9 {
                return false;
            }
        }
        for (m, q) in &p.model_quota_per_day {
            let limit = cap.model_quota_per_day.get(m).copied().unwrap_or(0.0)
                * selection.model_quota_multiplier;
            let used = model_use.get(&(m.clone(), day)).copied().unwrap_or(0.0);
            if used + *q > limit + 1e-9 {
                return false;
            }
        }
    }
    true
}

pub fn build_plan(
    source_state: &str,
    projects: &[ProjectDemand],
    selection: &OptimizerSelection,
    cap: &CapacitySnapshot,
    policy: &PlannerPolicy,
) -> Result<PortfolioPlan, PlannerError> {
    selection.validate(source_state)?;
    policy.validate()?;
    if cap.source_state_sha256 != source_state {
        return Err(PlannerError::SourceStateMismatch);
    }
    let order = dependency_order(projects)?;
    let by_id: BTreeMap<Uuid, &ProjectDemand> =
        projects.iter().map(|p| (p.project_uuid, p)).collect();
    let mut placed = BTreeMap::new();
    let mut agent_use = BTreeMap::new();
    let mut model_use = BTreeMap::new();
    let mut items = Vec::new();
    let mut total_budget = 0.0;
    for (idx, id) in order.iter().enumerate() {
        let p = by_id[id];
        let (mut start, min_finish) = dependency_bounds(p, &placed);
        if let Some(ms) = p.mandatory_start_day {
            start = start.max(ms)
        };
        let horizon = policy.planning_horizon_days as i32;
        let mut found = None;
        for s in start..=horizon {
            let mut finish = s + p.duration_days as i32;
            if let Some(mf) = min_finish {
                if finish < mf {
                    finish = mf;
                    let adjusted = (finish - s).max(1) as u32;
                    if !window_feasible(s, adjusted, p, selection, cap, &agent_use, &model_use) {
                        continue;
                    } else {
                        found = Some((s, finish, adjusted));
                        break;
                    }
                }
            }
            if window_feasible(
                s,
                p.duration_days,
                p,
                selection,
                cap,
                &agent_use,
                &model_use,
            ) {
                found = Some((s, finish, p.duration_days));
                break;
            }
        }
        let (s, f, dur) = found.ok_or(PlannerError::Capacity)?;
        total_budget += p.budget_required;
        if total_budget > cap.portfolio_budget_limit + 1e-9 {
            return Err(PlannerError::Budget);
        }
        for day in s..f {
            for (a, h) in &p.agent_hours_per_day {
                *agent_use.entry((*a, day)).or_insert(0.0) += *h
            }
            for (m, q) in &p.model_quota_per_day {
                *model_use.entry((m.clone(), day)).or_insert(0.0) += *q
            }
        }
        placed.insert(*id, (s, f));
        let recommendation = if p.locked {
            RecommendationKind::Continue
        } else if s > 0 {
            RecommendationKind::Hold
        } else {
            RecommendationKind::Start
        };
        let mut rationale = vec![
            format!(
                "selected_optimizer_candidate={}",
                selection.selected_candidate_hash
            ),
            format!("duration_days={dur}"),
        ];
        if p.locked {
            rationale.push("locked_project_preserved".into())
        }
        if s > 0 {
            rationale.push(format!("capacity_or_dependency_delay={s}d"))
        }
        items.push(ProjectRoadmapItem {
            project_uuid: *id,
            sequence: (idx + 1) as u32,
            planned_start_day: s,
            planned_finish_day: f,
            recommendation,
            agent_reservations: p.agent_hours_per_day.clone(),
            model_quota_reservations: p.model_quota_per_day.clone(),
            budget_reservation: p.budget_required,
            rationale,
        });
    }
    let plan_uuid = Uuid::now_v7();
    let requires_approval = items.iter().any(|x| {
        matches!(
            x.recommendation,
            RecommendationKind::Pause | RecommendationKind::Hold
        )
    });
    #[derive(Serialize)]
    struct H<'a> {
        source_state: &'a str,
        selected: &'a str,
        items: &'a [ProjectRoadmapItem],
        budget: f64,
        cloud_share: f64,
        opt_evidence: &'a str,
    }
    let plan_sha256 = canonical_sha256(&H {
        source_state,
        selected: &selection.selected_candidate_hash,
        items: &items,
        budget: total_budget,
        cloud_share: selection.cloud_share_cap,
        opt_evidence: &selection.optimizer_evidence_sha256,
    });
    Ok(PortfolioPlan {
        plan_uuid,
        source_state_sha256: source_state.into(),
        selected_candidate_hash: selection.selected_candidate_hash.clone(),
        items,
        total_budget_reserved: total_budget,
        cloud_share_cap: selection.cloud_share_cap,
        optimizer_evidence_sha256: selection.optimizer_evidence_sha256.clone(),
        plan_sha256,
        requires_approval,
    })
}

pub fn execution_envelope(plan: &PortfolioPlan) -> DecisionExecutionEnvelope {
    let required_approvals = if plan.requires_approval {
        vec!["portfolio_plan".into(), "project_pause_or_hold".into()]
    } else {
        vec!["portfolio_plan".into()]
    };
    #[derive(Serialize)]
    struct E<'a> {
        plan_uuid: Uuid,
        plan_sha256: &'a str,
        source_state: &'a str,
        delegate: &'a str,
        approvals: &'a [String],
    }
    let evidence_sha256 = canonical_sha256(&E {
        plan_uuid: plan.plan_uuid,
        plan_sha256: &plan.plan_sha256,
        source_state: &plan.source_state_sha256,
        delegate: DELEGATE_TO,
        approvals: &required_approvals,
    });
    DecisionExecutionEnvelope {
        plan_uuid: plan.plan_uuid,
        plan_sha256: plan.plan_sha256.clone(),
        source_state_sha256: plan.source_state_sha256.clone(),
        delegate_to: DELEGATE_TO.into(),
        direct_mutation: false,
        required_approvals,
        evidence_sha256,
    }
}

pub fn assert_no_direct_mutation() -> Result<(), PlannerError> {
    if DIRECT_MUTATION_ALLOWED {
        Err(PlannerError::DirectMutationForbidden)
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

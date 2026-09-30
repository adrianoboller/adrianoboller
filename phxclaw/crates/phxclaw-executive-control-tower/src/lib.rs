//! PhxClaw v0.48 — Executive Project Control Tower.
//!
//! Read-only-by-default portfolio/project executive intelligence. Operational mutations
//! are delegated to the governed v0.47 Autonomous Project Supervisor. This crate never
//! bypasses security, privacy, release, knowledge-promotion, fencing or hard-budget gates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use thiserror::Error;
use uuid::Uuid;

pub const CONTROL_TOWER_RULE: &str = "observe -> rank attention -> explain -> delegate governed action";
pub const COST_RULE: &str = "show estimated versus actual cost and Ollama/cloud share";

#[derive(Debug, Error)]
pub enum TowerError {
    #[error("snapshot stale")] Stale,
    #[error("tenant mismatch")] TenantMismatch,
    #[error("project snapshot source-state mismatch")] SourceStateMismatch,
    #[error("executive tower cannot bypass hard gates")] HardGate,
    #[error("operational mutation must be delegated to supervisor")] DelegateRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthWeights {
    pub deadline_risk: f64,
    pub budget_risk: f64,
    pub rework_risk: f64,
    pub quality_gap: f64,
    pub wip: f64,
    pub blocked_work: f64,
    pub agent_saturation: f64,
    pub model_risk: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectExecutiveRow {
    pub project_uuid: Uuid,
    pub project_name: String,
    pub source_state_sha256: String,
    pub generated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub deadline_risk: f64,
    pub budget_risk: f64,
    pub rework_risk: f64,
    pub predicted_quality: f64,
    pub wip_ratio: f64,
    pub blocked_ratio: f64,
    pub agent_saturation: f64,
    pub model_risk: f64,
    pub planned_cost_usd: f64,
    pub actual_cost_usd: f64,
    pub forecast_final_cost_usd: f64,
    pub ollama_cost_usd: f64,
    pub cloud_cost_usd: f64,
    pub active_agents: u32,
    pub queued_tasks: u32,
    pub blocked_tasks: u32,
    pub open_risks_high: u32,
    pub supervisor_open_actions: u32,
    pub fruitful_patterns: u32,
    pub unfruitful_patterns: u32,
    pub evidence_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedProject {
    pub project_uuid: Uuid,
    pub health_score: f64,
    pub attention_score: f64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioExecutiveSnapshot {
    pub snapshot_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub generated_at: DateTime<Utc>,
    pub projects: Vec<ProjectExecutiveRow>,
    pub portfolio_health: f64,
    pub planned_cost_usd: f64,
    pub actual_cost_usd: f64,
    pub forecast_final_cost_usd: f64,
    pub ollama_share: f64,
    pub cloud_share: f64,
    pub active_agents: u32,
    pub blocked_tasks: u32,
    pub high_risks: u32,
    pub evidence_sha256: String,
    pub snapshot_sha256: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum AlertSeverity { Low, Medium, High, Critical }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutiveAlert {
    pub alert_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Option<Uuid>,
    pub severity: AlertSeverity,
    pub kind: String,
    pub title: String,
    pub evidence_sha256: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutiveDecisionRequest {
    pub request_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub supervisor_plan_uuid: Uuid,
    pub action_kind: String,
    pub rationale: String,
    pub expected_effect: String,
    pub evidence_sha256: String,
    pub requires_approval: bool,
}

pub fn project_health(row: &ProjectExecutiveRow, w: &HealthWeights) -> f64 {
    let risk = w.deadline_risk*row.deadline_risk
        + w.budget_risk*row.budget_risk
        + w.rework_risk*row.rework_risk
        + w.quality_gap*(1.0-row.predicted_quality)
        + w.wip*row.wip_ratio
        + w.blocked_work*row.blocked_ratio
        + w.agent_saturation*row.agent_saturation
        + w.model_risk*row.model_risk;
    (1.0-risk).clamp(0.0,1.0)
}

pub fn rank_attention(rows: &[ProjectExecutiveRow], w: &HealthWeights) -> Vec<RankedProject> {
    let mut out:Vec<_> = rows.iter().map(|r| {
        let health=project_health(r,w);
        let mut reasons=Vec::new();
        if r.deadline_risk>=0.6 { reasons.push("deadline_risk".into()); }
        if r.budget_risk>=0.6 { reasons.push("budget_risk".into()); }
        if r.rework_risk>=0.5 { reasons.push("rework_risk".into()); }
        if r.blocked_tasks>0 { reasons.push("blocked_work".into()); }
        if r.open_risks_high>0 { reasons.push("high_risk".into()); }
        if r.unfruitful_patterns>r.fruitful_patterns { reasons.push("unfruitful_knowledge_pressure".into()); }
        let attention=(1.0-health) + (r.supervisor_open_actions as f64).min(5.0)*0.03;
        RankedProject{project_uuid:r.project_uuid,health_score:health,attention_score:attention.clamp(0.0,1.5),reasons}
    }).collect();
    out.sort_by(|a,b| b.attention_score.partial_cmp(&a.attention_score).unwrap_or(Ordering::Equal).then_with(||a.project_uuid.cmp(&b.project_uuid)));
    out
}

pub fn rollup_portfolio(tenant_uuid:Uuid, rows:Vec<ProjectExecutiveRow>, w:&HealthWeights, now:DateTime<Utc>) -> Result<PortfolioExecutiveSnapshot,TowerError> {
    if rows.iter().any(|r| r.expires_at < now) { return Err(TowerError::Stale); }
    let planned=rows.iter().map(|r|r.planned_cost_usd).sum();
    let actual=rows.iter().map(|r|r.actual_cost_usd).sum();
    let forecast=rows.iter().map(|r|r.forecast_final_cost_usd).sum();
    let ollama: f64=rows.iter().map(|r|r.ollama_cost_usd).sum();
    let cloud: f64=rows.iter().map(|r|r.cloud_cost_usd).sum();
    let total_ai=ollama+cloud;
    let health=if rows.is_empty(){1.0}else{rows.iter().map(|r|project_health(r,w)).sum::<f64>()/rows.len() as f64};
    let evidence_payload=serde_json::json!({"tenant_uuid":tenant_uuid,"projects":rows.iter().map(|r|(&r.project_uuid,&r.source_state_sha256,&r.evidence_sha256)).collect::<Vec<_>>()});
    let evidence_sha256=format!("{:x}",Sha256::digest(serde_json::to_vec(&evidence_payload).expect("evidence json")));
    let canonical=serde_json::json!({"tenant_uuid":tenant_uuid,"generated_at":now,"portfolio_health":health,"planned":planned,"actual":actual,"forecast":forecast,"ollama":ollama,"cloud":cloud,"evidence_sha256":evidence_sha256});
    let snapshot_sha256=format!("{:x}",Sha256::digest(serde_json::to_vec(&canonical).expect("snapshot json")));
    let active_agents=rows.iter().map(|r|r.active_agents).sum();
    let blocked_tasks=rows.iter().map(|r|r.blocked_tasks).sum();
    let high_risks=rows.iter().map(|r|r.open_risks_high).sum();
    Ok(PortfolioExecutiveSnapshot{
        snapshot_uuid:Uuid::now_v7(),tenant_uuid,generated_at:now,projects:rows,
        portfolio_health:health,planned_cost_usd:planned,actual_cost_usd:actual,forecast_final_cost_usd:forecast,
        ollama_share:if total_ai>0.0{ollama/total_ai}else{0.0},cloud_share:if total_ai>0.0{cloud/total_ai}else{0.0},
        active_agents,blocked_tasks,high_risks,evidence_sha256,snapshot_sha256
    })
}

pub fn mutation_allowed_locally(action_kind:&str)->Result<(),TowerError>{
    match action_kind {
        "dashboard_preference"|"alert_acknowledge" => Ok(()),
        _ => Err(TowerError::DelegateRequired),
    }
}

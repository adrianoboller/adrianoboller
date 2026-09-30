//! PhxClaw v0.46 — Predictive Project Intelligence & Autonomous Cost Optimizer.
//!
//! Forecasts are advisory and evidence-bound. This crate cannot relax model eligibility,
//! knowledge promotion, budget, lease or fencing policies owned by earlier PhxClaw layers.

use chrono::{DateTime, Duration, Utc};
use phxclaw_active_project_runtime::ProjectTask;
use phxclaw_agent_control_plane::{Complexity, FruitfulKnowledge, UnfruitfulKnowledge};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

pub const LOCAL_FIRST_POLICY: &str = "Ollama local-first when eligible and evidence-supported";

#[derive(Debug, Error)]
pub enum PredictiveError {
    #[error("insufficient governed history")]
    InsufficientHistory,
    #[error("historical evidence is stale")]
    StaleHistory,
    #[error("tenant/project mismatch")]
    ProjectMismatch,
    #[error("prediction input contains non-governed evidence")]
    UngovernedEvidence,
    #[error("no route satisfies the predictive policy")]
    NoRecommendedRoute,
    #[error("forecast source state mismatch")]
    SourceStateMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalOutcome {
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub run_uuid: Uuid,
    pub task_class: String,
    pub context_fingerprint: String,
    pub source_state_sha256: String,
    pub agent_uuid: Uuid,
    pub model_profile_uuid: Uuid,
    pub provider: String,
    pub local: bool,
    pub skill_set_sha256: String,
    pub complexity: Complexity,
    pub success: bool,
    pub quality_score: f64,
    pub actual_cost_usd: f64,
    pub duration_ms: u64,
    pub retry_count: u32,
    pub governed: bool,
    pub evidence_sha256: String,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictivePolicy {
    pub min_governed_samples: usize,
    pub history_window_days: i64,
    pub forecast_ttl_seconds: i64,
    pub confidence_floor: f64,
    pub max_predicted_rework_risk: f64,
    pub max_predicted_deadline_risk: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteForecast {
    pub forecast_uuid: Uuid,
    pub project_uuid: Uuid,
    pub task_class: String,
    pub context_fingerprint: String,
    pub agent_uuid: Uuid,
    pub model_profile_uuid: Uuid,
    pub provider: String,
    pub local: bool,
    pub sample_count: usize,
    pub success_probability: f64,
    pub expected_quality: f64,
    pub expected_cost_usd: f64,
    pub expected_duration_ms: u64,
    pub expected_retries: f64,
    pub effective_cost_usd: f64,
    pub confidence: f64,
    pub generated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub evidence_sha256: String,
    pub forecast_sha256: String,
}

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        0.0
    } else {
        xs.iter().sum::<f64>() / xs.len() as f64
    }
}

fn mean_u64(xs: &[u64]) -> u64 {
    if xs.is_empty() {
        0
    } else {
        (xs.iter().map(|x| *x as u128).sum::<u128>() / xs.len() as u128) as u64
    }
}

fn confidence(n: usize) -> f64 {
    (n as f64 / (n as f64 + 5.0)).clamp(0.0, 0.99)
}

#[allow(clippy::too_many_arguments)]
pub fn forecast_route(
    project_uuid: Uuid,
    task_class: &str,
    context_fingerprint: &str,
    agent_uuid: Uuid,
    model_profile_uuid: Uuid,
    history: &[HistoricalOutcome],
    policy: &PredictivePolicy,
    now: DateTime<Utc>,
) -> Result<RouteForecast, PredictiveError> {
    let cutoff = now - Duration::days(policy.history_window_days);
    let samples: Vec<&HistoricalOutcome> = history
        .iter()
        .filter(|h| {
            h.project_uuid == project_uuid
                && h.task_class == task_class
                && h.context_fingerprint == context_fingerprint
                && h.agent_uuid == agent_uuid
                && h.model_profile_uuid == model_profile_uuid
                && h.observed_at >= cutoff
        })
        .collect();
    if samples.iter().any(|h| !h.governed) {
        return Err(PredictiveError::UngovernedEvidence);
    }
    if samples.len() < policy.min_governed_samples {
        return Err(PredictiveError::InsufficientHistory);
    }
    let success_probability =
        samples.iter().filter(|h| h.success).count() as f64 / samples.len() as f64;
    let expected_quality = mean(&samples.iter().map(|h| h.quality_score).collect::<Vec<_>>());
    let expected_cost_usd = mean(
        &samples
            .iter()
            .map(|h| h.actual_cost_usd)
            .collect::<Vec<_>>(),
    );
    let expected_duration_ms = mean_u64(&samples.iter().map(|h| h.duration_ms).collect::<Vec<_>>());
    let expected_retries = mean(
        &samples
            .iter()
            .map(|h| h.retry_count as f64)
            .collect::<Vec<_>>(),
    );
    let effective_cost_usd = if success_probability > 0.0 {
        expected_cost_usd / success_probability
    } else {
        f64::INFINITY
    };
    let conf = confidence(samples.len());
    let provider = samples[0].provider.clone();
    let local = samples[0].local;
    let evidence_sha256 = format!(
        "{:x}",
        Sha256::digest(
            samples
                .iter()
                .flat_map(|h| h.evidence_sha256.as_bytes())
                .copied()
                .collect::<Vec<_>>()
        )
    );
    let forecast_uuid = Uuid::now_v7();
    let expires_at = now + Duration::seconds(policy.forecast_ttl_seconds);
    let canonical = serde_json::json!({
        "forecast_uuid": forecast_uuid, "project_uuid": project_uuid, "task_class": task_class,
        "context_fingerprint": context_fingerprint, "agent_uuid": agent_uuid,
        "model_profile_uuid": model_profile_uuid, "sample_count": samples.len(),
        "success_probability": success_probability, "expected_quality": expected_quality,
        "expected_cost_usd": expected_cost_usd, "expected_duration_ms": expected_duration_ms,
        "expected_retries": expected_retries, "evidence_sha256": evidence_sha256,
        "expires_at": expires_at,
    });
    let forecast_sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&canonical).expect("forecast canonical json"))
    );
    Ok(RouteForecast {
        forecast_uuid,
        project_uuid,
        task_class: task_class.into(),
        context_fingerprint: context_fingerprint.into(),
        agent_uuid,
        model_profile_uuid,
        provider,
        local,
        sample_count: samples.len(),
        success_probability,
        expected_quality,
        expected_cost_usd,
        expected_duration_ms,
        expected_retries,
        effective_cost_usd,
        confidence: conf,
        generated_at: now,
        expires_at,
        evidence_sha256,
        forecast_sha256,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRecommendation {
    pub recommendation_uuid: Uuid,
    pub task_uuid: Uuid,
    pub selected_forecast_uuid: Uuid,
    pub selected_model_profile_uuid: Uuid,
    pub selected_agent_uuid: Uuid,
    pub provider: String,
    pub local: bool,
    pub predicted_cost_usd: f64,
    pub predicted_quality: f64,
    pub predicted_success_probability: f64,
    pub predicted_duration_ms: u64,
    pub rationale: Vec<String>,
    pub requires_cloud_escalation: bool,
    pub recommendation_sha256: String,
}

pub fn recommend_route(
    task: &ProjectTask,
    forecasts: &[RouteForecast],
    policy: &PredictivePolicy,
    now: DateTime<Utc>,
) -> Result<RouteRecommendation, PredictiveError> {
    let mut eligible = forecasts
        .iter()
        .filter(|f| {
            f.project_uuid == task.project_uuid
                && f.task_class == task.task_class
                && f.context_fingerprint == task.context_fingerprint
                && f.expires_at >= now
                && f.confidence >= policy.confidence_floor
                && f.expected_quality >= task.quality_floor
                && f.expected_cost_usd <= task.max_estimated_cost_usd
                && (1.0 - f.success_probability) <= policy.max_predicted_rework_risk
        })
        .collect::<Vec<_>>();
    if eligible.is_empty() {
        return Err(PredictiveError::NoRecommendedRoute);
    }
    eligible.sort_by(|a, b| {
        // local-first only among routes that satisfy the same hard prediction gates.
        let la = if a.local { 0 } else { 1 };
        let lb = if b.local { 0 } else { 1 };
        la.cmp(&lb)
            .then_with(|| a.effective_cost_usd.total_cmp(&b.effective_cost_usd))
            .then_with(|| b.expected_quality.total_cmp(&a.expected_quality))
            .then_with(|| b.success_probability.total_cmp(&a.success_probability))
            .then_with(|| a.expected_duration_ms.cmp(&b.expected_duration_ms))
            .then_with(|| a.model_profile_uuid.cmp(&b.model_profile_uuid))
    });
    let s = eligible[0];
    let recommendation_uuid = Uuid::now_v7();
    let rationale = vec![
        "governed historical evidence matched exact task class and context".into(),
        if s.local {
            "local/Ollama route satisfies predictive quality and cost gates".into()
        } else {
            "cloud route justified because no eligible local forecast ranked ahead".into()
        },
        "final execution still requires v0.45 routing, budget, lease and fencing gates".into(),
    ];
    let canonical = serde_json::json!({"recommendation_uuid":recommendation_uuid,"task_uuid":task.task_uuid,
        "forecast_uuid":s.forecast_uuid,"model_profile_uuid":s.model_profile_uuid,"agent_uuid":s.agent_uuid,
        "forecast_sha256":s.forecast_sha256,"source_state_sha256":task.source_state_sha256});
    let recommendation_sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&canonical).expect("recommendation json"))
    );
    Ok(RouteRecommendation {
        recommendation_uuid,
        task_uuid: task.task_uuid,
        selected_forecast_uuid: s.forecast_uuid,
        selected_model_profile_uuid: s.model_profile_uuid,
        selected_agent_uuid: s.agent_uuid,
        provider: s.provider.clone(),
        local: s.local,
        predicted_cost_usd: s.expected_cost_usd,
        predicted_quality: s.expected_quality,
        predicted_success_probability: s.success_probability,
        predicted_duration_ms: s.expected_duration_ms,
        rationale,
        requires_cloud_escalation: !s.local,
        recommendation_sha256,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskForecast {
    pub task_uuid: Uuid,
    pub expected_cost_usd: f64,
    pub expected_duration_ms: u64,
    pub success_probability: f64,
    pub rework_risk: f64,
    pub recommendation_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectForecast {
    pub forecast_uuid: Uuid,
    pub project_uuid: Uuid,
    pub task_count: usize,
    pub expected_remaining_cost_usd: f64,
    pub expected_remaining_duration_ms: u64,
    pub expected_rework_cost_usd: f64,
    pub deadline_risk: f64,
    pub budget_overrun_risk: f64,
    pub generated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub source_state_sha256: String,
    pub forecast_sha256: String,
}

pub fn forecast_project(
    project_uuid: Uuid,
    task_forecasts: &[TaskForecast],
    remaining_budget_usd: f64,
    millis_to_deadline: u64,
    source_state_sha256: &str,
    ttl_seconds: i64,
    now: DateTime<Utc>,
) -> ProjectForecast {
    let expected_remaining_cost_usd = task_forecasts
        .iter()
        .map(|x| x.expected_cost_usd)
        .sum::<f64>();
    let expected_remaining_duration_ms = task_forecasts
        .iter()
        .map(|x| x.expected_duration_ms)
        .sum::<u64>();
    let expected_rework_cost_usd = task_forecasts
        .iter()
        .map(|x| x.expected_cost_usd * x.rework_risk)
        .sum::<f64>();
    // max/min e nao clamp de proposito: com NaN na entrada, max(0.0) devolve 0.0 e o risco
    // continua numero; clamp propagaria o NaN para o painel.
    #[allow(clippy::manual_clamp)]
    let deadline_risk = if millis_to_deadline == 0 {
        1.0
    } else {
        (expected_remaining_duration_ms as f64 / millis_to_deadline as f64 - 0.8)
            .max(0.0)
            .min(1.0)
    };
    let total_expected = expected_remaining_cost_usd + expected_rework_cost_usd;
    #[allow(clippy::manual_clamp)]
    let budget_overrun_risk = if remaining_budget_usd <= 0.0 {
        if total_expected > 0.0 {
            1.0
        } else {
            0.0
        }
    } else {
        (total_expected / remaining_budget_usd - 0.8)
            .max(0.0)
            .min(1.0)
    };
    let forecast_uuid = Uuid::now_v7();
    let expires_at = now + Duration::seconds(ttl_seconds);
    let canonical = serde_json::json!({"forecast_uuid":forecast_uuid,"project_uuid":project_uuid,"task_count":task_forecasts.len(),
        "expected_cost":expected_remaining_cost_usd,"expected_duration_ms":expected_remaining_duration_ms,
        "expected_rework_cost":expected_rework_cost_usd,"deadline_risk":deadline_risk,"budget_overrun_risk":budget_overrun_risk,
        "source_state_sha256":source_state_sha256,"expires_at":expires_at});
    let forecast_sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&canonical).expect("project forecast json"))
    );
    ProjectForecast {
        forecast_uuid,
        project_uuid,
        task_count: task_forecasts.len(),
        expected_remaining_cost_usd,
        expected_remaining_duration_ms,
        expected_rework_cost_usd,
        deadline_risk,
        budget_overrun_risk,
        generated_at: now,
        expires_at,
        source_state_sha256: source_state_sha256.into(),
        forecast_sha256,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeSignalSummary {
    pub fruitful_matches: usize,
    pub unfruitful_matches: usize,
    pub best_known_cost_usd: Option<f64>,
    pub best_known_quality: Option<f64>,
    pub avoidance_rules: Vec<String>,
}

pub fn summarize_knowledge(
    task_class: &str,
    context_fingerprint: &str,
    fruitful: &[FruitfulKnowledge],
    unfruitful: &[UnfruitfulKnowledge],
) -> KnowledgeSignalSummary {
    let fs = fruitful
        .iter()
        .filter(|k| {
            k.task_class == task_class
                && k.context_fingerprint == context_fingerprint
                && k.promotion_state != "candidate"
        })
        .collect::<Vec<_>>();
    let us = unfruitful
        .iter()
        .filter(|k| {
            k.task_class == task_class
                && k.context_fingerprint == context_fingerprint
                && k.promotion_state != "candidate"
        })
        .collect::<Vec<_>>();
    KnowledgeSignalSummary {
        fruitful_matches: fs.len(),
        unfruitful_matches: us.len(),
        best_known_cost_usd: fs
            .iter()
            .map(|k| k.actual_cost_usd)
            .min_by(|a, b| a.total_cmp(b)),
        best_known_quality: fs
            .iter()
            .map(|k| k.quality_score)
            .max_by(|a, b| a.total_cmp(b)),
        avoidance_rules: us.iter().map(|k| k.avoidance_rule.clone()).collect(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastError {
    pub forecast_uuid: Uuid,
    pub cost_absolute_error: f64,
    pub duration_absolute_error_ms: u64,
    pub quality_absolute_error: f64,
    pub success_prediction_correct: bool,
}

pub fn backtest_route(
    f: &RouteForecast,
    actual_success: bool,
    actual_quality: f64,
    actual_cost_usd: f64,
    actual_duration_ms: u64,
) -> ForecastError {
    ForecastError {
        forecast_uuid: f.forecast_uuid,
        cost_absolute_error: (f.expected_cost_usd - actual_cost_usd).abs(),
        duration_absolute_error_ms: f.expected_duration_ms.abs_diff(actual_duration_ms),
        quality_absolute_error: (f.expected_quality - actual_quality).abs(),
        success_prediction_correct: (f.success_probability >= 0.5) == actual_success,
    }
}

pub fn summarize_cost_by_provider(history: &[HistoricalOutcome]) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    for h in history {
        *out.entry(h.provider.clone()).or_insert(0.0) += h.actual_cost_usd;
    }
    out
}

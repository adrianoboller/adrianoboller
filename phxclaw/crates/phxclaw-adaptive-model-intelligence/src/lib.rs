use phxclaw_ai_benchmark::{ComplexityBand, PerformanceProfile, TaskFamily};
use phxclaw_ai_fabric::{route, ModelRecord, RouteRequest, RouteSelection, RoutingPolicy};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdaptivePolicy {
    pub profile_required: bool,
    pub min_samples: u32,
    pub min_coverage_basis_points: u16,
    pub min_success_basis_points: u16,
    pub min_quality_basis_points: u16,
    pub quality_weight: u16,
    pub success_weight: u16,
    pub latency_weight: u16,
    pub cost_weight: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromotedProfile {
    pub promotion_uuid: Uuid,
    pub policy_sha256: String,
    pub promoted_at_unix: i64,
    pub profile: PerformanceProfile,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdaptiveRequest {
    pub route: RouteRequest,
    pub task_family: TaskFamily,
    pub complexity: ComplexityBand,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdaptiveCandidate {
    pub provider_uuid: Uuid,
    pub model_id: String,
    pub profile_sha256: Option<String>,
    pub adaptive_score: Option<i64>,
    pub eligible: bool,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdaptiveSelection {
    pub base_decision_sha256: String,
    pub selected_provider_uuid: Uuid,
    pub selected_model_id: String,
    pub candidates: Vec<AdaptiveCandidate>,
    pub adaptive_decision_sha256: String,
    pub used_benchmark_profiles: bool,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum AdaptiveError {
    #[error("base routing failed: {0}")]
    Base(String),
    #[error("benchmark profile required")]
    ProfileRequired,
}
fn fresh(now: i64, p: &PerformanceProfile) -> bool {
    p.observed_at_unix <= now && now.saturating_sub(p.observed_at_unix) <= p.ttl_seconds as i64
}
fn complexity_ok(p: &PerformanceProfile, c: ComplexityBand) -> bool {
    p.complexity >= c
}
fn score(policy: &AdaptivePolicy, p: &PerformanceProfile) -> i64 {
    let q = p.quality_basis_points as i64 * policy.quality_weight as i64;
    let s = p.success_basis_points as i64 * policy.success_weight as i64;
    let latency_penalty = (p.p95_latency_ms.min(120_000) as i64) * policy.latency_weight as i64;
    let cost_penalty =
        (p.median_cost_micro_usd.unwrap_or(0).min(10_000_000) as i64) * policy.cost_weight as i64;
    q + s - latency_penalty - cost_penalty
}

pub fn route_adaptive(
    now: i64,
    fabric: &RoutingPolicy,
    policy: &AdaptivePolicy,
    req: &AdaptiveRequest,
    models: &[ModelRecord],
    profiles: &[PromotedProfile],
) -> Result<AdaptiveSelection, AdaptiveError> {
    let mut broad = fabric.clone();
    broad.max_fallbacks = models.len();
    let base: RouteSelection =
        route(now, &broad, &req.route, models).map_err(|e| AdaptiveError::Base(e.to_string()))?;
    let mut eligible_keys = Vec::new();
    eligible_keys.push((base.selected_provider_uuid, base.selected_model_id.clone()));
    eligible_keys.extend(base.fallback_chain.iter().cloned());
    eligible_keys.sort_by(|a, b| (a.0.to_string(), &a.1).cmp(&(b.0.to_string(), &b.1)));
    eligible_keys.dedup();
    let mut best: BTreeMap<(Uuid, String), &PerformanceProfile> = BTreeMap::new();
    for promoted in profiles {
        if promoted.policy_sha256.len() != 64
            || !promoted
                .policy_sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            continue;
        }
        let p = &promoted.profile;
        if p.tenant_uuid != req.route.tenant_uuid
            || p.task_family != req.task_family
            || !complexity_ok(p, req.complexity)
            || !fresh(now, p)
            || p.sample_count < policy.min_samples
            || p.evidence_coverage_basis_points < policy.min_coverage_basis_points
            || p.success_basis_points < policy.min_success_basis_points
            || p.quality_basis_points < policy.min_quality_basis_points
        {
            continue;
        }
        let key = (p.provider_uuid, p.model_id.clone());
        if !eligible_keys.contains(&key) {
            continue;
        }
        let replace = best
            .get(&key)
            .map(|old| {
                (p.observed_at_unix, &p.profile_sha256)
                    > (old.observed_at_unix, &old.profile_sha256)
            })
            .unwrap_or(true);
        if replace {
            best.insert(key, p);
        }
    }
    if policy.profile_required && best.is_empty() {
        return Err(AdaptiveError::ProfileRequired);
    }
    let mut candidates = Vec::new();
    let mut ranked = Vec::new();
    for key in &eligible_keys {
        if let Some(p) = best.get(key) {
            let sc = score(policy, p);
            ranked.push((
                std::cmp::Reverse(sc),
                key.0,
                key.1.clone(),
                p.profile_sha256.clone(),
            ));
            candidates.push(AdaptiveCandidate {
                provider_uuid: key.0,
                model_id: key.1.clone(),
                profile_sha256: Some(p.profile_sha256.clone()),
                adaptive_score: Some(sc),
                eligible: true,
                reason: "benchmark_profile".into(),
            });
        } else {
            candidates.push(AdaptiveCandidate {
                provider_uuid: key.0,
                model_id: key.1.clone(),
                profile_sha256: None,
                adaptive_score: None,
                eligible: !policy.profile_required,
                reason: "no_fresh_promoted_profile".into(),
            });
        }
    }
    candidates.sort_by(|a, b| {
        (a.provider_uuid.to_string(), &a.model_id).cmp(&(b.provider_uuid.to_string(), &b.model_id))
    });
    ranked.sort();
    let (selected_provider_uuid, selected_model_id, used) =
        if let Some((_, p, m, _)) = ranked.first() {
            (*p, m.clone(), true)
        } else {
            (
                base.selected_provider_uuid,
                base.selected_model_id.clone(),
                false,
            )
        };
    let canonical = serde_json::to_vec(&(
        &base.decision_sha256,
        selected_provider_uuid,
        &selected_model_id,
        &candidates,
    ))
    .expect("adaptive decision serialization");
    Ok(AdaptiveSelection {
        base_decision_sha256: base.decision_sha256,
        selected_provider_uuid,
        selected_model_id,
        candidates,
        adaptive_decision_sha256: hex::encode(Sha256::digest(canonical)),
        used_benchmark_profiles: used,
    })
}

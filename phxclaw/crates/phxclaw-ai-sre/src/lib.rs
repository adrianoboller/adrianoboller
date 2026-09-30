use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RequestClass {
    Critical,
    Interactive,
    Batch,
    Background,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum BudgetState {
    Healthy,
    SoftLimit,
    HardLimit,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum IncidentSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum IncidentSignalKind {
    SloBurn,
    Latency,
    QueueBackpressure,
    RateLimitExhaustion,
    BudgetSoft,
    BudgetHard,
    ProviderCapacity,
    CircuitOpen,
    Drift,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SloObjective {
    pub min_success_basis_points: u16,
    pub max_p95_latency_ms: u64,
    pub max_p95_queue_wait_ms: u64,
    pub fast_burn_limit_milli: u32,
    pub slow_burn_limit_milli: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForecastPolicy {
    pub min_samples: u16,
    pub horizon_seconds: u64,
    pub headroom_basis_points: u16,
    pub max_sample_age_seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueuePolicy {
    pub max_depth: u32,
    pub hard_reject_depth: u32,
    pub max_wait_ms: u64,
    pub class_weights: BTreeMap<RequestClass, u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CostPolicy {
    pub budget_account_uuid: Uuid,
    pub soft_limit_basis_points: u16,
    pub hard_limit_basis_points: u16,
    pub require_known_cost: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutopilotBounds {
    pub max_concurrency_increase_basis_points: u16,
    pub max_concurrency_decrease_basis_points: u16,
    pub max_share_move_basis_points: u16,
    pub max_auto_projected_cost_increase_basis_points: u16,
    pub allow_auto_capacity_purchase: bool,
    pub cooldown_seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SrePolicySpec {
    pub tenant_uuid: Uuid,
    pub policy_uuid: Uuid,
    pub service_name: String,
    pub portfolio_document_sha256: String,
    pub starts_at_unix: i64,
    pub expires_at_unix: i64,
    pub slo: SloObjective,
    pub forecast: ForecastPolicy,
    pub queue: QueuePolicy,
    pub cost: CostPolicy,
    pub autopilot: AutopilotBounds,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedSrePolicyDocument {
    pub document_sha256: String,
    pub signer_id: String,
    pub signature_hex: String,
    pub spec: SrePolicySpec,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifiedSrePolicyDocument {
    pub document_sha256: String,
    pub signer_id: String,
    pub spec: SrePolicySpec,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SloSample {
    pub sample_uuid: Uuid,
    pub success_count: u64,
    pub total_count: u64,
    pub p95_latency_ms: u64,
    pub p95_queue_wait_ms: u64,
    pub observed_at_unix: i64,
    pub window_seconds: u64,
    pub evidence_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SloEvaluation {
    pub availability_basis_points: u16,
    pub error_budget_burn_milli: u32,
    pub latency_violated: bool,
    pub queue_wait_violated: bool,
    pub fast_burn: bool,
    pub slow_burn: bool,
    pub source_evidence_hashes: Vec<String>,
    pub evaluation_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DemandSample {
    pub sample_uuid: Uuid,
    pub window_start_unix: i64,
    pub window_seconds: u64,
    pub requests: u64,
    pub tokens: u64,
    pub peak_concurrency: u32,
    pub evidence_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DemandForecast {
    pub requests_per_minute: u64,
    pub tokens_per_minute: u64,
    pub peak_concurrency: u32,
    pub recommended_concurrency: u32,
    pub horizon_seconds: u64,
    pub source_evidence_hashes: Vec<String>,
    pub forecast_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderRateLimitSnapshot {
    pub tenant_uuid: Uuid,
    pub snapshot_uuid: Uuid,
    pub provider_uuid: Uuid,
    pub request_limit: u64,
    pub requests_remaining: u64,
    pub token_limit: u64,
    pub tokens_remaining: u64,
    pub reset_at_unix: i64,
    pub observed_at_unix: i64,
    pub ttl_seconds: u64,
    pub snapshot_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedRateLimitSnapshot {
    pub signer_id: String,
    pub signature_hex: String,
    pub snapshot: ProviderRateLimitSnapshot,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifiedRateLimitSnapshot {
    pub signer_id: String,
    pub snapshot: ProviderRateLimitSnapshot,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdmissionRequest {
    pub request_uuid: Uuid,
    pub provider_uuid: Uuid,
    pub class: RequestClass,
    pub estimated_tokens: u64,
    pub queue_depth: u32,
    pub queue_wait_ms: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionAction {
    Admit,
    Queue,
    Reject,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdmissionDecision {
    pub request_uuid: Uuid,
    pub action: AdmissionAction,
    pub priority_score: u32,
    pub reason: String,
    pub rate_limit_snapshot_sha256: String,
    pub decision_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetAccountSnapshot {
    pub account_uuid: Uuid,
    pub limit_micro_usd: u64,
    pub spent_micro_usd: u64,
    pub reserved_micro_usd: u64,
    pub observed_at_unix: i64,
    pub snapshot_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CostSample {
    pub window_start_unix: i64,
    pub window_seconds: u64,
    pub spent_micro_usd: u64,
    pub evidence_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CostForecast {
    pub account_uuid: Uuid,
    pub budget_limit_micro_usd: u64,
    pub current_committed_micro_usd: u64,
    pub projected_total_micro_usd: u64,
    pub projected_budget_basis_points: u16,
    pub state: BudgetState,
    pub source_evidence_hashes: Vec<String>,
    pub forecast_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncidentSignal {
    pub kind: IncidentSignalKind,
    pub severity: IncidentSeverity,
    pub evidence_sha256: String,
    pub summary: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncidentDetection {
    pub incident_uuid: Uuid,
    pub severity: IncidentSeverity,
    pub signals: Vec<IncidentSignal>,
    pub detection_sha256: String,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SreError {
    #[error("invalid SRE specification")]
    InvalidSpec,
    #[error("invalid hash")]
    InvalidHash,
    #[error("signer is not trusted")]
    UntrustedSigner,
    #[error("invalid signature")]
    InvalidSignature,
    #[error("policy is inactive")]
    InactivePolicy,
    #[error("insufficient forecast samples")]
    InsufficientSamples,
    #[error("telemetry sample is stale or from the future")]
    StaleSample,
    #[error("rate-limit snapshot is stale")]
    StaleRateLimit,
    #[error("rate-limit snapshot is invalid")]
    InvalidRateLimit,
    #[error("budget account mismatch")]
    BudgetMismatch,
    #[error("contradictory incident evidence")]
    ContradictoryIncidentEvidence,
}

fn valid_sha(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn ceil_div(n: u128, d: u128) -> u128 {
    if d == 0 {
        u128::MAX
    } else {
        (n + d - 1) / d
    }
}
fn policy_active(now_unix: i64, p: &VerifiedSrePolicyDocument) -> bool {
    now_unix >= p.spec.starts_at_unix && now_unix <= p.spec.expires_at_unix
}
fn window_is_fresh(now_unix: i64, start: i64, seconds: u64, max_age: u64) -> bool {
    let end = start.saturating_add(seconds as i64);
    end <= now_unix && now_unix.saturating_sub(end) <= max_age as i64
}
pub fn rate_limit_is_fresh(now_unix: i64, s: &ProviderRateLimitSnapshot) -> bool {
    s.observed_at_unix <= now_unix
        && now_unix.saturating_sub(s.observed_at_unix) <= s.ttl_seconds as i64
        && now_unix < s.reset_at_unix
}

pub fn canonical_policy_bytes(spec: &SrePolicySpec) -> Vec<u8> {
    serde_json::to_vec(spec).expect("SRE policy serialization")
}

pub fn validate_policy(spec: &SrePolicySpec) -> Result<(), SreError> {
    if spec.service_name.trim().is_empty()
        || !valid_sha(&spec.portfolio_document_sha256)
        || spec.starts_at_unix >= spec.expires_at_unix
    {
        return Err(SreError::InvalidSpec);
    }
    if spec.slo.min_success_basis_points > 10_000
        || spec.slo.fast_burn_limit_milli == 0
        || spec.slo.slow_burn_limit_milli == 0
    {
        return Err(SreError::InvalidSpec);
    }
    if spec.forecast.min_samples < 2
        || spec.forecast.horizon_seconds == 0
        || spec.forecast.headroom_basis_points > 10_000
        || spec.forecast.max_sample_age_seconds == 0
    {
        return Err(SreError::InvalidSpec);
    }
    if spec.queue.max_depth == 0
        || spec.queue.hard_reject_depth < spec.queue.max_depth
        || spec.queue.class_weights.values().any(|v| *v == 0)
    {
        return Err(SreError::InvalidSpec);
    }
    if spec.cost.soft_limit_basis_points > spec.cost.hard_limit_basis_points
        || spec.cost.hard_limit_basis_points > 10_000
    {
        return Err(SreError::InvalidSpec);
    }
    if spec.autopilot.max_concurrency_increase_basis_points > 10_000
        || spec.autopilot.max_concurrency_decrease_basis_points > 10_000
        || spec.autopilot.max_share_move_basis_points > 10_000
        || spec.autopilot.max_auto_projected_cost_increase_basis_points > 10_000
    {
        return Err(SreError::InvalidSpec);
    }
    Ok(())
}

pub fn verify_signed_policy(
    now_unix: i64,
    doc: &SignedSrePolicyDocument,
    trusted_keys: &BTreeMap<String, String>,
) -> Result<VerifiedSrePolicyDocument, SreError> {
    validate_policy(&doc.spec)?;
    let provisional = VerifiedSrePolicyDocument {
        document_sha256: doc.document_sha256.clone(),
        signer_id: doc.signer_id.clone(),
        spec: doc.spec.clone(),
    };
    if !policy_active(now_unix, &provisional) {
        return Err(SreError::InactivePolicy);
    }
    let body = canonical_policy_bytes(&doc.spec);
    if sha(&body) != doc.document_sha256 || !valid_sha(&doc.document_sha256) {
        return Err(SreError::InvalidHash);
    }
    let key_hex = trusted_keys
        .get(&doc.signer_id)
        .ok_or(SreError::UntrustedSigner)?;
    let kb = hex::decode(key_hex).map_err(|_| SreError::InvalidSignature)?;
    let ka: [u8; 32] = kb.try_into().map_err(|_| SreError::InvalidSignature)?;
    let vk = VerifyingKey::from_bytes(&ka).map_err(|_| SreError::InvalidSignature)?;
    let sb = hex::decode(&doc.signature_hex).map_err(|_| SreError::InvalidSignature)?;
    let sa: [u8; 64] = sb.try_into().map_err(|_| SreError::InvalidSignature)?;
    vk.verify(&body, &Signature::from_bytes(&sa))
        .map_err(|_| SreError::InvalidSignature)?;
    Ok(provisional)
}

pub fn evaluate_slo(
    now_unix: i64,
    policy: &VerifiedSrePolicyDocument,
    samples: &[SloSample],
) -> Result<SloEvaluation, SreError> {
    if !policy_active(now_unix, policy) {
        return Err(SreError::InactivePolicy);
    }
    if samples.is_empty() {
        return Err(SreError::InsufficientSamples);
    }
    let mut total = 0u128;
    let mut success = 0u128;
    let mut max_latency = 0u64;
    let mut max_queue = 0u64;
    let mut hashes = Vec::new();
    for s in samples {
        if s.total_count == 0
            || s.success_count > s.total_count
            || s.window_seconds == 0
            || !valid_sha(&s.evidence_sha256)
        {
            return Err(SreError::InvalidSpec);
        }
        let start = s.observed_at_unix.saturating_sub(s.window_seconds as i64);
        if !window_is_fresh(
            now_unix,
            start,
            s.window_seconds,
            policy.spec.forecast.max_sample_age_seconds,
        ) {
            return Err(SreError::StaleSample);
        }
        total += s.total_count as u128;
        success += s.success_count as u128;
        max_latency = max_latency.max(s.p95_latency_ms);
        max_queue = max_queue.max(s.p95_queue_wait_ms);
        hashes.push(s.evidence_sha256.clone());
    }
    hashes.sort();
    hashes.dedup();
    let availability = ((success * 10_000) / total).min(10_000) as u16;
    let observed_error = 10_000u32.saturating_sub(availability as u32);
    let allowed_error = 10_000u32.saturating_sub(policy.spec.slo.min_success_basis_points as u32);
    let burn = if allowed_error == 0 {
        if observed_error == 0 {
            0
        } else {
            u32::MAX
        }
    } else {
        ((observed_error as u64 * 1000) / allowed_error as u64).min(u32::MAX as u64) as u32
    };
    let latency = max_latency > policy.spec.slo.max_p95_latency_ms;
    let queue = max_queue > policy.spec.slo.max_p95_queue_wait_ms;
    let fast = burn >= policy.spec.slo.fast_burn_limit_milli;
    let slow = burn >= policy.spec.slo.slow_burn_limit_milli;
    let body = serde_json::to_vec(&(availability, burn, latency, queue, fast, slow, &hashes))
        .expect("SLO evaluation serialization");
    Ok(SloEvaluation {
        availability_basis_points: availability,
        error_budget_burn_milli: burn,
        latency_violated: latency,
        queue_wait_violated: queue,
        fast_burn: fast,
        slow_burn: slow,
        source_evidence_hashes: hashes,
        evaluation_sha256: sha(&body),
    })
}

pub fn forecast_demand(
    now_unix: i64,
    policy: &VerifiedSrePolicyDocument,
    samples: &[DemandSample],
) -> Result<DemandForecast, SreError> {
    if !policy_active(now_unix, policy) {
        return Err(SreError::InactivePolicy);
    }
    if samples.len() < policy.spec.forecast.min_samples as usize {
        return Err(SreError::InsufficientSamples);
    }
    let mut ss = samples.to_vec();
    ss.sort_by_key(|s| (s.window_start_unix, s.sample_uuid));
    let mut wr = 0u128;
    let mut wt = 0u128;
    let mut wc = 0u128;
    let mut wsum = 0u128;
    let mut hashes = Vec::new();
    for (i, s) in ss.iter().enumerate() {
        if s.window_seconds == 0 || !valid_sha(&s.evidence_sha256) {
            return Err(SreError::InvalidSpec);
        }
        if !window_is_fresh(
            now_unix,
            s.window_start_unix,
            s.window_seconds,
            policy.spec.forecast.max_sample_age_seconds,
        ) {
            return Err(SreError::StaleSample);
        }
        let weight = (i + 1) as u128;
        let rpm = ceil_div(s.requests as u128 * 60, s.window_seconds as u128);
        let tpm = ceil_div(s.tokens as u128 * 60, s.window_seconds as u128);
        wr += rpm * weight;
        wt += tpm * weight;
        wc += s.peak_concurrency as u128 * weight;
        wsum += weight;
        hashes.push(s.evidence_sha256.clone());
    }
    hashes.sort();
    hashes.dedup();
    let rpm = ceil_div(wr, wsum).min(u64::MAX as u128) as u64;
    let tpm = ceil_div(wt, wsum).min(u64::MAX as u128) as u64;
    let peak = ceil_div(wc, wsum).min(u32::MAX as u128) as u32;
    let recommended = ceil_div(
        peak as u128 * (10_000 + policy.spec.forecast.headroom_basis_points as u128),
        10_000,
    )
    .max(1)
    .min(u32::MAX as u128) as u32;
    let body = serde_json::to_vec(&(
        rpm,
        tpm,
        peak,
        recommended,
        policy.spec.forecast.horizon_seconds,
        &hashes,
    ))
    .expect("demand forecast serialization");
    Ok(DemandForecast {
        requests_per_minute: rpm,
        tokens_per_minute: tpm,
        peak_concurrency: peak,
        recommended_concurrency: recommended,
        horizon_seconds: policy.spec.forecast.horizon_seconds,
        source_evidence_hashes: hashes,
        forecast_sha256: sha(&body),
    })
}

fn canonical_rate_snapshot(s: &ProviderRateLimitSnapshot) -> Vec<u8> {
    serde_json::to_vec(&(
        s.tenant_uuid,
        s.snapshot_uuid,
        s.provider_uuid,
        s.request_limit,
        s.requests_remaining,
        s.token_limit,
        s.tokens_remaining,
        s.reset_at_unix,
        s.observed_at_unix,
        s.ttl_seconds,
    ))
    .expect("rate limit serialization")
}

pub fn verify_rate_limit_snapshot(
    now_unix: i64,
    doc: &SignedRateLimitSnapshot,
    trusted_keys: &BTreeMap<String, String>,
) -> Result<VerifiedRateLimitSnapshot, SreError> {
    let s = &doc.snapshot;
    if s.request_limit == 0
        || s.token_limit == 0
        || s.requests_remaining > s.request_limit
        || s.tokens_remaining > s.token_limit
        || s.ttl_seconds == 0
        || s.reset_at_unix <= s.observed_at_unix
    {
        return Err(SreError::InvalidRateLimit);
    }
    if !rate_limit_is_fresh(now_unix, s) {
        return Err(SreError::StaleRateLimit);
    }
    let body = canonical_rate_snapshot(s);
    if sha(&body) != s.snapshot_sha256 || !valid_sha(&s.snapshot_sha256) {
        return Err(SreError::InvalidHash);
    }
    let key_hex = trusted_keys
        .get(&doc.signer_id)
        .ok_or(SreError::UntrustedSigner)?;
    let kb = hex::decode(key_hex).map_err(|_| SreError::InvalidSignature)?;
    let ka: [u8; 32] = kb.try_into().map_err(|_| SreError::InvalidSignature)?;
    let vk = VerifyingKey::from_bytes(&ka).map_err(|_| SreError::InvalidSignature)?;
    let sb = hex::decode(&doc.signature_hex).map_err(|_| SreError::InvalidSignature)?;
    let sa: [u8; 64] = sb.try_into().map_err(|_| SreError::InvalidSignature)?;
    vk.verify(&body, &Signature::from_bytes(&sa))
        .map_err(|_| SreError::InvalidSignature)?;
    Ok(VerifiedRateLimitSnapshot {
        signer_id: doc.signer_id.clone(),
        snapshot: s.clone(),
    })
}

pub fn admission_decision(
    now_unix: i64,
    policy: &VerifiedSrePolicyDocument,
    req: &AdmissionRequest,
    rate: &VerifiedRateLimitSnapshot,
) -> Result<AdmissionDecision, SreError> {
    if !policy_active(now_unix, policy) {
        return Err(SreError::InactivePolicy);
    }
    if req.provider_uuid != rate.snapshot.provider_uuid {
        return Err(SreError::InvalidRateLimit);
    }
    if !rate_limit_is_fresh(now_unix, &rate.snapshot) {
        return Err(SreError::StaleRateLimit);
    }
    let weight = *policy
        .spec
        .queue
        .class_weights
        .get(&req.class)
        .unwrap_or(&1u16) as u32;
    let priority = weight
        .saturating_mul(1_000_000)
        .saturating_add(req.queue_wait_ms.min(999_999) as u32);
    let enough = rate.snapshot.requests_remaining >= 1
        && rate.snapshot.tokens_remaining >= req.estimated_tokens;
    let (action, reason) = if enough {
        (AdmissionAction::Admit, "capacity_available")
    } else if req.queue_depth >= policy.spec.queue.hard_reject_depth {
        (AdmissionAction::Reject, "hard_backpressure")
    } else if req.queue_depth < policy.spec.queue.max_depth
        && (req.queue_wait_ms < policy.spec.queue.max_wait_ms
            || req.class == RequestClass::Critical)
    {
        (AdmissionAction::Queue, "rate_limited_queue")
    } else {
        (AdmissionAction::Reject, "queue_limit")
    };
    let body = serde_json::to_vec(&(
        req.request_uuid,
        action,
        priority,
        reason,
        &rate.snapshot.snapshot_sha256,
    ))
    .expect("admission serialization");
    Ok(AdmissionDecision {
        request_uuid: req.request_uuid,
        action,
        priority_score: priority,
        reason: reason.into(),
        rate_limit_snapshot_sha256: rate.snapshot.snapshot_sha256.clone(),
        decision_sha256: sha(&body),
    })
}

pub fn forecast_cost(
    now_unix: i64,
    policy: &VerifiedSrePolicyDocument,
    budget: &BudgetAccountSnapshot,
    samples: &[CostSample],
) -> Result<CostForecast, SreError> {
    if !policy_active(now_unix, policy) {
        return Err(SreError::InactivePolicy);
    }
    if budget.account_uuid != policy.spec.cost.budget_account_uuid
        || !valid_sha(&budget.snapshot_sha256)
        || budget.observed_at_unix > now_unix
        || now_unix.saturating_sub(budget.observed_at_unix)
            > policy.spec.forecast.max_sample_age_seconds as i64
    {
        return Err(SreError::BudgetMismatch);
    }
    if samples.len() < policy.spec.forecast.min_samples as usize {
        return Err(SreError::InsufficientSamples);
    }
    let mut ss = samples.to_vec();
    ss.sort_by_key(|s| s.window_start_unix);
    let mut weighted_hourly = 0u128;
    let mut weights = 0u128;
    let mut hashes = Vec::new();
    for (i, s) in ss.iter().enumerate() {
        if s.window_seconds == 0 || !valid_sha(&s.evidence_sha256) {
            return Err(SreError::InvalidSpec);
        }
        if !window_is_fresh(
            now_unix,
            s.window_start_unix,
            s.window_seconds,
            policy.spec.forecast.max_sample_age_seconds,
        ) {
            return Err(SreError::StaleSample);
        }
        let w = (i + 1) as u128;
        let hourly = ceil_div(s.spent_micro_usd as u128 * 3600, s.window_seconds as u128);
        weighted_hourly += hourly * w;
        weights += w;
        hashes.push(s.evidence_sha256.clone());
    }
    hashes.sort();
    hashes.dedup();
    let hourly = ceil_div(weighted_hourly, weights);
    let future = ceil_div(hourly * policy.spec.forecast.horizon_seconds as u128, 3600);
    let current = budget
        .spent_micro_usd
        .saturating_add(budget.reserved_micro_usd);
    let projected = (current as u128 + future).min(u64::MAX as u128) as u64;
    let bps = if budget.limit_micro_usd == 0 {
        if projected == 0 {
            0
        } else {
            10_000
        }
    } else {
        ((projected as u128 * 10_000) / budget.limit_micro_usd as u128).min(10_000) as u16
    };
    let state = if bps >= policy.spec.cost.hard_limit_basis_points {
        BudgetState::HardLimit
    } else if bps >= policy.spec.cost.soft_limit_basis_points {
        BudgetState::SoftLimit
    } else {
        BudgetState::Healthy
    };
    let body = serde_json::to_vec(&(
        budget.account_uuid,
        budget.limit_micro_usd,
        current,
        projected,
        bps,
        state,
        &hashes,
    ))
    .expect("cost forecast serialization");
    Ok(CostForecast {
        account_uuid: budget.account_uuid,
        budget_limit_micro_usd: budget.limit_micro_usd,
        current_committed_micro_usd: current,
        projected_total_micro_usd: projected,
        projected_budget_basis_points: bps,
        state,
        source_evidence_hashes: hashes,
        forecast_sha256: sha(&body),
    })
}

pub fn detect_incident(
    incident_uuid: Uuid,
    signals: &[IncidentSignal],
) -> Result<IncidentDetection, SreError> {
    if signals.is_empty() {
        return Err(SreError::InvalidSpec);
    }
    let mut by_evidence: BTreeMap<String, IncidentSignal> = BTreeMap::new();
    for s in signals {
        if !valid_sha(&s.evidence_sha256) || s.summary.trim().is_empty() {
            return Err(SreError::InvalidSpec);
        }
        if let Some(existing) = by_evidence.get(&s.evidence_sha256) {
            if existing.kind != s.kind
                || existing.severity != s.severity
                || existing.summary != s.summary
            {
                return Err(SreError::ContradictoryIncidentEvidence);
            }
        } else {
            by_evidence.insert(s.evidence_sha256.clone(), s.clone());
        }
    }
    let mut ss: Vec<IncidentSignal> = by_evidence.into_values().collect();
    ss.sort_by(|a, b| {
        (a.kind, a.severity, &a.evidence_sha256).cmp(&(b.kind, b.severity, &b.evidence_sha256))
    });
    let severity = ss
        .iter()
        .map(|s| s.severity)
        .max()
        .unwrap_or(IncidentSeverity::Info);
    let body = serde_json::to_vec(&(incident_uuid, severity, &ss)).expect("incident serialization");
    Ok(IncidentDetection {
        incident_uuid,
        severity,
        signals: ss,
        detection_sha256: sha(&body),
    })
}

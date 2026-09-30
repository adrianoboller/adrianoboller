use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use phxclaw_adaptive_model_intelligence::PromotedProfile;
use phxclaw_ai_benchmark::{ComplexityBand, EvidenceClass, TaskFamily};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct CandidateKey {
    pub provider_uuid: Uuid,
    pub model_id: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArenaMode {
    Shadow,
    Canary,
    Paired,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArenaState {
    Draft,
    Active,
    Paused,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArenaStateEvent {
    pub event_uuid: Uuid,
    pub arena_uuid: Uuid,
    pub from_state: ArenaState,
    pub to_state: ArenaState,
    pub actor_id: String,
    pub policy_sha256: String,
    pub occurred_at_unix: i64,
    pub event_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArenaSpec {
    pub tenant_uuid: Uuid,
    pub arena_uuid: Uuid,
    pub name: String,
    pub mode: ArenaMode,
    pub task_family: TaskFamily,
    pub complexity: ComplexityBand,
    pub evidence_class: EvidenceClass,
    pub champion: CandidateKey,
    pub challengers: Vec<CandidateKey>,
    pub challenger_traffic_basis_points: u16,
    pub assignment_salt_sha256: String,
    pub policy_sha256: String,
    pub min_paired_samples: u32,
    pub required_consecutive_wins: u16,
    pub min_quality_improvement_basis_points: i32,
    pub min_success_improvement_basis_points: i32,
    pub max_quality_regression_basis_points: u16,
    pub max_success_regression_basis_points: u16,
    pub max_latency_regression_basis_points: u16,
    pub max_cost_regression_basis_points: u16,
    pub starts_at_unix: i64,
    pub expires_at_unix: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedArenaDocument {
    pub document_sha256: String,
    pub signer_id: String,
    pub signature_hex: String,
    pub spec: ArenaSpec,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifiedArenaDocument {
    pub document_sha256: String,
    pub signer_id: String,
    pub spec: ArenaSpec,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArenaAssignment {
    pub arena_uuid: Uuid,
    pub request_uuid: Uuid,
    pub served: CandidateKey,
    pub challenger: Option<CandidateKey>,
    pub execute_challenger_in_shadow: bool,
    pub assignment_bucket: u16,
    pub assignment_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricOutcome {
    pub success: bool,
    pub quality_basis_points: u16,
    pub latency_ms: u64,
    pub actual_cost_micro_usd: Option<u64>,
    pub safety_violation: bool,
    pub output_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArenaPairObservation {
    pub tenant_uuid: Uuid,
    pub observation_uuid: Uuid,
    pub arena_uuid: Uuid,
    pub request_uuid: Uuid,
    pub case_uuid: Uuid,
    pub evidence_class: EvidenceClass,
    pub champion: CandidateKey,
    pub challenger: CandidateKey,
    pub champion_outcome: MetricOutcome,
    pub challenger_outcome: MetricOutcome,
    pub dataset_sha256: String,
    pub scorer_sha256: String,
    pub environment_sha256: String,
    pub observed_at_unix: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArenaWindow {
    pub window_uuid: Uuid,
    pub arena_uuid: Uuid,
    pub challenger: CandidateKey,
    pub sample_count: u32,
    pub champion_success_basis_points: u16,
    pub challenger_success_basis_points: u16,
    pub champion_quality_basis_points: u16,
    pub challenger_quality_basis_points: u16,
    pub quality_delta_basis_points: i32,
    pub success_delta_basis_points: i32,
    pub latency_regression_basis_points: i32,
    pub cost_regression_basis_points: Option<i32>,
    pub safety_violations: u32,
    pub evidence_class: EvidenceClass,
    pub window_sha256: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArenaVerdict {
    Insufficient,
    Continue,
    ChallengerWins,
    ChallengerRegressed,
    PauseSafety,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromotionRecommendation {
    pub recommendation_uuid: Uuid,
    pub arena_uuid: Uuid,
    pub challenger: CandidateKey,
    pub champion: CandidateKey,
    pub policy_sha256: String,
    pub supporting_window_hashes: Vec<String>,
    pub recommendation_sha256: String,
    pub requires_v032_promotion_gate: bool,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ArenaError {
    #[error("invalid hash field")]
    InvalidHash,
    #[error("invalid arena specification")]
    InvalidSpec,
    #[error("arena document signer is not trusted")]
    UntrustedSigner,
    #[error("arena document signature is invalid")]
    InvalidSignature,
    #[error("arena is outside its active time window")]
    InactiveArena,
    #[error("champion or challenger is not eligible under the base router")]
    CandidateIneligible,
    #[error("production arena candidate lacks a promoted v0.32 profile")]
    MissingPromotedProfile,
    #[error("fixture evidence cannot promote")]
    FixtureNotPromotable,
    #[error("observation mismatch")]
    ObservationMismatch,
    #[error("insufficient winning windows")]
    InsufficientWinningWindows,
    #[error("invalid arena state transition")]
    InvalidStateTransition,
}

fn valid_sha(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn sorted_spec(spec: &ArenaSpec) -> ArenaSpec {
    let mut x = spec.clone();
    x.challengers.sort();
    x
}
pub fn canonical_spec_bytes(spec: &ArenaSpec) -> Vec<u8> {
    serde_json::to_vec(&sorted_spec(spec)).expect("arena spec serialization")
}

pub fn validate_spec(spec: &ArenaSpec) -> Result<(), ArenaError> {
    if !valid_sha(&spec.assignment_salt_sha256)
        || !valid_sha(&spec.policy_sha256)
        || spec.starts_at_unix >= spec.expires_at_unix
        || spec.challenger_traffic_basis_points > 10_000
        || spec.min_paired_samples == 0
        || spec.required_consecutive_wins == 0
    {
        return Err(ArenaError::InvalidSpec);
    }
    if spec.challengers.is_empty() {
        return Err(ArenaError::InvalidSpec);
    }
    let mut seen = BTreeSet::new();
    for c in &spec.challengers {
        if c == &spec.champion || c.model_id.is_empty() || !seen.insert(c.clone()) {
            return Err(ArenaError::InvalidSpec);
        }
    }
    if spec.champion.model_id.is_empty() {
        return Err(ArenaError::InvalidSpec);
    }
    Ok(())
}

pub fn verify_signed_document(
    doc: &SignedArenaDocument,
    trusted_keys: &BTreeMap<String, String>,
) -> Result<VerifiedArenaDocument, ArenaError> {
    validate_spec(&doc.spec)?;
    let canonical = canonical_spec_bytes(&doc.spec);
    let digest = sha256_hex(&canonical);
    if digest != doc.document_sha256 || !valid_sha(&doc.document_sha256) {
        return Err(ArenaError::InvalidHash);
    }
    let key_hex = trusted_keys
        .get(&doc.signer_id)
        .ok_or(ArenaError::UntrustedSigner)?;
    let key_bytes = hex::decode(key_hex).map_err(|_| ArenaError::InvalidSignature)?;
    let key_array: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| ArenaError::InvalidSignature)?;
    let vk = VerifyingKey::from_bytes(&key_array).map_err(|_| ArenaError::InvalidSignature)?;
    let sig_bytes = hex::decode(&doc.signature_hex).map_err(|_| ArenaError::InvalidSignature)?;
    let sig_array: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| ArenaError::InvalidSignature)?;
    let sig = Signature::from_bytes(&sig_array);
    vk.verify(&canonical, &sig)
        .map_err(|_| ArenaError::InvalidSignature)?;
    Ok(VerifiedArenaDocument {
        document_sha256: doc.document_sha256.clone(),
        signer_id: doc.signer_id.clone(),
        spec: sorted_spec(&doc.spec),
    })
}

pub fn transition_state(
    doc: &VerifiedArenaDocument,
    current: ArenaState,
    next: ArenaState,
    actor_id: &str,
    occurred_at_unix: i64,
) -> Result<ArenaStateEvent, ArenaError> {
    let allowed = matches!(
        (current, next),
        (ArenaState::Draft, ArenaState::Active)
            | (ArenaState::Draft, ArenaState::Cancelled)
            | (ArenaState::Active, ArenaState::Paused)
            | (ArenaState::Active, ArenaState::Completed)
            | (ArenaState::Active, ArenaState::Cancelled)
            | (ArenaState::Paused, ArenaState::Active)
            | (ArenaState::Paused, ArenaState::Completed)
            | (ArenaState::Paused, ArenaState::Cancelled)
    );
    if !allowed || actor_id.is_empty() {
        return Err(ArenaError::InvalidStateTransition);
    }
    let event_uuid = Uuid::now_v7();
    let canonical = serde_json::to_vec(&(
        event_uuid,
        doc.spec.arena_uuid,
        current,
        next,
        actor_id,
        &doc.spec.policy_sha256,
        occurred_at_unix,
        &doc.document_sha256,
    ))
    .expect("arena state serialization");
    Ok(ArenaStateEvent {
        event_uuid,
        arena_uuid: doc.spec.arena_uuid,
        from_state: current,
        to_state: next,
        actor_id: actor_id.to_owned(),
        policy_sha256: doc.spec.policy_sha256.clone(),
        occurred_at_unix,
        event_sha256: sha256_hex(&canonical),
    })
}

fn assignment_hash(spec: &ArenaSpec, request_uuid: Uuid) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(spec.arena_uuid.as_bytes());
    h.update(request_uuid.as_bytes());
    h.update(spec.assignment_salt_sha256.as_bytes());
    h.finalize().into()
}

pub fn assign(
    now_unix: i64,
    doc: &VerifiedArenaDocument,
    eligible: &BTreeSet<CandidateKey>,
    promoted_profiles: &[PromotedProfile],
    request_uuid: Uuid,
) -> Result<ArenaAssignment, ArenaError> {
    if now_unix < doc.spec.starts_at_unix || now_unix > doc.spec.expires_at_unix {
        return Err(ArenaError::InactiveArena);
    }
    if !eligible.contains(&doc.spec.champion) {
        return Err(ArenaError::CandidateIneligible);
    }
    if doc.spec.evidence_class == EvidenceClass::Production
        && !has_promoted_candidate(doc, &doc.spec.champion, promoted_profiles)
    {
        return Err(ArenaError::MissingPromotedProfile);
    }
    let mut challengers: Vec<CandidateKey> = doc
        .spec
        .challengers
        .iter()
        .filter(|c| {
            eligible.contains(*c)
                && (doc.spec.evidence_class == EvidenceClass::Fixture
                    || has_promoted_candidate(doc, c, promoted_profiles))
        })
        .cloned()
        .collect();
    challengers.sort();
    if challengers.is_empty() {
        return Err(ArenaError::CandidateIneligible);
    }
    let digest = assignment_hash(&doc.spec, request_uuid);
    let bucket = u16::from_be_bytes([digest[0], digest[1]]) % 10_000;
    let challenger = challengers
        [(u16::from_be_bytes([digest[2], digest[3]]) as usize) % challengers.len()]
    .clone();
    let (served, shadow) = match doc.spec.mode {
        ArenaMode::Shadow | ArenaMode::Paired => (doc.spec.champion.clone(), true),
        ArenaMode::Canary => {
            if bucket < doc.spec.challenger_traffic_basis_points {
                (challenger.clone(), false)
            } else {
                (doc.spec.champion.clone(), false)
            }
        }
    };
    let canonical = serde_json::to_vec(&(
        doc.spec.arena_uuid,
        request_uuid,
        &served,
        &challenger,
        shadow,
        bucket,
        &doc.document_sha256,
    ))
    .expect("assignment serialization");
    Ok(ArenaAssignment {
        arena_uuid: doc.spec.arena_uuid,
        request_uuid,
        served,
        challenger: Some(challenger),
        execute_challenger_in_shadow: shadow,
        assignment_bucket: bucket,
        assignment_sha256: sha256_hex(&canonical),
    })
}

fn mean_bp(sum: u64, n: u32) -> u16 {
    if n == 0 {
        0
    } else {
        (sum / n as u64).min(10_000) as u16
    }
}
fn relative_regression(new: u64, old: u64) -> i32 {
    if old == 0 {
        return if new == 0 { 0 } else { i32::MAX };
    }
    (((new as i128 - old as i128) * 10_000i128) / (old as i128))
        .clamp(i32::MIN as i128, i32::MAX as i128) as i32
}

pub fn aggregate_window(
    doc: &VerifiedArenaDocument,
    challenger: &CandidateKey,
    observations: &[ArenaPairObservation],
) -> Result<ArenaWindow, ArenaError> {
    let spec = &doc.spec;
    let mut cq = 0u64;
    let mut qq = 0u64;
    let mut cs = 0u64;
    let mut qs = 0u64;
    let mut cl = 0u64;
    let mut ql = 0u64;
    let mut cc = 0u64;
    let mut qc = 0u64;
    let mut cost_pairs = 0u32;
    let mut safety = 0u32;
    for o in observations {
        if o.tenant_uuid != spec.tenant_uuid
            || o.arena_uuid != spec.arena_uuid
            || &o.champion != &spec.champion
            || &o.challenger != challenger
            || o.evidence_class != spec.evidence_class
            || !valid_sha(&o.dataset_sha256)
            || !valid_sha(&o.scorer_sha256)
            || !valid_sha(&o.environment_sha256)
            || !valid_sha(&o.champion_outcome.output_sha256)
            || !valid_sha(&o.challenger_outcome.output_sha256)
        {
            return Err(ArenaError::ObservationMismatch);
        }
        if o.champion_outcome.quality_basis_points > 10_000
            || o.challenger_outcome.quality_basis_points > 10_000
        {
            return Err(ArenaError::ObservationMismatch);
        }
        cq += o.champion_outcome.quality_basis_points as u64;
        qq += o.challenger_outcome.quality_basis_points as u64;
        cs += if o.champion_outcome.success {
            10_000
        } else {
            0
        };
        qs += if o.challenger_outcome.success {
            10_000
        } else {
            0
        };
        cl += o.champion_outcome.latency_ms;
        ql += o.challenger_outcome.latency_ms;
        if let (Some(a), Some(b)) = (
            o.champion_outcome.actual_cost_micro_usd,
            o.challenger_outcome.actual_cost_micro_usd,
        ) {
            cc += a;
            qc += b;
            cost_pairs += 1;
        }
        if o.champion_outcome.safety_violation || o.challenger_outcome.safety_violation {
            safety += 1;
        }
    }
    let n = observations.len() as u32;
    let cqbp = mean_bp(cq, n);
    let qqbp = mean_bp(qq, n);
    let csbp = mean_bp(cs, n);
    let qsbp = mean_bp(qs, n);
    let latency = if n == 0 {
        0
    } else {
        relative_regression(ql / n as u64, cl / n as u64)
    };
    let cost = if cost_pairs == 0 {
        None
    } else {
        Some(relative_regression(
            qc / cost_pairs as u64,
            cc / cost_pairs as u64,
        ))
    };
    let mut w = ArenaWindow {
        window_uuid: Uuid::now_v7(),
        arena_uuid: spec.arena_uuid,
        challenger: challenger.clone(),
        sample_count: n,
        champion_success_basis_points: csbp,
        challenger_success_basis_points: qsbp,
        champion_quality_basis_points: cqbp,
        challenger_quality_basis_points: qqbp,
        quality_delta_basis_points: qqbp as i32 - cqbp as i32,
        success_delta_basis_points: qsbp as i32 - csbp as i32,
        latency_regression_basis_points: latency,
        cost_regression_basis_points: cost,
        safety_violations: safety,
        evidence_class: spec.evidence_class,
        window_sha256: String::new(),
    };
    let bytes = serde_json::to_vec(&(
        &w.arena_uuid,
        &w.challenger,
        w.sample_count,
        w.champion_success_basis_points,
        w.challenger_success_basis_points,
        w.champion_quality_basis_points,
        w.challenger_quality_basis_points,
        w.quality_delta_basis_points,
        w.success_delta_basis_points,
        w.latency_regression_basis_points,
        w.cost_regression_basis_points,
        w.safety_violations,
        w.evidence_class,
    ))
    .expect("window serialization");
    w.window_sha256 = sha256_hex(&bytes);
    Ok(w)
}

pub fn evaluate_window(doc: &VerifiedArenaDocument, w: &ArenaWindow) -> ArenaVerdict {
    let spec = &doc.spec;
    if w.sample_count < spec.min_paired_samples {
        return ArenaVerdict::Insufficient;
    }
    if w.safety_violations > 0 {
        return ArenaVerdict::PauseSafety;
    }
    if w.quality_delta_basis_points < -(spec.max_quality_regression_basis_points as i32)
        || w.success_delta_basis_points < -(spec.max_success_regression_basis_points as i32)
        || w.latency_regression_basis_points > spec.max_latency_regression_basis_points as i32
        || w.cost_regression_basis_points
            .map(|x| x > spec.max_cost_regression_basis_points as i32)
            .unwrap_or(false)
    {
        return ArenaVerdict::ChallengerRegressed;
    }
    if w.quality_delta_basis_points >= spec.min_quality_improvement_basis_points
        && w.success_delta_basis_points >= spec.min_success_improvement_basis_points
    {
        return ArenaVerdict::ChallengerWins;
    }
    ArenaVerdict::Continue
}

pub fn recommend_promotion(
    doc: &VerifiedArenaDocument,
    windows: &[ArenaWindow],
) -> Result<PromotionRecommendation, ArenaError> {
    let spec = &doc.spec;
    if spec.evidence_class == EvidenceClass::Fixture {
        return Err(ArenaError::FixtureNotPromotable);
    }
    if windows.len() < spec.required_consecutive_wins as usize {
        return Err(ArenaError::InsufficientWinningWindows);
    }
    let tail = &windows[windows.len() - spec.required_consecutive_wins as usize..];
    let challenger = tail[0].challenger.clone();
    if tail.iter().any(|w| {
        w.arena_uuid != spec.arena_uuid
            || w.challenger != challenger
            || w.evidence_class != EvidenceClass::Production
            || evaluate_window(doc, w) != ArenaVerdict::ChallengerWins
            || !valid_sha(&w.window_sha256)
    }) {
        return Err(ArenaError::InsufficientWinningWindows);
    }
    let hashes: Vec<String> = tail.iter().map(|w| w.window_sha256.clone()).collect();
    let rec_uuid = Uuid::now_v7();
    let canonical = serde_json::to_vec(&(
        spec.tenant_uuid,
        rec_uuid,
        spec.arena_uuid,
        &challenger,
        &spec.champion,
        &spec.policy_sha256,
        &hashes,
        true,
    ))
    .expect("recommendation serialization");
    Ok(PromotionRecommendation {
        recommendation_uuid: rec_uuid,
        arena_uuid: spec.arena_uuid,
        challenger,
        champion: spec.champion.clone(),
        policy_sha256: spec.policy_sha256.clone(),
        supporting_window_hashes: hashes,
        recommendation_sha256: sha256_hex(&canonical),
        requires_v032_promotion_gate: true,
    })
}

pub fn promoted_profile_matches_arena(
    doc: &VerifiedArenaDocument,
    candidate: &CandidateKey,
    promoted: &PromotedProfile,
) -> bool {
    let spec = &doc.spec;
    let profile = &promoted.profile;
    valid_sha(&promoted.policy_sha256)
        && profile.tenant_uuid == spec.tenant_uuid
        && profile.provider_uuid == candidate.provider_uuid
        && profile.model_id == candidate.model_id
        && profile.task_family == spec.task_family
        && profile.complexity >= spec.complexity
        && profile.evidence_class == spec.evidence_class
        && valid_sha(&profile.profile_sha256)
}
fn has_promoted_candidate(
    doc: &VerifiedArenaDocument,
    candidate: &CandidateKey,
    profiles: &[PromotedProfile],
) -> bool {
    profiles
        .iter()
        .any(|p| promoted_profile_matches_arena(doc, candidate, p))
}

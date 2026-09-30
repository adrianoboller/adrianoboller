use phxclaw_adaptive_model_intelligence::PromotedProfile;
use phxclaw_ai_benchmark::EvidenceClass;
use phxclaw_model_arena::{ArenaWindow, CandidateKey, VerifiedArenaDocument};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriftPolicy {
    pub min_samples: u32,
    pub max_quality_drop_basis_points: u16,
    pub max_success_drop_basis_points: u16,
    pub max_latency_regression_basis_points: u16,
    pub max_cost_regression_basis_points: u16,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DriftSeverity {
    None,
    Warning,
    Critical,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DriftAction {
    Continue,
    PauseArena,
    FallbackBaseRouter,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriftEvent {
    pub event_uuid: Uuid,
    pub arena_uuid: Uuid,
    pub champion: CandidateKey,
    pub baseline_profile_sha256: String,
    pub observed_window_sha256: String,
    pub severity: DriftSeverity,
    pub action: DriftAction,
    pub reason: String,
    pub event_sha256: String,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum DriftError {
    #[error("baseline does not match arena champion")]
    BaselineMismatch,
    #[error("fixture evidence cannot drive production drift")]
    FixtureEvidence,
    #[error("invalid evidence hash")]
    InvalidHash,
}
fn valid_sha(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn detect_drift(
    doc: &VerifiedArenaDocument,
    policy: &DriftPolicy,
    baseline: &PromotedProfile,
    observed: &ArenaWindow,
) -> Result<DriftEvent, DriftError> {
    let spec = &doc.spec;
    let profile = &baseline.profile;
    if spec.evidence_class == EvidenceClass::Production
        && (profile.evidence_class != EvidenceClass::Production
            || observed.evidence_class != EvidenceClass::Production)
    {
        return Err(DriftError::FixtureEvidence);
    }
    if profile.tenant_uuid != spec.tenant_uuid
        || profile.provider_uuid != spec.champion.provider_uuid
        || profile.model_id != spec.champion.model_id
        || profile.task_family != spec.task_family
        || profile.complexity < spec.complexity
    {
        return Err(DriftError::BaselineMismatch);
    }
    if !valid_sha(&baseline.policy_sha256)
        || !valid_sha(&profile.profile_sha256)
        || !valid_sha(&observed.window_sha256)
    {
        return Err(DriftError::InvalidHash);
    }
    let quality_drop = profile
        .quality_basis_points
        .saturating_sub(observed.champion_quality_basis_points);
    let success_drop = profile
        .success_basis_points
        .saturating_sub(observed.champion_success_basis_points);
    let mut severity = DriftSeverity::None;
    let mut action = DriftAction::Continue;
    let mut reasons = Vec::new();
    if observed.sample_count >= policy.min_samples {
        if quality_drop > policy.max_quality_drop_basis_points {
            severity = DriftSeverity::Critical;
            action = DriftAction::FallbackBaseRouter;
            reasons.push("quality_drop");
        }
        if success_drop > policy.max_success_drop_basis_points {
            severity = DriftSeverity::Critical;
            action = DriftAction::FallbackBaseRouter;
            reasons.push("success_drop");
        }
        if observed.latency_regression_basis_points
            > policy.max_latency_regression_basis_points as i32
        {
            if severity == DriftSeverity::None {
                severity = DriftSeverity::Warning;
                action = DriftAction::PauseArena;
            }
            reasons.push("latency_regression");
        }
        if observed
            .cost_regression_basis_points
            .map(|x| x > policy.max_cost_regression_basis_points as i32)
            .unwrap_or(false)
        {
            if severity == DriftSeverity::None {
                severity = DriftSeverity::Warning;
                action = DriftAction::PauseArena;
            }
            reasons.push("cost_regression");
        }
        if observed.safety_violations > 0 {
            severity = DriftSeverity::Critical;
            action = DriftAction::FallbackBaseRouter;
            reasons.push("safety_violation");
        }
    } else {
        reasons.push("insufficient_samples");
    }
    let event_uuid = Uuid::now_v7();
    let reason = reasons.join(",");
    let canonical = serde_json::to_vec(&(
        event_uuid,
        spec.arena_uuid,
        &spec.champion,
        &profile.profile_sha256,
        &observed.window_sha256,
        severity,
        action,
        &reason,
    ))
    .expect("drift serialization");
    Ok(DriftEvent {
        event_uuid,
        arena_uuid: spec.arena_uuid,
        champion: spec.champion.clone(),
        baseline_profile_sha256: profile.profile_sha256.clone(),
        observed_window_sha256: observed.window_sha256.clone(),
        severity,
        action,
        reason,
        event_sha256: sha(&canonical),
    })
}

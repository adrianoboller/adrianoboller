//! F24 Auto-learning + Skill Evolution.
//! Learning may propose immutable skill candidates; it never mutates the PhxClaw microkernel,
//! Constitution, enforcement policies, or production state directly.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum EvolutionError {
    #[error("candidate boundary is forbidden for autonomous learning")]
    ForbiddenBoundary,
    #[error("target namespace is not a governed skill namespace")]
    InvalidNamespace,
    #[error("invalid sha256 digest")]
    InvalidDigest,
    #[error("evidence is bound to a different artifact or source state")]
    EvidenceBindingMismatch,
    #[error("evidence is stale")]
    StaleEvidence,
    #[error("required evaluation evidence is insufficient")]
    InsufficientEvidence,
    #[error("evaluation suite has failures")]
    TestFailure,
    #[error("security findings block promotion")]
    SecurityBlocker,
    #[error("benchmark regression exceeds policy")]
    BenchmarkRegression,
    #[error("human approval is required")]
    ApprovalRequired,
    #[error("approval is invalid or expired")]
    InvalidApproval,
    #[error("promotion decision contains blockers")]
    PromotionBlocked,
    #[error("rollback lineage is required for an update")]
    MissingRollbackLineage,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EpistemicState {
    RawObservation,
    UnverifiedContext,
    AcceptedEvidence,
    CandidateSkill,
    PromotedSkill,
    Rejected,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeBoundary {
    SkillPlugin,
    ContextEnricher,
    Microkernel,
    Constitution,
    EnforcementPolicy,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LearningObservation {
    pub observation_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub source_kind: String,
    pub source_ref: String,
    pub payload_sha256_hex: String,
    pub state: EpistemicState,
    pub observed_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillCandidate {
    pub candidate_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub skill_uuid: Uuid,
    pub base_release_uuid: Option<Uuid>,
    pub target_namespace: String,
    pub boundary: ChangeBoundary,
    pub artifact_sha256_hex: String,
    pub manifest_sha256_hex: String,
    pub source_state_sha256_hex: String,
    pub risk: RiskLevel,
    pub behavior_change: bool,
    pub reversible: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvidenceSummary {
    pub candidate_uuid: Uuid,
    pub artifact_sha256_hex: String,
    pub source_state_sha256_hex: String,
    pub required_suite_runs: u32,
    pub passed_checks: u32,
    pub failed_checks: u32,
    pub security_critical_open: u32,
    pub security_high_open: u32,
    pub benchmark_regression_bps: i32,
    pub independent_evaluators: u16,
    pub freshest_evidence_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct PromotionPolicy {
    pub min_independent_evaluators: u16,
    pub max_evidence_age: Duration,
    pub max_benchmark_regression_bps: i32,
    pub allow_auto_low_risk: bool,
}

impl Default for PromotionPolicy {
    fn default() -> Self {
        Self {
            min_independent_evaluators: 2,
            max_evidence_age: Duration::hours(24),
            max_benchmark_regression_bps: 0,
            allow_auto_low_risk: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromotionPath {
    AutoLowRisk,
    HumanApproval,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromotionDecision {
    pub candidate_uuid: Uuid,
    pub eligible: bool,
    pub requires_human_approval: bool,
    pub path: PromotionPath,
    pub blockers: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovalRef {
    pub approval_uuid: Uuid,
    pub candidate_uuid: Uuid,
    pub artifact_sha256_hex: String,
    pub approved_by_uuid: Uuid,
    pub approved_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillRelease {
    pub release_uuid: Uuid,
    pub candidate_uuid: Uuid,
    pub skill_uuid: Uuid,
    pub previous_release_uuid: Option<Uuid>,
    pub artifact_sha256_hex: String,
    pub manifest_sha256_hex: String,
    pub promoted_at: DateTime<Utc>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn valid_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn skill_namespace_allowed(namespace: &str) -> bool {
    namespace.starts_with("skills.")
        && namespace.len() <= 200
        && namespace
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

pub fn evaluate_candidate(
    candidate: &SkillCandidate,
    evidence: &EvidenceSummary,
    policy: &PromotionPolicy,
    now: DateTime<Utc>,
) -> Result<PromotionDecision, EvolutionError> {
    if matches!(
        candidate.boundary,
        ChangeBoundary::ContextEnricher
            | ChangeBoundary::Microkernel
            | ChangeBoundary::Constitution
            | ChangeBoundary::EnforcementPolicy
    ) {
        return Err(EvolutionError::ForbiddenBoundary);
    }
    if candidate.boundary != ChangeBoundary::SkillPlugin {
        return Err(EvolutionError::ForbiddenBoundary);
    }
    if !skill_namespace_allowed(&candidate.target_namespace) {
        return Err(EvolutionError::InvalidNamespace);
    }
    for digest in [
        &candidate.artifact_sha256_hex,
        &candidate.manifest_sha256_hex,
        &candidate.source_state_sha256_hex,
        &evidence.artifact_sha256_hex,
        &evidence.source_state_sha256_hex,
    ] {
        if !valid_sha256_hex(digest) {
            return Err(EvolutionError::InvalidDigest);
        }
    }
    if evidence.candidate_uuid != candidate.candidate_uuid
        || !evidence
            .artifact_sha256_hex
            .eq_ignore_ascii_case(&candidate.artifact_sha256_hex)
        || !evidence
            .source_state_sha256_hex
            .eq_ignore_ascii_case(&candidate.source_state_sha256_hex)
    {
        return Err(EvolutionError::EvidenceBindingMismatch);
    }
    if now.signed_duration_since(evidence.freshest_evidence_at) > policy.max_evidence_age {
        return Err(EvolutionError::StaleEvidence);
    }
    if evidence.required_suite_runs == 0
        || evidence.passed_checks == 0
        || evidence.independent_evaluators < policy.min_independent_evaluators
    {
        return Err(EvolutionError::InsufficientEvidence);
    }
    if evidence.failed_checks != 0 {
        return Err(EvolutionError::TestFailure);
    }
    if evidence.security_critical_open != 0 || evidence.security_high_open != 0 {
        return Err(EvolutionError::SecurityBlocker);
    }
    if evidence.benchmark_regression_bps > policy.max_benchmark_regression_bps {
        return Err(EvolutionError::BenchmarkRegression);
    }

    let requires_human_approval = candidate.behavior_change
        || !candidate.reversible
        || matches!(candidate.risk, RiskLevel::High | RiskLevel::Critical)
        || !matches!(candidate.risk, RiskLevel::Low)
        || !policy.allow_auto_low_risk;
    let path = if requires_human_approval {
        PromotionPath::HumanApproval
    } else {
        PromotionPath::AutoLowRisk
    };
    Ok(PromotionDecision {
        candidate_uuid: candidate.candidate_uuid,
        eligible: true,
        requires_human_approval,
        path,
        blockers: Vec::new(),
    })
}

pub fn authorize_promotion(
    candidate: &SkillCandidate,
    decision: &PromotionDecision,
    approval: Option<&ApprovalRef>,
    now: DateTime<Utc>,
) -> Result<(), EvolutionError> {
    if !decision.eligible
        || !decision.blockers.is_empty()
        || decision.candidate_uuid != candidate.candidate_uuid
    {
        return Err(EvolutionError::PromotionBlocked);
    }
    if decision.requires_human_approval {
        let approval = approval.ok_or(EvolutionError::ApprovalRequired)?;
        if approval.candidate_uuid != candidate.candidate_uuid
            || !approval
                .artifact_sha256_hex
                .eq_ignore_ascii_case(&candidate.artifact_sha256_hex)
            || now < approval.approved_at
            || now >= approval.expires_at
        {
            return Err(EvolutionError::InvalidApproval);
        }
    }
    Ok(())
}

pub fn build_release(
    candidate: &SkillCandidate,
    decision: &PromotionDecision,
    approval: Option<&ApprovalRef>,
    now: DateTime<Utc>,
) -> Result<SkillRelease, EvolutionError> {
    authorize_promotion(candidate, decision, approval, now)?;
    if candidate.base_release_uuid.is_some() && !candidate.reversible {
        return Err(EvolutionError::MissingRollbackLineage);
    }
    Ok(SkillRelease {
        release_uuid: Uuid::now_v7(),
        candidate_uuid: candidate.candidate_uuid,
        skill_uuid: candidate.skill_uuid,
        previous_release_uuid: candidate.base_release_uuid,
        artifact_sha256_hex: candidate.artifact_sha256_hex.clone(),
        manifest_sha256_hex: candidate.manifest_sha256_hex.clone(),
        promoted_at: now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(now: DateTime<Utc>) -> SkillCandidate {
        let digest = "a".repeat(64);
        SkillCandidate {
            candidate_uuid: Uuid::now_v7(),
            tenant_uuid: Uuid::now_v7(),
            skill_uuid: Uuid::now_v7(),
            base_release_uuid: Some(Uuid::now_v7()),
            target_namespace: "skills.phoenix.example".into(),
            boundary: ChangeBoundary::SkillPlugin,
            artifact_sha256_hex: digest.clone(),
            manifest_sha256_hex: "b".repeat(64),
            source_state_sha256_hex: "c".repeat(64),
            risk: RiskLevel::Low,
            behavior_change: false,
            reversible: true,
            created_at: now,
        }
    }

    fn evidence(c: &SkillCandidate, now: DateTime<Utc>) -> EvidenceSummary {
        EvidenceSummary {
            candidate_uuid: c.candidate_uuid,
            artifact_sha256_hex: c.artifact_sha256_hex.clone(),
            source_state_sha256_hex: c.source_state_sha256_hex.clone(),
            required_suite_runs: 3,
            passed_checks: 30,
            failed_checks: 0,
            security_critical_open: 0,
            security_high_open: 0,
            benchmark_regression_bps: 0,
            independent_evaluators: 2,
            freshest_evidence_at: now,
        }
    }

    #[test]
    fn microkernel_candidate_is_blocked() {
        let now = Utc::now();
        let mut c = candidate(now);
        c.boundary = ChangeBoundary::Microkernel;
        assert_eq!(
            evaluate_candidate(&c, &evidence(&c, now), &PromotionPolicy::default(), now),
            Err(EvolutionError::ForbiddenBoundary)
        );
    }

    #[test]
    fn failed_suite_blocks_promotion() {
        let now = Utc::now();
        let c = candidate(now);
        let mut e = evidence(&c, now);
        e.failed_checks = 1;
        assert_eq!(
            evaluate_candidate(&c, &e, &PromotionPolicy::default(), now),
            Err(EvolutionError::TestFailure)
        );
    }

    #[test]
    fn low_risk_reversible_candidate_can_use_auto_path() {
        let now = Utc::now();
        let c = candidate(now);
        let d =
            evaluate_candidate(&c, &evidence(&c, now), &PromotionPolicy::default(), now).unwrap();
        assert_eq!(d.path, PromotionPath::AutoLowRisk);
        assert!(!d.requires_human_approval);
    }

    #[test]
    fn behavior_change_requires_human_approval() {
        let now = Utc::now();
        let mut c = candidate(now);
        c.behavior_change = true;
        let d =
            evaluate_candidate(&c, &evidence(&c, now), &PromotionPolicy::default(), now).unwrap();
        assert!(d.requires_human_approval);
        assert_eq!(
            authorize_promotion(&c, &d, None, now),
            Err(EvolutionError::ApprovalRequired)
        );
    }

    #[test]
    fn stale_evidence_is_rejected() {
        let now = Utc::now();
        let c = candidate(now);
        let mut e = evidence(&c, now);
        e.freshest_evidence_at = now - Duration::hours(25);
        assert_eq!(
            evaluate_candidate(&c, &e, &PromotionPolicy::default(), now),
            Err(EvolutionError::StaleEvidence)
        );
    }
}

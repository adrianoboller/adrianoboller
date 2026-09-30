use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

pub const CORE_AUTO_MERGE: bool = false;
pub const FIXTURE_CAN_PROMOTE_PRODUCTION: bool = false;
pub const UNAVAILABLE_IS_PASS: bool = false;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceClass { Production, Fixture }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome { Success, Failure, Partial }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel { Low, Medium, High, Critical }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionState { Candidate, Accepted, Governed, Stale, Revoked }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateKind { Knowledge, Prompt, Skill, Workflow, Code, CorePolicy }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFingerprintInput {
    pub language: String,
    pub framework: String,
    pub module: String,
    pub project_type: String,
    pub task_class: String,
    pub risk: RiskLevel,
    pub environment: String,
    pub dependency_fingerprint: String,
    pub source_state_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFingerprint {
    pub fingerprint_sha256: String,
    pub input: ContextFingerprintInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperienceEpisode {
    pub episode_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub task_uuid: Uuid,
    pub agent_uuid: Uuid,
    pub model_profile_uuid: Option<Uuid>,
    pub task_class: String,
    pub context_fingerprint_sha256: String,
    pub source_state_sha256: String,
    pub prompt_plan_sha256: Option<String>,
    pub skill_versions: BTreeMap<String, String>,
    pub outcome: Outcome,
    pub quality_score: f64,
    pub actual_cost_microunits: u64,
    pub duration_ms: u64,
    pub evidence_class: EvidenceClass,
    pub evidence_sha256: String,
    pub created_at_epoch_s: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgePattern {
    pub pattern_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub pattern_key: String,
    pub context_fingerprint_sha256: String,
    pub promotion_state: PromotionState,
    pub evidence_class: EvidenceClass,
    pub evidence_count: u32,
    pub success_count: u32,
    pub failure_count: u32,
    pub reuse_count: u32,
    pub confidence: f64,
    pub root_cause: Option<String>,
    pub remediation: Option<String>,
    pub safe_retry_conditions: Option<String>,
    pub fresh_until_epoch_s: i64,
    pub source_state_sha256: String,
    pub evidence_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionCandidate {
    pub candidate_uuid: Uuid,
    pub kind: CandidateKind,
    pub risk: RiskLevel,
    pub reversible: bool,
    pub touches_core_or_policy: bool,
    pub evidence_class: EvidenceClass,
    pub source_state_sha256: String,
    pub artifact_sha256: String,
    pub independent_evaluators: u32,
    pub tests_passed: bool,
    pub security_high_critical_open: u32,
    pub benchmark_regressed: bool,
    pub unresolved_contradictions: u32,
    pub fresh_until_epoch_s: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionPolicy {
    pub autonomy_level: u8,
    pub allow_low_risk_prompt_auto_promotion: bool,
    pub min_independent_evaluators: u32,
    pub now_epoch_s: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionDecision {
    pub allowed: bool,
    pub automatic: bool,
    pub reason: String,
}

#[derive(Debug, Error)]
pub enum LearningError {
    #[error("fixture evidence cannot promote production knowledge")]
    FixturePromotion,
    #[error("core or policy changes require human governance")]
    CoreGovernance,
    #[error("knowledge is stale")]
    Stale,
    #[error("context mismatch")]
    ContextMismatch,
}

pub fn canonical_sha256<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serializable learning value");
    format!("{:x}", Sha256::digest(bytes))
}

pub fn create_context_fingerprint(input: ContextFingerprintInput) -> ContextFingerprint {
    let fingerprint_sha256 = canonical_sha256(&input);
    ContextFingerprint { fingerprint_sha256, input }
}

pub fn context_compatible(required: &ContextFingerprint, observed: &ContextFingerprint) -> bool {
    required.fingerprint_sha256 == observed.fingerprint_sha256
        || (required.input.language == observed.input.language
            && required.input.framework == observed.input.framework
            && required.input.module == observed.input.module
            && required.input.project_type == observed.input.project_type
            && required.input.task_class == observed.input.task_class
            && required.input.environment == observed.input.environment
            && required.input.dependency_fingerprint == observed.input.dependency_fingerprint)
}

pub fn strong_preference_allowed(pattern: &KnowledgePattern, now_epoch_s: i64) -> Result<bool, LearningError> {
    if pattern.evidence_class == EvidenceClass::Fixture { return Err(LearningError::FixturePromotion); }
    if pattern.fresh_until_epoch_s < now_epoch_s { return Err(LearningError::Stale); }
    Ok(pattern.promotion_state == PromotionState::Governed && pattern.evidence_count >= 2)
}

pub fn unfruitful_guard_blocks(
    pattern: &KnowledgePattern,
    required_context_sha256: &str,
    now_epoch_s: i64,
) -> bool {
    pattern.promotion_state == PromotionState::Governed
        && pattern.evidence_class == EvidenceClass::Production
        && pattern.evidence_count >= 2
        && pattern.fresh_until_epoch_s >= now_epoch_s
        && pattern.context_fingerprint_sha256 == required_context_sha256
        && pattern.failure_count > 0
}

pub fn can_auto_promote(candidate: &EvolutionCandidate, policy: &PromotionPolicy) -> PromotionDecision {
    if candidate.evidence_class != EvidenceClass::Production {
        return PromotionDecision { allowed: false, automatic: false, reason: "fixture evidence cannot promote production".into() };
    }
    if candidate.touches_core_or_policy || candidate.kind == CandidateKind::CorePolicy {
        return PromotionDecision { allowed: false, automatic: false, reason: "core/policy requires human governance".into() };
    }
    if candidate.fresh_until_epoch_s < policy.now_epoch_s {
        return PromotionDecision { allowed: false, automatic: false, reason: "candidate evidence is stale".into() };
    }
    if candidate.independent_evaluators < policy.min_independent_evaluators {
        return PromotionDecision { allowed: false, automatic: false, reason: "insufficient independent evaluators".into() };
    }
    if !candidate.tests_passed || candidate.security_high_critical_open > 0 || candidate.benchmark_regressed || candidate.unresolved_contradictions > 0 {
        return PromotionDecision { allowed: false, automatic: false, reason: "quality/security/benchmark/contradiction gate failed".into() };
    }
    let prompt_auto = candidate.kind == CandidateKind::Prompt
        && candidate.risk == RiskLevel::Low
        && candidate.reversible
        && policy.autonomy_level >= 2
        && policy.allow_low_risk_prompt_auto_promotion;
    if prompt_auto {
        return PromotionDecision { allowed: true, automatic: true, reason: "reversible low-risk prompt passed governed promotion gates".into() };
    }
    PromotionDecision { allowed: true, automatic: false, reason: "candidate passed gates but requires explicit promotion approval".into() }
}

pub fn confidence(success: u32, failure: u32, reuse: u32) -> f64 {
    let total = success.saturating_add(failure);
    if total == 0 { return 0.0; }
    let empirical = success as f64 / total as f64;
    let reuse_bonus = (reuse.min(20) as f64) / 200.0;
    (empirical * 0.95 + reuse_bonus).min(0.999)
}

pub fn should_create_skill_candidate(occurrences: u32, distinct_runs: u32) -> bool {
    occurrences >= 3 && distinct_runs >= 2
}

pub fn self_improvement_branch(change_uuid: Uuid) -> String {
    format!("agent/self-improve/{change_uuid}")
}

pub fn may_self_improve(kind: CandidateKind, touches_core_or_policy: bool) -> bool {
    !touches_core_or_policy && !matches!(kind, CandidateKind::CorePolicy)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_never_auto_promotes() {
        let c = EvolutionCandidate { candidate_uuid: Uuid::now_v7(), kind: CandidateKind::Prompt, risk: RiskLevel::Low, reversible:true,
          touches_core_or_policy:false, evidence_class:EvidenceClass::Fixture, source_state_sha256:"s".into(), artifact_sha256:"a".into(), independent_evaluators:2,
          tests_passed:true, security_high_critical_open:0, benchmark_regressed:false, unresolved_contradictions:0, fresh_until_epoch_s:100 };
        let p=PromotionPolicy{autonomy_level:2,allow_low_risk_prompt_auto_promotion:true,min_independent_evaluators:2,now_epoch_s:10};
        assert!(!can_auto_promote(&c,&p).allowed);
    }
    #[test]
    fn core_never_auto_merges() { assert!(!CORE_AUTO_MERGE); }
    #[test]
    fn governed_unfruitful_blocks_exact_context() {
        let p=KnowledgePattern{pattern_uuid:Uuid::now_v7(),tenant_uuid:Uuid::now_v7(),project_uuid:Uuid::now_v7(),pattern_key:"x".into(),context_fingerprint_sha256:"ctx".into(),
          promotion_state:PromotionState::Governed,evidence_class:EvidenceClass::Production,evidence_count:2,success_count:0,failure_count:3,reuse_count:0,confidence:0.9,
          root_cause:Some("bad context".into()),remediation:Some("retrieve more context".into()),safe_retry_conditions:Some("context complete".into()),fresh_until_epoch_s:100,
          source_state_sha256:"s".into(),evidence_sha256:"e".into()};
        assert!(unfruitful_guard_blocks(&p,"ctx",10));
        assert!(!unfruitful_guard_blocks(&p,"other",10));
    }
}

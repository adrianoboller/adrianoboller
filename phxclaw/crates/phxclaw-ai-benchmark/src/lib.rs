use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TaskFamily { Coding, RepoAnalysis, ToolUse, StructuredOutput, Reasoning, Vision, Embeddings, General }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ComplexityBand { Low, Medium, High, Extreme }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceClass { Production, Fixture }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchmarkCase {
    pub case_uuid: Uuid,
    pub prompt_sha256: String,
    pub expected_contract_sha256: String,
    pub tags: BTreeSet<String>,
    pub weight: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchmarkSuite {
    pub tenant_uuid: Uuid,
    pub suite_uuid: Uuid,
    pub name: String,
    pub version: String,
    pub task_family: TaskFamily,
    pub complexity: ComplexityBand,
    pub evidence_class: EvidenceClass,
    pub dataset_sha256: String,
    pub scorer_sha256: String,
    pub environment_sha256: String,
    pub cases: Vec<BenchmarkCase>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchmarkObservation {
    pub tenant_uuid: Uuid,
    pub run_uuid: Uuid,
    pub case_uuid: Uuid,
    pub provider_uuid: Uuid,
    pub model_id: String,
    pub suite_uuid: Uuid,
    pub dataset_sha256: String,
    pub scorer_sha256: String,
    pub environment_sha256: String,
    pub success: bool,
    pub quality_basis_points: u16,
    pub tool_accuracy_basis_points: Option<u16>,
    pub structured_validity_basis_points: Option<u16>,
    pub latency_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub actual_cost_micro_usd: Option<u64>,
    pub output_sha256: String,
    pub error_class: Option<String>,
    pub observed_at_unix: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PerformanceProfile {
    pub tenant_uuid: Uuid,
    pub profile_uuid: Uuid,
    pub provider_uuid: Uuid,
    pub model_id: String,
    pub suite_uuid: Uuid,
    pub task_family: TaskFamily,
    pub complexity: ComplexityBand,
    pub evidence_class: EvidenceClass,
    pub dataset_sha256: String,
    pub scorer_sha256: String,
    pub environment_sha256: String,
    pub sample_count: u32,
    pub success_basis_points: u16,
    pub quality_basis_points: u16,
    pub tool_accuracy_basis_points: Option<u16>,
    pub structured_validity_basis_points: Option<u16>,
    pub p95_latency_ms: u64,
    pub median_cost_micro_usd: Option<u64>,
    pub evidence_coverage_basis_points: u16,
    pub profile_sha256: String,
    pub observed_at_unix: i64,
    pub ttl_seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromotionPolicy {
    pub min_samples: u32,
    pub min_success_basis_points: u16,
    pub min_quality_basis_points: u16,
    pub min_evidence_coverage_basis_points: u16,
    pub max_quality_regression_basis_points: u16,
    pub max_success_regression_basis_points: u16,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum BenchmarkError {
    #[error("invalid sha256 field")] InvalidHash,
    #[error("suite/case mismatch")] SuiteMismatch,
    #[error("duplicate observation")] DuplicateObservation,
    #[error("no observations")] NoObservations,
    #[error("basis points out of range")] InvalidBasisPoints,
    #[error("profile is not promotable")] NotPromotable,
    #[error("fixture profile cannot be promoted")] FixtureNotPromotable,
    #[error("profile is stale")] StaleProfile,
}

fn valid_sha(s: &str) -> bool { s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()) }
fn sha256_hex(bytes: &[u8]) -> String { hex::encode(Sha256::digest(bytes)) }
fn percentile95(mut v: Vec<u64>) -> u64 {
    if v.is_empty() { return 0; }
    v.sort_unstable();
    let idx = ((v.len() * 95 + 99) / 100).saturating_sub(1).min(v.len() - 1);
    v[idx]
}
fn median(mut v: Vec<u64>) -> Option<u64> {
    if v.is_empty() { return None; }
    v.sort_unstable();
    Some(v[v.len() / 2])
}
fn weighted_mean(values: &[(u16,u16)]) -> u16 {
    let den: u64 = values.iter().map(|(_,w)| *w as u64).sum();
    if den == 0 { return 0; }
    let num: u64 = values.iter().map(|(v,w)| *v as u64 * *w as u64).sum();
    (num / den).min(10_000) as u16
}

pub fn validate_suite(s: &BenchmarkSuite) -> Result<(), BenchmarkError> {
    if !valid_sha(&s.dataset_sha256) || !valid_sha(&s.scorer_sha256) || !valid_sha(&s.environment_sha256) { return Err(BenchmarkError::InvalidHash); }
    let mut ids = BTreeSet::new();
    for c in &s.cases {
        if c.weight == 0 || !valid_sha(&c.prompt_sha256) || !valid_sha(&c.expected_contract_sha256) { return Err(BenchmarkError::InvalidHash); }
        if !ids.insert(c.case_uuid) { return Err(BenchmarkError::SuiteMismatch); }
    }
    if s.cases.is_empty() { return Err(BenchmarkError::NoObservations); }
    Ok(())
}

pub fn aggregate_profile(
    suite: &BenchmarkSuite,
    provider_uuid: Uuid,
    model_id: &str,
    observations: &[BenchmarkObservation],
    now_unix: i64,
    ttl_seconds: u64,
) -> Result<PerformanceProfile, BenchmarkError> {
    validate_suite(suite)?;
    let case_weights: BTreeMap<Uuid,u16> = suite.cases.iter().map(|c|(c.case_uuid,c.weight)).collect();
    let mut seen = BTreeSet::new();
    let mut quality = Vec::new(); let mut success = Vec::new(); let mut tools = Vec::new(); let mut structured = Vec::new();
    let mut latency = Vec::new(); let mut costs = Vec::new();
    for o in observations {
        if o.tenant_uuid != suite.tenant_uuid || o.suite_uuid != suite.suite_uuid || o.provider_uuid != provider_uuid || o.model_id != model_id || o.dataset_sha256 != suite.dataset_sha256 || o.scorer_sha256 != suite.scorer_sha256 || o.environment_sha256 != suite.environment_sha256 { return Err(BenchmarkError::SuiteMismatch); }
        if !valid_sha(&o.output_sha256) || o.quality_basis_points > 10_000 || o.tool_accuracy_basis_points.unwrap_or(0) > 10_000 || o.structured_validity_basis_points.unwrap_or(0) > 10_000 { return Err(BenchmarkError::InvalidBasisPoints); }
        if !seen.insert((o.run_uuid,o.case_uuid)) { return Err(BenchmarkError::DuplicateObservation); }
        let w = *case_weights.get(&o.case_uuid).ok_or(BenchmarkError::SuiteMismatch)?;
        quality.push((o.quality_basis_points,w)); success.push((if o.success {10_000} else {0},w));
        if let Some(v)=o.tool_accuracy_basis_points { tools.push((v,w)); }
        if let Some(v)=o.structured_validity_basis_points { structured.push((v,w)); }
        latency.push(o.latency_ms); if let Some(v)=o.actual_cost_micro_usd { costs.push(v); }
    }
    if observations.is_empty() { return Err(BenchmarkError::NoObservations); }
    let covered: BTreeSet<Uuid> = observations.iter().map(|o|o.case_uuid).collect();
    let coverage = ((covered.len() as u64 * 10_000) / suite.cases.len() as u64).min(10_000) as u16;
    let mut p = PerformanceProfile {
        tenant_uuid: suite.tenant_uuid, profile_uuid: Uuid::now_v7(), provider_uuid, model_id:model_id.to_owned(), suite_uuid:suite.suite_uuid,
        task_family:suite.task_family, complexity:suite.complexity, evidence_class:suite.evidence_class, dataset_sha256:suite.dataset_sha256.clone(), scorer_sha256:suite.scorer_sha256.clone(), environment_sha256:suite.environment_sha256.clone(),
        sample_count:observations.len() as u32, success_basis_points:weighted_mean(&success), quality_basis_points:weighted_mean(&quality),
        tool_accuracy_basis_points:if tools.is_empty(){None}else{Some(weighted_mean(&tools))}, structured_validity_basis_points:if structured.is_empty(){None}else{Some(weighted_mean(&structured))},
        p95_latency_ms:percentile95(latency), median_cost_micro_usd:median(costs), evidence_coverage_basis_points:coverage, profile_sha256:String::new(), observed_at_unix:now_unix, ttl_seconds,
    };
    let canonical = serde_json::to_vec(&(&p.tenant_uuid,&p.provider_uuid,&p.model_id,&p.suite_uuid,p.task_family,p.complexity,p.evidence_class,&p.dataset_sha256,&p.scorer_sha256,&p.environment_sha256,p.sample_count,p.success_basis_points,p.quality_basis_points,p.tool_accuracy_basis_points,p.structured_validity_basis_points,p.p95_latency_ms,p.median_cost_micro_usd,p.evidence_coverage_basis_points,p.observed_at_unix,p.ttl_seconds)).expect("profile serialization");
    p.profile_sha256=sha256_hex(&canonical); Ok(p)
}

pub fn can_promote(now_unix:i64, policy:&PromotionPolicy, candidate:&PerformanceProfile, previous:Option<&PerformanceProfile>) -> Result<(),BenchmarkError> {
    if candidate.evidence_class==EvidenceClass::Fixture { return Err(BenchmarkError::FixtureNotPromotable); }
    if candidate.observed_at_unix>now_unix || now_unix.saturating_sub(candidate.observed_at_unix)>candidate.ttl_seconds as i64 { return Err(BenchmarkError::StaleProfile); }
    if !valid_sha(&candidate.profile_sha256) { return Err(BenchmarkError::InvalidHash); }
    if candidate.sample_count < policy.min_samples || candidate.success_basis_points < policy.min_success_basis_points || candidate.quality_basis_points < policy.min_quality_basis_points || candidate.evidence_coverage_basis_points < policy.min_evidence_coverage_basis_points { return Err(BenchmarkError::NotPromotable); }
    if let Some(prev)=previous {
        if prev.quality_basis_points.saturating_sub(candidate.quality_basis_points) > policy.max_quality_regression_basis_points { return Err(BenchmarkError::NotPromotable); }
        if prev.success_basis_points.saturating_sub(candidate.success_basis_points) > policy.max_success_regression_basis_points { return Err(BenchmarkError::NotPromotable); }
    }
    Ok(())
}

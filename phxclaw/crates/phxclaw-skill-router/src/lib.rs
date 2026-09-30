use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all="snake_case")]
pub enum WorkflowPhase { ResearchValidate, UnderstandImprove, CreateExplain, RefineDeliver }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum LicenseState { VerifiedPermissive, VerifiedPermissiveAdapterOnly, VerifiedPermissiveSkillOnly, VerifiedPermissiveNameMatched, Unverified, Blocked }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillDescriptor {
    pub id: String,
    pub name: String,
    pub phase: WorkflowPhase,
    pub source_url: String,
    pub license: String,
    pub license_state: LicenseState,
    pub capability: String,
    pub permissions: BTreeSet<String>,
    pub network_required: bool,
    pub process_allowlist: BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskRequest {
    pub request_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub objective: String,
    pub requested_phases: Vec<WorkflowPhase>,
    pub preferred_skill_ids: Vec<String>,
    pub allowed_permissions: BTreeSet<String>,
    pub network_allowed: bool,
    pub max_skills_total: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteStep {
    pub ordinal: u16,
    pub phase: WorkflowPhase,
    pub skill_id: String,
    pub capability: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutePlan {
    pub request_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub catalog_sha256: String,
    pub steps: Vec<RouteStep>,
    pub plan_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillResultEvidence {
    pub request_uuid: Uuid,
    pub skill_id: String,
    pub capability: String,
    pub input_sha256: String,
    pub output_sha256: String,
    pub source_evidence: Vec<String>,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum RouterError {
    #[error("skill provenance/license not accepted")] LicenseBlocked,
    #[error("required permission is not allowed")] PermissionDenied,
    #[error("network access is not allowed")] NetworkDenied,
    #[error("process is not explicitly allowlisted")] ProcessDenied,
    #[error("route exceeds configured skill budget")] RouteTooLarge,
    #[error("requested phase has no eligible skill")] NoEligibleSkill,
    #[error("invalid catalog hash")] InvalidCatalogHash,
}

fn accepted(state: LicenseState) -> bool {
    matches!(state, LicenseState::VerifiedPermissive | LicenseState::VerifiedPermissiveAdapterOnly | LicenseState::VerifiedPermissiveSkillOnly | LicenseState::VerifiedPermissiveNameMatched)
}

pub fn eligible(skill: &SkillDescriptor, request: &TaskRequest) -> Result<(), RouterError> {
    if !accepted(skill.license_state) { return Err(RouterError::LicenseBlocked); }
    if skill.network_required && !request.network_allowed { return Err(RouterError::NetworkDenied); }
    if !skill.permissions.is_subset(&request.allowed_permissions) { return Err(RouterError::PermissionDenied); }
    if skill.process_allowlist.iter().any(|p| !skill.permissions.contains(&format!("process.exec:{p}"))) { return Err(RouterError::ProcessDenied); }
    Ok(())
}

pub fn build_route(catalog_sha256: &str, catalog: &[SkillDescriptor], request: &TaskRequest) -> Result<RoutePlan, RouterError> {
    if catalog_sha256.len()!=64 || !catalog_sha256.bytes().all(|b| b.is_ascii_hexdigit()) { return Err(RouterError::InvalidCatalogHash); }
    let max = request.max_skills_total.clamp(1, 8);
    let mut by_id = BTreeMap::new(); for s in catalog { by_id.insert(s.id.as_str(), s); }
    let mut steps=Vec::new(); let mut used=BTreeSet::new();
    for phase in &request.requested_phases {
        let mut candidates: Vec<&SkillDescriptor> = request.preferred_skill_ids.iter().filter_map(|id| by_id.get(id.as_str()).copied()).filter(|s| s.phase==*phase).collect();
        let mut rest: Vec<&SkillDescriptor> = catalog.iter().filter(|s| s.phase==*phase).collect(); rest.sort_by(|a,b| a.id.cmp(&b.id)); candidates.extend(rest);
        let chosen=candidates.into_iter().find(|s| !used.contains(&s.id) && eligible(s,request).is_ok()).ok_or(RouterError::NoEligibleSkill)?;
        used.insert(chosen.id.clone());
        steps.push(RouteStep{ordinal:steps.len() as u16 +1,phase:*phase,skill_id:chosen.id.clone(),capability:chosen.capability.clone(),reason:format!("eligible skill for {:?}; permissions/license checked",phase)});
        if steps.len()>max { return Err(RouterError::RouteTooLarge); }
    }
    let canonical=serde_json::to_vec(&(request.request_uuid,request.tenant_uuid,catalog_sha256,&steps)).expect("serializable");
    let plan_sha256=format!("{:x}",Sha256::digest(canonical));
    Ok(RoutePlan{request_uuid:request.request_uuid,tenant_uuid:request.tenant_uuid,catalog_sha256:catalog_sha256.to_owned(),steps,plan_sha256})
}

pub fn standard_engineering_pipeline() -> Vec<WorkflowPhase> {
    vec![WorkflowPhase::ResearchValidate, WorkflowPhase::UnderstandImprove, WorkflowPhase::CreateExplain, WorkflowPhase::RefineDeliver]
}

pub fn evidence_hash(e: &SkillResultEvidence) -> String {
    let bytes=serde_json::to_vec(e).expect("serializable"); format!("{:x}",Sha256::digest(bytes))
}

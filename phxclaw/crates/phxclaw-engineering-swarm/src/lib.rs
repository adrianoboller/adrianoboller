use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all="snake_case")]
pub enum TeamRole { Research, Architecture, Coding, Qa, Security, Documentation }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum TeamState { Pending, Ready, Running, Completed, Failed, RolledBack, Cancelled }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum GateDecision { Allow, Block, NeedsApproval }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum ConflictKind { SourceDrift, OverlappingWrite, EvidenceConflict, ArchitectureConflict, QaFailure, SecurityFinding, DocumentationMismatch }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all="snake_case")]
pub enum Severity { Info, Low, Medium, High, Critical }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SwarmSpec {
    pub swarm_uuid: Uuid,
    pub workflow_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub objective: String,
    pub source_state_sha256: String,
    pub policy_sha256: String,
    pub skill_catalog_sha256: String,
    pub max_parallel_teams: u8,
    pub enabled_teams: BTreeSet<TeamRole>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamAssignment {
    pub assignment_uuid: Uuid,
    pub swarm_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub team: TeamRole,
    pub actor_uuid: Uuid,
    pub base_source_sha256: String,
    pub worktree_path: String,
    pub branch: String,
    pub dependencies: BTreeSet<TeamRole>,
    pub lease_uuid: Uuid,
    pub fencing_token: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamEvidence {
    pub evidence_uuid: Uuid,
    pub swarm_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub team: TeamRole,
    pub actor_uuid: Uuid,
    pub source_state_sha256: String,
    pub artifact_sha256: String,
    pub evidence_refs: Vec<String>,
    pub passed: bool,
    pub highest_severity: Severity,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamCheckpoint {
    pub checkpoint_uuid: Uuid,
    pub swarm_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub team: TeamRole,
    pub workspace_state_sha256: String,
    pub git_commit: Option<String>,
    pub fencing_token: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MergeConflict {
    pub conflict_uuid: Uuid,
    pub swarm_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub kind: ConflictKind,
    pub teams: BTreeSet<TeamRole>,
    pub subject: String,
    pub evidence_refs: Vec<Uuid>,
    pub resolved_by: Option<Uuid>,
    pub resolution_sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MergeCandidate {
    pub candidate_uuid: Uuid,
    pub swarm_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub base_source_sha256: String,
    pub integration_tree_sha256: String,
    pub team_evidence: BTreeMap<TeamRole, Uuid>,
    pub unresolved_conflicts: BTreeSet<Uuid>,
    pub architecture_change: bool,
    pub public_or_behavioral_change: bool,
    pub target_main_or_production: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalProof {
    pub approval_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub swarm_uuid: Uuid,
    pub approver_uuid: Uuid,
    pub candidate_sha256: String,
    pub approved: bool,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SwarmError {
    #[error("invalid sha256")]
    InvalidHash,
    #[error("source state mismatch")]
    SourceStateMismatch,
    #[error("team not enabled")]
    TeamNotEnabled,
    #[error("dependency not completed")]
    DependencyNotCompleted,
    #[error("parallelism limit exceeded")]
    ParallelismExceeded,
    #[error("shared writable worktree forbidden")]
    SharedWritableWorktree,
    #[error("stale fencing token")]
    StaleFencing,
    #[error("evidence missing")]
    MissingEvidence,
    #[error("independent review required")]
    IndependenceRequired,
    #[error("unresolved conflict")]
    UnresolvedConflict,
    #[error("quality gate failed")]
    QualityGateFailed,
    #[error("security gate failed")]
    SecurityGateFailed,
    #[error("approval required")]
    ApprovalRequired,
    #[error("rollback checkpoint required")]
    CheckpointRequired,
}

fn valid_hash(v: &str) -> bool { v.len()==64 && v.bytes().all(|b| b.is_ascii_hexdigit()) }

pub fn swarm_hash(spec: &SwarmSpec) -> Result<String, SwarmError> {
    if !valid_hash(&spec.source_state_sha256) || !valid_hash(&spec.policy_sha256) || !valid_hash(&spec.skill_catalog_sha256) { return Err(SwarmError::InvalidHash); }
    let b=serde_json::to_vec(spec).expect("serializable"); Ok(format!("{:x}",Sha256::digest(b)))
}

pub fn ready_teams(spec: &SwarmSpec, assignments: &[TeamAssignment], completed: &BTreeSet<TeamRole>) -> Result<Vec<TeamRole>, SwarmError> {
    let running=assignments.iter().filter(|a| !completed.contains(&a.team)).count();
    if running > spec.max_parallel_teams as usize { return Err(SwarmError::ParallelismExceeded); }
    let mut out=Vec::new();
    for t in &spec.enabled_teams {
        if completed.contains(t) || assignments.iter().any(|a| a.team==*t) { continue; }
        let deps: BTreeSet<TeamRole> = match t {
            TeamRole::Research => BTreeSet::new(),
            TeamRole::Architecture => [TeamRole::Research].into_iter().collect(),
            TeamRole::Coding => [TeamRole::Architecture].into_iter().collect(),
            TeamRole::Qa|TeamRole::Security => [TeamRole::Coding].into_iter().collect(),
            TeamRole::Documentation => [TeamRole::Architecture].into_iter().collect(),
        };
        if deps.is_subset(completed) { out.push(*t); }
    }
    out.sort();
    Ok(out)
}

pub fn verify_assignment(spec: &SwarmSpec, a: &TeamAssignment, existing: &[TeamAssignment], current_fencing: u64) -> Result<(), SwarmError> {
    if !spec.enabled_teams.contains(&a.team) { return Err(SwarmError::TeamNotEnabled); }
    if a.tenant_uuid!=spec.tenant_uuid || a.swarm_uuid!=spec.swarm_uuid || a.base_source_sha256!=spec.source_state_sha256 { return Err(SwarmError::SourceStateMismatch); }
    if a.fencing_token < current_fencing { return Err(SwarmError::StaleFencing); }
    if existing.iter().any(|x| x.assignment_uuid!=a.assignment_uuid && x.worktree_path==a.worktree_path) { return Err(SwarmError::SharedWritableWorktree); }
    if existing.iter().any(|x| x.assignment_uuid!=a.assignment_uuid && x.branch==a.branch) { return Err(SwarmError::SharedWritableWorktree); }
    Ok(())
}

pub fn verify_team_evidence(spec: &SwarmSpec, ev: &TeamEvidence) -> Result<(), SwarmError> {
    if ev.swarm_uuid!=spec.swarm_uuid || ev.tenant_uuid!=spec.tenant_uuid || ev.source_state_sha256!=spec.source_state_sha256 { return Err(SwarmError::SourceStateMismatch); }
    if !valid_hash(&ev.artifact_sha256) { return Err(SwarmError::InvalidHash); }
    if ev.evidence_refs.is_empty() { return Err(SwarmError::MissingEvidence); }
    Ok(())
}

pub fn detect_write_conflicts(changed_paths: &BTreeMap<TeamRole,BTreeSet<String>>) -> Vec<(TeamRole,TeamRole,String)> {
    let teams: Vec<_>=changed_paths.keys().copied().collect(); let mut out=Vec::new();
    for i in 0..teams.len() { for j in i+1..teams.len() {
        for p in changed_paths[&teams[i]].intersection(&changed_paths[&teams[j]]) { out.push((teams[i],teams[j],p.clone())); }
    }}
    out.sort(); out
}

pub fn consensus_gate(candidate: &MergeCandidate, evidence: &BTreeMap<TeamRole,TeamEvidence>, conflicts: &[MergeConflict], coding_actor: Uuid) -> Result<GateDecision, SwarmError> {
    if !candidate.unresolved_conflicts.is_empty() || conflicts.iter().any(|c| c.resolved_by.is_none()) { return Err(SwarmError::UnresolvedConflict); }
    for required in [TeamRole::Coding,TeamRole::Qa,TeamRole::Security] {
        if !candidate.team_evidence.contains_key(&required) || !evidence.contains_key(&required) { return Err(SwarmError::MissingEvidence); }
    }
    let qa=&evidence[&TeamRole::Qa]; let sec=&evidence[&TeamRole::Security];
    if qa.actor_uuid==coding_actor || sec.actor_uuid==coding_actor { return Err(SwarmError::IndependenceRequired); }
    if !qa.passed { return Err(SwarmError::QualityGateFailed); }
    if !sec.passed || sec.highest_severity>=Severity::High { return Err(SwarmError::SecurityGateFailed); }
    if candidate.architecture_change {
        let arch=evidence.get(&TeamRole::Architecture).ok_or(SwarmError::MissingEvidence)?;
        if !arch.passed { return Err(SwarmError::QualityGateFailed); }
    }
    if candidate.public_or_behavioral_change {
        let docs=evidence.get(&TeamRole::Documentation).ok_or(SwarmError::MissingEvidence)?;
        if !docs.passed { return Err(SwarmError::QualityGateFailed); }
    }
    if candidate.target_main_or_production { return Ok(GateDecision::NeedsApproval); }
    Ok(GateDecision::Allow)
}

pub fn verify_approval(candidate: &MergeCandidate, proof: Option<&ApprovalProof>) -> Result<(), SwarmError> {
    if !candidate.target_main_or_production { return Ok(()); }
    let p=proof.ok_or(SwarmError::ApprovalRequired)?;
    let h=merge_candidate_hash(candidate)?;
    if !p.approved || p.tenant_uuid!=candidate.tenant_uuid || p.swarm_uuid!=candidate.swarm_uuid || p.candidate_sha256!=h { return Err(SwarmError::ApprovalRequired); }
    Ok(())
}

pub fn merge_candidate_hash(c: &MergeCandidate) -> Result<String, SwarmError> {
    if !valid_hash(&c.base_source_sha256) || !valid_hash(&c.integration_tree_sha256) { return Err(SwarmError::InvalidHash); }
    Ok(format!("{:x}",Sha256::digest(serde_json::to_vec(c).expect("serializable"))))
}

pub fn rollback_allowed(team: TeamRole, failed_actor: Uuid, cp: Option<&TeamCheckpoint>, current_fencing: u64) -> Result<(), SwarmError> {
    let c=cp.ok_or(SwarmError::CheckpointRequired)?;
    if c.team!=team || !valid_hash(&c.workspace_state_sha256) { return Err(SwarmError::CheckpointRequired); }
    let _=failed_actor;
    if c.fencing_token < current_fencing { return Err(SwarmError::StaleFencing); }
    Ok(())
}

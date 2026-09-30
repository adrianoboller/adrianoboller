use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum HermesBridgeError {
    #[error("write operation requires explicit approval")]
    ApprovalRequired,
    #[error("secret import is disabled")]
    SecretImportDenied,
    #[error("untrusted skill source")]
    UntrustedSkill,
    #[error("MCP reference must pin at least one tool")]
    UnpinnedMcp,
}

fn looks_secret_like(s: &str) -> bool {
    ["-----BEGIN PRIVATE KEY-----", "ghp_", "github_pat_", "AKIA", "sk-"].iter().any(|m| s.contains(m))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HermesSkillCandidate {
    pub candidate_uuid: Uuid,
    pub source_path: String,
    pub content_sha256: String,
    pub promotion_target: String,
    pub provenance: String,
}
pub fn stage_skill(path: &str, content: &str, provenance: &str) -> Result<HermesSkillCandidate, HermesBridgeError> {
    if !path.ends_with("SKILL.md") { return Err(HermesBridgeError::UntrustedSkill); }
    if looks_secret_like(content) { return Err(HermesBridgeError::SecretImportDenied); }
    Ok(HermesSkillCandidate { candidate_uuid: Uuid::now_v7(), source_path: path.into(), content_sha256: format!("{:x}", Sha256::digest(content.as_bytes())), promotion_target: "f24_candidate".into(), provenance: provenance.into() })
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryObservation { pub observation_uuid: Uuid, pub source: String, pub content_sha256: String, pub epistemic_state: String }
pub fn observe_memory(source: &str, content: &str) -> Result<MemoryObservation, HermesBridgeError> {
    if looks_secret_like(content) { return Err(HermesBridgeError::SecretImportDenied); }
    Ok(MemoryObservation { observation_uuid: Uuid::now_v7(), source: source.into(), content_sha256: format!("{:x}", Sha256::digest(content.as_bytes())), epistemic_state: "unverified_context".into() })
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GitHubOperation { ReadIssue, ReadPullRequest, ReadActions, SearchCode, CreateIssue, Comment, Merge, WorkflowDispatch }
pub fn authorize_github(op: GitHubOperation, approved: bool) -> Result<(), HermesBridgeError> {
    match op { GitHubOperation::ReadIssue | GitHubOperation::ReadPullRequest | GitHubOperation::ReadActions | GitHubOperation::SearchCode => Ok(()), _ if approved => Ok(()), _ => Err(HermesBridgeError::ApprovalRequired) }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServerRef { pub name: String, pub transport: String, pub command_or_url: String, pub pinned_tools: Vec<String> }
pub fn mcp_ref(name: &str, transport: &str, command_or_url: &str, pinned_tools: Vec<String>) -> Result<McpServerRef, HermesBridgeError> {
    if pinned_tools.is_empty() { return Err(HermesBridgeError::UnpinnedMcp); }
    Ok(McpServerRef { name: name.into(), transport: transport.into(), command_or_url: command_or_url.into(), pinned_tools })
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhoenixScheduleRef { pub expression: String, pub target: String, pub source: String }
pub fn translate_cron(expression: &str, target: &str) -> PhoenixScheduleRef { PhoenixScheduleRef { expression: expression.into(), target: target.into(), source: "hermes_cron".into() } }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhoenixTeamDispatchRef { pub task: String, pub isolated: bool, pub source: String }
pub fn translate_subagent(task: &str) -> PhoenixTeamDispatchRef { PhoenixTeamDispatchRef { task: task.into(), isolated: true, source: "hermes_subagent".into() } }

pub fn secret_import_allowed() -> bool { false }

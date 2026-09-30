use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::{Path, PathBuf}};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("path escapes project root")]
    PathEscape,
    #[error("destructive restore requires a clean tree and pre-restore backup")]
    UnsafeRestore,
    #[error("raw secret material is forbidden in project manifests")]
    SecretMaterial,
    #[error("historical project state must open read-only or in a new worktree")]
    HistoricalWrite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPaths {
    pub root: PathBuf,
    pub source: Vec<PathBuf>,
    pub docs: PathBuf,
    pub tests: PathBuf,
    pub logs: PathBuf,
    pub artifacts: PathBuf,
    pub backups: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionControl {
    pub provider: String,
    pub repository: Option<String>,
    pub default_branch: String,
    pub current_commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiBindings {
    pub agents: Vec<String>,
    pub skills: Vec<String>,
    pub models: Vec<String>,
    pub activity_bindings: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub schema_version: String,
    pub project_uuid: Uuid,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub created_at: String,
    pub last_opened_at: Option<String>,
    pub template_id: Option<String>,
    pub paths: ProjectPaths,
    pub version_control: VersionControl,
    pub ai: AiBindings,
    pub tags: Vec<String>,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVersion {
    pub version_uuid: Uuid,
    pub project_uuid: Uuid,
    pub semver: Option<String>,
    pub git_commit: Option<String>,
    pub source_state_sha256: String,
    pub config_revision: u64,
    pub backup_uuid: Option<Uuid>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub backup_uuid: Uuid,
    pub project_uuid: Uuid,
    pub source_state_sha256: String,
    pub backup_path: PathBuf,
    pub file_count: u64,
    pub byte_count: u64,
    pub created_at: String,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFinding {
    pub severity: String,
    pub category: String,
    pub message: String,
    pub correlation_id: Option<String>,
    pub source: String,
    pub evidence_sha256: String,
}

pub fn safe_join(root: &Path, relative: &Path) -> Result<PathBuf, WorkspaceError> {
    let root = root.components().collect::<PathBuf>();
    let joined = root.join(relative);
    if relative.is_absolute() || relative.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(WorkspaceError::PathEscape);
    }
    Ok(joined)
}

pub fn validate_manifest_no_secrets(value: &serde_json::Value) -> Result<(), WorkspaceError> {
    let text = value.to_string().to_lowercase();
    for marker in ["api_key\":\"", "password\":\"", "private_key\":\"", "bearer "] {
        if text.contains(marker) { return Err(WorkspaceError::SecretMaterial); }
    }
    Ok(())
}

pub fn source_state_hash(entries: &[(String, Vec<u8>)]) -> String {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a,b| a.0.cmp(&b.0));
    let mut h = Sha256::new();
    for (path, bytes) in sorted {
        h.update(path.as_bytes()); h.update([0]); h.update(Sha256::digest(&bytes)); h.update([0xff]);
    }
    format!("{:x}", h.finalize())
}

pub fn open_historical(read_only: bool, new_worktree: bool) -> Result<(), WorkspaceError> {
    if !read_only && !new_worktree { return Err(WorkspaceError::HistoricalWrite); }
    Ok(())
}

pub fn guard_restore(clean_tree: bool, backup_verified: bool) -> Result<(), WorkspaceError> {
    if !(clean_tree && backup_verified) { return Err(WorkspaceError::UnsafeRestore); }
    Ok(())
}

pub fn redact_log_line(line: &str) -> String {
    let mut out = line.to_string();
    for key in ["password=", "token=", "api_key=", "apikey=", "secret="] {
        if let Some(pos) = out.to_lowercase().find(key) {
            let start = pos + key.len();
            let end = out[start..].find(|c: char| c.is_whitespace() || c == '&' || c == ';').map(|x| start+x).unwrap_or(out.len());
            out.replace_range(start..end, "[REDACTED]");
        }
    }
    out
}

use chrono::{DateTime, Utc};
use phxclaw_system_automation::{CommandRequest, CommandResult, ExecutionPolicy, ShellExecutor};
use phxclaw_types::new_uuid_v7;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePolicy {
    pub allow_git_mutation: bool,
    pub allow_file_write: bool,
    pub allowed_programs: Vec<String>,
    pub command_timeout_ms: u64,
    pub max_output_bytes: usize,
}

impl Default for WorkspacePolicy {
    fn default() -> Self {
        Self {
            allow_git_mutation: false,
            allow_file_write: false,
            allowed_programs: vec!["git".into()],
            command_timeout_ms: 120_000,
            max_output_bytes: 1_048_576,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitSnapshot {
    pub head: String,
    pub branch: String,
    pub status_porcelain: String,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeHandle {
    pub uuid: Uuid,
    pub mission_uuid: Uuid,
    pub branch: String,
    pub path: PathBuf,
    pub base_ref: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteResult {
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: usize,
    pub written_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateSpec {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub name: String,
    pub command: CommandResult,
    pub passed: bool,
}

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("workspace does not exist: {0}")]
    MissingWorkspace(String),
    #[error("workspace path escaped root: {0}")]
    PathEscape(String),
    #[error("state/worktree root must be outside the project workspace: {0}")]
    StateInsideWorkspace(String),
    #[error("invalid git base ref: {0}")]
    InvalidBaseRef(String),
    #[error("file writes are disabled by policy")]
    FileWriteDisabled,
    #[error("git mutation is disabled by policy")]
    GitMutationDisabled,
    #[error("program not allowed by workspace policy: {0}")]
    ProgramDenied(String),
    #[error("command output exceeded workspace policy limit")]
    OutputLimit,
    #[error("workspace command failed: {0}")]
    Command(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub struct CodeWorkspace {
    root: PathBuf,
    state_root: PathBuf,
    policy: WorkspacePolicy,
    executor: ShellExecutor,
}

impl CodeWorkspace {
    pub fn open(
        root: impl AsRef<Path>,
        state_root: impl AsRef<Path>,
        policy: WorkspacePolicy,
    ) -> Result<Self, WorkspaceError> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(WorkspaceError::MissingWorkspace(root.display().to_string()));
        }
        let root = fs::canonicalize(root)?;
        let state_root = state_root.as_ref().to_path_buf();
        fs::create_dir_all(&state_root)?;
        let state_root = fs::canonicalize(&state_root)?;
        if state_root.starts_with(&root) {
            return Err(WorkspaceError::StateInsideWorkspace(state_root.display().to_string()));
        }
        let executor = ShellExecutor::new(ExecutionPolicy {
            enabled: true,
            allow_shells: false,
            max_timeout_ms: policy.command_timeout_ms,
            denied_programs: vec![],
        });
        let this = Self { root, state_root, policy, executor };
        this.git(&["rev-parse", "--is-inside-work-tree"], &this.root)?;
        Ok(this)
    }

    pub fn root(&self) -> &Path { &self.root }

    pub fn snapshot(&self) -> Result<GitSnapshot, WorkspaceError> {
        Ok(GitSnapshot {
            head: self.git_text(&["rev-parse", "HEAD"], &self.root)?,
            branch: self.git_text(&["branch", "--show-current"], &self.root)?,
            status_porcelain: self.git_text(&["status", "--porcelain=v1"], &self.root)?,
            captured_at: Utc::now(),
        })
    }

    pub fn diff(&self, cwd: Option<&Path>) -> Result<String, WorkspaceError> {
        let cwd = cwd.unwrap_or(&self.root);
        self.ensure_inside(cwd)?;
        self.git_text(&["diff", "--no-ext-diff", "--binary"], cwd)
    }

    pub fn create_worktree(
        &self,
        mission_uuid: Uuid,
        base_ref: &str,
    ) -> Result<WorktreeHandle, WorkspaceError> {
        if !self.policy.allow_git_mutation {
            return Err(WorkspaceError::GitMutationDisabled);
        }
        if base_ref.trim().is_empty() || base_ref.starts_with('-') || base_ref.contains(char::is_whitespace) {
            return Err(WorkspaceError::InvalidBaseRef(base_ref.to_string()));
        }
        let short = mission_uuid.to_string()[..12].to_string();
        let branch = format!("agent/mission/{short}");
        let path = self.state_root.join("worktrees").join(mission_uuid.to_string());
        if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
        let path_text = path.to_string_lossy().into_owned();
        self.git(&["worktree", "add", "-b", &branch, &path_text, base_ref], &self.root)?;
        Ok(WorktreeHandle {
            uuid: new_uuid_v7(),
            mission_uuid,
            branch,
            path,
            base_ref: base_ref.to_string(),
            created_at: Utc::now(),
        })
    }

    pub fn remove_worktree(&self, handle: &WorktreeHandle) -> Result<(), WorkspaceError> {
        if !self.policy.allow_git_mutation {
            return Err(WorkspaceError::GitMutationDisabled);
        }
        let path_text = handle.path.to_string_lossy().into_owned();
        self.git(&["worktree", "remove", "--force", &path_text], &self.root)?;
        Ok(())
    }

    pub fn write_file(
        &self,
        workspace_root: &Path,
        relative_path: &Path,
        contents: &[u8],
    ) -> Result<FileWriteResult, WorkspaceError> {
        if !self.policy.allow_file_write {
            return Err(WorkspaceError::FileWriteDisabled);
        }
        let root = fs::canonicalize(workspace_root)?;
        let relative = normalize_relative(relative_path)?;
        let target = root.join(relative);
        if let Some(parent) = target.parent() { fs::create_dir_all(parent)?; }
        if !target.starts_with(&root) {
            return Err(WorkspaceError::PathEscape(target.display().to_string()));
        }
        fs::write(&target, contents)?;
        let sha256 = hex_sha256(contents);
        Ok(FileWriteResult {
            path: target,
            sha256,
            bytes: contents.len(),
            written_at: Utc::now(),
        })
    }

    pub fn run_gate(&self, gate: &GateSpec, default_cwd: &Path) -> Result<GateResult, WorkspaceError> {
        let cwd = gate.cwd.as_deref().unwrap_or(default_cwd);
        self.ensure_inside(cwd)?;
        self.ensure_program_allowed(&gate.program)?;
        let mut request = CommandRequest::direct(gate.program.clone(), gate.args.clone());
        request.cwd = Some(cwd.to_path_buf());
        request.timeout_ms = self.policy.command_timeout_ms;
        let command = self.executor.execute(&request).map_err(|e| WorkspaceError::Command(e.to_string()))?;
        if command.stdout.len().saturating_add(command.stderr.len()) > self.policy.max_output_bytes {
            return Err(WorkspaceError::OutputLimit);
        }
        let passed = !command.timed_out && command.exit_code == Some(0);
        Ok(GateResult { name: gate.name.clone(), command, passed })
    }

    fn ensure_inside(&self, path: &Path) -> Result<(), WorkspaceError> {
        let canonical = fs::canonicalize(path)?;
        if canonical.starts_with(&self.root) || canonical.starts_with(&self.state_root) {
            Ok(())
        } else {
            Err(WorkspaceError::PathEscape(canonical.display().to_string()))
        }
    }

    fn ensure_program_allowed(&self, program: &str) -> Result<(), WorkspaceError> {
        let allowed = self.policy.allowed_programs.iter().any(|item| item == program);
        if allowed { Ok(()) } else { Err(WorkspaceError::ProgramDenied(program.to_string())) }
    }

    fn git_text(&self, args: &[&str], cwd: &Path) -> Result<String, WorkspaceError> {
        let result = self.git(args, cwd)?;
        Ok(result.stdout.trim().to_string())
    }

    fn git(&self, args: &[&str], cwd: &Path) -> Result<CommandResult, WorkspaceError> {
        self.ensure_program_allowed("git")?;
        let mut request = CommandRequest::direct(
            "git",
            args.iter().map(|v| (*v).to_string()).collect(),
        );
        request.cwd = Some(cwd.to_path_buf());
        request.timeout_ms = self.policy.command_timeout_ms;
        let result = self.executor.execute(&request).map_err(|e| WorkspaceError::Command(e.to_string()))?;
        if result.stdout.len().saturating_add(result.stderr.len()) > self.policy.max_output_bytes {
            return Err(WorkspaceError::OutputLimit);
        }
        if result.timed_out || result.exit_code != Some(0) {
            return Err(WorkspaceError::Command(format!(
                "git {:?} exit={:?} timeout={} stderr={}", args, result.exit_code, result.timed_out, result.stderr.trim()
            )));
        }
        Ok(result)
    }
}

fn normalize_relative(path: &Path) -> Result<PathBuf, WorkspaceError> {
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => return Err(WorkspaceError::PathEscape(path.display().to_string())),
        }
    }
    if clean.as_os_str().is_empty() {
        return Err(WorkspaceError::PathEscape(path.display().to_string()));
    }
    Ok(clean)
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_rejects_parent_escape() {
        let error = normalize_relative(Path::new("../secret")).unwrap_err();
        assert!(matches!(error, WorkspaceError::PathEscape(_)));
    }

    #[test]
    fn default_policy_is_mutation_deny_by_default() {
        let policy = WorkspacePolicy::default();
        assert!(!policy.allow_git_mutation);
        assert!(!policy.allow_file_write);
    }
}

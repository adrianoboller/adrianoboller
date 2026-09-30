use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OctopusBridgeConfig {
    pub executable: PathBuf,
    pub workspace_root: PathBuf,
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
}

impl OctopusBridgeConfig {
    pub fn from_env() -> Self {
        Self {
            executable: std::env::var_os("PHXCLAW_OCTOPUS_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("octopus-console")),
            workspace_root: std::env::var_os("PHXCLAW_OCTOPUS_WORKSPACE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".")),
            timeout_ms: 120_000,
            max_output_bytes: 8 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeResult {
    pub capability: String,
    pub command: String,
    pub args: Vec<String>,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub json: Option<Value>,
    pub elapsed_ms: u128,
}

#[derive(Debug, Error)]
pub enum OctopusBridgeError {
    #[error("unsupported Octopus capability: {0}")]
    UnsupportedCapability(String),
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
    #[error("path is outside the configured Octopus workspace: {0}")]
    PathOutsideWorkspace(String),
    #[error("Octopus executable not available: {0}")]
    ExecutableUnavailable(String),
    #[error("Octopus process timed out after {0}ms")]
    Timeout(u64),
    #[error("Octopus process I/O failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("Octopus returned more than the configured output limit")]
    OutputTooLarge,
}

pub struct OctopusBridge {
    config: OctopusBridgeConfig,
}

impl OctopusBridge {
    pub fn new(config: OctopusBridgeConfig) -> Self {
        Self { config }
    }

    pub fn supported_capabilities() -> &'static [&'static str] {
        &[
            "octopus.repo.map",
            "octopus.repo.grep_ast",
            "octopus.code.analyze",
            "octopus.quality.evaluate",
            "octopus.security.scan",
            "octopus.migration.plan",
            "octopus.contract.verify",
            "octopus.shadow.check",
            "octopus.dependency.map",
            "octopus.knowledge.search",
            "octopus.toolchain.status",
        ]
    }

    pub fn health(&self) -> Result<Value, OctopusBridgeError> {
        let executable = resolve_executable(&self.config.executable).ok_or_else(|| {
            OctopusBridgeError::ExecutableUnavailable(self.config.executable.display().to_string())
        })?;
        Ok(json!({
            "healthy": true,
            "executable": executable,
            "workspace_root": self.config.workspace_root,
            "supported_capabilities": Self::supported_capabilities(),
        }))
    }

    pub fn invoke(
        &self,
        capability: &str,
        payload: &Value,
    ) -> Result<BridgeResult, OctopusBridgeError> {
        if !Self::supported_capabilities().contains(&capability) {
            return Err(OctopusBridgeError::UnsupportedCapability(
                capability.to_owned(),
            ));
        }
        let executable = resolve_executable(&self.config.executable).ok_or_else(|| {
            OctopusBridgeError::ExecutableUnavailable(self.config.executable.display().to_string())
        })?;
        let args = self.build_args(capability, payload)?;
        let started = Instant::now();
        let mut child = Command::new(&executable)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let timeout = Duration::from_millis(self.config.timeout_ms.max(1));
        loop {
            if child.try_wait()?.is_some() {
                let output = child.wait_with_output()?;
                if output.stdout.len() > self.config.max_output_bytes
                    || output.stderr.len() > self.config.max_output_bytes
                {
                    return Err(OctopusBridgeError::OutputTooLarge);
                }
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                let parsed = serde_json::from_str(stdout.trim()).ok();
                return Ok(BridgeResult {
                    capability: capability.to_owned(),
                    command: executable.display().to_string(),
                    args,
                    exit_code: output.status.code().unwrap_or(-1),
                    stdout,
                    stderr,
                    json: parsed,
                    elapsed_ms: started.elapsed().as_millis(),
                });
            }
            if started.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(OctopusBridgeError::Timeout(self.config.timeout_ms));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn build_args(
        &self,
        capability: &str,
        payload: &Value,
    ) -> Result<Vec<String>, OctopusBridgeError> {
        match capability {
            "octopus.repo.map" => {
                let workspace = self.safe_path(require_str(payload, "workspace")?)?;
                let token_budget = payload
                    .get("token_budget")
                    .and_then(Value::as_u64)
                    .unwrap_or(2048);
                Ok(vec![
                    "repo-map".into(),
                    workspace.display().to_string(),
                    "--token-budget".into(),
                    token_budget.to_string(),
                ])
            }
            "octopus.repo.grep_ast" => {
                let workspace = self.safe_path(require_str(payload, "workspace")?)?;
                let pattern = require_str(payload, "pattern")?;
                let context = payload.get("context").and_then(Value::as_u64).unwrap_or(3);
                let exts = payload
                    .get("exts")
                    .and_then(Value::as_str)
                    .unwrap_or("rs,py,c,go");
                Ok(vec![
                    "repo-grep".into(),
                    workspace.display().to_string(),
                    pattern.into(),
                    "--context".into(),
                    context.to_string(),
                    "--exts".into(),
                    exts.into(),
                ])
            }
            "octopus.code.analyze" => {
                let path = self.safe_path(require_str(payload, "path")?)?;
                let mut args = vec![
                    "code-analyze".into(),
                    "--path".into(),
                    path.display().to_string(),
                ];
                if let Some(language) = payload.get("language").and_then(Value::as_str) {
                    args.extend(["--language".into(), language.into()]);
                }
                Ok(args)
            }
            "octopus.quality.evaluate" => {
                let path = self.safe_path(require_str(payload, "path")?)?;
                Ok(vec!["quality-evaluate".into(), path.display().to_string()])
            }
            "octopus.security.scan" => {
                let workspace = self.safe_path(require_str(payload, "workspace")?)?;
                let output = self.safe_path(
                    payload
                        .get("output")
                        .and_then(Value::as_str)
                        .unwrap_or("logs/security_scan.json"),
                )?;
                Ok(vec![
                    "security-scan".into(),
                    workspace.display().to_string(),
                    output.display().to_string(),
                ])
            }
            "octopus.migration.plan" => {
                let workspace = self.safe_path(require_str(payload, "workspace")?)?;
                let output = self.safe_path(
                    payload
                        .get("output")
                        .and_then(Value::as_str)
                        .unwrap_or("logs/migration_plan.json"),
                )?;
                Ok(vec![
                    "migration-plan".into(),
                    workspace.display().to_string(),
                    output.display().to_string(),
                ])
            }
            "octopus.contract.verify" => {
                let py_file = self.safe_path(require_str(payload, "py_file")?)?;
                let rs_file = self.safe_path(require_str(payload, "rs_file")?)?;
                Ok(vec![
                    "contract-verify".into(),
                    py_file.display().to_string(),
                    rs_file.display().to_string(),
                ])
            }
            "octopus.shadow.check" => {
                let file = self.safe_path(require_str(payload, "file")?)?;
                Ok(vec!["shadow-check".into(), file.display().to_string()])
            }
            "octopus.dependency.map" => {
                let workspace = self.safe_path(require_str(payload, "workspace")?)?;
                Ok(vec!["deps-map".into(), workspace.display().to_string()])
            }
            "octopus.knowledge.search" => {
                let db = self.safe_path(require_str(payload, "db")?)?;
                let query = require_str(payload, "query")?;
                Ok(vec![
                    "knowledge-search".into(),
                    db.display().to_string(),
                    query.into(),
                ])
            }
            "octopus.toolchain.status" => Ok(vec!["toolchain-status".into()]),
            other => Err(OctopusBridgeError::UnsupportedCapability(other.into())),
        }
    }

    fn safe_path(&self, input: &str) -> Result<PathBuf, OctopusBridgeError> {
        let root = canonical_or_current(&self.config.workspace_root)?;
        let candidate = Path::new(input);
        let joined = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            root.join(candidate)
        };
        let resolved = canonical_or_lexical(&joined)?;
        if !resolved.starts_with(&root) {
            return Err(OctopusBridgeError::PathOutsideWorkspace(input.into()));
        }
        Ok(resolved)
    }
}

fn require_str<'a>(payload: &'a Value, key: &str) -> Result<&'a str, OctopusBridgeError> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| OctopusBridgeError::InvalidPayload(format!("missing string field {key}")))
}

fn canonical_or_current(path: &Path) -> Result<PathBuf, OctopusBridgeError> {
    if path.exists() {
        Ok(fs::canonicalize(path)?)
    } else {
        Err(OctopusBridgeError::PathOutsideWorkspace(
            path.display().to_string(),
        ))
    }
}

fn canonical_or_lexical(path: &Path) -> Result<PathBuf, OctopusBridgeError> {
    if path.exists() {
        return Ok(fs::canonicalize(path)?);
    }
    let parent = path
        .parent()
        .ok_or_else(|| OctopusBridgeError::InvalidPayload("path has no parent".into()))?;
    let parent = fs::canonicalize(parent)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| OctopusBridgeError::InvalidPayload("path has no basename".into()))?;
    Ok(parent.join(file_name))
}

fn resolve_executable(program: &Path) -> Option<PathBuf> {
    if program.components().count() > 1 {
        return program.is_file().then(|| program.to_path_buf());
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(program))
            .find(|candidate| candidate.is_file())
    })
}

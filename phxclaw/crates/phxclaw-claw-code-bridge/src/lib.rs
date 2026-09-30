#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawCodeBridgeConfig {
    pub executable: PathBuf,
    pub workspace_root: PathBuf,
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
}

impl ClawCodeBridgeConfig {
    pub fn from_env() -> Self {
        Self {
            executable: std::env::var_os("PHXCLAW_CLAW_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("claw")),
            workspace_root: std::env::var_os("PHXCLAW_CLAW_WORKSPACE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".")),
            timeout_ms: std::env::var("PHXCLAW_CLAW_TIMEOUT_MS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(120_000),
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
pub enum ClawCodeBridgeError {
    #[error("unsupported Claw Code capability: {0}")]
    UnsupportedCapability(String),
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
    #[error("workspace path is outside the configured root: {0}")]
    PathOutsideWorkspace(String),
    #[error("Claw Code executable not available: {0}")]
    ExecutableUnavailable(String),
    #[error("Claw Code process timed out after {0}ms")]
    Timeout(u64),
    #[error("Claw Code process I/O failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("Claw Code returned more than the configured output limit")]
    OutputTooLarge,
}

pub struct ClawCodeBridge {
    config: ClawCodeBridgeConfig,
}

impl ClawCodeBridge {
    pub fn new(config: ClawCodeBridgeConfig) -> Self {
        Self { config }
    }

    pub fn supported_capabilities() -> &'static [&'static str] {
        &[
            "claw.health",
            "claw.doctor",
            "claw.status",
            "claw.sandbox.status",
            "claw.mcp.status",
            "claw.skills.list",
            "claw.agents.list",
            "claw.prompt",
        ]
    }

    pub fn health(&self) -> Result<Value, ClawCodeBridgeError> {
        let executable = resolve_executable(&self.config.executable).ok_or_else(|| {
            ClawCodeBridgeError::ExecutableUnavailable(self.config.executable.display().to_string())
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
    ) -> Result<BridgeResult, ClawCodeBridgeError> {
        if !Self::supported_capabilities().contains(&capability) {
            return Err(ClawCodeBridgeError::UnsupportedCapability(
                capability.to_owned(),
            ));
        }
        let executable = resolve_executable(&self.config.executable).ok_or_else(|| {
            ClawCodeBridgeError::ExecutableUnavailable(self.config.executable.display().to_string())
        })?;
        let cwd = self.resolve_workspace(payload)?;
        let (args, stdin_text) = self.build_invocation(capability, payload)?;
        let started = Instant::now();
        let mut child = Command::new(&executable)
            .args(&args)
            .current_dir(&cwd)
            .stdin(if stdin_text.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(text) = stdin_text {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes())?;
                stdin.write_all(b"\n")?;
                stdin.flush()?;
            }
        }

        let timeout = Duration::from_millis(self.config.timeout_ms.max(1));
        loop {
            if child.try_wait()?.is_some() {
                let output = child.wait_with_output()?;
                if output.stdout.len() > self.config.max_output_bytes
                    || output.stderr.len() > self.config.max_output_bytes
                {
                    return Err(ClawCodeBridgeError::OutputTooLarge);
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
                return Err(ClawCodeBridgeError::Timeout(self.config.timeout_ms));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn resolve_workspace(&self, payload: &Value) -> Result<PathBuf, ClawCodeBridgeError> {
        let root = fs::canonicalize(&self.config.workspace_root).map_err(|_| {
            ClawCodeBridgeError::PathOutsideWorkspace(
                self.config.workspace_root.display().to_string(),
            )
        })?;
        let requested = payload
            .get("workspace")
            .and_then(Value::as_str)
            .unwrap_or(".");
        let candidate = Path::new(requested);
        let joined = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            root.join(candidate)
        };
        let resolved = fs::canonicalize(&joined)
            .map_err(|_| ClawCodeBridgeError::PathOutsideWorkspace(requested.to_owned()))?;
        if !resolved.starts_with(&root) {
            return Err(ClawCodeBridgeError::PathOutsideWorkspace(
                requested.to_owned(),
            ));
        }
        Ok(resolved)
    }

    fn build_invocation(
        &self,
        capability: &str,
        payload: &Value,
    ) -> Result<(Vec<String>, Option<String>), ClawCodeBridgeError> {
        let json_output = vec!["--output-format".to_owned(), "json".to_owned()];
        let invocation = match capability {
            "claw.health" => (
                vec!["version".into(), "--output-format".into(), "json".into()],
                None,
            ),
            "claw.doctor" => (
                vec!["doctor".into(), "--output-format".into(), "json".into()],
                None,
            ),
            "claw.status" => (
                vec!["status".into(), "--output-format".into(), "json".into()],
                None,
            ),
            "claw.sandbox.status" => (
                vec!["sandbox".into(), "--output-format".into(), "json".into()],
                None,
            ),
            "claw.mcp.status" => (
                vec!["mcp".into(), "--output-format".into(), "json".into()],
                None,
            ),
            "claw.skills.list" => (
                vec!["skills".into(), "--output-format".into(), "json".into()],
                None,
            ),
            "claw.agents.list" => (
                vec!["agents".into(), "--output-format".into(), "json".into()],
                None,
            ),
            "claw.prompt" => {
                let prompt = payload
                    .get("prompt")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| ClawCodeBridgeError::InvalidPayload("missing prompt".into()))?;
                let mut args = vec!["prompt".into()];
                args.extend(json_output);
                if let Some(model) = payload.get("model").and_then(Value::as_str) {
                    args.splice(0..0, ["--model".into(), model.into()]);
                }
                if let Some(mode) = payload.get("permission_mode").and_then(Value::as_str) {
                    match mode {
                        "read-only" | "workspace-write" | "danger-full-access" => {
                            args.splice(0..0, ["--permission-mode".into(), mode.into()]);
                        }
                        _ => {
                            return Err(ClawCodeBridgeError::InvalidPayload(
                                "invalid permission_mode".into(),
                            ));
                        }
                    }
                }
                (args, Some(prompt.to_owned()))
            }
            other => return Err(ClawCodeBridgeError::UnsupportedCapability(other.into())),
        };
        Ok(invocation)
    }
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

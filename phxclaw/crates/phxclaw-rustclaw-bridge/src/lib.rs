#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustClawBridgeConfig {
    pub executable: PathBuf,
    pub workspace_root: PathBuf,
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
    pub allow_prompt_in_argv: bool,
}

impl RustClawBridgeConfig {
    /// `pontes.rustclaw.*` do config.json (ambiente `PHXCLAW_RUSTCLAW_*` por cima).
    pub fn from_env() -> Self {
        use phxclaw_config_runtime::agente::carga::{
            booleano_do_processo, caminho_do_processo, inteiro_do_processo,
        };
        Self {
            executable: caminho_do_processo("pontes.rustclaw.bin")
                .unwrap_or_else(|| PathBuf::from("rustclaw")),
            workspace_root: caminho_do_processo("pontes.rustclaw.workspace")
                .unwrap_or_else(|| PathBuf::from(".")),
            timeout_ms: inteiro_do_processo("pontes.rustclaw.timeout_ms")
                .and_then(|v| u64::try_from(v).ok())
                .unwrap_or(60_000),
            max_output_bytes: 8 * 1024 * 1024,
            allow_prompt_in_argv: booleano_do_processo("pontes.rustclaw.prompt_no_argv")
                .unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeResult {
    pub capability: String,
    pub command: String,
    pub args_redacted: Vec<String>,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub json: Option<Value>,
    pub elapsed_ms: u128,
}

#[derive(Debug, Error)]
pub enum RustClawBridgeError {
    #[error("unsupported RustClaw capability: {0}")]
    UnsupportedCapability(String),
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
    #[error(
        "RustClaw prompt would be exposed in process argv; set PHXCLAW_RUSTCLAW_ALLOW_PROMPT_ARGV=true only after explicit approval"
    )]
    PromptArgvDenied,
    #[error("workspace path is outside the configured root: {0}")]
    PathOutsideWorkspace(String),
    #[error("RustClaw executable not available: {0}")]
    ExecutableUnavailable(String),
    #[error("RustClaw process timed out after {0}ms")]
    Timeout(u64),
    #[error("RustClaw process I/O failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("RustClaw returned more than the configured output limit")]
    OutputTooLarge,
}

pub struct RustClawBridge {
    config: RustClawBridgeConfig,
}

impl RustClawBridge {
    pub fn new(config: RustClawBridgeConfig) -> Self {
        Self { config }
    }

    pub fn supported_capabilities() -> &'static [&'static str] {
        &[
            "rustclaw.health",
            "rustclaw.status",
            "rustclaw.github.scan",
            "rustclaw.agent.prompt",
        ]
    }

    pub fn health(&self) -> Value {
        let resolved = resolve_executable(&self.config.executable);
        json!({
            "healthy": resolved.is_some(),
            "executable": resolved,
            "workspace_root": self.config.workspace_root,
            "supported_capabilities": Self::supported_capabilities(),
            "prompt_in_argv_enabled": self.config.allow_prompt_in_argv,
            "license_status": "unverified_in_user_archive",
        })
    }

    pub fn invoke(
        &self,
        capability: &str,
        payload: &Value,
    ) -> Result<BridgeResult, RustClawBridgeError> {
        if !Self::supported_capabilities().contains(&capability) {
            return Err(RustClawBridgeError::UnsupportedCapability(
                capability.to_owned(),
            ));
        }
        let executable = resolve_executable(&self.config.executable).ok_or_else(|| {
            RustClawBridgeError::ExecutableUnavailable(self.config.executable.display().to_string())
        })?;
        let cwd = self.resolve_workspace(payload)?;
        let (args, args_redacted) = self.build_args(capability, payload)?;
        let started = Instant::now();
        let mut child = Command::new(&executable)
            .args(&args)
            .current_dir(&cwd)
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
                    return Err(RustClawBridgeError::OutputTooLarge);
                }
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                let parsed = serde_json::from_str(stdout.trim()).ok();
                return Ok(BridgeResult {
                    capability: capability.to_owned(),
                    command: executable.display().to_string(),
                    args_redacted,
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
                return Err(RustClawBridgeError::Timeout(self.config.timeout_ms));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn resolve_workspace(&self, payload: &Value) -> Result<PathBuf, RustClawBridgeError> {
        let root = fs::canonicalize(&self.config.workspace_root).map_err(|_| {
            RustClawBridgeError::PathOutsideWorkspace(
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
            .map_err(|_| RustClawBridgeError::PathOutsideWorkspace(requested.to_owned()))?;
        if !resolved.starts_with(&root) {
            return Err(RustClawBridgeError::PathOutsideWorkspace(
                requested.to_owned(),
            ));
        }
        Ok(resolved)
    }

    fn build_args(
        &self,
        capability: &str,
        payload: &Value,
    ) -> Result<(Vec<String>, Vec<String>), RustClawBridgeError> {
        match capability {
            "rustclaw.health" => Ok((vec!["health".into()], vec!["health".into()])),
            "rustclaw.status" => Ok((vec!["status".into()], vec!["status".into()])),
            "rustclaw.github.scan" => Ok((
                vec!["github".into(), "scan".into()],
                vec!["github".into(), "scan".into()],
            )),
            "rustclaw.agent.prompt" => {
                if !self.config.allow_prompt_in_argv {
                    return Err(RustClawBridgeError::PromptArgvDenied);
                }
                let prompt = payload
                    .get("prompt")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| RustClawBridgeError::InvalidPayload("missing prompt".into()))?;
                Ok((
                    vec![
                        "agent".into(),
                        prompt.to_owned(),
                        "--stream".into(),
                        "false".into(),
                    ],
                    vec![
                        "agent".into(),
                        "[REDACTED]".into(),
                        "--stream".into(),
                        "false".into(),
                    ],
                ))
            }
            other => Err(RustClawBridgeError::UnsupportedCapability(other.into())),
        }
    }
}

fn resolve_executable(program: &Path) -> Option<PathBuf> {
    if program.components().count() > 1 {
        return program.is_file().then(|| program.to_path_buf());
    }
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|dir| dir.join(program))
            .find(|candidate| candidate.is_file())
    })
}

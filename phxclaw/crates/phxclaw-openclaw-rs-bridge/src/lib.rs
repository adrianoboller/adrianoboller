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
pub struct OpenClawRsBridgeConfig {
    pub executable: PathBuf,
    pub workspace_root: PathBuf,
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
}

impl OpenClawRsBridgeConfig {
    pub fn from_env() -> Self {
        Self {
            executable: env::var_os("PHXCLAW_OPENCLAW_RS_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("openclaw")),
            workspace_root: env::var_os("PHXCLAW_OPENCLAW_RS_WORKSPACE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".")),
            timeout_ms: env::var("PHXCLAW_OPENCLAW_RS_TIMEOUT_MS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(60_000),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpstreamFeatureMap {
    pub gateway: bool,
    pub channels: bool,
    pub providers: bool,
    pub agents: bool,
    pub plugins: bool,
    pub ipc: bool,
    pub encrypted_credentials: bool,
    pub append_only_events: bool,
}

impl Default for UpstreamFeatureMap {
    fn default() -> Self {
        Self {
            gateway: true,
            channels: true,
            providers: true,
            agents: true,
            plugins: true,
            ipc: true,
            encrypted_credentials: true,
            append_only_events: true,
        }
    }
}

#[derive(Debug, Error)]
pub enum OpenClawRsBridgeError {
    #[error("unsupported openclaw-rs capability: {0}")]
    UnsupportedCapability(String),
    #[error("workspace path is outside the configured root: {0}")]
    PathOutsideWorkspace(String),
    #[error("openclaw-rs executable not available: {0}")]
    ExecutableUnavailable(String),
    #[error("openclaw-rs process timed out after {0}ms")]
    Timeout(u64),
    #[error("openclaw-rs process I/O failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("openclaw-rs returned more than the configured output limit")]
    OutputTooLarge,
}

pub struct OpenClawRsBridge {
    config: OpenClawRsBridgeConfig,
}

impl OpenClawRsBridge {
    pub fn new(config: OpenClawRsBridgeConfig) -> Self {
        Self { config }
    }

    pub fn supported_capabilities() -> &'static [&'static str] {
        &[
            "openclaw_rs.health",
            "openclaw_rs.doctor",
            "openclaw_rs.status",
            "openclaw_rs.gateway.status",
            "openclaw_rs.channels.list",
            "openclaw_rs.channels.probe",
            "openclaw_rs.config.show",
            "openclaw_rs.config.validate",
        ]
    }

    pub fn health(&self) -> Value {
        let resolved = resolve_executable(&self.config.executable);
        json!({
            "healthy": resolved.is_some(),
            "executable": resolved,
            "workspace_root": self.config.workspace_root,
            "supported_capabilities": Self::supported_capabilities(),
            "upstream_features": UpstreamFeatureMap::default(),
        })
    }

    pub fn invoke(
        &self,
        capability: &str,
        payload: &Value,
    ) -> Result<BridgeResult, OpenClawRsBridgeError> {
        if !Self::supported_capabilities().contains(&capability) {
            return Err(OpenClawRsBridgeError::UnsupportedCapability(
                capability.to_owned(),
            ));
        }
        let executable = resolve_executable(&self.config.executable).ok_or_else(|| {
            OpenClawRsBridgeError::ExecutableUnavailable(
                self.config.executable.display().to_string(),
            )
        })?;
        let cwd = self.resolve_workspace(payload)?;
        let args = build_args(capability);
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
                    return Err(OpenClawRsBridgeError::OutputTooLarge);
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
                return Err(OpenClawRsBridgeError::Timeout(self.config.timeout_ms));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn resolve_workspace(&self, payload: &Value) -> Result<PathBuf, OpenClawRsBridgeError> {
        let root = fs::canonicalize(&self.config.workspace_root).map_err(|_| {
            OpenClawRsBridgeError::PathOutsideWorkspace(
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
            .map_err(|_| OpenClawRsBridgeError::PathOutsideWorkspace(requested.to_owned()))?;
        if !resolved.starts_with(&root) {
            return Err(OpenClawRsBridgeError::PathOutsideWorkspace(
                requested.to_owned(),
            ));
        }
        Ok(resolved)
    }
}

fn build_args(capability: &str) -> Vec<String> {
    match capability {
        "openclaw_rs.health" => vec!["--version".into()],
        "openclaw_rs.doctor" => vec!["doctor".into()],
        "openclaw_rs.status" => vec!["status".into(), "--all".into()],
        "openclaw_rs.gateway.status" => vec!["gateway".into(), "status".into()],
        "openclaw_rs.channels.list" => vec!["channels".into(), "--list".into()],
        "openclaw_rs.channels.probe" => vec!["channels".into(), "--probe".into()],
        "openclaw_rs.config.show" => vec!["config".into(), "show".into()],
        "openclaw_rs.config.validate" => vec!["config".into(), "validate".into()],
        _ => Vec::new(),
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

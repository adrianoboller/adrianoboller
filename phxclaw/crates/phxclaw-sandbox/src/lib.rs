use phxclaw_types::{PluginManifest, SandboxNetworkMode};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub timeout: Duration,
}

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("sandbox backend not available: {0}")]
    BackendUnavailable(String),
    #[error("unsupported sandbox policy: {0}")]
    UnsupportedPolicy(String),
    #[error("invalid sandbox path: {0}")]
    InvalidPath(String),
    #[error("sandbox I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("sandbox process timed out after {0:?}")]
    Timeout(Duration),
}

pub trait SandboxBackend {
    fn name(&self) -> &'static str;
    fn probe(&self) -> Result<(), SandboxError>;
    fn plan(&self, manifest: &PluginManifest) -> Result<SandboxPlan, SandboxError>;
    fn execute(&self, manifest: &PluginManifest) -> Result<ExitStatus, SandboxError>;
}

#[derive(Debug, Clone)]
pub struct BwrapSandbox {
    package_root: PathBuf,
    bwrap_path: PathBuf,
}

impl BwrapSandbox {
    pub fn new(package_root: impl Into<PathBuf>) -> Self {
        let bwrap_path = find_in_path("bwrap").unwrap_or_else(|| PathBuf::from("bwrap"));
        Self {
            package_root: package_root.into(),
            bwrap_path,
        }
    }

    pub fn with_binary(package_root: impl Into<PathBuf>, bwrap_path: impl Into<PathBuf>) -> Self {
        Self {
            package_root: package_root.into(),
            bwrap_path: bwrap_path.into(),
        }
    }
}

impl SandboxBackend for BwrapSandbox {
    fn name(&self) -> &'static str {
        "bubblewrap"
    }

    fn probe(&self) -> Result<(), SandboxError> {
        if self.bwrap_path.is_file() || find_in_path(self.bwrap_path.to_string_lossy().as_ref()).is_some() {
            Ok(())
        } else {
            Err(SandboxError::BackendUnavailable(
                "bubblewrap (bwrap) was not found; execution remains fail-closed".into(),
            ))
        }
    }

    fn plan(&self, manifest: &PluginManifest) -> Result<SandboxPlan, SandboxError> {
        if manifest.entrypoint.kind != "process" {
            return Err(SandboxError::UnsupportedPolicy(format!(
                "bubblewrap backend only executes process entrypoints, got {}",
                manifest.entrypoint.kind
            )));
        }
        if manifest.sandbox.network == SandboxNetworkMode::Allowlist {
            return Err(SandboxError::UnsupportedPolicy(
                "network allowlists require the future network-proxy backend; refusing broad network access".into(),
            ));
        }

        let package_root = fs::canonicalize(&self.package_root)?;
        let entrypoint = fs::canonicalize(package_root.join(&manifest.entrypoint.value))?;
        if !entrypoint.starts_with(&package_root) {
            return Err(SandboxError::InvalidPath(
                "entrypoint escaped the package root".into(),
            ));
        }
        let relative_entrypoint = entrypoint
            .strip_prefix(&package_root)
            .map_err(|_| SandboxError::InvalidPath("entrypoint is not inside package root".into()))?;
        let guest_entrypoint = format!("/phxclaw/{}", relative_entrypoint.display());

        let mut args = vec![
            "--die-with-parent".into(),
            "--new-session".into(),
            "--unshare-all".into(),
            "--unshare-net".into(),
            "--proc".into(),
            "/proc".into(),
            "--dev".into(),
            "/dev".into(),
            "--tmpfs".into(),
            "/tmp".into(),
            "--ro-bind".into(),
            package_root.display().to_string(),
            "/phxclaw".into(),
            "--chdir".into(),
            "/phxclaw".into(),
            "--clearenv".into(),
        ];

        for system_path in ["/usr", "/bin", "/lib", "/lib64"] {
            if Path::new(system_path).exists() {
                args.extend([
                    "--ro-bind".into(),
                    system_path.into(),
                    system_path.into(),
                ]);
            }
        }

        for variable in &manifest.sandbox.environment_allowlist {
            if let Ok(value) = env::var(variable) {
                args.extend(["--setenv".into(), variable.clone(), value]);
            }
        }

        for relative in &manifest.sandbox.write_paths {
            let rel = Path::new(relative);
            if rel.is_absolute() || rel.components().any(|component| matches!(component, std::path::Component::ParentDir)) {
                return Err(SandboxError::InvalidPath(format!(
                    "writable path must be a package-relative path without '..': {relative}"
                )));
            }
            let host_path = package_root.join(rel);
            fs::create_dir_all(&host_path)?;
            let host_path = fs::canonicalize(&host_path)?;
            if !host_path.starts_with(&package_root) {
                return Err(SandboxError::InvalidPath(format!(
                    "writable path escaped package root: {relative}"
                )));
            }
            let guest_path = format!("/phxclaw/{}", rel.display());
            args.extend([
                "--bind".into(),
                host_path.display().to_string(),
                guest_path,
            ]);
        }

        args.push("--".into());
        args.push(guest_entrypoint);

        Ok(SandboxPlan {
            program: self.bwrap_path.clone(),
            args,
            timeout: Duration::from_millis(manifest.sandbox.timeout_ms),
        })
    }

    fn execute(&self, manifest: &PluginManifest) -> Result<ExitStatus, SandboxError> {
        self.probe()?;
        let plan = self.plan(manifest)?;
        let mut child = Command::new(&plan.program).args(&plan.args).spawn()?;
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
            if started.elapsed() >= plan.timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(SandboxError::Timeout(plan.timeout));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    if program.contains(std::path::MAIN_SEPARATOR) {
        let path = PathBuf::from(program);
        return path.is_file().then_some(path);
    }
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|path| path.join(program))
            .find(|path| path.is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_network_plan_is_fail_closed() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let input = include_str!("../../../plugins/builtin/manifests/morpheus.agent.plugin.json");
        let manifest: PluginManifest = serde_json::from_str(input).unwrap();
        let sandbox = BwrapSandbox::with_binary(&root, "/usr/bin/bwrap");
        let plan = sandbox.plan(&manifest).unwrap();
        assert!(plan.args.iter().any(|arg| arg == "--unshare-net"));
        assert!(plan.args.iter().any(|arg| arg == "--clearenv"));
    }
}

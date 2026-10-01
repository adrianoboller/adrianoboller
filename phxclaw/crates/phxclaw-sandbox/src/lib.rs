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
    /// Ambiente EXATO do processo bwrap (que o repassa ao plugin). Fica fora de `args`
    /// de proposito: argv e publico em /proc/<pid>/cmdline, e segredo la vaza para
    /// qualquer usuario local.
    pub env: Vec<(String, String)>,
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
        if self.bwrap_path.is_file()
            || find_in_path(self.bwrap_path.to_string_lossy().as_ref()).is_some()
        {
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
        let relative_entrypoint = entrypoint.strip_prefix(&package_root).map_err(|_| {
            SandboxError::InvalidPath("entrypoint is not inside package root".into())
        })?;
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
        ];

        for system_path in ["/usr", "/bin", "/lib", "/lib64"] {
            if Path::new(system_path).exists() {
                args.extend(["--ro-bind".into(), system_path.into(), system_path.into()]);
            }
        }

        // Nada de --setenv VAR VALOR: o valor iria para o argv. O bwrap herda o ambiente
        // do processo que o lanca, e o execute() limpa esse ambiente (env_clear) antes de
        // por so a lista permitida -- o mesmo efeito do --clearenv, sem o vazamento.
        let child_env: Vec<(String, String)> = manifest
            .sandbox
            .environment_allowlist
            .iter()
            .filter_map(|variable| {
                env::var(variable)
                    .ok()
                    .map(|value| (variable.clone(), value))
            })
            .collect();

        for relative in &manifest.sandbox.write_paths {
            let rel = Path::new(relative);
            if rel.is_absolute()
                || rel
                    .components()
                    .any(|component| matches!(component, std::path::Component::ParentDir))
            {
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
            args.extend(["--bind".into(), host_path.display().to_string(), guest_path]);
        }

        args.push("--".into());
        args.push(guest_entrypoint);

        Ok(SandboxPlan {
            program: self.bwrap_path.clone(),
            args,
            env: child_env,
            timeout: Duration::from_millis(manifest.sandbox.timeout_ms),
        })
    }

    fn execute(&self, manifest: &PluginManifest) -> Result<ExitStatus, SandboxError> {
        self.probe()?;
        let plan = self.plan(manifest)?;
        let mut child = Command::new(&plan.program)
            .args(&plan.args)
            .env_clear()
            .envs(plan.env.iter().map(|(k, v)| (k, v)))
            .spawn()?;
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

/// Comando avulso do agente dentro do bwrap, numa pasta de trabalho da tarefa que persiste
/// entre chamadas (o equivalente do "computador" do agente). Mesmo isolamento dos plugins:
/// sistema so leitura, sem rede por padrao, ambiente limpo, tudo fora de /work invisivel.
#[derive(Debug, Clone)]
pub struct WorkdirCommand {
    pub workdir: PathBuf,
    /// Linha de shell executada por /bin/sh -c dentro do sandbox.
    pub script: String,
    pub timeout: Duration,
    /// Rede so quando a politica da tarefa conceder; o padrao e desligada.
    pub network: bool,
    /// Teto de bytes guardados de stdout e de stderr, cada um.
    pub max_output_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkdirOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
}

/// O processo do sandbox, montado num lugar so: o `shell` sincrono e o de segundo plano
/// saem daqui, para a lista de binds e o ambiente limpo nunca divergirem entre os dois.
/// Quem chama escolhe so para onde vao stdin/stdout/stderr.
pub fn workdir_sandbox_command(
    bwrap: &Path,
    cmd: &WorkdirCommand,
) -> Result<Command, SandboxError> {
    workdir_sandbox_command_com(bwrap, cmd, &SandboxExtras::default())
}

/// O que uma ferramenta especializada acrescenta ao MESMO sandbox do shell: caminhos do
/// hospedeiro montados so leitura e variaveis de ambiente fixas. Existe para que o
/// `rust_project` monte o toolchain sem copiar a lista de binds -- uma segunda montagem
/// divergiria da primeira no dia em que alguem endurecesse so uma delas.
#[derive(Debug, Clone, Default)]
pub struct SandboxExtras {
    /// (caminho no hospedeiro, caminho dentro do sandbox), so leitura. Entram DEPOIS do
    /// `/tmp` em tmpfs, entao podem cair dentro dele.
    pub ro_binds: Vec<(PathBuf, String)>,
    /// Aplicadas depois do ambiente limpo; repetir `PATH` aqui o substitui.
    pub env: Vec<(String, String)>,
}

pub fn workdir_sandbox_command_com(
    bwrap: &Path,
    cmd: &WorkdirCommand,
    extras: &SandboxExtras,
) -> Result<Command, SandboxError> {
    fs::create_dir_all(&cmd.workdir)?;
    let workdir = fs::canonicalize(&cmd.workdir)?;
    let mut args: Vec<String> = vec![
        "--die-with-parent".into(),
        "--new-session".into(),
        "--unshare-all".into(),
    ];
    if cmd.network {
        args.push("--share-net".into());
        // resolvedor de nomes: sem isto a rede concedida nao resolve nada
        for f in [
            "/etc/resolv.conf",
            "/etc/hosts",
            "/etc/ssl",
            "/etc/ca-certificates",
        ] {
            if Path::new(f).exists() {
                args.extend(["--ro-bind".into(), f.into(), f.into()]);
            }
        }
    }
    for system_path in [
        "/usr",
        "/bin",
        "/lib",
        "/lib64",
        "/sbin",
        "/etc/alternatives",
    ] {
        if Path::new(system_path).exists() {
            args.extend(["--ro-bind".into(), system_path.into(), system_path.into()]);
        }
    }
    args.extend([
        "--proc".into(),
        "/proc".into(),
        "--dev".into(),
        "/dev".into(),
        "--tmpfs".into(),
        "/tmp".into(),
        "--bind".into(),
        workdir.display().to_string(),
        "/work".into(),
        "--chdir".into(),
        "/work".into(),
    ]);
    for (host, guest) in &extras.ro_binds {
        args.extend([
            "--ro-bind".into(),
            host.display().to_string(),
            guest.clone(),
        ]);
    }
    args.extend([
        "--".into(),
        "/bin/sh".into(),
        "-c".into(),
        cmd.script.clone(),
    ]);
    // Ambiente limpo e fixo: nada do processo pai (tokens, chaves) entra no sandbox.
    let mut c = Command::new(bwrap);
    c.args(&args)
        .env_clear()
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .env("HOME", "/work")
        .env("LANG", "C.UTF-8");
    for (k, v) in &extras.env {
        c.env(k, v);
    }
    Ok(c)
}

pub fn run_in_workdir(bwrap: &Path, cmd: &WorkdirCommand) -> Result<WorkdirOutput, SandboxError> {
    run_in_workdir_com(bwrap, cmd, &SandboxExtras::default())
}

pub fn run_in_workdir_com(
    bwrap: &Path,
    cmd: &WorkdirCommand,
    extras: &SandboxExtras,
) -> Result<WorkdirOutput, SandboxError> {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = workdir_sandbox_command_com(bwrap, cmd, extras)?
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // stdout e stderr lidos em threads enquanto o processo roda: ler so no fim trava o
    // filho quando ele enche o pipe (64 KiB) e vira timeout falso.
    let limite = cmd.max_output_bytes;
    let leitor = |mut r: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut guardado = Vec::new();
            let mut buf = [0u8; 8192];
            let mut total = 0usize;
            while let Ok(n) = r.read(&mut buf) {
                if n == 0 {
                    break;
                }
                total += n;
                let cabe = limite.saturating_sub(guardado.len()).min(n);
                guardado.extend_from_slice(&buf[..cabe]);
            }
            (guardado, total > limite)
        })
    };
    let out = leitor(Box::new(child.stdout.take().expect("stdout piped")));
    let err = leitor(Box::new(child.stderr.take().expect("stderr piped")));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= cmd.timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(SandboxError::Timeout(cmd.timeout));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let (o, ot) = out.join().unwrap_or_default();
    let (e, et) = err.join().unwrap_or_default();
    Ok(WorkdirOutput {
        exit_code: status.code(),
        stdout: String::from_utf8_lossy(&o).into_owned(),
        stderr: String::from_utf8_lossy(&e).into_owned(),
        truncated: ot || et,
    })
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
        // O ambiente do plugin e so a lista permitida (o morpheus nao pede nenhuma).
        assert!(plan.env.is_empty(), "{:?}", plan.env);
    }

    #[test]
    fn segredo_permitido_nao_vai_para_o_argv() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let input = include_str!("../../../plugins/builtin/manifests/morpheus.agent.plugin.json");
        let mut manifest: PluginManifest = serde_json::from_str(input).unwrap();
        // PATH existe em qualquer ambiente de teste e serve de segredo de mentira.
        let valor = env::var("PATH").unwrap();
        manifest.sandbox.environment_allowlist = vec!["PATH".into()];
        let plan = BwrapSandbox::with_binary(&root, "/usr/bin/bwrap")
            .plan(&manifest)
            .unwrap();
        assert!(
            plan.args.iter().all(|arg| arg != &valor),
            "valor de variavel permitida apareceu no argv"
        );
        assert_eq!(plan.env, vec![("PATH".to_string(), valor)]);
    }

    fn cmd(dir: &Path, script: &str) -> WorkdirCommand {
        WorkdirCommand {
            workdir: dir.to_path_buf(),
            script: script.into(),
            timeout: Duration::from_secs(10),
            network: false,
            max_output_bytes: 1024,
        }
    }

    fn bwrap() -> Option<PathBuf> {
        find_in_path("bwrap")
    }

    #[test]
    fn workdir_persiste_entre_chamadas_e_nada_fora_dele_aparece() {
        let Some(b) = bwrap() else {
            eprintln!("bwrap ausente: teste pulado");
            return;
        };
        let dir = env::temp_dir().join(format!("phx-wd-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let r = run_in_workdir(&b, &cmd(&dir, "echo 42 > dado.txt && pwd")).unwrap();
        assert_eq!(r.exit_code, Some(0), "{r:?}");
        assert_eq!(r.stdout.trim(), "/work");
        let r = run_in_workdir(&b, &cmd(&dir, "cat dado.txt; ls /home 2>&1 | head -1")).unwrap();
        assert!(r.stdout.starts_with("42"), "{r:?}");
        assert!(!r.stdout.contains("user"), "home do host visivel: {r:?}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn sem_rede_e_sem_segredo_do_pai() {
        let Some(b) = bwrap() else { return };
        let dir = env::temp_dir().join(format!("phx-wd2-{}", std::process::id()));
        // SAFETY de teste: variavel so deste processo de teste.
        let r = run_in_workdir(&b, &cmd(&dir, "env; cat /proc/net/dev | wc -l")).unwrap();
        assert!(
            !r.stdout.contains("CARGO"),
            "ambiente do pai vazou: {}",
            r.stdout
        );
        // so a interface lo existe (2 linhas de cabecalho + lo)
        assert!(r.stdout.trim().ends_with('3'), "rede visivel: {}", r.stdout);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn saida_grande_nao_trava_e_e_truncada() {
        let Some(b) = bwrap() else { return };
        let dir = env::temp_dir().join(format!("phx-wd3-{}", std::process::id()));
        // 200 KiB em stderr: com leitura so no fim o filho travaria no pipe
        let r = run_in_workdir(
            &b,
            &cmd(&dir, "head -c 204800 /dev/zero | tr '\\0' x >&2; echo fim"),
        )
        .unwrap();
        assert_eq!(r.stdout.trim(), "fim");
        assert!(r.truncated);
        assert_eq!(r.stderr.len(), 1024);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn timeout_mata_o_processo() {
        let Some(b) = bwrap() else { return };
        let dir = env::temp_dir().join(format!("phx-wd4-{}", std::process::id()));
        let mut c = cmd(&dir, "sleep 30");
        c.timeout = Duration::from_millis(300);
        assert!(matches!(
            run_in_workdir(&b, &c),
            Err(SandboxError::Timeout(_))
        ));
        let _ = fs::remove_dir_all(dir);
    }
}

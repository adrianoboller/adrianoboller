use crate::cdp::{BlockedRequest, Connection};
use crate::error::{BrowserError, Result, timeout_err};
use crate::page::Page;
use crate::policy::BrowserPolicy;
use serde_json::json;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;

/// Variavel que aponta o executavel do Chromium, acima dos caminhos padrao.
pub const CHROMIUM_ENV: &str = "PHXCLAW_CHROMIUM";

const DEFAULT_PATHS: &[&str] = &[
    "/opt/pw-browsers/chromium_headless_shell-1194/chrome-linux/headless_shell",
    "/opt/pw-browsers/chromium-1194/chrome-linux/chrome",
];

#[derive(Debug, Clone)]
pub struct LaunchOptions {
    /// Executavel explicito; `None` consulta `PHXCLAW_CHROMIUM` e os padroes.
    pub executable: Option<PathBuf>,
    pub policy: BrowserPolicy,
    pub launch_timeout: Duration,
    pub command_timeout: Duration,
    pub navigation_timeout: Duration,
    pub window_size: (u32, u32),
    pub extra_args: Vec<String>,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            executable: None,
            policy: BrowserPolicy::default(),
            launch_timeout: Duration::from_secs(20),
            command_timeout: Duration::from_secs(15),
            navigation_timeout: Duration::from_secs(30),
            window_size: (1280, 800),
            extra_args: Vec::new(),
        }
    }
}

impl LaunchOptions {
    pub fn with_policy(policy: BrowserPolicy) -> Self {
        Self {
            policy,
            ..Self::default()
        }
    }
}

/// Acha o Chromium sem lancar nada; os testes usam para pular com motivo.
pub fn find_chromium() -> Option<PathBuf> {
    resolve_executable(None).ok()
}

fn resolve_executable(explicit: Option<&Path>) -> Result<PathBuf> {
    let mut tentados = Vec::new();
    let mut candidatos: Vec<PathBuf> = Vec::new();
    if let Some(p) = explicit {
        candidatos.push(p.to_path_buf());
    } else {
        if let Some(p) = std::env::var_os(CHROMIUM_ENV).filter(|p| !p.is_empty()) {
            candidatos.push(PathBuf::from(p));
        }
        candidatos.extend(DEFAULT_PATHS.iter().map(PathBuf::from));
    }
    for c in candidatos {
        if c.is_file() {
            return Ok(c);
        }
        tentados.push(c.display().to_string());
    }
    Err(BrowserError::ExecutableNotFound { tried: tentados })
}

/// Le o UID efetivo em /proc para decidir o `--no-sandbox`. Sem libc e sem
/// `unsafe`: o /proc ja responde, e fora do Linux assume-se nao-root.
fn running_as_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(2).map(|u| u == "0"))
        })
        .unwrap_or(false)
}

/// Dono do processo e do perfil. Existe separado do `Browser` para que um
/// erro no meio do `launch` ja limpe tudo sem codigo de limpeza duplicado.
struct ProcessGuard {
    child: Option<Child>,
    profile_dir: PathBuf,
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        // Os filhos do Chromium (zygote, renderizador) morrem logo depois do
        // pai, mas ainda podem estar gravando no perfil; algumas tentativas
        // curtas evitam deixar o diretorio para tras.
        for _ in 0..20 {
            match std::fs::remove_dir_all(&self.profile_dir) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    std::thread::sleep(Duration::from_millis(25))
                }
                _ => break,
            }
        }
    }
}

/// Chromium headless dirigido pelo CDP. Mata o processo e apaga o perfil no
/// `Drop`; `close` faz o mesmo de forma ordeira.
pub struct Browser {
    conn: Arc<Connection>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
    navigation_timeout: Duration,
    guard: ProcessGuard,
}

impl Browser {
    pub async fn launch(opts: LaunchOptions) -> Result<Self> {
        let exe = resolve_executable(opts.executable.as_deref())?;
        let profile_dir =
            std::env::temp_dir().join(format!("phxclaw-browser-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&profile_dir)?;

        let mut cmd = Command::new(&exe);
        cmd.arg("--headless=new")
            .arg("--remote-debugging-port=0")
            .arg(format!("--user-data-dir={}", profile_dir.display()))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            // Trafego proprio do Chromium (atualizacao, sincronia) nao passa
            // pela interceptacao da pagina; desligado, o que sai e so o que
            // o agente pediu.
            .arg("--disable-background-networking")
            .arg("--disable-component-update")
            .arg("--disable-sync")
            .arg("--disable-default-apps")
            .arg("--disable-dev-shm-usage")
            .arg("--mute-audio")
            .arg("--hide-scrollbars")
            .arg(format!(
                "--window-size={},{}",
                opts.window_size.0, opts.window_size.1
            ));
        if running_as_root() {
            // O sandbox do Chromium recusa subir como root (conteiner, CI) e
            // o processo morre na largada. So nesse caso ele sai; fora dele o
            // sandbox e a segunda linha de defesa e fica ligado.
            cmd.arg("--no-sandbox");
        }
        cmd.args(&opts.extra_args)
            .arg("about:blank")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| BrowserError::Launch(format!("{}: {e}", exe.display())))?;
        let stderr = child.stderr.take();
        let mut guard = ProcessGuard {
            child: Some(child),
            profile_dir: profile_dir.clone(),
        };

        let (tx, rx) = oneshot::channel::<std::result::Result<String, String>>();
        if let Some(stderr) = stderr {
            // Thread e nao tarefa: a leitura e bloqueante e segue ate o fim
            // do processo, porque pipe de stderr cheio trava o Chromium.
            std::thread::spawn(move || drain_stderr(stderr, tx));
        }
        let ws_url = wait_ws_url(rx, &profile_dir, opts.launch_timeout, &mut guard).await?;

        let (ws, _) = tokio::time::timeout(
            opts.command_timeout,
            tokio_tungstenite::connect_async(ws_url.as_str()),
        )
        .await
        .map_err(|_| timeout_err("conectar websocket", opts.command_timeout))?
        .map_err(|e| BrowserError::WebSocket(e.to_string()))?;

        let (conn, tasks) = Connection::start(ws, opts.policy, opts.command_timeout);
        let browser = Self {
            conn,
            tasks,
            navigation_timeout: opts.navigation_timeout,
            guard,
        };
        browser.conn.enable_auto_attach_root().await?;
        Ok(browser)
    }

    /// Abre aba nova. Ela nasce pausada e so roda depois de a interceptacao
    /// de rede estar ligada nela.
    pub async fn new_page(&self) -> Result<Page> {
        let r = self
            .conn
            .call("Target.createTarget", json!({ "url": "about:blank" }), None)
            .await?;
        let target_id = r
            .get("targetId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrowserError::Protocol("createTarget sem targetId".into()))?
            .to_string();
        let session = self.conn.wait_page_session(&target_id).await?;
        Page::attach(
            self.conn.clone(),
            target_id,
            session,
            self.navigation_timeout,
        )
        .await
    }

    pub fn policy(&self) -> &BrowserPolicy {
        &self.conn.policy
    }

    /// Tudo o que a politica barrou desde o lancamento.
    pub fn blocked_requests(&self) -> Vec<BlockedRequest> {
        self.conn.blocked()
    }

    pub fn profile_dir(&self) -> &Path {
        &self.guard.profile_dir
    }

    pub fn pid(&self) -> Option<u32> {
        self.guard.child.as_ref().map(Child::id)
    }

    /// Fecha pelo protocolo e espera o processo sair; o `Drop` ainda limpa o
    /// perfil e mata o processo se ele nao saiu no prazo.
    pub async fn close(mut self) -> Result<()> {
        let _ = self
            .conn
            .call_timeout("Browser.close", json!({}), None, Duration::from_secs(3))
            .await;
        if let Some(c) = self.guard.child.as_mut() {
            for _ in 0..100 {
                if matches!(c.try_wait(), Ok(Some(_))) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
        }
        Ok(())
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}

fn drain_stderr(
    stderr: std::process::ChildStderr,
    tx: oneshot::Sender<std::result::Result<String, String>>,
) {
    let mut tx = Some(tx);
    let mut cauda: VecDeque<String> = VecDeque::new();
    for linha in BufReader::new(stderr).lines() {
        let Ok(linha) = linha else { break };
        if tx.is_none() {
            continue;
        }
        if linha.contains("DevTools listening on")
            && let Some(pos) = linha.find("ws://")
        {
            if let Some(t) = tx.take() {
                let _ = t.send(Ok(linha[pos..].trim().to_string()));
            }
            continue;
        }
        if cauda.len() == 20 {
            cauda.pop_front();
        }
        cauda.push_back(linha);
    }
    if let Some(t) = tx.take() {
        let _ = t.send(Err(Vec::from(cauda).join("\n")));
    }
}

/// Espera o endereco do DevTools pelo stderr ou, se a linha nao vier (alguns
/// empacotamentos redirecionam o stderr), pelo arquivo DevToolsActivePort.
async fn wait_ws_url(
    mut rx: oneshot::Receiver<std::result::Result<String, String>>,
    profile: &Path,
    prazo: Duration,
    guard: &mut ProcessGuard,
) -> Result<String> {
    let arquivo = profile.join("DevToolsActivePort");
    let espera = async {
        let mut stderr_aberto = true;
        loop {
            if stderr_aberto {
                tokio::select! {
                    r = &mut rx => match r {
                        Ok(Ok(url)) => return Ok(url),
                        Ok(Err(cauda)) => {
                            return Err(BrowserError::Launch(format!(
                                "o chromium saiu antes de abrir o DevTools: {cauda}"
                            )));
                        }
                        Err(_) => stderr_aberto = false,
                    },
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                }
            } else {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            if let Ok(s) = std::fs::read_to_string(&arquivo) {
                let mut l = s.lines();
                if let (Some(porta), Some(caminho)) = (l.next(), l.next()) {
                    return Ok(format!("ws://127.0.0.1:{}{}", porta.trim(), caminho.trim()));
                }
            }
            if let Some(c) = guard.child.as_mut()
                && let Ok(Some(status)) = c.try_wait()
            {
                return Err(BrowserError::Launch(format!(
                    "o chromium saiu na largada: {status}"
                )));
            }
        }
    };
    tokio::time::timeout(prazo, espera)
        .await
        .map_err(|_| timeout_err("lancar chromium", prazo))?
}

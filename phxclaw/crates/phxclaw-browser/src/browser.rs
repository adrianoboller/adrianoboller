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

/// Chave do executavel do Chromium (`PHXCLAW_CHROMIUM`), acima dos caminhos padrao.
pub const CHAVE_CHROMIUM: &str = "navegador.chromium";

/// A variavel de ambiente de `CHAVE_CHROMIUM`, para as mensagens ao operador.
pub fn variavel_do_chromium() -> &'static str {
    phxclaw_config_runtime::agente::carga::variavel(CHAVE_CHROMIUM)
}

const DEFAULT_PATHS: &[&str] = &[
    "/opt/pw-browsers/chromium_headless_shell-1194/chrome-linux/headless_shell",
    "/opt/pw-browsers/chromium-1194/chrome-linux/chrome",
];

/// Quem monta o processo do Chromium a partir do argv completo (`[exe, args...]`) e da
/// pasta do perfil: existe para o agente lancar o navegador DENTRO do bwrap do shell sem
/// esta crate conhecer o sandbox. O contrato: a pasta do perfil e a pasta de trabalho do
/// processo (quem envolve a monta como quiser), e o `DevToolsActivePort` continua sendo
/// lido nela pelo lado de fora.
pub type MontaProcesso =
    dyn Fn(&Path, Vec<String>) -> std::result::Result<Command, String> + Send + Sync;

#[derive(Clone)]
pub struct Envoltorio(pub Arc<MontaProcesso>);

impl std::fmt::Debug for Envoltorio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Envoltorio(..)")
    }
}

#[derive(Debug, Clone)]
pub struct LaunchOptions {
    /// Executavel explicito; `None` consulta `PHXCLAW_CHROMIUM` e os padroes.
    pub executable: Option<PathBuf>,
    /// Sem envoltorio o processo nasce direto (`Command::new(exe)`): e o caminho dos testes
    /// desta crate. O agente passa o bwrap aqui.
    pub envoltorio: Option<Envoltorio>,
    pub policy: BrowserPolicy,
    pub launch_timeout: Duration,
    pub command_timeout: Duration,
    pub navigation_timeout: Duration,
    pub window_size: (u32, u32),
    pub extra_args: Vec<String>,
    /// Idioma da pagina (BCP 47). `None` = o do `LANG` do sistema quando valido, senao pt-BR.
    pub idioma: Option<String>,
}

/// `LANG` do sistema em etiqueta BCP 47: "pt_BR.UTF-8" -> "pt-BR". "C" e "POSIX" nao sao
/// idioma: deixados ao Chromium viravam "en-US@posix", que `Intl.Locale` recusa -- e o app
/// Flutter web parava na largada com "Incorrect locale information provided".
pub fn idioma_do_sistema(lang: Option<&str>) -> Option<String> {
    let base = lang?.split(['.', '@']).next()?.trim();
    if base.is_empty() || base.eq_ignore_ascii_case("c") || base.eq_ignore_ascii_case("posix") {
        return None;
    }
    let mut partes = base.split(['_', '-']);
    let lingua = partes.next()?.to_ascii_lowercase();
    if !(2..=3).contains(&lingua.len()) || !lingua.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    match partes.next() {
        Some(r) if r.len() == 2 && r.bytes().all(|b| b.is_ascii_alphabetic()) => {
            Some(format!("{lingua}-{}", r.to_ascii_uppercase()))
        }
        None => Some(lingua),
        _ => None,
    }
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
            idioma: None,
            envoltorio: None,
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
        if let Some(p) = phxclaw_config_runtime::agente::carga::caminho_do_processo(CHAVE_CHROMIUM)
        {
            candidatos.push(p);
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
/// `unsafe`: o /proc ja responde, e fora do Linux assume-se nao-root. Publica porque quem
/// lanca o Chromium por outro caminho (o `--screenshot` do agente) decide igual.
pub fn running_as_root() -> bool {
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

        let mut args: Vec<String> = vec![
            "--headless=new".into(),
            "--remote-debugging-port=0".into(),
            format!("--user-data-dir={}", profile_dir.display()),
            "--no-first-run".into(),
            "--no-default-browser-check".into(),
            // Trafego proprio do Chromium (atualizacao, sincronia) nao passa
            // pela interceptacao da pagina; desligado, o que sai e so o que
            // o agente pediu.
            "--disable-background-networking".into(),
            "--disable-component-update".into(),
            "--disable-sync".into(),
            "--disable-default-apps".into(),
            "--disable-dev-shm-usage".into(),
            "--mute-audio".into(),
            "--hide-scrollbars".into(),
            format!(
                "--window-size={},{}",
                opts.window_size.0, opts.window_size.1
            ),
        ];
        let idioma = opts
            .idioma
            .clone()
            .or_else(|| {
                idioma_do_sistema(
                    std::env::var("LC_ALL")
                        .ok()
                        .or_else(|| std::env::var("LANG").ok())
                        .as_deref(),
                )
            })
            .unwrap_or_else(|| "pt-BR".into());
        args.push(format!("--lang={idioma}"));
        args.push(format!("--accept-lang={idioma}"));
        if running_as_root() {
            // O sandbox do Chromium recusa subir como root (conteiner, CI) e
            // o processo morre na largada. So nesse caso ele sai; fora dele o
            // sandbox e a segunda linha de defesa e fica ligado.
            args.push("--no-sandbox".into());
        }
        args.extend(opts.extra_args.iter().cloned());
        args.push("about:blank".into());
        let mut cmd = match &opts.envoltorio {
            Some(e) => {
                let mut argv = vec![exe.display().to_string()];
                argv.extend(args);
                (e.0)(&profile_dir, argv).map_err(BrowserError::Launch)?
            }
            None => {
                let mut c = Command::new(&exe);
                c.args(&args);
                c
            }
        };
        cmd.stdin(Stdio::null())
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

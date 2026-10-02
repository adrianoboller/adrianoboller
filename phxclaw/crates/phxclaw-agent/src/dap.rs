//! Depurador dentro do agente: a ferramenta `debug`, um cliente DAP proprio (stdio,
//! cabecalho `Content-Length`) falando com `gdb -i dap` (Rust e C) ou com o `debugpy`
//! (Python), e o `evaluate` como console de depuracao.
//!
//! Tres decisoes, cada uma com o porque:
//!
//! - **O adaptador roda no MESMO sandbox do shell** (`workdir_sandbox_command_com`), sem
//!   rede e com a pasta da tarefa em `/work`: depurar e executar o programa do modelo com
//!   ptrace em cima, e o bwrap deixa (medido em 02/10: `gdb` para num breakpoint e le a
//!   variavel dentro do `--unshare-all`). Nada aqui pede excecao em `FORA_DO_BWRAP`.
//! - **Breakpoint ANTES do `launch` quando o adaptador deixa.** O gdb 15 poe o programa
//!   para correr no proprio `launch` (o evento `process` chega antes da resposta do
//!   `setBreakpoints`), entao breakpoint mandado depois dele disputa com o programa: um
//!   `main` curto termina antes de o breakpoint existir -- medido em 02/10 no bwrap, com
//!   0,2 s de atraso o programa corre ate o fim; com o breakpoint antes do `launch`, para
//!   mesmo com 1 s. O gdb emite `initialized` logo depois do `initialize`, e e isso que
//!   permite configurar antes; o adaptador que so o emite depois do `launch` (debugpy)
//!   cai no caminho da norma: `launch` sem esperar a resposta, `initialized`, breakpoints,
//!   `configurationDone`, e so entao a resposta do `launch`.
//! - **A moldura do fio e a do runtime de MCP/LSP** (`encode_content_length_message` e
//!   `decode_content_length_message`): e o mesmo cabecalho do LSP, e uma segunda leitura
//!   de `Content-Length` divergiria da primeira no dia em que uma delas mudasse.
//!
//! Uma sessao por tarefa; `finish` a derruba. O que o programa escreve no stdout vem na
//! resposta, com teto, nunca em log.

use crate::sistema::{arg_opt, arg_str, caminho_do_projeto};
use crate::tarefa::confine;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_mcp_lsp_runtime::{
    ProtocolError, decode_content_length_message, encode_content_length_message,
};
use phxclaw_sandbox::{SandboxExtras, WorkdirCommand, workdir_sandbox_command_com};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin};
use tokio::sync::{Mutex, mpsc};

/// Onde o adaptador ve a pasta da tarefa: o mesmo `/work` do shell.
const RAIZ_NO_SANDBOX: &str = "/work";
/// Quanto se espera o programa parar num breakpoint (ou terminar) depois de `start`,
/// `continue` e afins.
const PRAZO_DE_PARADA: Duration = Duration::from_secs(60);
const PRAZO_DE_RESPOSTA: Duration = Duration::from_secs(30);
/// Quanto se espera o `initialized` ANTES do `launch`: o gdb o manda em milissegundos;
/// quem nao mandou ate aqui so o manda depois do `launch`.
const PRAZO_DE_INITIALIZED: Duration = Duration::from_millis(1500);
/// Silencio que encerra a drenagem depois de `terminated`: o `output` atrasado do gdb
/// chega em milissegundos; sem evento nesse intervalo, a fila esta vazia.
const SILENCIO_APOS_FIM: Duration = Duration::from_millis(400);
const DRENAGEM_MAX: Duration = Duration::from_secs(3);
/// Teto de linhas de saida do programa guardadas por sessao.
const MAX_SAIDA: usize = 200;
const MAX_VARIAVEIS: usize = 100;

/// Um adaptador de depuracao achado no hospedeiro.
#[derive(Clone)]
pub struct Adaptador {
    pub linguagem: &'static str,
    /// Linha de shell dentro do sandbox.
    pub script: String,
    pub extras: SandboxExtras,
}

/// Os adaptadores do hospedeiro e, para cada linguagem sem adaptador, o motivo.
pub fn adaptadores_do_hospedeiro(bwrap: &Path) -> (Vec<Adaptador>, Vec<String>) {
    let mut achados = Vec::new();
    let mut faltam = Vec::new();
    // O gdb do sistema entra no sandbox pelo `/usr` ja montado; o toolchain do
    // `rust_project` vai junto, para o binario compilado la dentro achar o que precisa.
    if Path::new("/usr/bin/gdb").is_file() {
        let extras = crate::sistema::RustProjectTool::detectar(bwrap.to_path_buf())
            .map(|r| r.extras())
            .unwrap_or_default();
        achados.push(Adaptador {
            linguagem: "rust",
            script: "exec gdb -q -i dap".into(),
            extras,
        });
    } else {
        faltam.push("rust: sem /usr/bin/gdb no hospedeiro".into());
    }
    match crate::python::PythonProjectTool::detectar(bwrap.to_path_buf()) {
        Ok(py) if py.interpretador.site.join("debugpy").is_dir() => {
            achados.push(Adaptador {
                linguagem: "python",
                script: format!(
                    "exec {} -m debugpy.adapter",
                    crate::python::aspas(&py.interpretador.executavel.display().to_string())
                ),
                extras: py.extras(None),
            });
        }
        Ok(py) => faltam.push(format!(
            "python: debugpy ausente em {} (uv pip install debugpy no hospedeiro)",
            py.interpretador.site.display()
        )),
        Err(e) => faltam.push(format!("python: {e}")),
    }
    (achados, faltam)
}

struct Sessao {
    linguagem: &'static str,
    stdin: ChildStdin,
    filho: Child,
    rx: mpsc::Receiver<Value>,
    seq: i64,
    /// Respostas que chegaram antes de alguem esperar por elas (o `launch` adiado).
    respostas: HashMap<i64, Value>,
    /// O corpo do ultimo evento `stopped`; `None` enquanto o programa corre.
    parado: Option<Value>,
    terminado: bool,
    saida: Vec<String>,
    /// Breakpoints por arquivo (caminho no sandbox): o DAP troca a lista inteira do arquivo.
    breakpoints: HashMap<String, Vec<u64>>,
    /// O que o adaptador disse de cada breakpoint (id, arquivo, linha, verificado): o gdb
    /// responde «pendente» antes do `launch` e so o confirma, por evento, quando o
    /// programa carrega -- a resposta do `setBreakpoints` sozinha diria «nao verificado».
    confirmados: Vec<Value>,
}

impl Sessao {
    async fn subir(bwrap: &Path, workdir: &Path, ad: &Adaptador) -> Result<Self, ToolError> {
        let cmd = WorkdirCommand {
            workdir: workdir.to_path_buf(),
            script: ad.script.clone(),
            timeout: Duration::from_secs(3600),
            network: false,
            max_output_bytes: 0,
        };
        let c = workdir_sandbox_command_com(bwrap, &cmd, &ad.extras)
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        let mut c = tokio::process::Command::from(c);
        c.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        let mut filho = c
            .spawn()
            .map_err(|e| ToolError::Failed(format!("depurador {}: {e}", ad.linguagem)))?;
        let stdin = filho.stdin.take().expect("stdin pedido");
        let mut stdout = filho.stdout.take().expect("stdout pedido");
        let (tx, rx) = mpsc::channel(256);
        tokio::spawn(async move {
            let mut buf: Vec<u8> = Vec::new();
            let mut pedaco = [0u8; 8192];
            loop {
                match stdout.read(&mut pedaco).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.extend_from_slice(&pedaco[..n]),
                }
                loop {
                    match decode_content_length_message(&buf) {
                        Ok((v, usado)) => {
                            buf.drain(..usado);
                            if tx.send(v).await.is_err() {
                                return;
                            }
                        }
                        Err(ProtocolError::Incomplete) => break,
                        // Fio corrompido: nada mais se le dele.
                        Err(_) => return,
                    }
                }
            }
        });
        Ok(Self {
            linguagem: ad.linguagem,
            stdin,
            filho,
            rx,
            seq: 1,
            respostas: HashMap::new(),
            parado: None,
            terminado: false,
            saida: Vec::new(),
            breakpoints: HashMap::new(),
            confirmados: Vec::new(),
        })
    }

    async fn enviar(&mut self, comando: &str, args: Value) -> Result<i64, ToolError> {
        let seq = self.seq;
        self.seq += 1;
        let m = json!({"seq": seq, "type": "request", "command": comando, "arguments": args});
        self.stdin
            .write_all(&encode_content_length_message(&m))
            .await
            .map_err(|e| ToolError::Failed(format!("depurador: {e}")))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| ToolError::Failed(format!("depurador: {e}")))?;
        Ok(seq)
    }

    fn evento(&mut self, m: &Value) {
        if m["type"] == "response"
            && let Some(rs) = m["request_seq"].as_i64()
        {
            self.respostas.insert(rs, m.clone());
            return;
        }
        match m["event"].as_str() {
            Some("stopped") => self.parado = Some(m["body"].clone()),
            Some("continued") => self.parado = None,
            Some("terminated") | Some("exited") => {
                self.terminado = true;
                self.parado = None;
            }
            Some("breakpoint") => {
                let b = &m["body"]["breakpoint"];
                if let Some(c) = self.confirmados.iter_mut().find(|c| c["id"] == b["id"]) {
                    c["verificado"] = b["verified"].clone();
                    if !b["line"].is_null() {
                        c["linha"] = b["line"].clone();
                    }
                    c["mensagem"] = b["message"].clone();
                }
            }
            Some("output") => {
                if let Some(t) = m["body"]["output"].as_str()
                    && self.saida.len() < MAX_SAIDA
                {
                    self.saida.push(t.trim_end_matches('\n').to_string());
                }
            }
            _ => {}
        }
    }

    /// Le o fio ate a resposta do pedido `seq`, tratando os eventos no caminho.
    async fn resposta(&mut self, seq: i64, prazo: Duration) -> Result<Value, ToolError> {
        let fim = Instant::now() + prazo;
        loop {
            if let Some(r) = self.respostas.remove(&seq) {
                if r["success"].as_bool() != Some(true) {
                    return Err(ToolError::Failed(format!(
                        "depurador {}: {} recusado: {}",
                        self.linguagem,
                        r["command"].as_str().unwrap_or("?"),
                        r["message"].as_str().unwrap_or("sem motivo")
                    )));
                }
                return Ok(r["body"].clone());
            }
            let resto = fim.saturating_duration_since(Instant::now());
            if resto.is_zero() {
                return Err(ToolError::Timeout(prazo.as_millis() as u64));
            }
            match tokio::time::timeout(resto, self.rx.recv()).await {
                Ok(Some(m)) => self.evento(&m),
                Ok(None) => return Err(ToolError::Failed("o depurador encerrou".into())),
                Err(_) => return Err(ToolError::Timeout(prazo.as_millis() as u64)),
            }
        }
    }

    async fn pedir(&mut self, comando: &str, args: Value) -> Result<Value, ToolError> {
        let seq = self.enviar(comando, args).await?;
        self.resposta(seq, PRAZO_DE_RESPOSTA).await
    }

    /// Espera o programa parar (breakpoint, passo) ou terminar. Depois de terminar,
    /// DRENA o que ainda vem: o gdb le o stdout do programa numa thread propria e manda
    /// cada linha como evento `output` por uma fila, enquanto `exited`/`terminated` saem
    /// da thread do gdb -- sob carga o `output` chega DEPOIS do `terminated` (medido em
    /// 02/10, 12 corridas com `cargo check` ao lado). Devolver no `terminated` perdia a
    /// saida do programa; esperar a fila esvaziar (silencio de `SILENCIO_APOS_FIM`, ou o
    /// fim do fio) e o que a traz de forma determinista.
    async fn esperar_parada(&mut self, prazo: Duration) -> Result<(), ToolError> {
        let fim = Instant::now() + prazo;
        while self.parado.is_none() && !self.terminado {
            let resto = fim.saturating_duration_since(Instant::now());
            if resto.is_zero() {
                return Err(ToolError::Timeout(prazo.as_millis() as u64));
            }
            match tokio::time::timeout(resto, self.rx.recv()).await {
                Ok(Some(m)) => self.evento(&m),
                Ok(None) => {
                    self.terminado = true;
                }
                Err(_) => return Err(ToolError::Timeout(prazo.as_millis() as u64)),
            }
        }
        if self.terminado {
            self.drenar().await;
        }
        Ok(())
    }

    /// Le a fila ate um silencio de `SILENCIO_APOS_FIM` (teto `DRENAGEM_MAX`) ou o fim
    /// do fio.
    async fn drenar(&mut self) {
        let fim = Instant::now() + DRENAGEM_MAX;
        while Instant::now() < fim {
            match tokio::time::timeout(SILENCIO_APOS_FIM, self.rx.recv()).await {
                Ok(Some(m)) => self.evento(&m),
                Ok(None) | Err(_) => break,
            }
        }
    }

    /// Espera um evento com o nome dado (o `initialized`), tratando os outros.
    async fn esperar_evento(&mut self, nome: &str, prazo: Duration) -> Result<(), ToolError> {
        let fim = Instant::now() + prazo;
        loop {
            let resto = fim.saturating_duration_since(Instant::now());
            if resto.is_zero() {
                return Err(ToolError::Timeout(prazo.as_millis() as u64));
            }
            match tokio::time::timeout(resto, self.rx.recv()).await {
                Ok(Some(m)) => {
                    let e = m["event"] == nome;
                    self.evento(&m);
                    if e {
                        return Ok(());
                    }
                }
                Ok(None) => return Err(ToolError::Failed("o depurador encerrou".into())),
                Err(_) => return Err(ToolError::Timeout(prazo.as_millis() as u64)),
            }
        }
    }

    fn thread(&self) -> i64 {
        self.parado
            .as_ref()
            .and_then(|p| p["threadId"].as_i64())
            .unwrap_or(1)
    }

    fn estado(&self) -> Value {
        let parado = self.parado.as_ref().map(|p| {
            json!({
                "motivo": p["reason"],
                "thread": p["threadId"],
                "descricao": p["description"],
                "texto": p["text"],
            })
        });
        json!({
            "linguagem": self.linguagem,
            "parado": parado,
            "terminado": self.terminado,
            "saida": self.saida,
        })
    }

    async fn set_breakpoints(&mut self, arquivo: &str) -> Result<Value, ToolError> {
        let linhas = self.breakpoints.get(arquivo).cloned().unwrap_or_default();
        let bps: Vec<Value> = linhas.iter().map(|l| json!({"line": l})).collect();
        let r = self
            .pedir(
                "setBreakpoints",
                json!({"source": {"path": arquivo}, "breakpoints": bps}),
            )
            .await?;
        self.confirmados.retain(|c| c["arquivo"] != arquivo);
        for (b, pedida) in r["breakpoints"]
            .as_array()
            .into_iter()
            .flatten()
            .zip(&linhas)
        {
            self.confirmados.push(json!({
                "id": b["id"],
                "arquivo": arquivo,
                "linha": if b["line"].is_null() { json!(pedida) } else { b["line"].clone() },
                "verificado": b["verified"],
                "mensagem": b["message"],
            }));
        }
        Ok(self.breakpoints_de(arquivo))
    }

    /// Os breakpoints do arquivo como o modelo os ve (caminho da tarefa, estado atual).
    fn breakpoints_de(&self, arquivo: &str) -> Value {
        let lista: Vec<Value> = self
            .confirmados
            .iter()
            .filter(|c| c["arquivo"] == arquivo)
            .map(|c| json!({"linha": c["linha"], "verificado": c["verificado"], "mensagem": c["mensagem"]}))
            .collect();
        json!({"arquivo": de_sandbox(arquivo), "breakpoints": lista})
    }

    async fn encerrar(mut self) {
        let _ = tokio::time::timeout(
            Duration::from_secs(3),
            self.pedir("disconnect", json!({"terminateDebuggee": true})),
        )
        .await;
        let _ = self.filho.start_kill();
        let _ = self.filho.wait().await;
    }
}

/// As sessoes vivas, uma por tarefa.
pub struct Depurador {
    pub adaptadores: Vec<Adaptador>,
    pub faltam: Vec<String>,
    bwrap: PathBuf,
    sessoes: Mutex<HashMap<String, Sessao>>,
}

impl Depurador {
    pub fn do_hospedeiro(bwrap: PathBuf) -> Self {
        let (adaptadores, faltam) = adaptadores_do_hospedeiro(&bwrap);
        Self {
            adaptadores,
            faltam,
            bwrap,
            sessoes: Mutex::new(HashMap::new()),
        }
    }

    pub fn adaptador(&self, linguagem: &str) -> Option<&Adaptador> {
        self.adaptadores.iter().find(|a| a.linguagem == linguagem)
    }

    /// Quantas sessoes vivas (para a prova de que `finish` solta).
    pub async fn vivas(&self) -> usize {
        self.sessoes.lock().await.len()
    }

    pub async fn encerrar(&self, task_id: &str) {
        let s = self.sessoes.lock().await.remove(task_id);
        if let Some(s) = s {
            s.encerrar().await;
        }
    }

    async fn iniciar(&self, args: &Value, ctx: &ToolContext) -> Result<Value, ToolError> {
        let programa = arg_str(args, "program")?;
        let rel = caminho_do_projeto(&ctx.workdir, programa)?;
        if rel.is_empty() || !ctx.workdir.join(&rel).is_file() {
            return Err(ToolError::InvalidArguments(format!(
                "program tem de ser um arquivo da pasta da tarefa: {programa}"
            )));
        }
        let linguagem = match arg_opt(args, "language") {
            Some(l) => l.to_string(),
            None if rel.ends_with(".py") => "python".into(),
            None => "rust".into(),
        };
        let ad = self.adaptador(&linguagem).cloned().ok_or_else(|| {
            ToolError::InvalidArguments(format!(
                "sem adaptador de depuracao para {linguagem} neste hospedeiro: {}",
                self.faltam.join("; ")
            ))
        })?;
        let argumentos: Vec<String> = args["args"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        // Uma sessao por tarefa: `start` de novo derruba a anterior.
        self.encerrar(&ctx.task_id).await;
        let mut s = Sessao::subir(&self.bwrap, &ctx.workdir, &ad).await?;
        s.pedir(
            "initialize",
            json!({
                "clientID": "phxclaw", "adapterID": linguagem,
                "linesStartAt1": true, "columnsStartAt1": true,
                "pathFormat": "path", "supportsRunInTerminalRequest": false,
            }),
        )
        .await?;
        let programa_no_sandbox = format!("{RAIZ_NO_SANDBOX}/{rel}");
        let launch = if linguagem == "python" {
            json!({"program": programa_no_sandbox, "args": argumentos, "cwd": RAIZ_NO_SANDBOX,
                   "console": "internalConsole", "justMyCode": true})
        } else {
            json!({"program": programa_no_sandbox, "args": argumentos, "cwd": RAIZ_NO_SANDBOX,
                   "stopAtBeginningOfMainSubprogram": false})
        };
        for bp in args["breakpoints"].as_array().into_iter().flatten() {
            let (arquivo, linha) = arquivo_e_linha(&ctx.workdir, bp)?;
            s.breakpoints
                .entry(arquivo.clone())
                .or_default()
                .push(linha);
        }
        let arquivos: Vec<String> = s.breakpoints.keys().cloned().collect();
        // `initialized` antes do `launch` (gdb): os breakpoints entram antes de o
        // programa existir. Sem ele no prazo curto (debugpy): o caminho da norma.
        let antes = s
            .esperar_evento("initialized", PRAZO_DE_INITIALIZED)
            .await
            .is_ok();
        if antes {
            for arquivo in &arquivos {
                s.set_breakpoints(arquivo).await?;
            }
        }
        let seq_launch = s.enviar("launch", launch).await?;
        if !antes {
            s.esperar_evento("initialized", PRAZO_DE_RESPOSTA).await?;
            for arquivo in &arquivos {
                s.set_breakpoints(arquivo).await?;
            }
        }
        // O gdb responde «notStopped» aqui quando o programa ja parou no breakpoint; o
        // debugpy exige o pedido. Um e outro seguem: a parada (ou o fim) e o que se confere.
        let _ = s.pedir("configurationDone", json!({})).await;
        let _ = s.resposta(seq_launch, PRAZO_DE_RESPOSTA).await;
        s.esperar_parada(PRAZO_DE_PARADA).await?;
        // O estado DEPOIS da parada: o pendente do gdb ja virou verificado pelo evento.
        let mut estado = s.estado();
        estado["breakpoints"] =
            Value::Array(arquivos.iter().map(|a| s.breakpoints_de(a)).collect());
        self.sessoes.lock().await.insert(ctx.task_id.clone(), s);
        Ok(estado)
    }

    async fn com_sessao<F>(&self, ctx: &ToolContext, f: F) -> Result<Value, ToolError>
    where
        F: for<'a> FnOnce(&'a mut Sessao) -> BoxFut<'a, Result<Value, ToolError>>,
    {
        let mut todas = self.sessoes.lock().await;
        let s = todas.get_mut(&ctx.task_id).ok_or_else(|| {
            ToolError::InvalidArguments("nenhuma sessao de depuracao: use action=start".into())
        })?;
        f(s).await
    }
}

/// `{file, line}` do modelo para o caminho no sandbox e a linha (a partir de 1).
fn arquivo_e_linha(workdir: &Path, bp: &Value) -> Result<(String, u64), ToolError> {
    let arquivo = bp["file"]
        .as_str()
        .ok_or_else(|| ToolError::InvalidArguments("breakpoint pede 'file'".into()))?;
    let linha = bp["line"].as_u64().filter(|l| *l >= 1).ok_or_else(|| {
        ToolError::InvalidArguments("breakpoint pede 'line' (a partir de 1)".into())
    })?;
    let alvo = confine(workdir, arquivo).map_err(ToolError::Denied)?;
    let base = std::fs::canonicalize(workdir).map_err(|e| ToolError::Failed(e.to_string()))?;
    let canon = std::fs::canonicalize(&alvo)
        .map_err(|e| ToolError::InvalidArguments(format!("{arquivo}: {e}")))?;
    let no_sandbox = match canon.strip_prefix(&base) {
        Ok(rel) => format!("{RAIZ_NO_SANDBOX}/{}", rel.display()),
        Err(_) => canon.display().to_string(),
    };
    Ok((no_sandbox, linha))
}

fn de_sandbox(caminho: &str) -> String {
    caminho
        .strip_prefix(&format!("{RAIZ_NO_SANDBOX}/"))
        .unwrap_or(caminho)
        .to_string()
}

pub struct DebugTool {
    pub depurador: Arc<Depurador>,
}

impl Tool for DebugTool {
    fn spec(&self) -> ToolSpec {
        let tem: Vec<&str> = self
            .depurador
            .adaptadores
            .iter()
            .map(|a| a.linguagem)
            .collect();
        ToolSpec {
            name: "debug".into(),
            description: format!(
                "Debug a program of the task directory with a real debugger (DAP) inside the \
sandbox. Adapters on this host: {}{}. action=start (program, optional args, language, \
breakpoints=[{{file,line}}]; runs until the first breakpoint or the end), breakpoint (file, \
line: adds one and reports whether it was verified), continue (runs to the next stop), stack \
(frames of the stopped thread), variables (locals of a frame; frame=0 is the top), evaluate \
(expression in the frame -- the debug console; console=true sends it to the adapter's own \
command line instead), stop. Returns JSON with the state (stopped reason, program output).",
                if tem.is_empty() {
                    "none".to_string()
                } else {
                    tem.join(", ")
                },
                if self.depurador.faltam.is_empty() {
                    String::new()
                } else {
                    format!("; missing: {}", self.depurador.faltam.join("; "))
                }
            ),
            parameters: json!({"type":"object","properties":{
                "action":{"type":"string","enum":["start","breakpoint","continue","stack","variables","evaluate","stop"]},
                "program":{"type":"string"},
                "args":{"type":"array","items":{"type":"string"}},
                "language":{"type":"string","enum":["rust","python"]},
                "breakpoints":{"type":"array","items":{"type":"object","properties":{
                    "file":{"type":"string"},"line":{"type":"integer","minimum":1}},
                    "required":["file","line"]}},
                "file":{"type":"string"},
                "line":{"type":"integer","minimum":1},
                "frame":{"type":"integer","minimum":0},
                "expression":{"type":"string"},
                "console":{"type":"boolean"}
            },"required":["action"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "shell.exec"
    }
    /// O que roda de verdade: regra sobre `gdb` ou `python` alcanca esta ferramenta.
    fn comando_de_shell(&self, args: &Value) -> Option<String> {
        if args.get("action").and_then(Value::as_str) != Some("start") {
            return None;
        }
        let prog = args.get("program").and_then(Value::as_str).unwrap_or("");
        Some(match args.get("language").and_then(Value::as_str) {
            Some("python") => format!("python -m debugpy {}", crate::python::aspas(prog)),
            Some(_) => format!("gdb {}", crate::python::aspas(prog)),
            None if prog.ends_with(".py") => {
                format!("python -m debugpy {}", crate::python::aspas(prog))
            }
            None => format!("gdb {}", crate::python::aspas(prog)),
        })
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let acao = arg_str(&args, "action")?.to_string();
            let d = &self.depurador;
            let v = match acao.as_str() {
                "start" => d.iniciar(&args, ctx).await?,
                "stop" => {
                    let havia = d.vivas().await;
                    d.encerrar(&ctx.task_id).await;
                    json!({"encerrado": havia > 0})
                }
                "breakpoint" => {
                    let (arquivo, linha) = arquivo_e_linha(&ctx.workdir, &args)?;
                    d.com_sessao(ctx, move |s| {
                        Box::pin(async move {
                            let lista = s.breakpoints.entry(arquivo.clone()).or_default();
                            if !lista.contains(&linha) {
                                lista.push(linha);
                            }
                            s.set_breakpoints(&arquivo).await
                        })
                    })
                    .await?
                }
                "continue" => {
                    d.com_sessao(ctx, |s| {
                        Box::pin(async move {
                            if s.terminado {
                                return Err(ToolError::InvalidArguments(
                                    "o programa ja terminou".into(),
                                ));
                            }
                            let t = s.thread();
                            s.parado = None;
                            s.pedir("continue", json!({"threadId": t})).await?;
                            s.esperar_parada(PRAZO_DE_PARADA).await?;
                            Ok(s.estado())
                        })
                    })
                    .await?
                }
                "stack" => {
                    d.com_sessao(ctx, |s| {
                        Box::pin(async move {
                            let t = s.thread();
                            let r = s
                                .pedir("stackTrace", json!({"threadId": t, "levels": 30}))
                                .await?;
                            let quadros: Vec<Value> = r["stackFrames"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .enumerate()
                                .map(|(n, f)| {
                                    json!({
                                        "frame": n,
                                        "id": f["id"],
                                        "funcao": f["name"],
                                        "arquivo": f["source"]["path"].as_str().map(de_sandbox),
                                        "linha": f["line"],
                                    })
                                })
                                .collect();
                            Ok(json!({"quadros": quadros}))
                        })
                    })
                    .await?
                }
                "variables" => {
                    let n = args["frame"].as_u64().unwrap_or(0) as usize;
                    d.com_sessao(ctx, move |s| {
                        Box::pin(async move {
                            let id = quadro(s, n).await?;
                            let escopos = s.pedir("scopes", json!({"frameId": id})).await?;
                            let mut saida = Vec::new();
                            for e in escopos["scopes"].as_array().into_iter().flatten() {
                                // Registradores sao dezenas de linhas que o modelo nao pediu.
                                if e["presentationHint"] == "registers" {
                                    continue;
                                }
                                let r = e["variablesReference"].as_i64().unwrap_or(0);
                                let vs = s.pedir("variables", json!({"variablesReference": r})).await?;
                                let lista: Vec<Value> = vs["variables"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .take(MAX_VARIAVEIS)
                                    .map(|v| json!({"nome": v["name"], "valor": v["value"], "tipo": v["type"]}))
                                    .collect();
                                saida.push(json!({"escopo": e["name"], "variaveis": lista}));
                            }
                            Ok(json!({"frame": n, "escopos": saida}))
                        })
                    })
                    .await?
                }
                "evaluate" => {
                    let expr = arg_str(&args, "expression")?.to_string();
                    let console = args["console"].as_bool().unwrap_or(false);
                    let n = args["frame"].as_u64().unwrap_or(0) as usize;
                    d.com_sessao(ctx, move |s| {
                        Box::pin(async move {
                            let id = quadro(s, n).await?;
                            let r = s
                                .pedir(
                                    "evaluate",
                                    json!({"expression": expr, "frameId": id,
                                           "context": if console { "repl" } else { "watch" }}),
                                )
                                .await?;
                            Ok(json!({"expressao": expr, "resultado": r["result"], "tipo": r["type"]}))
                        })
                    })
                    .await?
                }
                outra => {
                    return Err(ToolError::InvalidArguments(format!(
                        "action desconhecida: {outra}"
                    )));
                }
            };
            Ok(ToolOutput::text(
                serde_json::to_string_pretty(&v).unwrap_or_default(),
            ))
        })
    }
    fn finish<'a>(&'a self, task_id: &'a str) -> BoxFut<'a, ()> {
        Box::pin(async move { self.depurador.encerrar(task_id).await })
    }
}

/// O id do quadro `n` da thread parada (o DAP numera por sessao, nao por posicao).
async fn quadro(s: &mut Sessao, n: usize) -> Result<i64, ToolError> {
    if s.parado.is_none() {
        return Err(ToolError::InvalidArguments(
            "o programa nao esta parado: nao ha quadro para ler".into(),
        ));
    }
    let t = s.thread();
    let r = s
        .pedir(
            "stackTrace",
            json!({"threadId": t, "startFrame": n, "levels": 1}),
        )
        .await?;
    r["stackFrames"][0]["id"]
        .as_i64()
        .ok_or_else(|| ToolError::InvalidArguments(format!("nao ha quadro {n}")))
}

/// Registra a ferramenta quando ha bwrap; sem adaptador nenhum ela existe para dizer o
/// que falta, em vez de sumir calada.
pub fn ferramenta(bwrap: PathBuf) -> Arc<dyn Tool> {
    Arc::new(DebugTool {
        depurador: Arc::new(Depurador::do_hospedeiro(bwrap)),
    })
}

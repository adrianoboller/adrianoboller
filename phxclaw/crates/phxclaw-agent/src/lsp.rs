//! Servidor de linguagem (LSP) dentro do agente: a ferramenta `lsp`, so de leitura
//! (definicao, referencias, simbolos, hover, diagnosticos), e o diagnostico anexado ao
//! resultado de `write_file`/`edit_file`, como o editor faz depois de cada gravacao.
//!
//! Tres decisoes, cada uma com o porque:
//!
//! - **O servidor roda no MESMO sandbox do shell** (`workdir_sandbox_command_com`), sem
//!   rede e com a pasta da tarefa em `/work`. Servidor de linguagem le codigo escrito pelo
//!   modelo, e o rust-analyzer padrao executa `build.rs` e macros procedurais: fora do
//!   bwrap isso seria o modelo rodando codigo no hospedeiro por uma porta lateral.
//! - **Analise estatica so**: build scripts, macros procedurais e `cargo check` ao salvar
//!   desligados. A ferramenta pede `fs.read`, e o que ela faz tem de caber nisso; o
//!   `cargo check` de verdade continua sendo o `rust_project`, que pede `shell.exec`.
//! - **Os metodos permitidos sao a lista abaixo** (`METODOS`), conferida pela
//!   `CapabilityPolicy` do runtime: `rename`, `codeAction`, `executeCommand` e formatacao
//!   nao passam, e e isso que faz «so de leitura» valer no fio, e nao so na descricao.
//!
//! Diagnostico vem por PULL (`textDocument/diagnostic`), nao pelo `publishDiagnostics`:
//! medido no rust-analyzer 1.98.1, o empurrado chega vazio, depois cheio, depois vazio de
//! novo enquanto o projeto carrega, e o resultado dependia de quando se parava de ouvir.
//! O pull so vale depois do `experimental/serverStatus` com `quiescent: true`.

use crate::tarefa::confine;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_mcp_lsp_runtime::{
    Cancellation, CapabilityPolicy, JsonRpcRequest, LspStdioSession, RuntimeAudit, RuntimeError,
    SessionPolicy, Url, exchange_result,
};
use phxclaw_sandbox::{SandboxExtras, WorkdirCommand};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Metodos que o agente pode mandar ao servidor. Tudo o que muda arquivo fica de fora.
const METODOS: &[&str] = &[
    "initialize",
    "initialized",
    "textDocument/didOpen",
    "textDocument/didChange",
    "textDocument/didClose",
    "textDocument/diagnostic",
    "textDocument/definition",
    "textDocument/references",
    "textDocument/documentSymbol",
    "textDocument/foldingRange",
    "textDocument/hover",
    "workspace/symbol",
    "shutdown",
    "exit",
];

/// Onde o servidor ve a pasta da tarefa: o mesmo `/work` do shell.
const RAIZ_NO_SANDBOX: &str = "/work";
/// Teto de itens devolvidos ao modelo por resposta: referencia de simbolo comum passa de
/// mil num projeto real, e a lista inteira so gasta contexto.
const MAX_ITENS: usize = 50;
/// Erros que o servidor devolve enquanto o projeto muda por baixo: tentar de novo.
const RPC_CANCELADO_PELO_SERVIDOR: i64 = -32802;
const RPC_CONTEUDO_MUDOU: i64 = -32801;

/// Um servidor de linguagem achado no hospedeiro.
#[derive(Clone)]
pub struct ServidorDeLinguagem {
    pub linguagem: &'static str,
    pub extensoes: &'static [&'static str],
    /// Linha de shell dentro do sandbox.
    pub script: String,
    pub extras: SandboxExtras,
    pub opcoes: Value,
    /// O servidor anuncia `serverStatus`? Entao o pull so vale depois do `quiescent`.
    pub espera_quiescencia: bool,
}

/// Os servidores do hospedeiro, achados uma vez por processo: achar o rust-analyzer roda
/// `rustc --print sysroot`, e a montagem acontece a cada tarefa da API.
pub fn servidores_do_hospedeiro() -> Vec<ServidorDeLinguagem> {
    static ACHADOS: OnceLock<Vec<ServidorDeLinguagem>> = OnceLock::new();
    ACHADOS
        .get_or_init(|| {
            let Some(bwrap) = crate::arquivos::achar_bwrap() else {
                return Vec::new();
            };
            [rust_analyzer(bwrap), pyright()]
                .into_iter()
                .flatten()
                .collect()
        })
        .clone()
}

/// O rust-analyzer do MESMO toolchain do `rust_project`, com o mesmo `SandboxExtras`:
/// dois jeitos de montar o toolchain no sandbox divergiriam no dia em que um mudasse.
fn rust_analyzer(bwrap: PathBuf) -> Option<ServidorDeLinguagem> {
    let rust = crate::sistema::RustProjectTool::detectar(bwrap)?;
    rust.sysroot
        .join("bin/rust-analyzer")
        .is_file()
        .then(|| ServidorDeLinguagem {
            linguagem: "rust",
            extensoes: &["rs"],
            script: "exec rust-analyzer".into(),
            extras: rust.extras(),
            opcoes: json!({
                "cargo": {"buildScripts": {"enable": false}},
                "procMacro": {"enable": false},
                "checkOnSave": false,
            }),
            espera_quiescencia: true,
        })
}

/// O pyright do pacote Python (`pyright-langserver` no PATH aponta para um `python` que
/// so faz chamar o node com o `langserver.index.js`). Chama-se o node direto: o pacote
/// Python baixaria o node pela rede, que o sandbox nao tem.
fn pyright() -> Option<ServidorDeLinguagem> {
    let lanc = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .chain([PathBuf::from("/root/.local/bin")])
        .map(|d| d.join("pyright-langserver"))
        .find(|p| p.is_file())?;
    let pacote = std::fs::canonicalize(lanc)
        .ok()?
        .parent()?
        .parent()?
        .join("lib");
    let dist = std::fs::read_dir(&pacote)
        .ok()?
        .flatten()
        .map(|e| e.path().join("site-packages/pyright/dist"))
        .find(|d| d.join("langserver.index.js").is_file())?;
    let node = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .chain([PathBuf::from("/opt/node22/bin"), PathBuf::from("/usr/bin")])
        .map(|d| d.join("node"))
        .find(|p| p.is_file())
        .and_then(|p| std::fs::canonicalize(p).ok())?;
    // O node fora de /usr precisa da propria instalacao montada (lib/ ao lado de bin/).
    let raiz_node = node.parent()?.parent()?.to_path_buf();
    let mut ro_binds = vec![(dist.clone(), dist.display().to_string())];
    if !raiz_node.starts_with("/usr") {
        ro_binds.push((raiz_node.clone(), raiz_node.display().to_string()));
    }
    Some(ServidorDeLinguagem {
        linguagem: "python",
        extensoes: &["py", "pyi"],
        script: format!(
            "exec {} {} --stdio",
            crate::python::aspas(&node.display().to_string()),
            crate::python::aspas(&dist.join("langserver.index.js").display().to_string())
        ),
        extras: SandboxExtras {
            ro_binds,
            env: Vec::new(),
        },
        opcoes: json!({}),
        espera_quiescencia: false,
    })
}

/// (tarefa, linguagem).
type ChaveDeSessao = (String, &'static str);

struct Aberto {
    versao: i64,
    texto: String,
}

struct Sessao {
    s: LspStdioSession,
    servidor: ServidorDeLinguagem,
    quiescente: bool,
    abertos: HashMap<String, Aberto>,
}

/// As sessoes vivas, uma por (tarefa, linguagem): o rust-analyzer indexa a biblioteca
/// padrao ao subir (segundos), e subir de novo a cada pergunta pagaria isso toda vez.
pub struct Lsp {
    pub servidores: Vec<ServidorDeLinguagem>,
    sessoes: Mutex<HashMap<ChaveDeSessao, Arc<Mutex<Sessao>>>>,
}

impl Lsp {
    pub fn new(servidores: Vec<ServidorDeLinguagem>) -> Self {
        Self {
            servidores,
            sessoes: Mutex::new(HashMap::new()),
        }
    }

    /// `None` sem bwrap, sem servidor nenhum, ou com `PHXCLAW_LSP=0`. O bwrap em si e
    /// achado por `processo::espec_no_bwrap` na hora de subir cada servidor.
    pub fn do_hospedeiro() -> Option<Arc<Self>> {
        if std::env::var("PHXCLAW_LSP").as_deref() == Ok("0") {
            return None;
        }
        crate::arquivos::achar_bwrap()?;
        let s = servidores_do_hospedeiro();
        (!s.is_empty()).then(|| Arc::new(Self::new(s)))
    }

    pub fn servidor_de(&self, rel: &str) -> Option<&ServidorDeLinguagem> {
        let ext = Path::new(rel).extension()?.to_str()?;
        self.servidores.iter().find(|s| s.extensoes.contains(&ext))
    }

    async fn sessao(
        &self,
        task_id: &str,
        workdir: &Path,
        servidor: &ServidorDeLinguagem,
    ) -> Result<Arc<Mutex<Sessao>>, ToolError> {
        let mut todas = self.sessoes.lock().await;
        let chave = (task_id.to_string(), servidor.linguagem);
        if let Some(s) = todas.get(&chave) {
            return Ok(s.clone());
        }
        let s = Arc::new(Mutex::new(self.subir(workdir, servidor).await?));
        todas.insert(chave, s.clone());
        Ok(s)
    }

    async fn subir(&self, workdir: &Path, sv: &ServidorDeLinguagem) -> Result<Sessao, ToolError> {
        let falha = |e: RuntimeError| ToolError::Failed(format!("lsp {}: {e}", sv.linguagem));
        let cmd = WorkdirCommand {
            workdir: workdir.to_path_buf(),
            script: sv.script.clone(),
            timeout: Duration::from_secs(3600),
            network: false,
            max_output_bytes: 0,
        };
        // As outras raizes do workspace entram no sandbox SO LEITURA, no mesmo caminho do
        // hospedeiro, e cada uma vira um `workspaceFolder`: o servidor resolve simbolo da
        // segunda raiz, e o modelo continua sem poder grava-la por aqui.
        let raizes = crate::workspace::raizes().map_err(ToolError::Failed)?;
        let mut extras = sv.extras.clone();
        for r in &raizes {
            extras.ro_binds.push((r.clone(), r.display().to_string()));
        }
        // O comando do sandbox vira `ProcessSpec` + politica pelo MESMO motor dos servidores
        // MCP (processo.rs): a lista de binds e o ambiente limpo nao se montam duas vezes.
        let (spec, seguranca) =
            crate::processo::espec_no_bwrap(&cmd, &extras).map_err(ToolError::Failed)?;
        let politica = SessionPolicy {
            request_timeout_ms: 60_000,
            startup_timeout_ms: 30_000,
            // O rust-analyzer manda status e progresso entre uma resposta e outra.
            max_unsolicited_messages: 4096,
            ..SessionPolicy::default()
        };
        let metodos = CapabilityPolicy {
            allowed_methods: METODOS.iter().map(|m| m.to_string()).collect(),
            allowed_tools: Default::default(),
        };
        let mut s = LspStdioSession::spawn(
            &spec,
            &seguranca,
            politica,
            metodos,
            RuntimeAudit::new("phxclaw-agent.lsp", None),
        )
        .await
        .map_err(falha)?;
        // `initialize` montado aqui, nao o do runtime: ele manda `capabilities: {}`, e sem
        // declarar o pull e o `serverStatus` o diagnostico volta ao empurrado instavel.
        let raiz = uri_de(RAIZ_NO_SANDBOX);
        let mut pastas = vec![json!({"uri": raiz, "name": "work"})];
        for r in &raizes {
            let nome = r.file_name().map(|n| n.to_string_lossy().into_owned());
            pastas.push(json!({"uri": uri_de(&r.display().to_string()),
                               "name": nome.unwrap_or_else(|| r.display().to_string())}));
        }
        let ex = s
            .request(
                JsonRpcRequest::new(
                    "initialize",
                    json!({
                        "processId": Value::Null,
                        "rootUri": raiz,
                        "workspaceFolders": pastas,
                        "capabilities": {
                            "textDocument": {
                                "diagnostic": {"dynamicRegistration": false},
                                "hover": {"contentFormat": ["plaintext", "markdown"]},
                                "documentSymbol": {"hierarchicalDocumentSymbolSupport": true},
                                // So linhas: quem dobra e a tela do IDE, que esconde linha
                                // inteira; coluna de inicio e fim nao teriam onde cair.
                                "foldingRange": {"lineFoldingOnly": true}
                            },
                            "experimental": {"serverStatusNotification": true}
                        },
                        "initializationOptions": sv.opcoes,
                    }),
                ),
                Cancellation::default(),
            )
            .await
            .map_err(falha)?;
        exchange_result(ex).map_err(falha)?;
        s.notify("initialized", json!({})).await.map_err(falha)?;
        Ok(Sessao {
            s,
            servidor: sv.clone(),
            quiescente: !sv.espera_quiescencia,
            abertos: HashMap::new(),
        })
    }

    /// Solta as sessoes da tarefa: o servidor segura centenas de MB de indice.
    pub async fn encerrar(&self, task_id: &str) {
        let tirar: Vec<_> = {
            let mut todas = self.sessoes.lock().await;
            let chaves: Vec<_> = todas.keys().filter(|k| k.0 == task_id).cloned().collect();
            chaves.iter().filter_map(|k| todas.remove(k)).collect()
        };
        for s in tirar {
            if let Ok(m) = Arc::try_unwrap(s) {
                let _ = m.into_inner().s.shutdown().await;
            }
        }
    }

    /// Quantas sessoes vivas (para a prova de que `finish` solta).
    pub async fn vivas(&self) -> usize {
        self.sessoes.lock().await.len()
    }

    /// Diagnosticos do arquivo, ja formatados. `Ok(None)` se nenhum servidor cobre a
    /// extensao. Prazo estourado vira texto dizendo que estourou, nao lista vazia: «sem
    /// erros» dito por um servidor que ainda nao terminou de ler seria mentira.
    pub async fn diagnosticos(
        &self,
        ctx: &ToolContext,
        rel: &str,
        prazo: Duration,
    ) -> Result<Option<String>, ToolError> {
        let Some(sv) = self.servidor_de(rel).cloned() else {
            return Ok(None);
        };
        let rel = relativo(&ctx.workdir, rel)?;
        let sessao = self.sessao(&ctx.task_id, &ctx.workdir, &sv).await?;
        let mut s = sessao.lock().await;
        sincronizar(&mut s, &ctx.workdir, &rel).await?;
        let r = pedir_pronto(
            &mut s,
            "textDocument/diagnostic",
            json!({"textDocument": {"uri": uri_no_sandbox(&rel)}}),
            prazo,
        )
        .await?;
        let Some(r) = r else {
            return Ok(Some(format!(
                "{}: o servidor ainda esta indexando o projeto ({} s); diagnostico nao conferido",
                sv.linguagem,
                prazo.as_secs()
            )));
        };
        let itens = r
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(Some(formatar_diagnosticos(&ctx.workdir, &rel, &itens)))
    }

    /// Os simbolos de um arquivo do projeto (`textDocument/documentSymbol`, JSON cru do
    /// servidor), para a barra de caminho do IDE. `Ok(None)` quando o prazo acaba antes de o
    /// servidor ficar quiescente -- lista vazia dita por um servidor que ainda indexa seria
    /// mentira. A sessao e a da chave `ide`, uma por linguagem, compartilhada por todas as
    /// consultas da tela: cada arquivo aberto no Helix nao pode custar um rust-analyzer.
    pub async fn simbolos_do_arquivo(
        &self,
        workdir: &Path,
        rel: &str,
        prazo: Duration,
    ) -> Result<Option<Value>, ToolError> {
        self.pedido_do_ide(workdir, rel, "textDocument/documentSymbol", prazo)
            .await
    }

    /// As regioes dobraveis do arquivo (`textDocument/foldingRange`, JSON cru), para o
    /// painel de leitura do IDE. Mesma sessao `ide` e mesmo `Ok(None)` dos simbolos: quem
    /// chama cai na reserva por chaves/indentacao e diz que caiu.
    pub async fn dobras_do_arquivo(
        &self,
        workdir: &Path,
        rel: &str,
        prazo: Duration,
    ) -> Result<Option<Value>, ToolError> {
        self.pedido_do_ide(workdir, rel, "textDocument/foldingRange", prazo)
            .await
    }

    /// Um pedido da tela sobre UM documento: o servidor da extensao, o caminho pelo
    /// `confine`, a sessao `ide` e o texto do disco sincronizado antes. Os dois pedidos da
    /// tela passam por aqui para a sincronizacao e o confinamento nao se escreverem duas vezes.
    async fn pedido_do_ide(
        &self,
        workdir: &Path,
        rel: &str,
        metodo: &str,
        prazo: Duration,
    ) -> Result<Option<Value>, ToolError> {
        let sv = self
            .servidor_de(rel)
            .cloned()
            .ok_or_else(|| sem_servidor(rel, &self.servidores))?;
        let rel = relativo(workdir, rel)?;
        let sessao = self.sessao("ide", workdir, &sv).await?;
        let mut s = sessao.lock().await;
        sincronizar(&mut s, workdir, &rel).await?;
        pedir_pronto(
            &mut s,
            metodo,
            json!({"textDocument": {"uri": uri_no_sandbox(&rel)}}),
            prazo,
        )
        .await
    }

    async fn consultar(
        &self,
        ctx: &ToolContext,
        acao: &str,
        args: &Value,
    ) -> Result<String, ToolError> {
        let prazo = ctx.timeout.min(Duration::from_secs(120));
        if acao == "symbols" && args.get("path").is_none() {
            let consulta = args.get("query").and_then(Value::as_str).ok_or_else(|| {
                ToolError::InvalidArguments("symbols pede 'path' ou 'query'".into())
            })?;
            let sv = self.servidores.first().cloned().ok_or_else(|| {
                ToolError::Failed("nenhum servidor de linguagem neste hospedeiro".into())
            })?;
            let sessao = self.sessao(&ctx.task_id, &ctx.workdir, &sv).await?;
            let mut s = sessao.lock().await;
            let r = pedir_pronto(
                &mut s,
                "workspace/symbol",
                json!({"query": consulta}),
                prazo,
            )
            .await?
            .ok_or(ToolError::Timeout(prazo.as_millis() as u64))?;
            return Ok(formatar_simbolos(&ctx.workdir, &r, 0));
        }
        let rel_pedido = args
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("falta 'path'".into()))?;
        if acao == "diagnostics" {
            return self
                .diagnosticos(ctx, rel_pedido, prazo)
                .await?
                .ok_or_else(|| sem_servidor(rel_pedido, &self.servidores));
        }
        let sv = self
            .servidor_de(rel_pedido)
            .cloned()
            .ok_or_else(|| sem_servidor(rel_pedido, &self.servidores))?;
        let rel = relativo(&ctx.workdir, rel_pedido)?;
        let sessao = self.sessao(&ctx.task_id, &ctx.workdir, &sv).await?;
        let mut s = sessao.lock().await;
        sincronizar(&mut s, &ctx.workdir, &rel).await?;
        let doc = json!({"uri": uri_no_sandbox(&rel)});
        let (metodo, params) = match acao {
            "symbols" => ("textDocument/documentSymbol", json!({"textDocument": doc})),
            "definition" | "references" | "hover" => {
                let pos = posicao(&s, &rel, args)?;
                let mut p = json!({"textDocument": doc, "position": pos});
                if acao == "references" {
                    p["context"] = json!({"includeDeclaration": true});
                }
                let m = match acao {
                    "definition" => "textDocument/definition",
                    "references" => "textDocument/references",
                    _ => "textDocument/hover",
                };
                (m, p)
            }
            outra => {
                return Err(ToolError::InvalidArguments(format!(
                    "acao desconhecida: {outra}"
                )));
            }
        };
        let r = pedir_pronto(&mut s, metodo, params, prazo)
            .await?
            .ok_or(ToolError::Timeout(prazo.as_millis() as u64))?;
        Ok(match acao {
            "symbols" => formatar_simbolos(&ctx.workdir, &r, 0),
            "hover" => formatar_hover(&r),
            _ => formatar_locais(&ctx.workdir, &r),
        })
    }
}

fn sem_servidor(rel: &str, servidores: &[ServidorDeLinguagem]) -> ToolError {
    let ha: Vec<String> = servidores
        .iter()
        .map(|s| format!("{} (.{})", s.linguagem, s.extensoes.join(", .")))
        .collect();
    ToolError::InvalidArguments(format!(
        "nenhum servidor de linguagem cobre {rel}; neste hospedeiro: {}",
        ha.join("; ")
    ))
}

/// O caminho normalizado pelo MESMO `confine` das ferramentas de arquivo: relativo a
/// pasta da tarefa, ou absoluto quando esta noutra raiz do workspace (la dentro o
/// caminho e o mesmo do hospedeiro).
fn relativo(workdir: &Path, rel: &str) -> Result<String, ToolError> {
    let alvo = confine(workdir, rel).map_err(ToolError::Denied)?;
    let base = std::fs::canonicalize(workdir).map_err(|e| ToolError::Failed(e.to_string()))?;
    let alvo = std::fs::canonicalize(&alvo)
        .map_err(|e| ToolError::InvalidArguments(format!("{rel}: {e}")))?;
    Ok(match alvo.strip_prefix(&base) {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(_) => alvo.to_string_lossy().into_owned(),
    })
}

fn uri_de(caminho: &str) -> String {
    Url::from_file_path(caminho)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| format!("file://{caminho}"))
}

fn uri_no_sandbox(rel: &str) -> String {
    if rel.starts_with('/') {
        return uri_de(rel);
    }
    uri_de(&format!("{RAIZ_NO_SANDBOX}/{rel}"))
}

/// URI do servidor de volta para o que o modelo entende: relativo a pasta da tarefa, ou o
/// caminho absoluto quando aponta para fora (a biblioteca padrao, por exemplo).
fn de_uri(uri: &str) -> String {
    let caminho = Url::parse(uri)
        .ok()
        .and_then(|u| u.to_file_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| uri.to_string());
    caminho
        .strip_prefix(&format!("{RAIZ_NO_SANDBOX}/"))
        .map(str::to_string)
        .unwrap_or(caminho)
}

/// Abre ou atualiza o documento com o texto do DISCO: o shell e as outras ferramentas
/// tambem mudam arquivo, e o servidor nao ve o disco da tarefa pelo `didChange`.
async fn sincronizar(s: &mut Sessao, workdir: &Path, rel: &str) -> Result<(), ToolError> {
    let texto = std::fs::read_to_string(workdir.join(rel))
        .map_err(|e| ToolError::InvalidArguments(format!("{rel}: {e}")))?;
    let uri = uri_no_sandbox(rel);
    let falha = |e: RuntimeError| ToolError::Failed(format!("lsp: {e}"));
    match s.abertos.get_mut(rel) {
        Some(a) if a.texto == texto => {}
        Some(a) => {
            a.versao += 1;
            a.texto = texto.clone();
            let v = a.versao;
            s.s.notify(
                "textDocument/didChange",
                json!({"textDocument": {"uri": uri, "version": v},
                       "contentChanges": [{"text": texto}]}),
            )
            .await
            .map_err(falha)?;
        }
        None => {
            let lingua = s.servidor.linguagem;
            s.s.notify(
                "textDocument/didOpen",
                json!({"textDocument": {"uri": uri, "languageId": lingua, "version": 1,
                                        "text": texto}}),
            )
            .await
            .map_err(falha)?;
            s.abertos
                .insert(rel.to_string(), Aberto { versao: 1, texto });
        }
    }
    Ok(())
}

/// Pede e devolve o `result`, esperando o servidor terminar de carregar o projeto.
/// `Ok(None)` quando o prazo acaba antes. Cada pedido tambem e a bomba que le as
/// notificacoes acumuladas -- e e assim que o `quiescent` chega sem um leitor a parte.
async fn pedir_pronto(
    s: &mut Sessao,
    metodo: &str,
    params: Value,
    prazo: Duration,
) -> Result<Option<Value>, ToolError> {
    let fim = Instant::now() + prazo;
    loop {
        // O estado e lido ANTES de pedir: um `quiescent` que chega junto com a resposta
        // nao garante que a resposta foi calculada depois dele.
        let pronto = s.quiescente;
        let ex =
            s.s.request(
                JsonRpcRequest::new(metodo, params.clone()),
                Cancellation::default(),
            )
            .await
            .map_err(|e| ToolError::Failed(format!("lsp {metodo}: {e}")))?;
        for n in &ex.notifications {
            if n.get("method").and_then(Value::as_str) == Some("experimental/serverStatus") {
                s.quiescente = n["params"]["quiescent"].as_bool().unwrap_or(false);
            }
        }
        // `result: null` e resposta valida do LSP («nada»), e o runtime a le como falta de
        // result. Medido com duas raizes no rust-analyzer: enquanto o projeto recarrega, o
        // documentSymbol volta null e depois volta cheio -- null antes do quiescente e
        // «tente de novo», nao erro; depois dele e a resposta.
        let nulo = ex.response.error.is_none() && ex.response.result.is_none();
        if nulo {
            if pronto {
                return Ok(Some(Value::Null));
            }
        } else {
            match exchange_result(ex) {
                Ok(v) if pronto => return Ok(Some(v)),
                Ok(_) => {}
                Err(RuntimeError::Rpc { code, .. })
                    if code == RPC_CANCELADO_PELO_SERVIDOR || code == RPC_CONTEUDO_MUDOU => {}
                Err(e) => return Err(ToolError::Failed(format!("lsp {metodo}: {e}"))),
            }
        }
        if Instant::now() >= fim {
            return Ok(None);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Linha e coluna do modelo (a partir de 1, coluna em caracteres) para a posicao do LSP
/// (a partir de 0, coluna em unidades UTF-16, o padrao do protocolo).
fn posicao(s: &Sessao, rel: &str, args: &Value) -> Result<Value, ToolError> {
    let linha = args
        .get("line")
        .and_then(Value::as_u64)
        .filter(|l| *l >= 1)
        .ok_or_else(|| ToolError::InvalidArguments("falta 'line' (a partir de 1)".into()))?;
    let coluna = args
        .get("column")
        .and_then(Value::as_u64)
        .filter(|c| *c >= 1)
        .ok_or_else(|| ToolError::InvalidArguments("falta 'column' (a partir de 1)".into()))?;
    let texto = s.abertos.get(rel).map(|a| a.texto.as_str()).unwrap_or("");
    let l = texto
        .lines()
        .nth((linha - 1) as usize)
        .ok_or_else(|| ToolError::InvalidArguments(format!("{rel} nao tem a linha {linha}")))?;
    let utf16: usize = l
        .chars()
        .take((coluna - 1) as usize)
        .map(char::len_utf16)
        .sum();
    Ok(json!({"line": linha - 1, "character": utf16}))
}

/// Coluna UTF-16 do protocolo de volta para caracteres, quando o arquivo e da tarefa.
fn coluna_em_caracteres(workdir: &Path, rel: &str, linha: u64, utf16: u64) -> u64 {
    let Ok(t) = std::fs::read_to_string(workdir.join(rel)) else {
        return utf16;
    };
    let Some(l) = t.lines().nth(linha as usize) else {
        return utf16;
    };
    let mut gasto = 0u64;
    let mut n = 0u64;
    for c in l.chars() {
        if gasto >= utf16 {
            break;
        }
        gasto += c.len_utf16() as u64;
        n += 1;
    }
    n
}

fn local(workdir: &Path, uri: &str, range: &Value) -> String {
    let rel = de_uri(uri);
    let linha = range["start"]["line"].as_u64().unwrap_or(0);
    let col = range["start"]["character"].as_u64().unwrap_or(0);
    let col = coluna_em_caracteres(workdir, &rel, linha, col);
    let trecho = std::fs::read_to_string(workdir.join(&rel))
        .ok()
        .and_then(|t| t.lines().nth(linha as usize).map(|l| l.trim().to_string()))
        .map(|l| format!("  | {}", l.chars().take(160).collect::<String>()))
        .unwrap_or_default();
    format!("{rel}:{}:{}{trecho}", linha + 1, col + 1)
}

fn formatar_locais(workdir: &Path, r: &Value) -> String {
    let lista: Vec<Value> = match r {
        Value::Array(v) => v.clone(),
        Value::Null => Vec::new(),
        outro => vec![outro.clone()],
    };
    if lista.is_empty() {
        return "nenhum resultado".into();
    }
    let mut out: Vec<String> = lista
        .iter()
        .take(MAX_ITENS)
        .map(|l| {
            // `Location` ou `LocationLink`: o segundo traz o alvo em outros campos.
            let uri = l["uri"].as_str().or(l["targetUri"].as_str()).unwrap_or("");
            let range = if l.get("targetSelectionRange").is_some() {
                &l["targetSelectionRange"]
            } else {
                &l["range"]
            };
            local(workdir, uri, range)
        })
        .collect();
    if lista.len() > MAX_ITENS {
        out.push(format!("[... mais {}]", lista.len() - MAX_ITENS));
    }
    out.join("\n")
}

fn formatar_hover(r: &Value) -> String {
    fn texto(c: &Value) -> String {
        match c {
            Value::String(s) => s.clone(),
            Value::Array(v) => v.iter().map(texto).collect::<Vec<_>>().join("\n"),
            Value::Object(o) => o
                .get("value")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            _ => String::new(),
        }
    }
    let t = texto(&r["contents"]);
    if t.trim().is_empty() {
        "nenhuma informacao nesta posicao".into()
    } else {
        phxclaw_agent_core::truncate_for_model(&t, 4000)
    }
}

/// O numero de `SymbolKind` do protocolo em palavra (so os que aparecem em codigo).
pub(crate) fn tipo_de_simbolo(k: u64) -> &'static str {
    match k {
        2 => "modulo",
        5 => "classe",
        6 => "metodo",
        7 => "propriedade",
        8 => "campo",
        9 => "construtor",
        10 => "enum",
        11 => "interface",
        12 => "funcao",
        13 => "variavel",
        14 => "constante",
        22 => "variante",
        23 => "struct",
        26 => "parametro_de_tipo",
        _ => "simbolo",
    }
}

fn formatar_simbolos(workdir: &Path, r: &Value, nivel: usize) -> String {
    let Some(lista) = r.as_array() else {
        return "nenhum simbolo".into();
    };
    if lista.is_empty() && nivel == 0 {
        return "nenhum simbolo".into();
    }
    let mut out = Vec::new();
    for s in lista.iter().take(MAX_ITENS * 4) {
        let nome = s["name"].as_str().unwrap_or("?");
        let tipo = tipo_de_simbolo(s["kind"].as_u64().unwrap_or(0));
        // `DocumentSymbol` (com filhos) ou `SymbolInformation` (com local).
        let onde = if let Some(l) = s.get("location") {
            local(workdir, l["uri"].as_str().unwrap_or(""), &l["range"])
        } else {
            format!(
                "linha {}",
                s["selectionRange"]["start"]["line"].as_u64().unwrap_or(0) + 1
            )
        };
        out.push(format!("{}{tipo} {nome} -- {onde}", "  ".repeat(nivel)));
        if let Some(f) = s.get("children").filter(|f| f.is_array()) {
            let filhos = formatar_simbolos(workdir, f, nivel + 1);
            if !filhos.is_empty() {
                out.push(filhos);
            }
        }
    }
    out.join("\n")
}

fn formatar_diagnosticos(workdir: &Path, rel: &str, itens: &[Value]) -> String {
    // Erro e aviso so: dica e informacao inundariam o resultado da gravacao.
    let mut graves: Vec<&Value> = itens
        .iter()
        .filter(|d| d["severity"].as_u64().unwrap_or(1) <= 2)
        .collect();
    graves.sort_by_key(|d| d["severity"].as_u64().unwrap_or(1));
    if graves.is_empty() {
        return format!("{rel}: nenhum erro ou aviso");
    }
    let mut out: Vec<String> = graves
        .iter()
        .take(MAX_ITENS)
        .map(|d| {
            let linha = d["range"]["start"]["line"].as_u64().unwrap_or(0);
            let col = coluna_em_caracteres(
                workdir,
                rel,
                linha,
                d["range"]["start"]["character"].as_u64().unwrap_or(0),
            );
            let nivel = if d["severity"].as_u64().unwrap_or(1) == 1 {
                "erro"
            } else {
                "aviso"
            };
            let codigo = match &d["code"] {
                Value::String(c) => format!(" {c}"),
                Value::Number(c) => format!(" {c}"),
                _ => String::new(),
            };
            let fonte = d["source"].as_str().unwrap_or("lsp");
            format!(
                "{rel}:{}:{} {nivel}{codigo}: {} ({fonte})",
                linha + 1,
                col + 1,
                d["message"].as_str().unwrap_or("").replace('\n', " ")
            )
        })
        .collect();
    if graves.len() > MAX_ITENS {
        out.push(format!("[... mais {}]", graves.len() - MAX_ITENS));
    }
    out.join("\n")
}

/// A ferramenta `lsp`, so de leitura.
pub struct LspTool {
    pub lsp: Arc<Lsp>,
}

impl Tool for LspTool {
    fn spec(&self) -> ToolSpec {
        let linguas: Vec<String> = self
            .lsp
            .servidores
            .iter()
            .map(|s| format!("{} (.{})", s.linguagem, s.extensoes.join(", .")))
            .collect();
        ToolSpec {
            name: "lsp".into(),
            description: format!(
                "Read-only language server queries on files in the task directory ({}). \
action=definition|references|hover need path, line and column (1-based); action=symbols with \
path lists the file's symbols, with query searches the workspace; action=diagnostics returns \
the file's errors and warnings. Never edits files.",
                linguas.join(", ")
            ),
            parameters: json!({"type":"object","properties":{
                "action":{"type":"string","enum":["definition","references","hover","symbols","diagnostics"]},
                "path":{"type":"string"},
                "line":{"type":"integer"},
                "column":{"type":"integer"},
                "query":{"type":"string"}
            },"required":["action"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    /// Sobe o servidor de linguagem (processo): a regra de comando a alcanca por `lsp <action>`.
    fn comando_de_shell(&self, args: &Value) -> Option<String> {
        Some(crate::motor::linha_sintetica(
            "lsp",
            args,
            &["action", "path"],
        ))
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let acao = args
                .get("action")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'action'".into()))?;
            self.lsp
                .consultar(ctx, acao, &args)
                .await
                .map(ToolOutput::text)
        })
    }
    fn finish<'a>(&'a self, task_id: &'a str) -> BoxFut<'a, ()> {
        Box::pin(self.lsp.encerrar(task_id))
    }
}

/// Envolve uma ferramenta que grava arquivo: roda a gravacao e anexa ao MESMO resultado
/// o diagnostico do arquivo gravado. Anexado, e nao numa ferramenta a parte, porque o
/// modelo que nao pergunta nao descobre o erro de tipo que acabou de plantar.
pub struct ComDiagnostico {
    pub inner: Arc<dyn Tool>,
    pub lsp: Arc<Lsp>,
}

/// Quanto a gravacao espera pelo diagnostico. O primeiro do rust-analyzer inclui indexar
/// a biblioteca padrao; os seguintes voltam em menos de um segundo.
pub const PRAZO_DO_DIAGNOSTICO: Duration = Duration::from_secs(60);

impl Tool for ComDiagnostico {
    fn spec(&self) -> ToolSpec {
        self.inner.spec()
    }
    fn capability(&self) -> &'static str {
        self.inner.capability()
    }
    fn comando_de_shell(&self, args: &Value) -> Option<String> {
        self.inner.comando_de_shell(args)
    }
    fn finish<'a>(&'a self, task_id: &'a str) -> BoxFut<'a, ()> {
        self.inner.finish(task_id)
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let mut out = self.inner.run(args.clone(), ctx).await?;
            let Some(rel) = args.get("path").and_then(Value::as_str) else {
                return Ok(out);
            };
            // Diagnostico que falha nao desfaz a gravacao, que ja aconteceu: vira nota.
            let prazo = ctx.timeout.min(PRAZO_DO_DIAGNOSTICO);
            match self.lsp.diagnosticos(ctx, rel, prazo).await {
                Ok(Some(d)) => out.content.push_str(&format!("\n\ndiagnosticos:\n{d}")),
                Ok(None) => {}
                Err(e) => out
                    .content
                    .push_str(&format!("\n\ndiagnosticos indisponiveis: {e}")),
            }
            Ok(out)
        })
    }
}

/// Liga o LSP numa lista de ferramentas ja montada: envolve `write_file` e `edit_file` e
/// acrescenta `lsp`. Sem servidor no hospedeiro, a lista fica como estava.
pub fn ligar(tools: &mut Vec<Arc<dyn Tool>>) {
    let Some(lsp) = Lsp::do_hospedeiro() else {
        return;
    };
    ligar_com(tools, lsp);
}

pub fn ligar_com(tools: &mut Vec<Arc<dyn Tool>>, lsp: Arc<Lsp>) {
    for t in tools.iter_mut() {
        let nome = t.spec().name;
        if nome == "write_file" || nome == "edit_file" {
            *t = Arc::new(ComDiagnostico {
                inner: t.clone(),
                lsp: lsp.clone(),
            });
        }
    }
    tools.push(Arc::new(LspTool { lsp }));
}

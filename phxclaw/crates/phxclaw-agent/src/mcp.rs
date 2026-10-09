//! MCP no agente, nos dois sentidos.
//!
//! - **Cliente**: os servidores MCP que o OPERADOR declarou num arquivo (caminho em
//!   `PHXCLAW_MCP_CONFIG`) viram ferramentas do agente, `mcp__<servidor>__<ferramenta>`,
//!   cada uma com a capacidade `mcp.<servidor>`. O modelo nunca escolhe comando nem URL:
//!   so chama, pelo nome, o que a configuracao trouxe. Nenhuma `mcp.*` esta no padrao.
//! - **Servidor**: `servir` expoe por stdio as ferramentas de um `Agent` ja montado. A
//!   lista e a do `Agent::visible_specs` e a chamada e a do `Agent::call_tool`: o mesmo
//!   portao de capacidade e a mesma evidencia do laco do modelo, e nao uma copia deles.
//!   `rotas` expoe o MESMO `responder` por streamable HTTP (`POST /mcp` do `servir`, com
//!   o Bearer da API), que e o transporte que o MCP Client Tool do n8n e os clientes
//!   remotos aceitam; stdio continua para o Claude Desktop e os editores.
//!
//! O fio (JSON-RPC, `initialize`, `tools/list`, `tools/call`) mora no
//! `phxclaw-mcp-lsp-runtime`; aqui so se decide politica e ciclo de vida.

use crate::motor::Agent;
use crate::oauth::{AutorizacaoMcp, ConfigOauth};
use phxclaw_agent_core::{
    BoxFut, Tool, ToolCall, ToolContext, ToolError, ToolOutput, ToolSpec, truncate_for_model,
};
use phxclaw_evidence_ledger::EvidenceLedger;
use phxclaw_mcp_lsp_runtime::{
    Cancellation, CapabilityPolicy, DEFAULT_MAX_FRAME_BYTES, HttpEndpointPolicy,
    JSONRPC_INVALID_PARAMS, JSONRPC_INVALID_REQUEST, JSONRPC_METHOD_NOT_FOUND, JSONRPC_PARSE_ERROR,
    JsonRpcRequest, McpStdioSession, McpStreamableHttpClient, McpToolDescriptor, ProcessSpec,
    RuntimeAudit, RuntimeError, SessionPolicy, Url, encode_mcp_line, exchange_result,
    jsonrpc_error, jsonrpc_result, negotiate_legacy_version, negotiate_server_version,
    normalize_mcp_name, qualified_tool_name, tool_call_result, tool_result_is_error,
    tool_result_text, tools_list_result,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Chave do arquivo de declaracao dos servidores MCP (`PHXCLAW_MCP_CONFIG`).
pub const CHAVE_CONFIG: &str = "mcp.config";
const CLIENTE: &str = "phxclaw";
const VERSAO: &str = env!("CARGO_PKG_VERSION");
/// Teto do texto devolvido ao modelo por chamada; o motor corta de novo no proprio teto.
const TEXTO_MAX: usize = 12_000;
/// Prazo de subir + handshake + listar, por servidor, quando a configuracao nao diz.
const PRAZO_INICIO_PADRAO: Duration = Duration::from_secs(15);
/// Cortesia ao fechar no fim da tarefa: stdin fechado, depois SIGKILL.
const CORTESIA_FECHAR: Duration = Duration::from_secs(2);

// ------------------------------------------------------------------ configuracao

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigMcp {
    #[serde(default)]
    pub servidores: Vec<ServidorDeclarado>,
}

/// Um servidor como o operador o declara: `comando` (+`args`) para stdio, ou `url` para
/// HTTP streamable. Os dois juntos e recusa: um servidor so tem um transporte.
#[derive(Debug, Clone, Deserialize)]
pub struct ServidorDeclarado {
    pub nome: String,
    #[serde(default)]
    pub comando: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    /// Ambiente do filho. O resto e limpo (o runtime faz `env_clear`): o filho nao herda
    /// chave de API que o agente tenha no ambiente. So o `PATH` passa, se nao vier aqui.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub prazo_inicio_ms: Option<u64>,
    /// Credencial do servidor remoto. O valor nunca mora aqui: so o TIPO, e os endpoints do
    /// OAuth; o segredo vai para o broker pelo `phxclaw mcp token|login`.
    #[serde(default)]
    pub auth: Option<AuthDeclarada>,
    /// Servidor oficial conhecido (`linear`, `gmail`, `drive`, `calendar`): preenche a URL, o
    /// tipo de credencial e os endpoints que faltarem. O que a declaracao trouxer vale mais.
    #[serde(default)]
    pub preset: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "tipo", rename_all = "lowercase")]
pub enum AuthDeclarada {
    /// `Authorization: Bearer <segredo>` fixo (a chave de API do Linear).
    Bearer,
    /// OAuth 2.0 com PKCE; o refresh token fica no broker.
    Oauth(ConfigOauth),
}

/// Os MCP oficiais que o agente conhece pelo nome. Linear: `mcp.linear.app/mcp` aceita a
/// chave de API como Bearer. Google Workspace: os endpoints de developers.google.com
/// (lidos em 01/10/2026), cliente OAuth do operador, escopos so de leitura por padrao --
/// escrever (gmail.compose, drive.file) e o operador que acrescenta, sabendo.
fn preset(nome: &str) -> Option<(&'static str, AuthDeclarada)> {
    let google = |escopos: &[&str]| {
        AuthDeclarada::Oauth(ConfigOauth {
            autorizacao: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token: "https://oauth2.googleapis.com/token".into(),
            escopos: escopos.iter().map(|s| s.to_string()).collect(),
            segredo_cliente: true,
            // Sem `access_type=offline` o Google nao devolve refresh token; sem
            // `prompt=consent` ele so o devolve na PRIMEIRA autorizacao da conta.
            parametros: [("access_type", "offline"), ("prompt", "consent")]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            ..ConfigOauth::default()
        })
    };
    Some(match nome {
        "linear" => ("https://mcp.linear.app/mcp", AuthDeclarada::Bearer),
        "gmail" => (
            "https://gmailmcp.googleapis.com/mcp/v1",
            google(&["https://www.googleapis.com/auth/gmail.readonly"]),
        ),
        "drive" => (
            "https://drivemcp.googleapis.com/mcp/v1",
            google(&["https://www.googleapis.com/auth/drive.readonly"]),
        ),
        "calendar" => (
            "https://calendarmcp.googleapis.com/mcp/v1",
            google(&[
                "https://www.googleapis.com/auth/calendar.calendarlist.readonly",
                "https://www.googleapis.com/auth/calendar.events.readonly",
            ]),
        ),
        _ => return None,
    })
}

impl ServidorDeclarado {
    /// A declaracao com o preset aplicado: URL e credencial que faltam vem dele; campo do
    /// OAuth em branco vem dele; o `cliente_id` e sempre do operador.
    pub fn resolvida(&self) -> Result<ServidorDeclarado, String> {
        let mut d = self.clone();
        let Some(p) = &self.preset else {
            return Ok(d);
        };
        let (url, auth) = preset(p).ok_or_else(|| format!("preset desconhecido: {p}"))?;
        if d.comando.is_none() && d.url.is_none() {
            d.url = Some(url.to_string());
        }
        d.auth = Some(match (d.auth.take(), auth) {
            (None, a) => a,
            (Some(AuthDeclarada::Oauth(mut o)), AuthDeclarada::Oauth(base)) => {
                if o.autorizacao.is_empty() {
                    o.autorizacao = base.autorizacao;
                }
                if o.token.is_empty() {
                    o.token = base.token;
                }
                if o.escopos.is_empty() {
                    o.escopos = base.escopos;
                }
                for (k, v) in base.parametros {
                    o.parametros.entry(k).or_insert(v);
                }
                o.segredo_cliente |= base.segredo_cliente;
                AuthDeclarada::Oauth(o)
            }
            (Some(a), _) => a,
        });
        Ok(d)
    }
}

enum Transporte {
    /// A politica de processo nasce na hora do spawn, em `processo::servidor_no_bwrap`: o
    /// executavel permitido e o bwrap, e o comando do operador vai no argv dele.
    Stdio { spec: ProcessSpec },
    Http {
        url: Url,
        origem: HttpEndpointPolicy,
        auth: Option<Arc<AutorizacaoMcp>>,
    },
}

/// Um servidor MCP configurado. As sessoes sao por tarefa: duas tarefas (ou dois
/// subagentes) nunca dividem um processo, e o fim de uma mata so o dela.
pub struct ServidorMcp {
    nome: String,
    capacidade: &'static str,
    transporte: Transporte,
    prazo_inicio: Duration,
    /// Nomes originais descobertos; so eles passam no `tools/call` do runtime.
    ferramentas: BTreeSet<String>,
    sessoes: Mutex<HashMap<String, Arc<tokio::sync::Mutex<Option<Conexao>>>>>,
}

enum Conexao {
    Stdio(McpStdioSession),
    Http {
        cliente: McpStreamableHttpClient,
        versao: String,
    },
}

/// A capacidade de um servidor MCP sai da CASA, nunca do servidor: `mcp.<servidor>` para o
/// que o operador declarou, `mcp.<pacote>.<servidor>` para o que um pacote assinado trouxe.
/// O nome normalizado nao tem ponto, entao as duas formas nunca se encontram. Medido (R7,
/// 09/10/2026): com a forma unica, o pacote que declarava um servidor `eco` ao lado do `eco`
/// do operador tinha o `mcp__eco__apagar` dele rodando sob o `mcp.eco` que o operador
/// concedera ao proprio servidor -- a concessao de um virava a do outro pelo NOME, e o nome
/// quem escolhe e o pacote. Tambem nada do `tools/list` (descricao, `annotations` como
/// `readOnlyHint`) entra aqui: o que a ferramenta de fora diz de si nao a rebaixa.
fn capacidade_de(servidor: &str, pacote: Option<&str>) -> String {
    match pacote {
        None => format!("mcp.{servidor}"),
        Some(p) => format!("mcp.{}.{servidor}", normalize_mcp_name(p.trim())),
    }
}

fn resolver_comando(comando: &str, base: &Path) -> Result<PathBuf, String> {
    if comando.contains('/') {
        let p = Path::new(comando);
        return Ok(if p.is_absolute() {
            p.to_path_buf()
        } else {
            base.join(p)
        });
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .map(|d| d.join(comando))
        .find(|c| c.is_file())
        .ok_or_else(|| format!("comando '{comando}' nao achado no PATH"))
}

impl ServidorMcp {
    fn de_declarado(
        d: &ServidorDeclarado,
        base: &Path,
        raiz_do_agente: Option<&Path>,
        pacote: Option<&str>,
    ) -> Result<Self, String> {
        let d = &d.resolvida()?;
        let nome = normalize_mcp_name(d.nome.trim());
        if nome.is_empty() {
            return Err("servidor sem nome".into());
        }
        let transporte = match (&d.comando, &d.url) {
            (Some(_), None) if d.auth.is_some() => {
                return Err("'auth' so vale para servidor por 'url'".into());
            }
            (Some(c), None) => {
                let cwd = match &d.cwd {
                    Some(c) if c.is_absolute() => c.clone(),
                    Some(c) => base.join(c),
                    None => base.to_path_buf(),
                };
                let executable = resolver_comando(c, &cwd)?;
                let mut env = d.env.clone();
                if !env.contains_key("PATH")
                    && let Ok(p) = std::env::var("PATH")
                {
                    env.insert("PATH".into(), p);
                }
                Transporte::Stdio {
                    spec: ProcessSpec {
                        executable,
                        args: d.args.clone(),
                        cwd,
                        env,
                    },
                }
            }
            (None, Some(u)) => {
                let url = Url::parse(u).map_err(|e| format!("url: {e}"))?;
                let origem = HttpEndpointPolicy {
                    allowed_origins: BTreeSet::from([url.origin().ascii_serialization()]),
                    ..HttpEndpointPolicy::default()
                };
                let auth = match &d.auth {
                    None => None,
                    Some(a) => {
                        let raiz = raiz_do_agente
                            .ok_or("credencial MCP so na configuracao do operador")?;
                        let oauth = match a {
                            AuthDeclarada::Bearer => None,
                            AuthDeclarada::Oauth(o) => Some(o),
                        };
                        // A chave do segredo e (nome, endpoint): renomear a declaracao
                        // para outra URL nao herda o token guardado para a antiga.
                        let alvo = crate::oauth::Alvo::novo(&nome, u)?;
                        Some(Arc::new(AutorizacaoMcp::da_pasta(raiz, &alvo, oauth)?))
                    }
                };
                Transporte::Http { url, origem, auth }
            }
            (Some(_), Some(_)) => return Err("declare 'comando' OU 'url', nao os dois".into()),
            (None, None) => return Err("falta 'comando' ou 'url'".into()),
        };
        // `Tool::capability` devolve `&'static str`: o texto vaza UMA vez por servidor
        // configurado, na montagem, e nao por chamada.
        let capacidade: &'static str = Box::leak(capacidade_de(&nome, pacote).into_boxed_str());
        Ok(Self {
            nome,
            capacidade,
            transporte,
            prazo_inicio: d
                .prazo_inicio_ms
                .map(Duration::from_millis)
                .unwrap_or(PRAZO_INICIO_PADRAO),
            ferramentas: BTreeSet::new(),
            sessoes: Mutex::new(HashMap::new()),
        })
    }

    fn politica_de_metodos(&self) -> CapabilityPolicy {
        CapabilityPolicy {
            allowed_methods: [
                "initialize",
                "notifications/initialized",
                "tools/list",
                "tools/call",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            allowed_tools: self.ferramentas.clone(),
        }
    }

    /// Sobe (ou abre) e faz o handshake. O prazo de cada pedido fica no do chamador.
    async fn conectar(&self, prazo_pedido: Duration) -> Result<Conexao, RuntimeError> {
        let sessao = SessionPolicy {
            request_timeout_ms: prazo_pedido.as_millis() as u64,
            startup_timeout_ms: self.prazo_inicio.as_millis() as u64,
            shutdown_timeout_ms: CORTESIA_FECHAR.as_millis() as u64,
            ..SessionPolicy::default()
        };
        let auditoria = RuntimeAudit::new("phxclaw-agent", None);
        match &self.transporte {
            Transporte::Stdio { spec, .. } => {
                // O comando do operador nasce no bwrap do shell, nao no hospedeiro: a
                // politica do runtime passa a liberar o bwrap, e o resto vai no argv dele.
                let (spec, seguranca) =
                    crate::processo::servidor_no_bwrap(spec).map_err(RuntimeError::ProcessSpec)?;
                let mut s = McpStdioSession::spawn(
                    &spec,
                    &seguranca,
                    sessao,
                    self.politica_de_metodos(),
                    auditoria,
                )
                .await?;
                s.initialize_negotiated(CLIENTE, VERSAO).await?;
                Ok(Conexao::Stdio(s))
            }
            Transporte::Http { url, origem, auth } => {
                let mut cliente = McpStreamableHttpClient::new(
                    url.clone(),
                    sessao,
                    origem.clone(),
                    self.politica_de_metodos(),
                    auditoria,
                )?;
                if let Some(a) = auth {
                    cliente = cliente.with_authorization(a.clone());
                }
                let versao = cliente
                    .initialize_negotiated(CLIENTE, VERSAO, Cancellation::default())
                    .await?;
                Ok(Conexao::Http { cliente, versao })
            }
        }
    }

    async fn listar(c: &mut Conexao) -> Result<Vec<McpToolDescriptor>, RuntimeError> {
        match c {
            Conexao::Stdio(s) => s.list_tools().await,
            Conexao::Http { cliente, versao } => {
                cliente.list_tools(versao, Cancellation::default()).await
            }
        }
    }

    async fn chamar(c: &mut Conexao, nome: &str, args: Value) -> Result<Value, RuntimeError> {
        match c {
            Conexao::Stdio(s) => s.call_tool(nome, args, Cancellation::default()).await,
            Conexao::Http { cliente, versao } => {
                cliente
                    .call_tool(nome, args, versao, Cancellation::default())
                    .await
            }
        }
    }

    async fn fechar(c: Conexao) {
        if let Conexao::Stdio(s) = c {
            let _ = s.close().await;
        }
    }

    fn vaga(&self, task_id: &str) -> Arc<tokio::sync::Mutex<Option<Conexao>>> {
        self.sessoes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(task_id.to_string())
            .or_default()
            .clone()
    }

    async fn executar(
        &self,
        original: &str,
        args: Value,
        ctx: &ToolContext,
    ) -> Result<Value, ToolError> {
        let vaga = self.vaga(&ctx.task_id);
        let trabalho = async {
            let mut vigia = Vigia {
                guarda: vaga.lock_owned().await,
                inteira: false,
            };
            let mut tentativa = 0;
            loop {
                // O 401 pode vir ja no `initialize` de uma conexao nova (acesso revogado entre
                // duas tarefas): ele tambem passa pela renovacao abaixo, nao so o da chamada.
                let r = match vigia.guarda.as_mut() {
                    Some(c) => Self::chamar(c, original, args.clone()).await,
                    None => match self.conectar(ctx.timeout).await {
                        Ok(c) => {
                            let c = vigia.guarda.insert(c);
                            Self::chamar(c, original, args.clone()).await
                        }
                        Err(e) => Err(e),
                    },
                };
                // Erro JSON-RPC e resposta inteira: a sessao continua boa. Qualquer outro
                // erro (fio fechado, quadro grande demais) deixa o fio em estado desconhecido.
                vigia.inteira = matches!(r, Ok(_) | Err(RuntimeError::Rpc { .. }));
                // 401 com OAuth: o acesso venceu antes do que o servidor disse (revogado,
                // relogio). Renova UMA vez e repete; o segundo 401 e resposta.
                if tentativa == 0
                    && matches!(r, Err(RuntimeError::HttpStatus(401)))
                    && let Some(a) = self.autorizacao()
                    && a.invalidar().await
                {
                    tentativa += 1;
                    *vigia.guarda = None;
                    continue;
                }
                return r;
            }
        };
        match tokio::time::timeout(ctx.timeout, trabalho).await {
            Err(_) => Err(ToolError::Timeout(ctx.timeout.as_millis() as u64)),
            Ok(Err(e)) => Err(ToolError::Failed(
                self.limpar(format!("mcp {}: {e}", self.nome)),
            )),
            Ok(Ok(v)) => Ok(v),
        }
    }

    fn autorizacao(&self) -> Option<&Arc<AutorizacaoMcp>> {
        match &self.transporte {
            Transporte::Http { auth, .. } => auth.as_ref(),
            Transporte::Stdio { .. } => None,
        }
    }

    /// Todo texto deste servidor que vai ao modelo, a evidencia ou ao aviso passa aqui.
    fn limpar(&self, texto: String) -> String {
        match self.autorizacao() {
            Some(a) => a.limpar(texto),
            None => texto,
        }
    }

    async fn encerrar(&self, task_id: &str) {
        let vaga = self
            .sessoes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(task_id);
        if let Some(v) = vaga
            && let Some(c) = v.lock().await.take()
        {
            Self::fechar(c).await;
        }
    }
}

/// Segura a vaga da sessao durante a chamada. Se a chamada nao terminou inteira --
/// inclusive quando o futuro e DESCARTADO pelo prazo, do motor ou nosso --, a conexao
/// sai da vaga e cai: o filho tem `kill_on_drop`, entao cair e levar SIGKILL. Cortar so
/// o futuro deixava o processo vivo (a mesma licao do `ToolContext::timeout`).
struct Vigia {
    guarda: tokio::sync::OwnedMutexGuard<Option<Conexao>>,
    inteira: bool,
}

impl Drop for Vigia {
    fn drop(&mut self) {
        if !self.inteira {
            drop(self.guarda.take());
        }
    }
}

/// Uma ferramenta de um servidor MCP, vista pelo agente.
pub struct McpTool {
    servidor: Arc<ServidorMcp>,
    original: String,
    spec: ToolSpec,
}

impl Tool for McpTool {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }
    fn capability(&self) -> &'static str {
        self.servidor.capacidade
    }
    /// Servidor por stdio nasce como processo na primeira chamada: a regra de comando do
    /// operador o alcanca pelo nome da ferramenta (`mcp__servidor__ferramenta`). O de
    /// `url` nao cria processo e fica so com a capacidade.
    fn comando_de_shell(&self, _args: &Value) -> Option<String> {
        match self.servidor.transporte {
            Transporte::Stdio { .. } => Some(self.spec.name.clone()),
            Transporte::Http { .. } => None,
        }
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let args = if args.is_null() { json!({}) } else { args };
            let r = self.servidor.executar(&self.original, args, ctx).await?;
            // Limpa ANTES de cortar: o corte poderia partir o token e deixar meio segredo.
            let texto = truncate_for_model(&self.servidor.limpar(tool_result_text(&r)), TEXTO_MAX);
            if tool_result_is_error(&r) {
                return Err(ToolError::Failed(texto));
            }
            Ok(ToolOutput::text(texto))
        })
    }
    fn finish<'a>(&'a self, task_id: &'a str) -> BoxFut<'a, ()> {
        Box::pin(self.servidor.encerrar(task_id))
    }
}

/// Le a configuracao, sobe cada servidor, lista as ferramentas e devolve-as como `Tool`.
/// Servidor que nao sobe vira aviso e some da lista: um servidor de terceiros quebrado
/// nao pode derrubar o agente inteiro.
///
/// Sincrona porque a montagem e: a descoberta roda num runtime proprio, numa thread, e
/// fecha os processos que subiu. As chamadas de verdade sobem os seus no runtime de quem
/// chama -- processo e conexao do tokio ficam presos ao runtime que os criou.
pub fn carregar(caminho: &Path) -> (Vec<Arc<dyn Tool>>, Vec<String>) {
    carregar_em(caminho, None)
}

/// `carregar` com a pasta do agente, onde mora o broker das credenciais (`mcp/`). Sem ela,
/// servidor que declara `auth` vira aviso: credencial nao se le de outro lugar.
pub fn carregar_em(
    caminho: &Path,
    raiz_do_agente: Option<&Path>,
) -> (Vec<Arc<dyn Tool>>, Vec<String>) {
    let mut avisos = Vec::new();
    let (cfg, base) = match ler_config(caminho) {
        Ok(c) => c,
        Err(e) => {
            avisos.push(e);
            return (vec![], avisos);
        }
    };
    let (tools, mais) = carregar_config_em(&cfg, &base, raiz_do_agente, None);
    avisos.extend(mais);
    (tools, avisos)
}

/// O arquivo do operador lido, e a pasta dele (a base dos caminhos relativos).
fn ler_config(caminho: &Path) -> Result<(ConfigMcp, PathBuf), String> {
    let cfg: ConfigMcp = std::fs::read(caminho)
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
        .map_err(|e| format!("{}: {e}", caminho.display()))?;
    let base = caminho
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let base = std::fs::canonicalize(&base).unwrap_or(base);
    Ok((cfg, base))
}

/// A mesma subida para uma configuracao do operador ja lida: um caminho so do declarado ate
/// a ferramenta.
pub fn carregar_config(cfg: &ConfigMcp, base: &Path) -> (Vec<Arc<dyn Tool>>, Vec<String>) {
    carregar_config_em(cfg, base, None, None)
}

/// A subida do `.mcp.json` de um pacote assinado (traduzido): a mesma do operador, com a
/// capacidade no espaco do pacote (`mcp.<pacote>.<servidor>`, ver `capacidade_de`).
pub fn carregar_config_de_pacote(
    cfg: &ConfigMcp,
    base: &Path,
    pacote: &str,
) -> (Vec<Arc<dyn Tool>>, Vec<String>) {
    carregar_config_em(cfg, base, None, Some(pacote))
}

fn carregar_config_em(
    cfg: &ConfigMcp,
    base: &Path,
    raiz_do_agente: Option<&Path>,
    pacote: Option<&str>,
) -> (Vec<Arc<dyn Tool>>, Vec<String>) {
    let mut avisos = Vec::new();
    let base = base.to_path_buf();
    let mut servidores = Vec::new();
    let mut vistos = BTreeSet::new();
    for d in &cfg.servidores {
        match ServidorMcp::de_declarado(d, &base, raiz_do_agente, pacote) {
            Ok(s) if !vistos.insert(s.nome.clone()) => {
                avisos.push(format!("servidor '{}' declarado duas vezes", s.nome))
            }
            Ok(s) => servidores.push(s),
            Err(e) => avisos.push(format!("servidor '{}': {e}", d.nome)),
        }
    }
    if servidores.is_empty() {
        return (vec![], avisos);
    }
    let descoberta = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        Ok::<_, String>(rt.block_on(async move {
            let mut out = Vec::new();
            for s in servidores {
                let r = tokio::time::timeout(s.prazo_inicio, async {
                    let mut c = s.conectar(s.prazo_inicio).await?;
                    let lista = ServidorMcp::listar(&mut c).await;
                    ServidorMcp::fechar(c).await;
                    lista
                })
                .await;
                let r = match r {
                    Ok(r) => r.map_err(|e| s.limpar(e.to_string())),
                    Err(_) => Err(format!("sem resposta em {:?}", s.prazo_inicio)),
                };
                out.push((s, r));
            }
            out
        }))
    })
    .join();
    let descobertos = match descoberta {
        Ok(Ok(d)) => d,
        Ok(Err(e)) => {
            avisos.push(format!("runtime da descoberta: {e}"));
            return (vec![], avisos);
        }
        Err(_) => {
            avisos.push("descoberta MCP entrou em panico".into());
            return (vec![], avisos);
        }
    };
    let mut tools: Vec<Arc<dyn Tool>> = Vec::new();
    for (mut s, r) in descobertos {
        let lista = match r {
            Ok(l) => l,
            Err(e) => {
                avisos.push(format!("servidor '{}' nao subiu: {e}", s.nome));
                continue;
            }
        };
        let mut nomes = BTreeSet::new();
        let mut aceitas = Vec::new();
        for t in lista {
            let qualificado = qualified_tool_name(&s.nome, &t.name);
            if !nomes.insert(qualificado.clone()) {
                avisos.push(format!(
                    "{qualificado}: nome repetido no servidor, ignorada"
                ));
                continue;
            }
            aceitas.push((qualificado, t));
        }
        s.ferramentas = aceitas.iter().map(|(_, t)| t.name.clone()).collect();
        let s = Arc::new(s);
        for (qualificado, t) in aceitas {
            let descricao = t.description.clone().unwrap_or_default();
            tools.push(Arc::new(McpTool {
                servidor: s.clone(),
                spec: ToolSpec {
                    name: qualificado,
                    description: format!("[MCP {}] {descricao}", s.nome),
                    parameters: t.input_schema.clone(),
                },
                original: t.name,
            }));
        }
    }
    (tools, avisos)
}

/// A declaracao de `nome` no arquivo de `PHXCLAW_MCP_CONFIG`, com o preset aplicado: o que
/// o `phxclaw mcp login` usa, pela mesma leitura da montagem.
pub fn declarado_no_ambiente(nome: &str) -> Result<ServidorDeclarado, String> {
    let var = crate::config::variavel(CHAVE_CONFIG);
    let caminho = crate::config::caminho_de(CHAVE_CONFIG).ok_or(format!("falta {var}"))?;
    let cfg: ConfigMcp = std::fs::read(&caminho)
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))?;
    let alvo = normalize_mcp_name(nome.trim());
    cfg.servidores
        .iter()
        .find(|d| normalize_mcp_name(d.nome.trim()) == alvo)
        .ok_or(format!("servidor '{nome}' nao declarado em {var}"))?
        .resolvida()
}

/// O servidor `nome` do arquivo do OPERADOR (`config`, o de `PHXCLAW_MCP_CONFIG`), pronto e
/// sem subir nada: o que o gatilho de notificacao assina. Servidor de pacote nao esta neste
/// arquivo, e por isso nao arma gatilho.
pub fn servidor_do_operador(
    config: &Path,
    nome: &str,
    raiz_do_agente: Option<&Path>,
) -> Result<ServidorMcp, String> {
    let (cfg, base) = ler_config(config)?;
    let alvo = normalize_mcp_name(nome.trim());
    let d = cfg
        .servidores
        .iter()
        .find(|d| normalize_mcp_name(d.nome.trim()) == alvo)
        .ok_or_else(|| format!("servidor '{nome}' nao declarado em {}", config.display()))?;
    ServidorMcp::de_declarado(d, &base, raiz_do_agente, None)
}

// ------------------------------------------------------------------ assinatura de recursos

/// O que a conexao de ASSINATURA pode mandar: o handshake e o `resources/subscribe`. Nenhum
/// `tools/call`: quem assina so escuta, e o que a notificacao dispara roda pelo portao.
const METODOS_DA_ASSINATURA: &[&str] = &[
    "initialize",
    "notifications/initialized",
    "resources/subscribe",
];
/// Fila entre o leitor do fio e o gatilho. Cheia, o leitor espera: o servidor que inunda
/// enche o proprio cano, e nao a memoria do agente.
const FILA_DA_ASSINATURA: usize = 256;

/// Uma assinatura viva: o processo do servidor, no bwrap como o da ferramenta, lido por uma
/// tarefa propria. Cair a `Assinatura` aborta a tarefa, e com ela o processo
/// (`kill_on_drop`): a leitura sem prazo do fio nunca e cortada no meio de uma linha com a
/// sessao ainda em uso.
pub struct Assinatura {
    rx: tokio::sync::mpsc::Receiver<Result<Value, String>>,
    leitor: tokio::task::JoinHandle<()>,
}

impl Drop for Assinatura {
    fn drop(&mut self) {
        self.leitor.abort();
    }
}

impl Assinatura {
    /// A proxima mensagem que o servidor mandou por conta propria; `Err` e o motivo de o fio
    /// ter caido, `None` o fim. O pedido do servidor (o `ping` dele) ja foi respondido pelo
    /// leitor, e vem tambem: quem conta a inundacao conta tudo.
    pub async fn proxima(&mut self) -> Option<Result<Value, String>> {
        self.rx.recv().await
    }
}

impl ServidorMcp {
    pub fn nome(&self) -> &str {
        &self.nome
    }

    /// A capacidade que a casa deu a este servidor (`capacidade_de`).
    pub fn capacidade(&self) -> &'static str {
        self.capacidade
    }

    /// Sobe o servidor, faz o handshake, confere que ele ANUNCIA o que se vai pedir
    /// (`resources.subscribe` para os `recursos`, `resources.listChanged` para a `lista`) e
    /// assina cada recurso. So stdio: o streamable HTTP entrega notificacao fora de pedido
    /// pelo GET em SSE, que o runtime ainda nao abre, e dizer isso e melhor que assinar e
    /// nunca ouvir nada.
    pub async fn assinar(
        &self,
        recursos: &[String],
        lista: bool,
        auditoria: RuntimeAudit,
    ) -> Result<Assinatura, String> {
        let Transporte::Stdio { spec } = &self.transporte else {
            return Err(format!(
                "servidor '{}': a assinatura de recursos so vale por stdio nesta versao (o \
                 servidor por url manda a notificacao pelo GET em SSE, que o runtime nao abre)",
                self.nome
            ));
        };
        let sessao = SessionPolicy {
            request_timeout_ms: self.prazo_inicio.as_millis() as u64,
            startup_timeout_ms: self.prazo_inicio.as_millis() as u64,
            shutdown_timeout_ms: CORTESIA_FECHAR.as_millis() as u64,
            ..SessionPolicy::default()
        };
        let politica = CapabilityPolicy {
            allowed_methods: METODOS_DA_ASSINATURA
                .iter()
                .map(|m| m.to_string())
                .collect(),
            allowed_tools: BTreeSet::new(),
        };
        let nome = self.nome.clone();
        let subir = async {
            let (spec, seguranca) =
                crate::processo::servidor_no_bwrap(spec).map_err(|e| e.to_string())?;
            let mut s = McpStdioSession::spawn(&spec, &seguranca, sessao, politica, auditoria)
                .await
                .map_err(|e| e.to_string())?;
            let ex = s
                .initialize_legacy(CLIENTE, VERSAO)
                .await
                .map_err(|e| e.to_string())?;
            let mut recebidas = ex.notifications.clone();
            let r = exchange_result(ex).map_err(|e| e.to_string())?;
            let versao = r
                .get("protocolVersion")
                .and_then(Value::as_str)
                .ok_or("initialize sem protocolVersion")?;
            negotiate_legacy_version(versao).map_err(|e| e.to_string())?;
            let anuncia = |k: &str| {
                r.pointer(&format!("/capabilities/resources/{k}")) == Some(&Value::Bool(true))
            };
            if !recursos.is_empty() && !anuncia("subscribe") {
                return Err(
                    "o servidor nao anuncia resources.subscribe no initialize: nao ha o que assinar"
                        .to_string(),
                );
            }
            if lista && !anuncia("listChanged") {
                return Err(
                    "o servidor nao anuncia resources.listChanged no initialize: a lista nunca \
                     avisaria"
                        .to_string(),
                );
            }
            for uri in recursos {
                let ex = s
                    .request(
                        JsonRpcRequest::new("resources/subscribe", json!({"uri": uri})),
                        Cancellation::default(),
                    )
                    .await
                    .map_err(|e| format!("resources/subscribe {uri}: {e}"))?;
                recebidas.extend(ex.notifications.iter().cloned());
                exchange_result(ex).map_err(|e| format!("resources/subscribe {uri}: {e}"))?;
            }
            Ok::<_, String>((s, recebidas))
        };
        let (mut s, recebidas) = tokio::time::timeout(self.prazo_inicio, subir)
            .await
            .map_err(|_| format!("sem resposta em {:?}", self.prazo_inicio))?
            .map_err(|e| format!("servidor '{nome}': {e}"))?;
        let (tx, rx) = tokio::sync::mpsc::channel(FILA_DA_ASSINATURA);
        let leitor = tokio::spawn(async move {
            for v in recebidas {
                if tx.send(Ok(v)).await.is_err() {
                    return;
                }
            }
            loop {
                let v = match s.next_message().await {
                    Ok(v) => v,
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string())).await;
                        break;
                    }
                };
                // Pedido do servidor: responde (o `ping` com `{}`, o resto com metodo
                // desconhecido) para ele nao derrubar a sessao esperando.
                if let (Some(id), Some(m)) = (v.get("id"), v.get("method").and_then(Value::as_str))
                {
                    let resposta = if m == "ping" {
                        jsonrpc_result(id.clone(), json!({}))
                    } else {
                        jsonrpc_error(
                            id.clone(),
                            JSONRPC_METHOD_NOT_FOUND,
                            "o cliente que assina so escuta",
                        )
                    };
                    if let Err(e) = s.reply(&resposta).await {
                        let _ = tx.send(Err(e.to_string())).await;
                        break;
                    }
                }
                if tx.send(Ok(v)).await.is_err() {
                    break;
                }
            }
            let _ = s.kill().await;
        });
        Ok(Assinatura { rx, leitor })
    }
}

/// O que a montagem chama: le `PHXCLAW_MCP_CONFIG`, escreve os avisos no stderr (o
/// stdout do `mcp-serve` e o fio do protocolo) e devolve as ferramentas.
pub fn carregar_do_ambiente(raiz_do_agente: &Path) -> Vec<Arc<dyn Tool>> {
    let Some(caminho) = crate::config::caminho_de(CHAVE_CONFIG) else {
        return vec![];
    };
    let (tools, avisos) = carregar_em(&caminho, Some(raiz_do_agente));
    for a in avisos {
        eprintln!("aviso: MCP: {a}");
    }
    tools
}

// ------------------------------------------------------------------ servidor

/// Uma sessao de chamadas pelo portao fora do laco do modelo: um id novo, a evidencia dele
/// e a pasta de trabalho. E UMA so porque o `mcp-serve` e o `phxclaw ferramenta` chamam o
/// mesmo `Agent::call_tool`; duas montagens desta sessao seriam duas regras de onde a
/// evidencia mora, e a que alguem esquecesse chamaria a ferramenta sem deixar rastro.
pub struct Sessao {
    pub ctx: ToolContext,
    pub ledger: EvidenceLedger,
}

impl Sessao {
    pub fn abrir(agente: &Agent, workdir: PathBuf) -> std::io::Result<Self> {
        let task_id = phxclaw_types::new_uuid_v7().to_string();
        let ledger = EvidenceLedger::open(agente.store.evidence_path(&task_id))
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        std::fs::create_dir_all(&workdir)?;
        let ctx = ToolContext {
            task_id,
            workdir,
            timeout: agente.config.tool_timeout,
        };
        Ok(Self { ctx, ledger })
    }

    /// Solta o que as ferramentas seguraram para a sessao (navegador, processo).
    pub async fn fechar(&self, agente: &Agent) {
        for t in &agente.tools {
            t.finish(&self.ctx.task_id).await;
        }
    }
}

/// Serve por stdio (ou qualquer par leitor/escritor) as ferramentas que a politica do
/// `agente` concede. A lista sai do `visible_specs` e a chamada do `call_tool`: a mesma
/// porta do laco do modelo, com a mesma evidencia. Termina no fim da entrada, soltando o
/// que as ferramentas seguraram para a sessao.
pub async fn servir<R, W>(
    agente: &Agent,
    workdir: PathBuf,
    mut entrada: R,
    mut saida: W,
) -> std::io::Result<()>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let sessao = Sessao::abrir(agente, workdir)?;
    let (ctx, ledger) = (&sessao.ctx, &sessao.ledger);
    let mut r = Ok(());
    loop {
        let linha = match ler_linha(&mut entrada).await? {
            Linha::Fim => break,
            Linha::Grande(e) => {
                let _ = saida.write_all(&encode_mcp_line(&e)).await;
                r = Err(std::io::Error::other("mensagem MCP acima do teto"));
                break;
            }
            Linha::Bytes(b) => b,
        };
        if linha.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let resposta = match serde_json::from_slice::<Value>(&linha) {
            Err(e) => Some(jsonrpc_error(
                Value::Null,
                JSONRPC_PARSE_ERROR,
                &e.to_string(),
            )),
            Ok(v) => responder(agente, ctx, ledger, v).await,
        };
        if let Some(resp) = resposta {
            saida.write_all(&encode_mcp_line(&resp)).await?;
            saida.flush().await?;
        }
    }
    sessao.fechar(agente).await;
    r
}

// ------------------------------------------------------------------ streamable HTTP

/// Cabecalho de sessao do streamable HTTP (MCP 2025-03-26). Aqui a sessao e so a pasta de
/// trabalho e a evidencia: o cliente propoe o id, o servidor confina a forma dele.
const SESSAO: &str = "mcp-session-id";

/// `POST /mcp` no router da API (`servir`): o mesmo `responder` do stdio, uma mensagem por
/// pedido, resposta em JSON (o streamable HTTP aceita `application/json` no lugar do SSE
/// quando a resposta e uma so). GET e 405 porque o servidor nao inicia fluxo nenhum, e
/// DELETE e 204: a sessao nao segura nada alem da pasta.
///
/// O agente nasce por pedido pela `factory` da API, com o modelo padrao: e a mesma
/// montagem de uma tarefa, e o portao de capacidade e o dela. Custa a montagem por
/// chamada -- medido como aceitavel para o laco do n8n, que chama uma ferramenta por no.
pub fn rotas() -> axum::Router<crate::api::ApiState> {
    axum::Router::new().route(
        "/mcp",
        axum::routing::post(mcp_http)
            .get(|| async { axum::http::StatusCode::METHOD_NOT_ALLOWED })
            .delete(|| async { axum::http::StatusCode::NO_CONTENT }),
    )
}

/// O id de sessao: o cliente repete o que o `initialize` devolveu. So vale a forma de
/// UUID (hex e `-`, ate 64), porque ele vira nome de pasta sob `tasks/` pelo mesmo
/// `safe_id` das tarefas; qualquer outra coisa ganha um id novo, devolvido no cabecalho.
fn sessao_de(h: &axum::http::HeaderMap) -> String {
    h.get(SESSAO)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| {
            (1..=64).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
        })
        .map(str::to_string)
        .unwrap_or_else(|| phxclaw_types::new_uuid_v7().to_string())
}

async fn mcp_http(
    axum::extract::State(s): axum::extract::State<crate::api::ApiState>,
    h: axum::http::HeaderMap,
    corpo: axum::body::Bytes,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    if let Err(e) = crate::api::auth(&s, &h) {
        return e.into_response();
    }
    if corpo.len() > DEFAULT_MAX_FRAME_BYTES {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            axum::Json(jsonrpc_error(
                Value::Null,
                JSONRPC_INVALID_REQUEST,
                "mensagem grande demais",
            )),
        )
            .into_response();
    }
    let v: Value = match serde_json::from_slice(&corpo) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                axum::Json(jsonrpc_error(
                    Value::Null,
                    JSONRPC_PARSE_ERROR,
                    &e.to_string(),
                )),
            )
                .into_response();
        }
    };
    // Notificacao e resposta do cliente nao tem resposta: 202 sem corpo, como o transporte manda.
    if v.get("id").is_none() && v.get("method").is_some() {
        return StatusCode::ACCEPTED.into_response();
    }
    let sessao = sessao_de(&h);
    let agente = match (s.factory)(&s.default_model) {
        Ok(a) => a,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(jsonrpc_error(
                    v.get("id").cloned().unwrap_or(Value::Null),
                    JSONRPC_INVALID_REQUEST,
                    &e,
                )),
            )
                .into_response();
        }
    };
    let workdir = s.store.workdir(&sessao);
    let ledger = match std::fs::create_dir_all(&workdir)
        .map_err(|e| e.to_string())
        .and_then(|_| {
            EvidenceLedger::open(s.store.evidence_path(&sessao)).map_err(|e| e.to_string())
        }) {
        Ok(l) => l,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(json!({"error": e})),
            )
                .into_response();
        }
    };
    let ctx = ToolContext {
        task_id: sessao.clone(),
        workdir,
        timeout: agente.config.tool_timeout,
    };
    let resposta = responder(&agente, &ctx, &ledger, v).await;
    // O que a ferramenta segurou para a chamada (sessao de navegador, processo MCP filho)
    // se solta aqui: nao ha fim de fio para solta-lo depois.
    for t in &agente.tools {
        t.finish(&sessao).await;
    }
    let corpo = resposta
        .unwrap_or_else(|| jsonrpc_error(Value::Null, JSONRPC_INVALID_REQUEST, "nao e JSON-RPC"));
    (
        StatusCode::OK,
        [(SESSAO, sessao.clone())],
        axum::Json(corpo),
    )
        .into_response()
}

/// Uma linha de JSON-RPC lida com teto: o `mcp-serve` e o `acp` leem o fio pela MESMA
/// funcao, para o teto e a resposta ao estouro nunca divergirem entre os dois.
pub(crate) enum Linha {
    Fim,
    /// Passou do teto; carrega o erro JSON-RPC a mandar antes de fechar. Sem o fim da
    /// linha nao ha como achar o comeco da proxima: quem le responde e fecha.
    Grande(Value),
    Bytes(Vec<u8>),
}

pub(crate) async fn ler_linha<R: AsyncBufRead + Unpin>(entrada: &mut R) -> std::io::Result<Linha> {
    let mut linha = Vec::new();
    let lidos = (&mut *entrada)
        .take(DEFAULT_MAX_FRAME_BYTES as u64 + 1)
        .read_until(b'\n', &mut linha)
        .await?;
    if lidos == 0 {
        return Ok(Linha::Fim);
    }
    if linha.len() > DEFAULT_MAX_FRAME_BYTES {
        return Ok(Linha::Grande(jsonrpc_error(
            Value::Null,
            JSONRPC_INVALID_REQUEST,
            "mensagem grande demais",
        )));
    }
    Ok(Linha::Bytes(linha))
}

async fn responder(
    agente: &Agent,
    ctx: &ToolContext,
    ledger: &EvidenceLedger,
    v: Value,
) -> Option<Value> {
    let Some(metodo) = v.get("method").and_then(Value::as_str) else {
        // Resposta do cliente a algo que nao pedimos, ou lixo: nada a responder.
        return (!v.is_object() || v.get("jsonrpc").is_none())
            .then(|| jsonrpc_error(Value::Null, JSONRPC_INVALID_REQUEST, "nao e JSON-RPC"));
    };
    // Notificacao (sem id) nao tem resposta, nem a de erro.
    let id = v.get("id")?.clone();
    let params = v.get("params").cloned().unwrap_or_else(|| json!({}));
    let r = match metodo {
        "initialize" => json!({
            "protocolVersion": negotiate_server_version(
                params.get("protocolVersion").and_then(Value::as_str)
            ),
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": {"name": CLIENTE, "version": VERSAO},
        }),
        "ping" => json!({}),
        "tools/list" => {
            let lista: Vec<McpToolDescriptor> = agente
                .visible_specs()
                .into_iter()
                .map(|s| McpToolDescriptor {
                    name: s.name,
                    description: Some(s.description),
                    input_schema: s.parameters,
                })
                .collect();
            tools_list_result(&lista)
        }
        "tools/call" => {
            let Some(nome) = params.get("name").and_then(Value::as_str) else {
                return Some(jsonrpc_error(id, JSONRPC_INVALID_PARAMS, "falta 'name'"));
            };
            let chamada = ToolCall {
                id: id.to_string(),
                name: nome.to_string(),
                arguments: params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({})),
            };
            let (texto, desfecho, _) = agente.call_tool(&chamada, ctx, ledger, &ctx.task_id).await;
            tool_call_result(
                &truncate_for_model(&texto, agente.config.max_tool_output_chars),
                desfecho != "ok",
            )
        }
        outro => {
            return Some(jsonrpc_error(
                id,
                JSONRPC_METHOD_NOT_FOUND,
                &format!("metodo desconhecido: {outro}"),
            ));
        }
    };
    Some(jsonrpc_result(id, r))
}

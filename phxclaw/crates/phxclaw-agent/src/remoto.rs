//! Controle remoto: o celular (ou qualquer navegador) fala com uma PONTE, e o agente fala
//! com a ponte por uma conexao de SAIDA. O agente nao abre porta para a rede.
//!
//! Por que assim, e o que se reaproveita:
//! - **A ponte e o servidor de dispositivos** (`ServidorDispositivos`, WSS com TLS) e o
//!   agente e um NO dela: pareia uma vez por token, entra com chave Ed25519, cada mensagem
//!   assinada, cerca por sessao, e o pedido do cliente vira `device.command` com a
//!   capacidade `agent.http`, aprovada pelo operador no pareamento. Nada disso foi escrito
//!   de novo aqui; o laco do no e o `no::atender` do binario `phxclaw-device-node`.
//! - **O agente executa o pedido no PROPRIO router** (`oneshot` em processo): a mesma API
//!   do `servir`, com a mesma conferencia, sem porta local nenhuma no caminho.
//! - **O token da API nunca sai do agente.** O cliente se autentica na ponte com o token
//!   DELA; a ponte tira o cabecalho e o agente poe o proprio token ao executar. Alem de
//!   nao expor o segredo, e o que a regra dos comandos exige: segredo cru em argumento de
//!   comando e recusado pela autorizacao do dominio.
//! - **So as rotas de tarefa, do tunel e da sincronizacao passam** (`permitido`),
//!   conferidas nos DOIS lados pela mesma funcao: na ponte para recusar cedo, no agente
//!   porque e ele quem obedece. O tunel (`tunel.rs`: terminal e LSP do projeto) e a
//!   sincronizacao do `config.json` (`sincronizar.rs`) viajam no MESMO canal multiplexado
//!   das tarefas, um `device.command` por pedido, com a mesma autenticacao.

use crate::pwa;
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use phxclaw_device_nodes::DeviceCapability;
use phxclaw_device_nodes::DeviceCommand;
use phxclaw_device_transport::servidor::{
    FalhaDeComando, ResultadoDeComando, ServidorDispositivos,
};
use phxclaw_device_transport::{
    DevicePlatform, EnrollmentRequest, NodeHello, NodeIdentity, WssDeviceClient, parear,
};
use phxclaw_key_provider::{ArquivoKeyProvider, KeyProvider};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
pub use uuid::Uuid;

/// A capacidade que o agente declara e o operador aprova no pareamento.
pub const CAPACIDADE_HTTP: &str = "agent.http";
/// Quanto a ponte espera a resposta do agente.
pub const PRAZO_DO_PEDIDO: Duration = Duration::from_secs(60);
/// Teto do corpo que atravessa a ponte, nos dois sentidos.
pub const TETO_DO_CORPO: usize = 8 * 1024 * 1024;

/// As rotas da API que o controle remoto alcanca: criar, listar e acompanhar tarefa,
/// aprovar plano, responder pergunta, cancelar e baixar artefato; o terminal e o LSP do
/// projeto (`tunel.rs`); e o retrato/gravacao do `config.json` para a sincronizacao
/// (`sincronizar.rs`). Agenda, sites e o resto ficam de fora: controle remoto e
/// acompanhar, decidir e editar, nao administrar o agente.
pub fn permitido(metodo: &str, caminho: &str) -> bool {
    if caminho.contains('?') || caminho.contains("//") {
        return false;
    }
    let p: Vec<&str> = caminho.trim_start_matches('/').split('/').collect();
    if p.iter().any(|s| s.is_empty() || *s == "." || *s == "..") {
        return false;
    }
    matches!(
        (metodo, p.as_slice()),
        ("GET" | "POST", ["v1", "tasks"])
            | ("GET", ["v1", "tasks", _])
            | (
                "POST",
                ["v1", "tasks", _, "approve" | "cancel" | "answer" | "plan"]
            )
            | ("GET", ["v1", "tasks", _, "artifacts", _, ..])
            | ("POST", ["v1", "tunel", "terminal" | "lsp"])
            | ("GET" | "PUT", ["v1", "config", "sincronizar"])
    )
}

// ------------------------------------------------------------------ lado do agente

/// Executa no router do agente o pedido que chegou pela ponte e devolve a resposta
/// como a ponte a remonta: `{status, tipo, corpo_b64}`.
pub async fn executar_pedido(app: Router, token_api: &str, args: &Value) -> Result<Value, String> {
    let metodo = args["metodo"].as_str().ok_or("falta 'metodo'")?;
    let caminho = args["caminho"].as_str().ok_or("falta 'caminho'")?;
    if !permitido(metodo, caminho) {
        return Err(format!("rota fora do controle remoto: {metodo} {caminho}"));
    }
    let corpo = args["corpo"].as_str().unwrap_or("").to_string();
    let pedido = axum::http::Request::builder()
        .method(metodo)
        .uri(caminho)
        .header(header::AUTHORIZATION, format!("Bearer {token_api}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(corpo))
        .map_err(|e| e.to_string())?;
    let r = app.oneshot(pedido).await.map_err(|e| e.to_string())?;
    let status = r.status().as_u16();
    let tipo = r
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    let corpo = axum::body::to_bytes(r.into_body(), TETO_DO_CORPO)
        .await
        .map_err(|e| format!("resposta acima de {TETO_DO_CORPO} bytes: {e}"))?;
    Ok(json!({"status": status, "tipo": tipo, "corpo_b64": B64.encode(&corpo)}))
}

/// Como o agente se liga a ponte.
pub struct ConfigDaPonte {
    /// `wss://host:porta/`
    pub url: String,
    /// CA da ponte (PEM); sem ela valem as raizes publicas.
    pub ca_pem: Option<Vec<u8>>,
    pub tenant: Uuid,
    pub no: Uuid,
    /// Token de pareamento, so no primeiro uso.
    pub token_pareamento: Option<String>,
    /// Onde a chave Ed25519 do no fica (arquivo 0600).
    pub pasta_da_chave: PathBuf,
}

fn hello(tenant: Uuid) -> NodeHello {
    NodeHello {
        tenant_uuid: tenant,
        platform: DevicePlatform::Linux,
        agent_version: env!("CARGO_PKG_VERSION").into(),
        capabilities: ["device.heartbeat", CAPACIDADE_HTTP]
            .iter()
            .map(|n| DeviceCapability {
                name: n.to_string(),
                version: "1".into(),
            })
            .collect(),
    }
}

async fn conectar(cfg: &ConfigDaPonte) -> Result<WssDeviceClient, String> {
    match &cfg.ca_pem {
        Some(ca) => WssDeviceClient::connect_with_ca(&cfg.url, ca).await,
        None => WssDeviceClient::connect(&cfg.url).await,
    }
    .map_err(|e| e.to_string())
}

/// Uma sessao com a ponte: conecta, pareia ou carrega a chave, e atende ate cair.
pub async fn sessao_com_a_ponte(
    cfg: &ConfigDaPonte,
    app: Router,
    token_api: Arc<str>,
) -> Result<(), String> {
    let chaveiro = ArquivoKeyProvider::new(&cfg.pasta_da_chave).map_err(|e| e.to_string())?;
    let mut cliente = conectar(cfg).await?;
    let h = hello(cfg.tenant);
    // O token e de uso unico: com a chave ja guardada, religar e so com ela.
    let (identidade, seq) = match (NodeIdentity::load(&chaveiro, cfg.no), &cfg.token_pareamento) {
        (Ok(id), _) => (id, 0),
        (Err(_), Some(token)) => {
            let pedido = EnrollmentRequest {
                tenant_uuid: cfg.tenant,
                node_uuid: cfg.no,
                display_name: "phxclaw-agente".into(),
                platform: DevicePlatform::Linux,
                agent_version: env!("CARGO_PKG_VERSION").into(),
                public_key_ed25519_b64: String::new(),
                enrollment_token: token.clone(),
                capabilities: h.capabilities.clone(),
            };
            let id = parear(&mut cliente, &chaveiro as &dyn KeyProvider, pedido)
                .await
                .map_err(|e| format!("pareamento: {e}"))?;
            (id, 1)
        }
        (Err(e), None) => {
            return Err(format!(
                "sem chave do no em {} ({e}) e sem PHXCLAW_ENROLLMENT_TOKEN para parear",
                cfg.pasta_da_chave.display()
            ));
        }
    };
    phxclaw_device_transport::no::atender(
        cliente,
        &identidade,
        &h,
        seq,
        |b| eprintln!("ponte: sessao {} aberta", b.session_uuid),
        move |cmd: DeviceCommand| {
            let (app, token) = (app.clone(), token_api.clone());
            async move {
                let r = if cmd.capability == CAPACIDADE_HTTP {
                    executar_pedido(app, &token, &cmd.arguments).await
                } else {
                    Err(format!("{} nao implementada no agente", cmd.capability))
                };
                match r {
                    Ok(saida) => ResultadoDeComando {
                        command_uuid: cmd.command_uuid,
                        ok: true,
                        saida,
                        erro: None,
                    },
                    Err(e) => ResultadoDeComando {
                        command_uuid: cmd.command_uuid,
                        ok: false,
                        saida: Value::Null,
                        erro: Some(e),
                    },
                }
            }
        },
    )
    .await
    .map_err(|e| e.to_string())
}

/// Liga e religa para sempre, com espera crescente: ponte fora do ar nao derruba o
/// agente, e o celular volta a alcanca-lo quando ela voltar.
pub async fn ligar_a_ponte(cfg: ConfigDaPonte, app: Router, token_api: String) {
    let token: Arc<str> = token_api.into();
    let mut espera = Duration::from_secs(1);
    loop {
        match sessao_com_a_ponte(&cfg, app.clone(), token.clone()).await {
            Ok(()) => espera = Duration::from_secs(1),
            Err(e) => eprintln!("ponte: {e}; religando em {} s", espera.as_secs()),
        }
        tokio::time::sleep(espera).await;
        espera = (espera * 2).min(Duration::from_secs(30));
    }
}

// ------------------------------------------------------------------ lado da ponte

#[derive(Clone)]
struct Ponte {
    srv: Arc<ServidorDispositivos>,
    token_cliente: Arc<str>,
}

fn recusa(status: StatusCode, m: impl Into<String>) -> Response {
    (status, axum::Json(json!({"error": m.into()}))).into_response()
}

async fn rele(
    State(p): State<Ponte>,
    metodo: Method,
    uri: Uri,
    h: HeaderMap,
    corpo: Bytes,
) -> Response {
    if !phxclaw_api_gateway::authorized(&h, &p.token_cliente) {
        return recusa(
            StatusCode::UNAUTHORIZED,
            "token da ponte ausente ou invalido",
        );
    }
    let caminho = uri.path();
    if uri.query().is_some() || !permitido(metodo.as_str(), caminho) {
        return recusa(StatusCode::FORBIDDEN, "rota fora do controle remoto");
    }
    if corpo.len() > TETO_DO_CORPO {
        return recusa(StatusCode::PAYLOAD_TOO_LARGE, "corpo grande demais");
    }
    let Ok(corpo) = String::from_utf8(corpo.to_vec()) else {
        return recusa(StatusCode::BAD_REQUEST, "corpo nao e texto");
    };
    // O agente da ponte: o no conectado que o operador aprovou para `agent.http`.
    let Some(no) = p
        .srv
        .nos()
        .into_iter()
        .find(|n| n.conectado && n.aprovadas.contains(CAPACIDADE_HTTP))
    else {
        return recusa(
            StatusCode::SERVICE_UNAVAILABLE,
            "nenhum agente ligado a ponte",
        );
    };
    let args = json!({"metodo": metodo.as_str(), "caminho": caminho, "corpo": corpo});
    match p
        .srv
        .comandar(
            no.no.tenant_uuid,
            no.no.node_uuid,
            CAPACIDADE_HTTP,
            args,
            PRAZO_DO_PEDIDO,
        )
        .await
    {
        Ok(r) if r.ok => remontar(&r.saida),
        Ok(r) => recusa(StatusCode::BAD_GATEWAY, r.erro.unwrap_or_default()),
        Err(FalhaDeComando::Negado(m)) => recusa(StatusCode::FORBIDDEN, m),
        Err(FalhaDeComando::Falhou(m)) => recusa(StatusCode::BAD_GATEWAY, m),
    }
}

fn remontar(saida: &Value) -> Response {
    let status = saida["status"]
        .as_u64()
        .and_then(|s| StatusCode::from_u16(s as u16).ok())
        .unwrap_or(StatusCode::BAD_GATEWAY);
    let Ok(corpo) = B64.decode(saida["corpo_b64"].as_str().unwrap_or("")) else {
        return recusa(StatusCode::BAD_GATEWAY, "resposta do agente ilegivel");
    };
    let tipo = saida["tipo"]
        .as_str()
        .unwrap_or("application/octet-stream")
        .to_string();
    (status, [(header::CONTENT_TYPE, tipo)], corpo).into_response()
}

/// O HTTP da ponte: a mesma tela do `servir` e o rele das rotas de tarefa.
pub fn rotas_da_ponte(srv: Arc<ServidorDispositivos>, token_cliente: String) -> Router {
    Router::new()
        .route("/v1/{*resto}", any(rele))
        .merge(pwa::rotas())
        // O mesmo ponto do `servir`: a tela pela ponte sai com a mesma CSP.
        .layer(axum::middleware::from_fn(pwa::blindar))
        .with_state(Ponte {
            srv,
            token_cliente: token_cliente.into(),
        })
}

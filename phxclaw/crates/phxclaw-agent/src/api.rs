//! API HTTP de tarefas do agente: criar, acompanhar, aprovar plano, cancelar, baixar
//! artefato, agendar. E o equivalente da API de tarefas do Manus, rodando local.
//!
//! Decisoes que valem saber:
//! - o Bearer e conferido pela MESMA funcao do api-gateway (tempo constante);
//! - so ouve em loopback por padrao; expor e decisao do operador;
//! - o webhook de fim de tarefa sai pelo EgressBroker, com lista de origens: um webhook
//!   livre seria uma porta de SSRF controlada por quem cria a tarefa;
//! - artefato so se serve de dentro da pasta da tarefa (mesmo `confine` das ferramentas);
//! - criar tarefa passa por um balde de fichas: cada tarefa gasta modelo, e um laco de
//!   cliente com defeito esvaziaria a cota do provedor. Consulta nao gasta ficha, porque
//!   acompanhar tarefa e sondagem legitima.

use crate::agenda::Agenda;
use crate::motor::{Agent, CancelFlag, NoObserver};
use crate::tarefa::{Task, TaskStatus, TaskStore, confine};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use phxclaw_egress_broker::{EgressBroker, EgressPolicy};
use phxclaw_http_client::HttpRequestSpec;
use phxclaw_rustclaw_native::ScheduleSpec;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Monta o agente de uma tarefa a partir do modelo pedido ("ollama:qwen2.5:1.5b", ...).
pub type AgentFactory = Arc<dyn Fn(&str) -> Result<Agent, String> + Send + Sync>;

#[derive(Clone)]
pub struct ApiState {
    pub store: TaskStore,
    pub factory: AgentFactory,
    pub default_model: String,
    pub token: String,
    pub running: Arc<Mutex<HashMap<String, CancelFlag>>>,
    pub agenda: Arc<Mutex<Agenda>>,
    /// Origens para onde webhook pode ir. Vazio = webhook recusado.
    pub webhook_origins: Vec<String>,
    /// Teto de criacao de tarefas (balde de fichas).
    pub limite: Arc<Limite>,
}

/// Balde de fichas: `capacidade` de rajada, reposto a `por_minuto`.
pub struct Limite {
    capacidade: f64,
    por_seg: f64,
    estado: Mutex<(f64, Instant)>,
}

impl Limite {
    pub fn por_minuto(n: u32) -> Self {
        let n = f64::from(n.max(1));
        Self {
            capacidade: n,
            por_seg: n / 60.0,
            estado: Mutex::new((n, Instant::now())),
        }
    }

    /// Gasta uma ficha, ou diz em quantos segundos a proxima chega.
    pub fn tomar(&self) -> Result<(), u64> {
        let mut e = self.estado.lock().unwrap_or_else(|p| p.into_inner());
        let agora = Instant::now();
        let fichas =
            (e.0 + agora.duration_since(e.1).as_secs_f64() * self.por_seg).min(self.capacidade);
        if fichas >= 1.0 {
            *e = (fichas - 1.0, agora);
            Ok(())
        } else {
            *e = (fichas, agora);
            Err(((1.0 - fichas) / self.por_seg).ceil().max(1.0) as u64)
        }
    }
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(|| async { Json(json!({"ok": true})) }))
        .route("/v1/tasks", post(criar).get(listar))
        .route("/v1/tasks/{id}", get(obter))
        .route("/v1/tasks/{id}/plan", post(editar_plano))
        .route("/v1/tasks/{id}/approve", post(aprovar))
        .route("/v1/tasks/{id}/cancel", post(cancelar))
        .route("/v1/tasks/{id}/answer", post(responder))
        .route("/v1/tasks/{id}/artifacts/{*path}", get(artefato))
        .route("/v1/schedules", post(agendar).get(agenda))
        .route("/sites/{id}/{*path}", get(site))
        .merge(crate::canvas::rotas())
        // A tela instalavel (PWA): a mesma interface no navegador, no celular e na ponte.
        .merge(crate::pwa::rotas())
        .with_state(state)
}

type Resp = Result<Response, (StatusCode, Json<Value>)>;

fn erro(code: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<Value>) {
    let corpo = ErroDaApi {
        error: msg.into(),
        retry_after: None,
    };
    (code, Json(json!(corpo)))
}

fn auth(s: &ApiState, h: &HeaderMap) -> Result<(), (StatusCode, Json<Value>)> {
    if phxclaw_api_gateway::authorized(h, &s.token) {
        Ok(())
    } else {
        Err(erro(StatusCode::UNAUTHORIZED, "token ausente ou invalido"))
    }
}

// Os corpos de fio sao os do contrato comum: o SDK serializa o mesmo tipo que a rota le.
pub use phxclaw_agent_core::tarefa::{ErroDaApi, NovaTarefa, TarefaCriada, TaskSummary};

/// Recusa de criacao, com o codigo HTTP que a rota devolve. O canal de mensagens recebe a
/// mesma recusa e a traduz em resposta ao chat.
#[derive(Debug)]
pub struct Recusa {
    pub status: StatusCode,
    pub erro: String,
    /// Segundos ate a proxima ficha, quando a recusa e o limite de criacao.
    pub retry_after: Option<u64>,
}

/// Tarefa criada e ja gravada: `fim` entrega o estado final da execucao (ou, no Plan Mode,
/// a tarefa esperando aprovacao).
pub struct Criada {
    pub id: String,
    pub fim: tokio::task::JoinHandle<Task>,
}

async fn criar(State(s): State<ApiState>, h: HeaderMap, Json(n): Json<NovaTarefa>) -> Resp {
    auth(&s, &h)?;
    match criar_tarefa(&s, n) {
        Ok(c) => Ok((StatusCode::ACCEPTED, Json(TarefaCriada { id: c.id })).into_response()),
        Err(Recusa {
            retry_after: Some(seg),
            erro: e,
            ..
        }) => Ok((
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, seg.to_string())],
            Json(ErroDaApi {
                error: e,
                retry_after: Some(seg),
            }),
        )
            .into_response()),
        Err(r) => Err(erro(r.status, r.erro)),
    }
}

/// O UNICO caminho de criar tarefa: a rota HTTP e os canais de mensagem passam por aqui,
/// para a validacao, o modelo padrao e o balde de fichas nao divergirem entre as portas.
pub fn criar_tarefa(s: &ApiState, n: NovaTarefa) -> Result<Criada, Recusa> {
    criar_tarefa_com(s, n, |_| Ok(()))
}

/// `criar_tarefa` com um `preparo` da pasta de trabalho, rodado depois de a tarefa ser
/// gravada e ANTES de a execucao comecar: o gatilho de arquivo copia o arquivo mudado para
/// la, e o agente o encontra no primeiro passo em vez de correr contra a copia.
pub fn criar_tarefa_com(
    s: &ApiState,
    n: NovaTarefa,
    preparo: impl FnOnce(&std::path::Path) -> std::io::Result<()>,
) -> Result<Criada, Recusa> {
    let recusa = |status, e: String| Recusa {
        status,
        erro: e,
        retry_after: None,
    };
    let objetivo = n.objective.trim();
    if objetivo.is_empty() || objetivo.len() > 20_000 {
        return Err(recusa(
            StatusCode::BAD_REQUEST,
            "objective vazio ou maior que 20000".into(),
        ));
    }
    if let Some(w) = &n.webhook {
        webhook_permitido(s, w).map_err(|e| recusa(StatusCode::BAD_REQUEST, e))?;
    }
    let modelo = n.model.unwrap_or_else(|| s.default_model.clone());
    let agente = (s.factory)(&modelo).map_err(|e| recusa(StatusCode::BAD_REQUEST, e))?;
    // Depois da validacao: pedido invalido nao gasta a cota de quem errou o campo.
    if let Err(seg) = s.limite.tomar() {
        return Err(Recusa {
            status: StatusCode::TOO_MANY_REQUESTS,
            erro: "limite de criacao de tarefas".into(),
            retry_after: Some(seg),
        });
    }
    let mut t = Task::new(objetivo, modelo);
    t.webhook = n.webhook;
    s.store
        .save(&t)
        .map_err(|e| recusa(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if let Err(e) = preparo(&s.store.workdir(&t.id)) {
        // A tarefa ja esta no disco: fica FALHA, dizendo por que, e nao `pending` para sempre.
        t.status = TaskStatus::Failed;
        t.error = Some(format!("preparo da pasta: {e}"));
        let _ = s.store.save(&t);
        return Err(recusa(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("preparo da pasta: {e}"),
        ));
    }
    let id = t.id.clone();
    let st = s.clone();
    let fim = if n.plan_first {
        tokio::spawn(async move {
            let mut t = t;
            match agente.plan(&mut t).await {
                Ok(()) => t.status = TaskStatus::AwaitingApproval,
                Err(e) => {
                    t.status = TaskStatus::Failed;
                    t.error = Some(format!("plano: {e}"));
                }
            }
            let _ = st.store.save(&t);
            t
        })
    } else {
        executar(st, agente, t)
    };
    Ok(Criada { id, fim })
}

/// Roda em segundo plano, registra o cancelamento e chama o webhook no fim.
pub fn executar(s: ApiState, agente: Agent, t: Task) -> tokio::task::JoinHandle<Task> {
    let cancel = CancelFlag::default();
    s.running
        .lock()
        .unwrap()
        .insert(t.id.clone(), cancel.clone());
    tokio::spawn(async move {
        let id = t.id.clone();
        let fim = agente.run(t, &cancel, &NoObserver).await;
        s.running.lock().unwrap().remove(&id);
        if let Some(w) = fim.webhook.clone() {
            chamar_webhook(&s, &w, &fim).await;
        }
        fim
    })
}

fn webhook_permitido(s: &ApiState, url: &str) -> Result<(), String> {
    broker_webhook(s)
        .validate_url(url)
        .map(|_| ())
        .map_err(|e| format!("webhook: {e}"))
}

fn broker_webhook(s: &ApiState) -> EgressBroker {
    let mut p = EgressPolicy {
        enabled: !s.webhook_origins.is_empty(),
        allow_http: s.webhook_origins.iter().any(|o| o.starts_with("http://")),
        ..EgressPolicy::default()
    };
    p.allowed_origins.extend(s.webhook_origins.iter().cloned());
    EgressBroker::new(p)
}

async fn chamar_webhook(s: &ApiState, url: &str, t: &Task) {
    let mut spec = HttpRequestSpec::get(url.to_string());
    spec.method = "POST".into();
    spec.json = Some(json!({"event": "task.finished", "task": resumo(t)}));
    spec.follow_redirects = false;
    let _ = broker_webhook(s).request(&spec).await;
}

fn resumo(t: &Task) -> Value {
    json!(TaskSummary::from(t))
}

async fn listar(State(s): State<ApiState>, h: HeaderMap) -> Resp {
    auth(&s, &h)?;
    let l = s
        .store
        .list()
        .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(l.iter().map(resumo).collect::<Vec<_>>()).into_response())
}

fn carregar(s: &ApiState, id: &str) -> Result<Task, (StatusCode, Json<Value>)> {
    s.store
        .load(id)
        .map_err(|_| erro(StatusCode::NOT_FOUND, "tarefa inexistente"))
}

async fn obter(State(s): State<ApiState>, h: HeaderMap, Path(id): Path<String>) -> Resp {
    auth(&s, &h)?;
    Ok(Json(carregar(&s, &id)?).into_response())
}

#[derive(Deserialize)]
struct Plano {
    steps: Vec<String>,
}

async fn editar_plano(
    State(s): State<ApiState>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(p): Json<Plano>,
) -> Resp {
    auth(&s, &h)?;
    let mut t = carregar(&s, &id)?;
    if t.status != TaskStatus::AwaitingApproval {
        return Err(erro(
            StatusCode::CONFLICT,
            "so se edita plano de tarefa esperando aprovacao",
        ));
    }
    t.plan = p
        .steps
        .into_iter()
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .take(20)
        .collect();
    t.updated_at = Utc::now();
    s.store
        .save(&t)
        .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(t).into_response())
}

async fn aprovar(State(s): State<ApiState>, h: HeaderMap, Path(id): Path<String>) -> Resp {
    auth(&s, &h)?;
    let t = carregar(&s, &id)?;
    if t.status != TaskStatus::AwaitingApproval {
        return Err(erro(
            StatusCode::CONFLICT,
            "tarefa nao esta esperando aprovacao",
        ));
    }
    let agente = (s.factory)(&t.model).map_err(|e| erro(StatusCode::BAD_REQUEST, e))?;
    executar(s.clone(), agente, t);
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({"id": id, "status": "running"})),
    )
        .into_response())
}

#[derive(Deserialize)]
struct RespostaDoUsuario {
    answer: String,
}

/// Resposta a uma pergunta da tarefa (`ask_user` ou comando que a regra manda perguntar).
/// A CLI entrega pela mesma `perguntas::responder`.
async fn responder(
    State(s): State<ApiState>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(r): Json<RespostaDoUsuario>,
) -> Resp {
    auth(&s, &h)?;
    let t = carregar(&s, &id)?;
    if t.status != TaskStatus::AwaitingInput {
        return Err(erro(
            StatusCode::CONFLICT,
            "tarefa nao esta esperando resposta",
        ));
    }
    crate::perguntas::responder(&t.id, &r.answer).map_err(|e| erro(StatusCode::CONFLICT, e))?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({"id": t.id, "status": "running"})),
    )
        .into_response())
}

async fn cancelar(State(s): State<ApiState>, h: HeaderMap, Path(id): Path<String>) -> Resp {
    auth(&s, &h)?;
    let mut t = carregar(&s, &id)?;
    if let Some(c) = s.running.lock().unwrap().get(&id) {
        c.cancel();
        return Ok((
            StatusCode::ACCEPTED,
            Json(json!({"id": id, "status": "cancelling"})),
        )
            .into_response());
    }
    if t.status.is_final() {
        return Err(erro(StatusCode::CONFLICT, "tarefa ja terminou"));
    }
    // plano esperando aprovacao: cancela direto
    t.status = TaskStatus::Cancelled;
    t.updated_at = Utc::now();
    let _ = s.store.save(&t);
    Ok(Json(json!({"id": id, "status": "cancelled"})).into_response())
}

async fn artefato(
    State(s): State<ApiState>,
    h: HeaderMap,
    Path((id, path)): Path<(String, String)>,
) -> Resp {
    auth(&s, &h)?;
    let t = carregar(&s, &id)?;
    let a = t
        .artifacts
        .iter()
        .find(|a| a.path == path)
        .ok_or_else(|| erro(StatusCode::NOT_FOUND, "artefato inexistente"))?;
    let alvo = confine(&s.store.workdir(&id), &path).map_err(|e| erro(StatusCode::FORBIDDEN, e))?;
    let bytes = std::fs::read(alvo).map_err(|e| erro(StatusCode::NOT_FOUND, e.to_string()))?;
    let nome = path
        .rsplit('/')
        .next()
        .unwrap_or("artefato")
        .replace('"', "");
    Ok((
        [
            (header::CONTENT_TYPE, a.media_type.clone()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{nome}\""),
            ),
            // o conteudo foi produzido pelo agente a partir da web: nunca executa no navegador
            (header::CONTENT_SECURITY_POLICY, "sandbox".to_string()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        bytes,
    )
        .into_response())
}

#[derive(Deserialize)]
struct NovoAgendamento {
    name: String,
    objective: String,
    /// Cron de 5 campos (UTC) ou `every_seconds` (>= 60).
    #[serde(default)]
    cron: Option<String>,
    #[serde(default)]
    every_seconds: Option<u64>,
}

async fn agendar(State(s): State<ApiState>, h: HeaderMap, Json(n): Json<NovoAgendamento>) -> Resp {
    auth(&s, &h)?;
    let spec = match (n.cron, n.every_seconds) {
        (Some(c), None) => ScheduleSpec::CronExpression(c),
        (None, Some(x)) => ScheduleSpec::EverySeconds(x),
        _ => {
            return Err(erro(
                StatusCode::BAD_REQUEST,
                "informe cron OU every_seconds",
            ));
        }
    };
    let item = s
        .agenda
        .lock()
        .unwrap()
        .add(&n.name, &n.objective, spec, Utc::now())
        .map_err(|e| erro(StatusCode::BAD_REQUEST, e))?;
    Ok((StatusCode::CREATED, Json(item)).into_response())
}

async fn agenda(State(s): State<ApiState>, h: HeaderMap) -> Resp {
    auth(&s, &h)?;
    Ok(Json(s.agenda.lock().unwrap().items.clone()).into_response())
}

/// Dispara o que venceu na agenda; o servidor chama isto periodicamente.
pub fn disparar_agenda(s: &ApiState) -> usize {
    let vencidos = s.agenda.lock().unwrap().due(Utc::now());
    let mut n = 0;
    for v in vencidos {
        if let Ok(agente) = (s.factory)(&s.default_model) {
            let t = Task::new(v.objective.clone(), s.default_model.clone());
            if let Some(item) = s
                .agenda
                .lock()
                .unwrap()
                .items
                .iter_mut()
                .find(|x| x.id == v.id)
            {
                item.last_task = Some(t.id.clone());
            }
            let _ = s.agenda.lock().unwrap().save();
            executar(s.clone(), agente, t);
            n += 1;
        }
    }
    n
}

/// Site publicado pelo agente. Sem Bearer: e para abrir no navegador. So serve pasta que a
/// ferramenta publish_site marcou (marca fora do alcance do shell), com CSP sandbox.
async fn site(State(s): State<ApiState>, Path((id, path)): Path<(String, String)>) -> Resp {
    let publicadas = crate::site::published(&s.store.dir(&id));
    let rel = path.trim_start_matches('/');
    let rel = if rel.ends_with('/') || rel.is_empty() {
        format!("{rel}index.html")
    } else {
        rel.to_string()
    };
    if !publicadas.iter().any(|p| rel.starts_with(&format!("{p}/"))) {
        return Err(erro(StatusCode::NOT_FOUND, "site nao publicado"));
    }
    let alvo = confine(&s.store.workdir(&id), &rel)
        .map_err(|_| erro(StatusCode::NOT_FOUND, "fora do site"))?;
    let bytes =
        std::fs::read(alvo).map_err(|_| erro(StatusCode::NOT_FOUND, "arquivo inexistente"))?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                crate::motor::media_type(&rel).to_string(),
            ),
            (
                header::CONTENT_SECURITY_POLICY,
                "sandbox allow-scripts allow-forms".to_string(),
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        bytes,
    )
        .into_response())
}

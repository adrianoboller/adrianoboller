//! Conexao CDP unica com o Chromium, em modo "flatten": todas as sessoes de
//! alvo (paginas, iframes fora de processo, workers) viajam no mesmo
//! WebSocket, distinguidas por `sessionId`. Um so leitor despacha respostas e
//! eventos; a interceptacao de rede mora aqui e nao na `Page` porque tem de
//! valer para alvos que a `Page` nunca viu (popup, iframe, service worker).

use crate::error::{BrowserError, Result, timeout_err};
use crate::policy::BrowserPolicy;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{Notify, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Requisicao que a politica recusou, guardada para o agente (e o teste)
/// enxergarem o que foi barrado em vez de so ver uma pagina de erro.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockedRequest {
    pub url: String,
    pub reason: String,
}

#[derive(Debug)]
pub(crate) struct Event {
    pub method: String,
    #[allow(dead_code)]
    pub params: Value,
}

pub(crate) struct Connection {
    out: mpsc::UnboundedSender<Message>,
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    listeners: Mutex<HashMap<String, mpsc::UnboundedSender<Event>>>,
    /// targetId -> sessionId das paginas ja preparadas (Fetch ligado).
    pages: Mutex<HashMap<String, String>>,
    pages_ready: Notify,
    blocked: Mutex<Vec<BlockedRequest>>,
    closed: AtomicBool,
    pub(crate) policy: BrowserPolicy,
    pub(crate) command_timeout: Duration,
}

/// Padrao unico de interceptacao: TODA requisicao, nao so documento. O pedido
/// original era so documento, mas `fetch()` e `<img>` da propria pagina para
/// 169.254.169.254 sao o mesmo SSRF por outra porta.
fn fetch_enable_params() -> Value {
    json!({ "patterns": [ { "urlPattern": "*", "requestStage": "Request" } ] })
}

fn auto_attach_params() -> Value {
    // waitForDebuggerOnStart: o alvo novo nasce pausado e so roda depois de
    // o Fetch estar ligado nele. Sem isso haveria uma janela em que o
    // primeiro pedido do alvo sairia sem passar pela politica.
    json!({ "autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true })
}

impl Connection {
    pub(crate) fn start(
        ws: Ws,
        policy: BrowserPolicy,
        command_timeout: Duration,
    ) -> (Arc<Self>, Vec<tokio::task::JoinHandle<()>>) {
        let (sink, stream) = ws.split();
        let (tx, rx) = mpsc::unbounded_channel();
        let conn = Arc::new(Self {
            out: tx,
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            listeners: Mutex::new(HashMap::new()),
            pages: Mutex::new(HashMap::new()),
            pages_ready: Notify::new(),
            blocked: Mutex::new(Vec::new()),
            closed: AtomicBool::new(false),
            policy,
            command_timeout,
        });
        let w = tokio::spawn(writer(sink, rx));
        let r = tokio::spawn(reader(conn.clone(), stream));
        (conn, vec![w, r])
    }

    pub(crate) async fn call(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> Result<Value> {
        self.call_timeout(method, params, session, self.command_timeout)
            .await
    }

    pub(crate) async fn call_timeout(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
        timeout: Duration,
    ) -> Result<Value> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(BrowserError::ConnectionClosed);
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let mut msg = json!({ "id": id, "method": method, "params": params });
        if let Some(s) = session {
            msg["sessionId"] = Value::String(s.to_string());
        }
        if self
            .out
            .send(Message::Text(msg.to_string().into()))
            .is_err()
        {
            self.pending.lock().unwrap().remove(&id);
            return Err(BrowserError::ConnectionClosed);
        }
        let resp = match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(v)) => v,
            Ok(Err(_)) => return Err(BrowserError::ConnectionClosed),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                return Err(timeout_err(method, timeout));
            }
        };
        if let Some(err) = resp.get("error") {
            return Err(BrowserError::Cdp {
                method: method.to_string(),
                message: err
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("erro sem mensagem")
                    .to_string(),
            });
        }
        Ok(resp.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Envio sem esperar resposta, para o `Drop` (que nao pode aguardar).
    pub(crate) fn send_nowait(&self, method: &str, params: Value, session: Option<&str>) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut msg = json!({ "id": id, "method": method, "params": params });
        if let Some(s) = session {
            msg["sessionId"] = Value::String(s.to_string());
        }
        let _ = self.out.send(Message::Text(msg.to_string().into()));
    }

    pub(crate) async fn enable_auto_attach_root(&self) -> Result<()> {
        self.call("Target.setAutoAttach", auto_attach_params(), None)
            .await
            .map(|_| ())
    }

    pub(crate) fn listen(&self, session: &str) -> mpsc::UnboundedReceiver<Event> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.listeners
            .lock()
            .unwrap()
            .insert(session.to_string(), tx);
        rx
    }

    pub(crate) fn unlisten(&self, session: &str) {
        self.listeners.lock().unwrap().remove(session);
    }

    /// Espera a sessao da pagina `target_id` ficar pronta (Fetch ligado).
    pub(crate) async fn wait_page_session(&self, target_id: &str) -> Result<String> {
        let prazo = self.command_timeout;
        let espera = async {
            loop {
                // O `Notified` nasce antes da consulta: assim um aviso que
                // chegue entre a consulta e o await nao se perde.
                let aviso = self.pages_ready.notified();
                if let Some(s) = self.pages.lock().unwrap().get(target_id) {
                    return Ok(s.clone());
                }
                if self.closed.load(Ordering::SeqCst) {
                    return Err(BrowserError::ConnectionClosed);
                }
                aviso.await;
            }
        };
        tokio::time::timeout(prazo, espera)
            .await
            .map_err(|_| timeout_err("preparar pagina", prazo))?
    }

    pub(crate) fn blocked(&self) -> Vec<BlockedRequest> {
        self.blocked.lock().unwrap().clone()
    }

    pub(crate) fn blocked_len(&self) -> usize {
        self.blocked.lock().unwrap().len()
    }

    pub(crate) fn blocked_since(&self, from: usize) -> Vec<BlockedRequest> {
        self.blocked
            .lock()
            .unwrap()
            .get(from..)
            .map(<[_]>::to_vec)
            .unwrap_or_default()
    }

    fn record_block(&self, url: &str, reason: &str) {
        self.blocked.lock().unwrap().push(BlockedRequest {
            url: url.to_string(),
            reason: reason.to_string(),
        });
    }

    fn shutdown(&self) {
        self.closed.store(true, Ordering::SeqCst);
        // Largar os remetentes acorda quem espera resposta com erro de
        // conexao fechada, em vez de deixar cada um esgotar o proprio prazo.
        self.pending.lock().unwrap().clear();
        self.listeners.lock().unwrap().clear();
        self.pages_ready.notify_waiters();
    }
}

async fn writer(mut sink: SplitSink<Ws, Message>, mut rx: mpsc::UnboundedReceiver<Message>) {
    while let Some(m) = rx.recv().await {
        if sink.send(m).await.is_err() {
            break;
        }
    }
}

async fn reader(conn: Arc<Connection>, mut stream: SplitStream<Ws>) {
    while let Some(msg) = stream.next().await {
        let texto = match msg {
            Ok(Message::Text(t)) => t.to_string(),
            Ok(Message::Binary(b)) => String::from_utf8_lossy(&b).into_owned(),
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => continue,
        };
        let Ok(v) = serde_json::from_str::<Value>(&texto) else {
            continue;
        };
        if let Some(id) = v.get("id").and_then(Value::as_u64) {
            let tx = conn.pending.lock().unwrap().remove(&id);
            if let Some(tx) = tx {
                let _ = tx.send(v);
            }
            continue;
        }
        let method = v
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let session = v
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_string);
        let params = v.get("params").cloned().unwrap_or(Value::Null);
        match method.as_str() {
            // As duas decisoes de politica rodam em tarefa propria: a
            // resolucao de DNS nao pode travar o leitor, senao as respostas
            // de todas as outras sessoes esperariam por ela.
            "Target.attachedToTarget" => {
                tokio::spawn(prepare_session(conn.clone(), params));
            }
            "Fetch.requestPaused" => {
                tokio::spawn(decide_request(conn.clone(), session, params));
            }
            "Target.detachedFromTarget" => {
                if let Some(s) = params.get("sessionId").and_then(Value::as_str) {
                    conn.unlisten(s);
                }
            }
            _ => {
                if let Some(s) = session {
                    let l = conn.listeners.lock().unwrap().get(&s).cloned();
                    if let Some(l) = l {
                        let _ = l.send(Event { method, params });
                    }
                }
            }
        }
    }
    conn.shutdown();
}

async fn prepare_session(conn: Arc<Connection>, params: Value) {
    let Some(session) = params.get("sessionId").and_then(Value::as_str) else {
        return;
    };
    let info = params.get("targetInfo").cloned().unwrap_or(Value::Null);
    let tipo = info.get("type").and_then(Value::as_str).unwrap_or_default();
    let target_id = info
        .get("targetId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let abridor = info
        .get("openerId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());

    if tipo == "page" && abridor.is_some() {
        // Popup aberto pela pagina: fora do controle do agente, e a `Page`
        // dele nunca o veria. Fecha-se em vez de deixa-lo navegar sozinho.
        let url = info.get("url").and_then(Value::as_str).unwrap_or_default();
        conn.record_block(url, "janela nova (popup) aberta pela pagina");
        let _ = conn
            .call("Target.closeTarget", json!({ "targetId": target_id }), None)
            .await;
        return;
    }

    let fetch = conn
        .call("Fetch.enable", fetch_enable_params(), Some(session))
        .await;
    let _ = conn
        .call("Target.setAutoAttach", auto_attach_params(), Some(session))
        .await;
    if fetch.is_err() && tipo != "worker" {
        // Falha fechada: alvo onde a politica nao pode ser imposta nao roda.
        // Worker dedicado e a excecao porque a rede dele passa pelo quadro
        // que o criou, e esse ja esta interceptado.
        conn.record_block(
            info.get("url").and_then(Value::as_str).unwrap_or_default(),
            "alvo sem interceptacao de rede",
        );
        let _ = conn
            .call("Target.closeTarget", json!({ "targetId": target_id }), None)
            .await;
        return;
    }
    let _ = conn
        .call("Runtime.runIfWaitingForDebugger", json!({}), Some(session))
        .await;
    if tipo == "page" {
        conn.pages
            .lock()
            .unwrap()
            .insert(target_id.to_string(), session.to_string());
        conn.pages_ready.notify_waiters();
    }
}

async fn decide_request(conn: Arc<Connection>, session: Option<String>, params: Value) {
    let Some(request_id) = params.get("requestId").and_then(Value::as_str) else {
        return;
    };
    let url = params
        .pointer("/request/url")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let decisao = conn.policy.check_url(url).await;
    let (method, body) = match decisao {
        Ok(_) => ("Fetch.continueRequest", json!({ "requestId": request_id })),
        Err(e) => {
            let motivo = match e {
                BrowserError::PolicyDenied { reason, .. } => reason,
                outro => outro.to_string(),
            };
            // Registra ANTES de falhar a requisicao: quem aguarda a resposta
            // do Page.navigate ja encontra o motivo quando ela chegar.
            conn.record_block(url, &motivo);
            (
                "Fetch.failRequest",
                json!({ "requestId": request_id, "errorReason": "BlockedByClient" }),
            )
        }
    };
    let _ = conn.call(method, body, session.as_deref()).await;
}

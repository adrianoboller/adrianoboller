use axum::{
    Json, Router,
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use phxclaw_live_bus::LiveEventHub;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use thiserror::Error;
use tokio::{net::TcpListener, sync::oneshot};
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiGatewayConfig {
    pub bind_ip: IpAddr,
    pub port: u16,
    pub bearer_token: String,
    pub allow_publish: bool,
    pub allow_host_control_topics: bool,
    pub replay_limit: usize,
}

impl Default for ApiGatewayConfig {
    fn default() -> Self {
        Self {
            bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 48_187,
            bearer_token: generate_bearer_token(),
            allow_publish: true,
            allow_host_control_topics: false,
            replay_limit: 500,
        }
    }
}

#[derive(Clone)]
struct AppState {
    hub: LiveEventHub,
    config: ApiGatewayConfig,
    started_at: DateTime<Utc>,
    build_version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiServerInfo {
    pub addr: SocketAddr,
    pub started_at: DateTime<Utc>,
    pub build_version: String,
}

pub struct ApiServerHandle {
    pub info: ApiServerInfo,
    shutdown: Option<oneshot::Sender<()>>,
}

impl ApiServerHandle {
    pub fn shutdown(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

#[derive(Debug, Error)]
pub enum ApiGatewayError {
    #[error("API gateway refuses non-loopback bind address: {0}")]
    NonLoopback(IpAddr),
    #[error("failed to bind API gateway: {0}")]
    Bind(#[from] std::io::Error),
}

#[derive(Debug, Deserialize)]
struct SnapshotQuery {
    limit: Option<usize>,
    after: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct PublishRequest {
    pub topic: String,
    pub event_type: String,
    #[serde(default)]
    pub payload: Value,
    pub correlation_uuid: Option<Uuid>,
    pub causation_uuid: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct ApiErrorBody {
    error: &'static str,
    message: String,
}

pub async fn start(
    hub: LiveEventHub,
    config: ApiGatewayConfig,
    build_version: impl Into<String>,
) -> Result<ApiServerHandle, ApiGatewayError> {
    if !config.bind_ip.is_loopback() {
        return Err(ApiGatewayError::NonLoopback(config.bind_ip));
    }

    let requested = SocketAddr::new(config.bind_ip, config.port);
    let listener = TcpListener::bind(requested).await?;
    let addr = listener.local_addr()?;
    let started_at = Utc::now();
    let build_version = build_version.into();
    let state = Arc::new(AppState {
        hub,
        config,
        started_at,
        build_version: build_version.clone(),
    });

    let app = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/events", get(snapshot))
        .route("/v1/events/sse", get(sse))
        .route("/v1/events/ws", get(ws))
        .route("/v1/events/publish", post(publish))
        .with_state(state);

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        if let Err(error) = server.await {
            eprintln!("PhxClaw API gateway stopped with error: {error}");
        }
    });

    Ok(ApiServerHandle {
        info: ApiServerInfo {
            addr,
            started_at,
            build_version,
        },
        shutdown: Some(shutdown_tx),
    })
}

async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let stats = state.hub.stats().ok();
    Json(json!({
        "service": "phxclaw-api-gateway",
        "status": "healthy",
        "version": state.build_version,
        "started_at": state.started_at,
        "live_bus": stats,
        "transport": ["tauri-events", "sse", "websocket"],
        "bind_policy": "loopback_only"
    }))
}

async fn snapshot(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<SnapshotQuery>,
) -> Response {
    if !authorized(&headers, &state.config.bearer_token) {
        return unauthorized();
    }
    let limit = query
        .limit
        .unwrap_or(100)
        .min(state.config.replay_limit.max(1));
    let result = match query.after {
        Some(after) => state.hub.since(after, limit),
        None => state.hub.snapshot(limit),
    };
    match result {
        Ok(events) => Json(json!({"events": events})).into_response(),
        Err(error) => internal(error.to_string()),
    }
}

async fn sse(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.config.bearer_token) {
        return unauthorized();
    }
    let rx = state.hub.subscribe();
    let stream = BroadcastStream::new(rx).map(|item| match item {
        Ok(event) => Ok::<Event, Infallible>(
            Event::default()
                .id(event.uuid.to_string())
                .event(format!("{}:{}", event.topic, event.event_type))
                .json_data(event)
                .unwrap_or_else(|_| Event::default().event("serialization_error").data("{}")),
        ),
        Err(BroadcastStreamRecvError::Lagged(dropped)) => Ok(Event::default()
            .event("stream_lagged")
            .json_data(json!({"dropped": dropped}))
            .unwrap_or_else(|_| Event::default().event("stream_lagged").data("{}"))),
    });
    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("phxclaw"),
        )
        .into_response()
}

async fn ws(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !authorized(&headers, &state.config.bearer_token) {
        return unauthorized();
    }
    upgrade
        .on_upgrade(move |socket| websocket_session(socket, state))
        .into_response()
}

async fn websocket_session(mut socket: WebSocket, state: Arc<AppState>) {
    let replay = state.hub.snapshot(50).unwrap_or_default();
    for event in replay {
        if socket
            .send(Message::Text(
                serde_json::to_string(&event).unwrap_or_default().into(),
            ))
            .await
            .is_err()
        {
            return;
        }
    }

    let mut stream = BroadcastStream::new(state.hub.subscribe());
    loop {
        tokio::select! {
            live = stream.next() => {
                match live {
                    Some(Ok(event)) => {
                        let payload = serde_json::to_string(&event).unwrap_or_else(|_| "{}".into());
                        if socket.send(Message::Text(payload.into())).await.is_err() { break; }
                    }
                    Some(Err(BroadcastStreamRecvError::Lagged(dropped))) => {
                        let payload = json!({"type":"stream_lagged","dropped":dropped}).to_string();
                        if socket.send(Message::Text(payload.into())).await.is_err() { break; }
                    }
                    None => break,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Ping(bytes))) => {
                        if socket.send(Message::Pong(bytes)).await.is_err() { break; }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => { /* stream is server-authoritative in v0.6 */ }
                    Some(Err(_)) => break,
                }
            }
        }
    }
}

async fn publish(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<PublishRequest>,
) -> Response {
    if !authorized(&headers, &state.config.bearer_token) {
        return unauthorized();
    }
    if !state.config.allow_publish {
        return forbidden("event publishing is disabled".into());
    }
    if is_host_control_topic(&request.topic) && !state.config.allow_host_control_topics {
        return forbidden("host-control topics are disabled on the HTTP API".into());
    }
    if request.topic.trim().is_empty() || request.event_type.trim().is_empty() {
        return bad_request("topic and event_type are required".into());
    }
    match state.hub.publish_json(
        request.topic,
        request.event_type,
        request.payload,
        request.correlation_uuid,
        request.causation_uuid,
    ) {
        Ok(event) => (StatusCode::ACCEPTED, Json(json!({"event": event}))).into_response(),
        Err(error) => bad_request(error.to_string()),
    }
}

fn is_host_control_topic(topic: &str) -> bool {
    [
        "desktop.",
        "system.command",
        "system.input",
        "screen.",
        "webview.action",
    ]
    .iter()
    .any(|prefix| topic.starts_with(prefix))
}

fn authorized(headers: &HeaderMap, expected: &str) -> bool {
    let Some(value) = headers.get(axum::http::header::AUTHORIZATION) else {
        return false;
    };
    let Ok(value) = value.to_str() else {
        return false;
    };
    let Some(provided) = value.strip_prefix("Bearer ") else {
        return false;
    };
    constant_time_eq(provided.as_bytes(), expected.as_bytes())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right) {
        diff |= a ^ b;
    }
    diff == 0
}

pub fn generate_bearer_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)
        .expect("PhxClaw API requires an operating-system CSPRNG; refusing weak token fallback");
    URL_SAFE_NO_PAD.encode(bytes)
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ApiErrorBody {
            error: "unauthorized",
            message: "valid bearer token required".into(),
        }),
    )
        .into_response()
}
fn forbidden(message: String) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(ApiErrorBody {
            error: "forbidden",
            message,
        }),
    )
        .into_response()
}
fn bad_request(message: String) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiErrorBody {
            error: "bad_request",
            message,
        }),
    )
        .into_response()
}
fn internal(message: String) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiErrorBody {
            error: "internal_error",
            message,
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_has_entropy_shape() {
        let a = generate_bearer_token();
        let b = generate_bearer_token();
        assert!(a.len() >= 32);
        assert_ne!(a, b);
    }

    #[test]
    fn host_topics_are_detected() {
        assert!(is_host_control_topic("desktop.action.requested"));
        assert!(is_host_control_topic("system.command.execute"));
        assert!(!is_host_control_topic("ui.selection.changed"));
    }
}

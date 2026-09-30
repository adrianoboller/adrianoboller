use phxclaw_api_gateway::{generate_bearer_token, start as start_api, ApiGatewayConfig, ApiServerHandle, ApiServerInfo};
use phxclaw_desktop_protocol::{
    DesktopAction, DesktopActionRequest, DesktopActionResult, DesktopActionStatus, DESKTOP_PROTOCOL,
};
use phxclaw_evidence_ledger::{
    EvidenceDraft, EvidenceLedger, EvidenceOutcome, EvidenceRecord, VerifyReport,
};
use phxclaw_event_bus::EventEnvelope;
use phxclaw_live_bus::{LiveBusStats, LiveEventHub};
use phxclaw_system_automation::{
    capture_primary_monitor, CommandRequest, CommandResult, EnigoInputProvider, ExecutionPolicy,
    InputAction, InputProvider, LaunchRequest, LaunchResult, ShellExecutor,
};
use phxclaw_types::new_uuid_v7;
use phxclaw_webview_control::WebViewCommand;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeSet, HashMap},
    net::{IpAddr, Ipv4Addr},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{
    Emitter, Manager, State,
    utils::config::WebviewUrl,
    webview::WebviewWindowBuilder,
};
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct HostPolicy {
    pub command_execution: bool,
    pub shell_execution: bool,
    pub desktop_input: bool,
    pub screen_capture: bool,
    pub external_webviews: bool,
    pub webview_control: bool,
    pub api_host_control: bool,
    pub webview_allowed_origins: BTreeSet<String>,
}

impl HostPolicy {
    fn from_env() -> Self {
        Self {
            command_execution: env_flag("PHXCLAW_ENABLE_HOST_EXEC"),
            shell_execution: env_flag("PHXCLAW_ENABLE_SHELLS"),
            desktop_input: env_flag("PHXCLAW_ENABLE_INPUT"),
            screen_capture: env_flag("PHXCLAW_ENABLE_SCREEN_CAPTURE"),
            external_webviews: env_flag("PHXCLAW_ENABLE_EXTERNAL_WEBVIEWS"),
            webview_control: env_flag("PHXCLAW_ENABLE_WEBVIEW_CONTROL"),
            api_host_control: env_flag("PHXCLAW_API_HOST_CONTROL"),
            webview_allowed_origins: env_csv("PHXCLAW_WEBVIEW_ALLOWED_ORIGINS"),
        }
    }

    fn allows_url(&self, url: &Url) -> bool {
        if !self.external_webviews { return false; }
        if !matches!(url.scheme(), "http" | "https") { return false; }
        if !url.username().is_empty() || url.password().is_some() { return false; }
        let Some(host) = url.host_str() else { return false; };
        let default_port = match url.scheme() { "http" => 80, "https" => 443, _ => return false };
        let port = url.port().unwrap_or(default_port);
        let origin = if port == default_port {
            format!("{}://{}", url.scheme(), host)
        } else {
            format!("{}://{}:{}", url.scheme(), host, port)
        };
        self.webview_allowed_origins.contains(&origin)
    }
}

struct DesktopState {
    session_uuid: Uuid,
    hub: LiveEventHub,
    ledger: EvidenceLedger,
    shell: Arc<ShellExecutor>,
    policy: HostPolicy,
    captures_dir: PathBuf,
    api: Mutex<Option<ApiServerHandle>>,
    api_token_path: PathBuf,
    pending_webview: Mutex<HashMap<Uuid, DesktopActionRequest>>,
    managed_webviews: Mutex<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostStatus {
    pub version: String,
    pub session_uuid: Uuid,
    pub protocol: &'static str,
    pub api: Option<ApiServerInfo>,
    pub api_token_path: PathBuf,
    pub policy: HostPolicy,
    pub live_bus: Option<LiveBusStats>,
    pub evidence_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PublishUiEvent {
    pub topic: String,
    pub event_type: String,
    #[serde(default)]
    pub payload: Value,
    pub correlation_uuid: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebViewCompletion {
    pub request_uuid: Uuid,
    pub ok: bool,
    #[serde(default)]
    pub value: Value,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManagedWebView {
    pub label: String,
    pub url: String,
}

#[tauri::command]
fn host_status(state: State<'_, DesktopState>) -> HostStatus {
    HostStatus {
        version: env!("CARGO_PKG_VERSION").into(),
        session_uuid: state.session_uuid,
        protocol: DESKTOP_PROTOCOL,
        api: state.api.lock().ok().and_then(|g| g.as_ref().map(|h| h.info.clone())),
        api_token_path: state.api_token_path.clone(),
        policy: state.policy.clone(),
        live_bus: state.hub.stats().ok(),
        evidence_path: state.ledger.path().to_path_buf(),
    }
}

#[tauri::command]
fn events_snapshot(state: State<'_, DesktopState>, limit: Option<usize>) -> Result<Vec<EventEnvelope>, String> {
    state.hub.snapshot(limit.unwrap_or(100).min(1000)).map_err(|e| e.to_string())
}

#[tauri::command]
fn publish_ui_event(state: State<'_, DesktopState>, request: PublishUiEvent) -> Result<EventEnvelope, String> {
    if request.topic.starts_with("desktop.") || request.topic.starts_with("system.command") {
        return Err("UI event cannot publish host-control topics directly".into());
    }
    state.hub.publish_json(
        request.topic,
        request.event_type,
        request.payload,
        request.correlation_uuid,
        None,
    ).map_err(|e| e.to_string())
}

#[tauri::command]
async fn execute_shell(state: State<'_, DesktopState>, request: CommandRequest) -> Result<DesktopActionResult, String> {
    let action = DesktopActionRequest::new("command-center", "system.command.execute", DesktopAction::ExecuteCommand { request });
    execute_shell_action(&state, action).await
}

#[tauri::command]
async fn launch_application(
    state: State<'_, DesktopState>,
    program: String,
    args: Vec<String>,
    cwd: Option<PathBuf>,
) -> Result<DesktopActionResult, String> {
    let action = DesktopActionRequest::new(
        "command-center",
        "system.application.launch",
        DesktopAction::LaunchApplication { program, args, cwd },
    );
    execute_launch_action(&state, action).await
}

#[tauri::command]
fn apply_input(state: State<'_, DesktopState>, action: InputAction) -> Result<DesktopActionResult, String> {
    let request = DesktopActionRequest::new("command-center", "system.input.control", DesktopAction::Input { action });
    execute_input_action(&state, request)
}

#[tauri::command]
fn capture_screen(state: State<'_, DesktopState>, format: Option<String>) -> Result<DesktopActionResult, String> {
    let request = DesktopActionRequest::new(
        "command-center",
        "screen.capture",
        DesktopAction::CaptureScreen {
            format: format.unwrap_or_else(|| "png".into()),
            target: "primary_monitor".into(),
        },
    );
    execute_capture_action(&state, request)
}

#[tauri::command]
fn request_webview_action(
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
    view_label: String,
    command: WebViewCommand,
) -> Result<DesktopActionResult, String> {
    let request = DesktopActionRequest::new(
        "command-center",
        "webview.dom.control",
        DesktopAction::WebView { view_label: view_label.clone(), command },
    );
    dispatch_webview_action(&app, &state, request, &view_label)
}

#[tauri::command]
fn complete_webview_action(
    state: State<'_, DesktopState>,
    completion: WebViewCompletion,
) -> Result<DesktopActionResult, String> {
    let request = state.pending_webview.lock().map_err(|_| "pending webview lock poisoned")?
        .remove(&completion.request_uuid)
        .ok_or_else(|| "unknown webview request".to_string())?;

    let status = if completion.ok { DesktopActionStatus::Succeeded } else { DesktopActionStatus::Failed };
    let output = if completion.ok {
        completion.value
    } else {
        json!({"error": completion.error.unwrap_or_else(|| "webview execution failed".into())})
    };
    finalize_action(&state, &request, status, output, vec![])
}

#[tauri::command]
fn evidence_tail(state: State<'_, DesktopState>, limit: Option<usize>) -> Result<Vec<EvidenceRecord>, String> {
    state.ledger.tail(limit.unwrap_or(50).min(500)).map_err(|e| e.to_string())
}

#[tauri::command]
fn verify_evidence(state: State<'_, DesktopState>) -> Result<VerifyReport, String> {
    state.ledger.verify().map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_managed_webview(
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
    url: String,
) -> Result<ManagedWebView, String> {
    let parsed = Url::parse(&url).map_err(|e| format!("invalid URL: {e}"))?;
    if !state.policy.allows_url(&parsed) {
        return Err("external webview denied by origin policy".into());
    }

    let label = format!("managed-{}", new_uuid_v7().simple());
    let policy = state.policy.clone();
    let requested_url = parsed.clone();
    WebviewWindowBuilder::new(&app, label.clone(), WebviewUrl::External(parsed))
        .title(format!("PhxClaw • {}", requested_url.host_str().unwrap_or("WebView")))
        .inner_size(1280.0, 820.0)
        .on_navigation(move |candidate| policy.allows_url(candidate))
        .build()
        .map_err(|e| format!("failed to create managed webview: {e}"))?;
    state.managed_webviews.lock().map_err(|_| "managed webview lock poisoned")?
        .insert(label.clone(), origin_of(&requested_url));

    let event = state.hub.publish_json(
        "desktop.webview",
        "managed_webview_created",
        json!({"label": label, "url": requested_url}),
        None,
        None,
    ).map_err(|e| e.to_string())?;
    let _ = record_simple_evidence(
        &state,
        event.uuid,
        "command-center",
        "webview.window.create",
        "create_managed_webview",
        EvidenceOutcome::Succeeded,
        json!({"origin": origin_of(&requested_url)}),
        json!({"label": label}),
        vec![],
    );

    Ok(ManagedWebView { label, url: requested_url.to_string() })
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data)?;
            let captures_dir = app_data.join("captures");
            std::fs::create_dir_all(&captures_dir)?;
            let ledger = EvidenceLedger::open(app_data.join("evidence/evidence.jsonl"))?;
            let policy = HostPolicy::from_env();
            let hub = LiveEventHub::new(2_048, 2_000);

            let execution_policy = ExecutionPolicy {
                enabled: policy.command_execution,
                allow_shells: policy.shell_execution,
                max_timeout_ms: 300_000,
                denied_programs: vec![],
            };
            let shell = Arc::new(ShellExecutor::new(execution_policy));

            let api_token = std::env::var("PHXCLAW_API_TOKEN").unwrap_or_else(|_| generate_bearer_token());
            let api_token_path = app_data.join("api/api.token");
            write_secret_file(&api_token_path, &api_token)?;
            let api_port = std::env::var("PHXCLAW_API_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(48_187);
            let api_config = ApiGatewayConfig {
                bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: api_port,
                bearer_token: api_token,
                allow_publish: true,
                allow_host_control_topics: policy.api_host_control,
                replay_limit: 2_000,
            };
            let api = tauri::async_runtime::block_on(start_api(hub.clone(), api_config, env!("CARGO_PKG_VERSION")))?;

            app.manage(DesktopState {
                session_uuid: new_uuid_v7(),
                hub: hub.clone(),
                ledger: ledger.clone(),
                shell,
                policy: policy.clone(),
                captures_dir,
                api: Mutex::new(Some(api)),
                api_token_path,
                pending_webview: Mutex::new(HashMap::new()),
                managed_webviews: Mutex::new(HashMap::new()),
            });

            let app_handle = app.handle().clone();
            let mut ui_rx = hub.subscribe();
            tauri::async_runtime::spawn(async move {
                loop {
                    match ui_rx.recv().await {
                        Ok(event) => { let _ = app_handle.emit("phoenix:event", &event); }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(dropped)) => {
                            let lag = LiveEventHub::lag_event(dropped, "tauri-ui-bridge");
                            let _ = app_handle.emit("phoenix:event", &lag);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });

            let dispatcher_app = app.handle().clone();
            let mut action_rx = hub.subscribe();
            tauri::async_runtime::spawn(async move {
                loop {
                    let event = match action_rx.recv().await {
                        Ok(event) => event,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    };
                    if event.topic != "desktop.action" || event.event_type != "requested" { continue; }
                    let Ok(request) = serde_json::from_value::<DesktopActionRequest>(event.payload.clone()) else { continue; };
                    let state = dispatcher_app.state::<DesktopState>();
                    let result = dispatch_agent_action(&dispatcher_app, &state, request).await;
                    if let Err(error) = result {
                        let _ = state.hub.publish_json(
                            "desktop.action",
                            "dispatch_failed",
                            json!({"error": error}),
                            event.correlation_uuid,
                            Some(event.uuid),
                        );
                    }
                }
            });

            let state = app.state::<DesktopState>();
            let startup = state.hub.publish_json(
                "system.desktop_host",
                "started",
                json!({
                    "session_uuid": state.session_uuid,
                    "version": env!("CARGO_PKG_VERSION"),
                    "api": state.api.lock().ok().and_then(|g| g.as_ref().map(|h| h.info.clone())),
                    "policy": policy,
                }),
                None,
                None,
            )?;
            let _ = record_simple_evidence(
                &state,
                startup.uuid,
                "desktop-host",
                "desktop.host.start",
                "startup",
                EvidenceOutcome::Succeeded,
                json!({}),
                json!({"version": env!("CARGO_PKG_VERSION")}),
                vec![],
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            host_status,
            events_snapshot,
            publish_ui_event,
            execute_shell,
            launch_application,
            apply_input,
            capture_screen,
            request_webview_action,
            complete_webview_action,
            evidence_tail,
            verify_evidence,
            create_managed_webview,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PhxClaw Desktop Host");
}

async fn dispatch_agent_action(
    app: &tauri::AppHandle,
    state: &DesktopState,
    request: DesktopActionRequest,
) -> Result<DesktopActionResult, String> {
    match request.action.clone() {
        DesktopAction::LaunchApplication { .. } => execute_launch_action_ref(state, request).await,
        DesktopAction::ExecuteCommand { .. } => execute_shell_action_ref(state, request).await,
        DesktopAction::Input { .. } => execute_input_action_ref(state, request),
        DesktopAction::CaptureScreen { .. } => execute_capture_action_ref(state, request),
        DesktopAction::WebView { view_label, .. } => dispatch_webview_action(app, state, request, &view_label),
    }
}

async fn execute_shell_action(state: &DesktopState, request: DesktopActionRequest) -> Result<DesktopActionResult, String> {
    execute_shell_action_ref(state, request).await
}

async fn execute_shell_action_ref(state: &DesktopState, request: DesktopActionRequest) -> Result<DesktopActionResult, String> {
    let DesktopAction::ExecuteCommand { request: command } = &request.action else { return Err("wrong desktop action".into()); };
    let command = command.clone();
    let shell = state.shell.clone();
    let summary = safe_command_summary(&command);
    let result = tauri::async_runtime::spawn_blocking(move || shell.execute(&command)).await.map_err(|e| e.to_string())?;
    match result {
        Ok(output) => finalize_command(state, &request, EvidenceOutcome::Succeeded, output),
        Err(error) => finalize_denied_or_failed(state, &request, error.to_string()),
    }
    .map(|mut r| { if r.output.get("request").is_none() { r.output["request"] = summary; } r })
}

async fn execute_launch_action(state: &DesktopState, request: DesktopActionRequest) -> Result<DesktopActionResult, String> {
    execute_launch_action_ref(state, request).await
}

async fn execute_launch_action_ref(state: &DesktopState, request: DesktopActionRequest) -> Result<DesktopActionResult, String> {
    let DesktopAction::LaunchApplication { program, args, cwd } = &request.action else { return Err("wrong desktop action".into()); };
    let mut launch = LaunchRequest::new(program.clone(), args.clone());
    launch.cwd = cwd.clone();
    let shell = state.shell.clone();
    let result = tauri::async_runtime::spawn_blocking(move || shell.launch(&launch)).await.map_err(|e| e.to_string())?;
    match result {
        Ok(output) => finalize_launch(state, &request, output),
        Err(error) => finalize_denied_or_failed(state, &request, error.to_string()),
    }
}

fn execute_input_action(state: &DesktopState, request: DesktopActionRequest) -> Result<DesktopActionResult, String> {
    execute_input_action_ref(state, request)
}

fn execute_input_action_ref(state: &DesktopState, request: DesktopActionRequest) -> Result<DesktopActionResult, String> {
    if !state.policy.desktop_input {
        return finalize_action(state, &request, DesktopActionStatus::Denied, json!({"error":"desktop input disabled by policy"}), vec![]);
    }
    let DesktopAction::Input { action } = &request.action else { return Err("wrong desktop action".into()); };
    let mut provider = match EnigoInputProvider::new() {
        Ok(provider) => provider,
        Err(error) => return finalize_action(
            state,
            &request,
            DesktopActionStatus::Failed,
            json!({"error": format!("input provider: {error}")}),
            vec![],
        ),
    };
    match provider.apply(action) {
        Ok(()) => finalize_action(state, &request, DesktopActionStatus::Succeeded, json!({"applied": true}), vec![]),
        Err(error) => finalize_action(state, &request, DesktopActionStatus::Failed, json!({"error":error}), vec![]),
    }
}

fn execute_capture_action(state: &DesktopState, request: DesktopActionRequest) -> Result<DesktopActionResult, String> {
    execute_capture_action_ref(state, request)
}

fn execute_capture_action_ref(state: &DesktopState, request: DesktopActionRequest) -> Result<DesktopActionResult, String> {
    if !state.policy.screen_capture {
        return finalize_action(state, &request, DesktopActionStatus::Denied, json!({"error":"screen capture disabled by policy"}), vec![]);
    }
    let DesktopAction::CaptureScreen { format, target } = &request.action else { return Err("wrong desktop action".into()); };
    if target != "primary_monitor" {
        return finalize_action(state, &request, DesktopActionStatus::Denied, json!({"error":"only primary_monitor is enabled in v0.6"}), vec![]);
    }
    if !matches!(format.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg") {
        return finalize_action(state, &request, DesktopActionStatus::Denied, json!({"error":"capture format must be png/jpg/jpeg"}), vec![]);
    }
    let extension = if format.eq_ignore_ascii_case("jpeg") { "jpg" } else { format.as_str() };
    let path = state.captures_dir.join(format!("{}.{}", request.uuid, extension));
    match capture_primary_monitor(&path) {
        Ok(()) => finalize_action(
            state,
            &request,
            DesktopActionStatus::Succeeded,
            json!({"path": path, "target": target, "format": extension}),
            vec![format!("file://{}", path.display())],
        ),
        Err(error) => finalize_action(state, &request, DesktopActionStatus::Failed, json!({"error":error.to_string()}), vec![]),
    }
}

fn dispatch_webview_action(
    app: &tauri::AppHandle,
    state: &DesktopState,
    request: DesktopActionRequest,
    view_label: &str,
) -> Result<DesktopActionResult, String> {
    if !state.policy.webview_control {
        return finalize_action(
            state,
            &request,
            DesktopActionStatus::Denied,
            json!({"error":"webview control disabled by policy"}),
            vec![],
        );
    }

    let is_main = view_label == "main";
    let is_managed = state.managed_webviews.lock().map_err(|_| "managed webview lock poisoned")?.contains_key(view_label);
    if !is_main && !is_managed {
        return finalize_action(
            state,
            &request,
            DesktopActionStatus::Denied,
            json!({"error":"unknown or unmanaged webview"}),
            vec![],
        );
    }

    // Native eval provides arbitrary JavaScript side effects without enabling unsafe-eval
    // in the trusted Command Center CSP. Query-like DOM commands still use the typed
    // frontend bridge so results can be returned and evidenced.
    if let DesktopAction::WebView { command: WebViewCommand::EvaluateJavascript { script }, .. } = &request.action {
        let Some(window) = app.get_webview_window(view_label) else {
            return finalize_action(state, &request, DesktopActionStatus::Failed, json!({"error":"webview not found"}), vec![]);
        };
        if let Err(error) = window.eval(script) {
            return finalize_action(
                state,
                &request,
                DesktopActionStatus::Failed,
                json!({"error": error.to_string()}),
                vec![],
            );
        }
        return finalize_action(
            state,
            &request,
            DesktopActionStatus::Succeeded,
            json!({"dispatched": true, "result": "native_eval_has_no_return_value"}),
            vec![],
        );
    }

    if !is_main {
        return finalize_action(
            state,
            &request,
            DesktopActionStatus::Denied,
            json!({"error":"managed remote webviews accept native evaluate_javascript only in v0.6"}),
            vec![],
        );
    }

    state.pending_webview.lock().map_err(|_| "pending webview lock poisoned")?.insert(request.uuid, request.clone());
    if let Err(error) = app.emit_to(view_label, "phoenix:webview-request", &request) {
        let _ = state.pending_webview.lock().map(|mut pending| pending.remove(&request.uuid));
        return finalize_action(
            state,
            &request,
            DesktopActionStatus::Failed,
            json!({"error": error.to_string()}),
            vec![],
        );
    }
    let result = DesktopActionResult::finished(&request, DesktopActionStatus::Accepted, json!({"queued": true, "view_label": view_label}));
    let _ = state.hub.publish_json(
        "desktop.action",
        "accepted",
        serde_json::to_value(&result).unwrap_or_else(|_| json!({})),
        request.correlation_uuid,
        Some(request.uuid),
    );
    Ok(result)
}

fn finalize_launch(
    state: &DesktopState,
    request: &DesktopActionRequest,
    output: LaunchResult,
) -> Result<DesktopActionResult, String> {
    finalize_action_with_outcome(
        state,
        request,
        DesktopActionStatus::Succeeded,
        json!({"pid": output.pid, "launched_at": output.launched_at}),
        vec![],
        EvidenceOutcome::Succeeded,
    )
}

fn finalize_command(
    state: &DesktopState,
    request: &DesktopActionRequest,
    outcome: EvidenceOutcome,
    output: CommandResult,
) -> Result<DesktopActionResult, String> {
    let payload = json!({
        "exit_code": output.exit_code,
        "timed_out": output.timed_out,
        "elapsed_ms": output.elapsed_ms,
        "stdout": truncate(&output.stdout, 8192),
        "stderr": truncate(&output.stderr, 8192),
        "finished_at": output.finished_at,
    });
    let status = if output.exit_code == Some(0) && !output.timed_out { DesktopActionStatus::Succeeded } else { DesktopActionStatus::Failed };
    finalize_action_with_outcome(state, request, status, payload, vec![], outcome)
}

fn finalize_denied_or_failed(state: &DesktopState, request: &DesktopActionRequest, error: String) -> Result<DesktopActionResult, String> {
    let denied = error.contains("disabled by policy") || error.contains("denied by policy");
    let status = if denied { DesktopActionStatus::Denied } else { DesktopActionStatus::Failed };
    let outcome = if denied { EvidenceOutcome::Denied } else { EvidenceOutcome::Failed };
    finalize_action_with_outcome(state, request, status, json!({"error": error}), vec![], outcome)
}

fn finalize_action(
    state: &DesktopState,
    request: &DesktopActionRequest,
    status: DesktopActionStatus,
    output: Value,
    artifact_uris: Vec<String>,
) -> Result<DesktopActionResult, String> {
    let outcome = match status {
        DesktopActionStatus::Succeeded => EvidenceOutcome::Succeeded,
        DesktopActionStatus::Denied => EvidenceOutcome::Denied,
        DesktopActionStatus::Failed => EvidenceOutcome::Failed,
        DesktopActionStatus::Accepted => EvidenceOutcome::Requested,
    };
    finalize_action_with_outcome(state, request, status, output, artifact_uris, outcome)
}

fn finalize_action_with_outcome(
    state: &DesktopState,
    request: &DesktopActionRequest,
    status: DesktopActionStatus,
    output: Value,
    artifact_uris: Vec<String>,
    outcome: EvidenceOutcome,
) -> Result<DesktopActionResult, String> {
    let record = state.ledger.append(EvidenceDraft {
        action_uuid: request.uuid,
        correlation_uuid: request.correlation_uuid,
        actor: request.actor.clone(),
        capability: request.capability.clone(),
        action: action_name(&request.action).into(),
        outcome,
        request_summary: safe_action_summary(&request.action),
        result_summary: bounded_value(output.clone(), 8192),
        artifact_uris,
    }).map_err(|e| e.to_string())?;

    let mut result = DesktopActionResult::finished(request, status, output);
    result.evidence_uuid = Some(record.uuid);
    let _ = state.hub.publish_json(
        "desktop.action",
        match result.status {
            DesktopActionStatus::Succeeded => "succeeded",
            DesktopActionStatus::Failed => "failed",
            DesktopActionStatus::Denied => "denied",
            DesktopActionStatus::Accepted => "accepted",
        },
        serde_json::to_value(&result).unwrap_or_else(|_| json!({})),
        request.correlation_uuid,
        Some(request.uuid),
    );
    let _ = state.hub.publish_json(
        "evidence.ledger",
        "recorded",
        json!({"evidence_uuid": record.uuid, "action_uuid": request.uuid, "record_hash": record.record_hash}),
        request.correlation_uuid,
        Some(request.uuid),
    );
    Ok(result)
}

fn record_simple_evidence(
    state: &DesktopState,
    action_uuid: Uuid,
    actor: &str,
    capability: &str,
    action: &str,
    outcome: EvidenceOutcome,
    request_summary: Value,
    result_summary: Value,
    artifact_uris: Vec<String>,
) -> Result<EvidenceRecord, String> {
    state.ledger.append(EvidenceDraft {
        action_uuid,
        correlation_uuid: None,
        actor: actor.into(),
        capability: capability.into(),
        action: action.into(),
        outcome,
        request_summary,
        result_summary,
        artifact_uris,
    }).map_err(|e| e.to_string())
}

fn action_name(action: &DesktopAction) -> &'static str {
    match action {
        DesktopAction::LaunchApplication { .. } => "launch_application",
        DesktopAction::ExecuteCommand { .. } => "execute_command",
        DesktopAction::Input { .. } => "input",
        DesktopAction::CaptureScreen { .. } => "capture_screen",
        DesktopAction::WebView { .. } => "webview",
    }
}

fn safe_action_summary(action: &DesktopAction) -> Value {
    match action {
        DesktopAction::LaunchApplication { program, args, cwd } => json!({"program": program, "arg_count": args.len(), "cwd": cwd}),
        DesktopAction::ExecuteCommand { request } => safe_command_summary(request),
        DesktopAction::Input { action } => match action {
            InputAction::MoveMouse { x, y } => json!({"action":"move_mouse","x":x,"y":y}),
            InputAction::MouseButton { button, state } => json!({"action":"mouse_button","button":button,"state":state}),
            InputAction::Scroll { dx, dy } => json!({"action":"scroll","dx":dx,"dy":dy}),
            InputAction::Key { key, state } => json!({"action":"key","key":key,"state":state}),
            InputAction::Text { text } => json!({"action":"text","characters":text.chars().count(),"content":"[redacted]"}),
            InputAction::Hotkey { keys } => json!({"action":"hotkey","keys":keys}),
        },
        DesktopAction::CaptureScreen { format, target } => json!({"format":format,"target":target}),
        DesktopAction::WebView { view_label, command } => json!({"view_label":view_label,"command":webview_command_name(command)}),
    }
}

fn safe_command_summary(request: &CommandRequest) -> Value {
    json!({
        "request_uuid": request.uuid,
        "shell": request.shell,
        "program_or_script": request.program_or_script,
        "arg_count": request.args.len(),
        "cwd": request.cwd,
        "env_keys": request.env.keys().collect::<Vec<_>>(),
        "stdin": if request.stdin.is_some() { "[redacted]" } else { "" },
        "timeout_ms": request.timeout_ms,
    })
}

fn webview_command_name(command: &WebViewCommand) -> &'static str {
    match command {
        WebViewCommand::Navigate { .. } => "navigate",
        WebViewCommand::LoadHtml { .. } => "load_html",
        WebViewCommand::EvaluateJavascript { .. } => "evaluate_javascript",
        WebViewCommand::InjectCss { .. } => "inject_css",
        WebViewCommand::QuerySelector { .. } => "query_selector",
        WebViewCommand::QuerySelectorAll { .. } => "query_selector_all",
        WebViewCommand::GetOuterHtml { .. } => "get_outer_html",
        WebViewCommand::SetInnerHtml { .. } => "set_inner_html",
        WebViewCommand::SetAttribute { .. } => "set_attribute",
        WebViewCommand::RemoveAttribute { .. } => "remove_attribute",
        WebViewCommand::Click { .. } => "click",
        WebViewCommand::Focus { .. } => "focus",
        WebViewCommand::TypeText { .. } => "type_text",
        WebViewCommand::DispatchEvent { .. } => "dispatch_event",
        WebViewCommand::ScrollIntoView { .. } => "scroll_into_view",
        WebViewCommand::GetComputedStyle { .. } => "get_computed_style",
        WebViewCommand::DomToSvg { .. } => "dom_to_svg",
    }
}

fn bounded_value(value: Value, max_string_chars: usize) -> Value {
    match value {
        Value::String(s) => Value::String(truncate(&s, max_string_chars)),
        Value::Array(values) => Value::Array(values.into_iter().take(256).map(|v| bounded_value(v, max_string_chars)).collect()),
        Value::Object(map) => Value::Object(map.into_iter().take(256).map(|(k, v)| (k, bounded_value(v, max_string_chars))).collect()),
        other => other,
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut out = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars { out.push_str("…[truncated]"); }
    out
}

fn origin_of(url: &Url) -> String {
    url.origin().ascii_serialization()
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).ok().map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")).unwrap_or(false)
}

fn env_csv(name: &str) -> BTreeSet<String> {
    std::env::var(name).ok().into_iter().flat_map(|v| v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect::<Vec<_>>()).collect()
}

fn write_secret_file(path: &Path, value: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(path, value.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

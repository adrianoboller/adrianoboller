#![forbid(unsafe_code)]

use phxclaw_types::new_uuid_v7;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

pub const MCP_PROTOCOL_2024_11_05: &str = "2024-11-05";
pub const MCP_PROTOCOL_2025_03_26: &str = "2025-03-26";
pub const MCP_PROTOCOL_2025_06_18: &str = "2025-06-18";
pub const MCP_PROTOCOL_2025_11_25: &str = "2025-11-25";
pub const MCP_PROTOCOL_2026_07_28: &str = "2026-07-28";
pub const MCP_DEFAULT_LEGACY_PROTOCOL: &str = MCP_PROTOCOL_2025_11_25;
pub const MCP_DEFAULT_MODERN_PROTOCOL: &str = MCP_PROTOCOL_2026_07_28;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(untagged)]
pub enum JsonRpcId {
    Number(i64),
    String(String),
    Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: JsonRpcId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip)]
    pub correlation_uuid: Uuid,
}

impl JsonRpcRequest {
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        let correlation_uuid = new_uuid_v7();
        Self {
            jsonrpc: "2.0".into(),
            id: JsonRpcId::String(new_uuid_v7().to_string()),
            method: method.into(),
            params: Some(params),
            correlation_uuid,
        }
    }

    pub fn notification(method: impl Into<String>, params: Value) -> JsonRpcNotification {
        JsonRpcNotification {
            jsonrpc: "2.0".into(),
            method: method.into(),
            params: Some(params),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcErrorBody {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: JsonRpcId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcErrorBody>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpProtocolEra {
    Legacy,
    Modern,
}

pub fn protocol_era(version: &str) -> Option<McpProtocolEra> {
    match version {
        MCP_PROTOCOL_2024_11_05
        | MCP_PROTOCOL_2025_03_26
        | MCP_PROTOCOL_2025_06_18
        | MCP_PROTOCOL_2025_11_25 => Some(McpProtocolEra::Legacy),
        MCP_PROTOCOL_2026_07_28 => Some(McpProtocolEra::Modern),
        _ => None,
    }
}

pub fn supported_protocol_versions() -> &'static [&'static str] {
    &[
        MCP_PROTOCOL_2026_07_28,
        MCP_PROTOCOL_2025_11_25,
        MCP_PROTOCOL_2025_06_18,
        MCP_PROTOCOL_2025_03_26,
        MCP_PROTOCOL_2024_11_05,
    ]
}

pub fn negotiate_legacy_version(server_version: &str) -> Result<&'static str, ProtocolError> {
    supported_protocol_versions()
        .iter()
        .copied()
        .find(|candidate| *candidate == server_version && protocol_era(candidate) == Some(McpProtocolEra::Legacy))
        .ok_or_else(|| ProtocolError::UnsupportedProtocolVersion(server_version.to_owned()))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpTransportKind {
    Stdio,
    StreamableHttp,
    Sse,
    WebSocket,
    InProcess,
    ManagedProxy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum McpLifecyclePhase {
    ConfigLoad,
    ServerRegistration,
    SpawnConnect,
    InitializeHandshake,
    ToolDiscovery,
    ResourceDiscovery,
    Ready,
    Invocation,
    ErrorSurfacing,
    Shutdown,
    Cleanup,
}

pub fn validate_lifecycle_transition(
    from: &McpLifecyclePhase,
    to: &McpLifecyclePhase,
    recoverable_error: bool,
) -> Result<(), ProtocolError> {
    use McpLifecyclePhase::*;
    let valid = matches!(
        (from, to),
        (ConfigLoad, ServerRegistration)
            | (ServerRegistration, SpawnConnect)
            | (SpawnConnect, InitializeHandshake)
            | (InitializeHandshake, ToolDiscovery)
            | (ToolDiscovery, ResourceDiscovery)
            | (ToolDiscovery, Ready)
            | (ResourceDiscovery, Ready)
            | (Ready, Invocation)
            | (Invocation, Ready)
            | (Shutdown, Cleanup)
    ) || (matches!(to, ErrorSurfacing) && !matches!(from, Shutdown | Cleanup))
        || (matches!(to, Shutdown) && !matches!(from, Cleanup))
        || (matches!((from, to), (ErrorSurfacing, Ready)) && recoverable_error);

    if valid {
        Ok(())
    } else {
        Err(ProtocolError::IllegalLifecycleTransition {
            from: format!("{from:?}"),
            to: format!("{to:?}"),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpErrorSurface {
    pub phase: McpLifecyclePhase,
    pub server_name: Option<String>,
    pub message: String,
    pub context: BTreeMap<String, String>,
    pub recoverable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpFailedServer {
    pub server_name: String,
    pub phase: McpLifecyclePhase,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpDegradedReport {
    pub working_servers: Vec<String>,
    pub failed_servers: Vec<McpFailedServer>,
    pub available_tools: Vec<String>,
    pub missing_tools: Vec<String>,
}

impl McpDegradedReport {
    pub fn new(
        working_servers: Vec<String>,
        failed_servers: Vec<McpFailedServer>,
        available_tools: Vec<String>,
        expected_tools: Vec<String>,
    ) -> Self {
        let mut working = working_servers.into_iter().collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
        let available = available_tools.into_iter().collect::<BTreeSet<_>>();
        let expected = expected_tools.into_iter().collect::<BTreeSet<_>>();
        let missing_tools = expected.difference(&available).cloned().collect::<Vec<_>>();
        working.sort();
        Self {
            working_servers: working,
            failed_servers,
            available_tools: available.into_iter().collect(),
            missing_tools,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JsonRpcMessageClass {
    Request,
    Notification,
    Success,
    Error,
}

pub fn classify_jsonrpc_message(value: &Value) -> Result<JsonRpcMessageClass, ProtocolError> {
    if value.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(ProtocolError::InvalidJsonRpcVersion);
    }
    let has_method = value.get("method").and_then(Value::as_str).is_some();
    let has_id = value.get("id").is_some();
    if has_method && has_id {
        return Ok(JsonRpcMessageClass::Request);
    }
    if has_method && !has_id {
        return Ok(JsonRpcMessageClass::Notification);
    }
    if value.get("error").is_some() {
        return Ok(JsonRpcMessageClass::Error);
    }
    if value.get("result").is_some() && has_id {
        return Ok(JsonRpcMessageClass::Success);
    }
    Err(ProtocolError::InvalidJsonRpcShape)
}

pub fn tool_result_is_error(value: &Value) -> bool {
    value
        .get("isError")
        .or_else(|| value.get("is_error"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub fn normalize_mcp_name(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-') { ch } else { '_' })
        .collect()
}

pub fn qualified_tool_name(server_name: &str, tool_name: &str) -> String {
    format!(
        "mcp__{}__{}",
        normalize_mcp_name(server_name),
        normalize_mcp_name(tool_name)
    )
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("missing Content-Length header")]
    MissingContentLength,
    #[error("invalid Content-Length")]
    InvalidContentLength,
    #[error("incomplete framed payload")]
    Incomplete,
    #[error("JSON-RPC version must be 2.0")]
    InvalidJsonRpcVersion,
    #[error("invalid JSON-RPC message shape")]
    InvalidJsonRpcShape,
    #[error("unsupported MCP protocol version: {0}")]
    UnsupportedProtocolVersion(String),
    #[error("illegal MCP lifecycle transition: {from} -> {to}")]
    IllegalLifecycleTransition { from: String, to: String },
    #[error("JSON-RPC id mismatch")]
    IdMismatch,
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn encode_content_length_message(value: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).expect("JSON serialization");
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend(body);
    out
}

pub fn encode_lsp_message(value: &Value) -> Vec<u8> {
    encode_content_length_message(value)
}

pub fn decode_content_length_message(input: &[u8]) -> Result<(Value, usize), ProtocolError> {
    let crlf = b"\r\n\r\n";
    let lf = b"\n\n";
    let (header_end, marker_len) = if let Some(pos) = input.windows(crlf.len()).position(|window| window == crlf) {
        (pos, crlf.len())
    } else if let Some(pos) = input.windows(lf.len()).position(|window| window == lf) {
        (pos, lf.len())
    } else {
        return Err(ProtocolError::Incomplete);
    };

    let header = std::str::from_utf8(&input[..header_end]).map_err(|_| ProtocolError::InvalidContentLength)?;
    let length = header
        .lines()
        .find_map(|line| {
            let mut parts = line.splitn(2, ':');
            let key = parts.next()?.trim();
            let value = parts.next()?.trim();
            key.eq_ignore_ascii_case("Content-Length").then_some(value)
        })
        .ok_or(ProtocolError::MissingContentLength)?
        .parse::<usize>()
        .map_err(|_| ProtocolError::InvalidContentLength)?;

    let body_start = header_end + marker_len;
    let body_end = body_start.checked_add(length).ok_or(ProtocolError::InvalidContentLength)?;
    if input.len() < body_end {
        return Err(ProtocolError::Incomplete);
    }
    Ok((serde_json::from_slice(&input[body_start..body_end])?, body_end))
}

pub fn decode_lsp_message(input: &[u8]) -> Result<(Value, usize), ProtocolError> {
    decode_content_length_message(input)
}

pub fn encode_mcp_line(value: &Value) -> Vec<u8> {
    let mut out = serde_json::to_vec(value).expect("JSON serialization");
    out.push(b'\n');
    out
}

pub fn decode_mcp_line(input: &[u8]) -> Result<(Value, usize), ProtocolError> {
    let end = input.iter().position(|byte| *byte == b'\n').ok_or(ProtocolError::Incomplete)?;
    Ok((serde_json::from_slice(&input[..end])?, end + 1))
}

pub fn legacy_initialize_request(client_name: &str, client_version: &str) -> JsonRpcRequest {
    JsonRpcRequest::new(
        "initialize",
        json!({
            "protocolVersion": MCP_DEFAULT_LEGACY_PROTOCOL,
            "capabilities": {},
            "clientInfo": {"name": client_name, "version": client_version}
        }),
    )
}

pub fn legacy_initialized_notification() -> JsonRpcNotification {
    JsonRpcRequest::notification("notifications/initialized", json!({}))
}

pub fn modern_discover_request(client_name: &str, client_version: &str) -> JsonRpcRequest {
    JsonRpcRequest::new(
        "server/discover",
        json!({
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": MCP_DEFAULT_MODERN_PROTOCOL,
                "io.modelcontextprotocol/clientInfo": {"name": client_name, "version": client_version},
                "io.modelcontextprotocol/clientCapabilities": {}
            }
        }),
    )
}

pub fn modern_request_meta(client_name: &str, client_version: &str) -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": MCP_DEFAULT_MODERN_PROTOCOL,
        "io.modelcontextprotocol/clientInfo": {"name": client_name, "version": client_version},
        "io.modelcontextprotocol/clientCapabilities": {}
    })
}

pub fn validate_response_id(request: &JsonRpcRequest, response: &JsonRpcResponse) -> Result<(), ProtocolError> {
    if request.id == response.id {
        Ok(())
    } else {
        Err(ProtocolError::IdMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_length_round_trip_supports_crlf() {
        let value = json!({"jsonrpc":"2.0","id":1,"result":{"ok":true}});
        let encoded = encode_content_length_message(&value);
        let (decoded, used) = decode_content_length_message(&encoded).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(used, encoded.len());
    }

    #[test]
    fn content_length_accepts_lf_only_headers() {
        let body = br#"{"jsonrpc":"2.0","id":1,"result":{}}"#;
        let input = format!("Content-Length: {}\n\n", body.len()).into_bytes();
        let mut frame = input;
        frame.extend_from_slice(body);
        let (decoded, _) = decode_content_length_message(&frame).unwrap();
        assert_eq!(decoded["id"], 1);
    }

    #[test]
    fn notification_is_classified_without_id() {
        let value = json!({"jsonrpc":"2.0","method":"notifications/progress","params":{}});
        assert_eq!(classify_jsonrpc_message(&value).unwrap(), JsonRpcMessageClass::Notification);
    }

    #[test]
    fn legacy_and_modern_eras_are_explicit() {
        assert_eq!(protocol_era(MCP_PROTOCOL_2025_11_25), Some(McpProtocolEra::Legacy));
        assert_eq!(protocol_era(MCP_PROTOCOL_2026_07_28), Some(McpProtocolEra::Modern));
    }

    #[test]
    fn tool_prefix_is_stable() {
        assert_eq!(qualified_tool_name("claude.ai Example Server", "weather tool"), "mcp__claude_ai_Example_Server__weather_tool");
    }

    #[test]
    fn degraded_report_computes_missing_tools() {
        let report = McpDegradedReport::new(
            vec!["a".into(), "a".into()],
            vec![],
            vec!["mcp__a__read".into()],
            vec!["mcp__a__read".into(), "mcp__b__fetch".into()],
        );
        assert_eq!(report.working_servers, vec!["a"]);
        assert_eq!(report.missing_tools, vec!["mcp__b__fetch"]);
    }

    #[test]
    fn lifecycle_blocks_illegal_jump() {
        let err = validate_lifecycle_transition(
            &McpLifecyclePhase::ServerRegistration,
            &McpLifecyclePhase::Ready,
            false,
        )
        .unwrap_err();
        assert!(matches!(err, ProtocolError::IllegalLifecycleTransition { .. }));
    }

    #[test]
    fn initialized_notification_is_emitted_for_legacy() {
        let n = legacy_initialized_notification();
        assert_eq!(n.method, "notifications/initialized");
    }
}

// ---- v0.64 managed MCP/LSP runtime -------------------------------------------------
// The legacy framing/contracts above remain public for compatibility.  This module
// adds the actual process/session and Streamable HTTP runtime.  External executables
// are deny-by-default and must be explicitly authorized by canonical absolute path.

pub mod managed_runtime {
    use super::{
        classify_jsonrpc_message, encode_content_length_message, encode_mcp_line,
        JsonRpcId, JsonRpcMessageClass, JsonRpcRequest, JsonRpcResponse,
        MCP_DEFAULT_LEGACY_PROTOCOL, MCP_DEFAULT_MODERN_PROTOCOL,
    };
    use futures_util::StreamExt;
    use phxclaw_evidence_ledger::{EvidenceDraft, EvidenceLedger, EvidenceOutcome, LedgerError};
    use phxclaw_event_bus::EventEnvelope;
    use phxclaw_types::new_uuid_v7;
    use serde::{Deserialize, Serialize};
    use serde_json::{json, Value};
    use std::{
        collections::{BTreeMap, BTreeSet},
        future::Future,
        path::PathBuf,
        process::Stdio,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::Duration,
    };
    use thiserror::Error;
    use tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
        process::{Child, ChildStdin, ChildStdout, Command},
        sync::Notify,
        task::JoinHandle,
        time::{sleep, timeout},
    };
    use url::Url;
    use uuid::Uuid;

    pub const DEFAULT_MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
    pub const DEFAULT_STDERR_BYTES: u64 = 1024 * 1024;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum SessionKind {
        McpStdio,
        McpStreamableHttp,
        LspStdio,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ProcessSpec {
        pub executable: PathBuf,
        #[serde(default)]
        pub args: Vec<String>,
        pub cwd: PathBuf,
        #[serde(default)]
        pub env: BTreeMap<String, String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ProcessSecurityPolicy {
        pub allowed_executables: BTreeSet<PathBuf>,
        pub allowed_cwd_roots: Vec<PathBuf>,
        pub allowed_env_keys: BTreeSet<String>,
        pub max_args: usize,
        pub max_arg_bytes: usize,
    }

    impl Default for ProcessSecurityPolicy {
        fn default() -> Self {
            Self {
                allowed_executables: BTreeSet::new(),
                allowed_cwd_roots: Vec::new(),
                allowed_env_keys: BTreeSet::new(),
                max_args: 128,
                max_arg_bytes: 64 * 1024,
            }
        }
    }

    impl ProcessSecurityPolicy {
        pub fn validate(&self, spec: &ProcessSpec) -> Result<ValidatedProcessSpec, RuntimeError> {
            if spec.args.len() > self.max_args {
                return Err(RuntimeError::PolicyDenied("too many process arguments".into()));
            }
            let arg_bytes = spec.args.iter().map(String::len).sum::<usize>();
            if arg_bytes > self.max_arg_bytes {
                return Err(RuntimeError::PolicyDenied("process arguments exceed byte limit".into()));
            }
            if spec.env.keys().any(|k| !self.allowed_env_keys.contains(k)) {
                return Err(RuntimeError::PolicyDenied("environment key is not allowlisted".into()));
            }
            let executable = std::fs::canonicalize(&spec.executable)
                .map_err(|e| RuntimeError::ProcessSpec(format!("executable: {e}")))?;
            let allowed = self.allowed_executables.iter().filter_map(|p| std::fs::canonicalize(p).ok())
                .any(|p| p == executable);
            if !allowed {
                return Err(RuntimeError::PolicyDenied(format!("executable not allowlisted: {}", executable.display())));
            }
            let cwd = std::fs::canonicalize(&spec.cwd)
                .map_err(|e| RuntimeError::ProcessSpec(format!("cwd: {e}")))?;
            let cwd_allowed = self.allowed_cwd_roots.iter().filter_map(|p| std::fs::canonicalize(p).ok())
                .any(|root| cwd.starts_with(root));
            if !cwd_allowed {
                return Err(RuntimeError::PolicyDenied(format!("cwd outside allowlisted roots: {}", cwd.display())));
            }
            Ok(ValidatedProcessSpec { executable, args: spec.args.clone(), cwd, env: spec.env.clone() })
        }
    }

    #[derive(Debug, Clone)]
    pub struct ValidatedProcessSpec {
        pub executable: PathBuf,
        pub args: Vec<String>,
        pub cwd: PathBuf,
        pub env: BTreeMap<String, String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SessionPolicy {
        pub request_timeout_ms: u64,
        pub startup_timeout_ms: u64,
        pub shutdown_timeout_ms: u64,
        pub max_frame_bytes: usize,
        pub max_unsolicited_messages: usize,
        pub max_restarts: u32,
        pub restart_backoff_ms: u64,
    }

    impl Default for SessionPolicy {
        fn default() -> Self {
            Self {
                request_timeout_ms: 30_000,
                startup_timeout_ms: 15_000,
                shutdown_timeout_ms: 5_000,
                max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
                max_unsolicited_messages: 128,
                max_restarts: 3,
                restart_backoff_ms: 250,
            }
        }
    }

    #[derive(Debug, Clone, Default)]
    pub struct Cancellation {
        inner: Arc<CancellationInner>,
    }

    #[derive(Debug, Default)]
    struct CancellationInner {
        cancelled: AtomicBool,
        notify: Notify,
    }

    impl Cancellation {
        pub fn cancel(&self) {
            if !self.inner.cancelled.swap(true, Ordering::SeqCst) {
                self.inner.notify.notify_waiters();
            }
        }
        pub fn is_cancelled(&self) -> bool { self.inner.cancelled.load(Ordering::SeqCst) }
        pub async fn cancelled(&self) {
            if self.is_cancelled() { return; }
            self.inner.notify.notified().await;
        }
    }

    async fn controlled<T, F>(future: F, duration: Duration, cancellation: Cancellation) -> Result<T, RuntimeError>
    where
        F: Future<Output = Result<T, RuntimeError>>,
    {
        tokio::select! {
            _ = cancellation.cancelled() => Err(RuntimeError::Cancelled),
            result = timeout(duration, future) => match result {
                Ok(v) => v,
                Err(_) => Err(RuntimeError::Timeout),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct CapabilityPolicy {
        pub allowed_methods: BTreeSet<String>,
        pub allowed_tools: BTreeSet<String>,
    }

    impl CapabilityPolicy {
        pub fn deny_all() -> Self { Self { allowed_methods: BTreeSet::new(), allowed_tools: BTreeSet::new() } }
        pub fn allow_method(&self, method: &str) -> Result<(), RuntimeError> {
            if self.allowed_methods.contains(method) { Ok(()) }
            else { Err(RuntimeError::CapabilityDenied(method.to_owned())) }
        }
        pub fn allow_tool(&self, tool: &str) -> Result<(), RuntimeError> {
            if self.allowed_tools.contains(tool) { Ok(()) }
            else { Err(RuntimeError::CapabilityDenied(format!("tool:{tool}"))) }
        }
    }

    #[derive(Clone)]
    pub struct RuntimeAudit {
        actor: String,
        ledger: Option<EvidenceLedger>,
    }

    impl RuntimeAudit {
        pub fn new(actor: impl Into<String>, ledger: Option<EvidenceLedger>) -> Self {
            Self { actor: actor.into(), ledger }
        }

        pub fn record(
            &self,
            session_uuid: Uuid,
            capability: &str,
            action: &str,
            outcome: EvidenceOutcome,
            request: Value,
            result: Value,
        ) -> Result<EventEnvelope, RuntimeError> {
            if let Some(ledger) = &self.ledger {
                ledger.append(EvidenceDraft {
                    action_uuid: session_uuid,
                    correlation_uuid: Some(session_uuid),
                    actor: self.actor.clone(),
                    capability: capability.to_owned(),
                    action: action.to_owned(),
                    outcome: outcome.clone(),
                    request_summary: request.clone(),
                    result_summary: result.clone(),
                    artifact_uris: Vec::new(),
                })?;
            }
            let mut event = EventEnvelope::new(
                "phxclaw.runtime.protocol",
                action,
                json!({"capability":capability,"outcome":outcome,"request":request,"result":result}),
            );
            event.aggregate_uuid = Some(session_uuid);
            event.correlation_uuid = Some(session_uuid);
            Ok(event)
        }
    }

    struct ManagedChild {
        child: Child,
        stdin: ChildStdin,
        stdout: BufReader<ChildStdout>,
        stderr_task: JoinHandle<Vec<u8>>,
    }

    impl ManagedChild {
        async fn spawn(spec: &ValidatedProcessSpec) -> Result<Self, RuntimeError> {
            let mut command = Command::new(&spec.executable);
            command.args(&spec.args)
                .current_dir(&spec.cwd)
                .env_clear()
                .envs(&spec.env)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            let mut child = command.spawn()?;
            let stdin = child.stdin.take().ok_or(RuntimeError::MissingPipe("stdin"))?;
            let stdout = child.stdout.take().ok_or(RuntimeError::MissingPipe("stdout"))?;
            let stderr = child.stderr.take().ok_or(RuntimeError::MissingPipe("stderr"))?;
            let stderr_task = tokio::spawn(async move {
                let mut bytes = Vec::new();
                let _ = stderr.take(DEFAULT_STDERR_BYTES).read_to_end(&mut bytes).await;
                bytes
            });
            Ok(Self { child, stdin, stdout: BufReader::new(stdout), stderr_task })
        }

        async fn write_line(&mut self, value: &Value) -> Result<(), RuntimeError> {
            let bytes = encode_mcp_line(value);
            self.stdin.write_all(&bytes).await?;
            self.stdin.flush().await?;
            Ok(())
        }

        async fn read_line(&mut self, max: usize) -> Result<Value, RuntimeError> {
            let mut bytes = Vec::new();
            let read = self.stdout.read_until(b'\n', &mut bytes).await?;
            if read == 0 { return Err(RuntimeError::ProcessClosed); }
            if bytes.len() > max { return Err(RuntimeError::FrameTooLarge(bytes.len())); }
            if bytes.last() == Some(&b'\n') { bytes.pop(); }
            if bytes.last() == Some(&b'\r') { bytes.pop(); }
            Ok(serde_json::from_slice(&bytes)?)
        }

        async fn write_content_length(&mut self, value: &Value) -> Result<(), RuntimeError> {
            let bytes = encode_content_length_message(value);
            self.stdin.write_all(&bytes).await?;
            self.stdin.flush().await?;
            Ok(())
        }

        async fn read_content_length(&mut self, max: usize) -> Result<Value, RuntimeError> {
            let mut content_length: Option<usize> = None;
            let mut header_bytes = 0usize;
            loop {
                let mut line = String::new();
                let read = self.stdout.read_line(&mut line).await?;
                if read == 0 { return Err(RuntimeError::ProcessClosed); }
                header_bytes += read;
                if header_bytes > 64 * 1024 { return Err(RuntimeError::FrameTooLarge(header_bytes)); }
                if line == "\r\n" || line == "\n" { break; }
                if let Some((key, value)) = line.split_once(':') {
                    if key.trim().eq_ignore_ascii_case("content-length") {
                        content_length = Some(value.trim().parse().map_err(|_| RuntimeError::Protocol("invalid Content-Length".into()))?);
                    }
                }
            }
            let length = content_length.ok_or_else(|| RuntimeError::Protocol("missing Content-Length".into()))?;
            if length > max { return Err(RuntimeError::FrameTooLarge(length)); }
            let mut body = vec![0u8; length];
            self.stdout.read_exact(&mut body).await?;
            Ok(serde_json::from_slice(&body)?)
        }


        async fn terminate(mut self, grace: Duration) -> Result<Vec<u8>, RuntimeError> {
            drop(self.stdin);
            if timeout(grace, self.child.wait()).await.is_err() {
                let _ = self.child.kill().await;
                let _ = self.child.wait().await;
            }
            Ok(self.stderr_task.await.unwrap_or_default())
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct JsonRpcExchange {
        pub response: JsonRpcResponse,
        pub notifications: Vec<Value>,
    }

    pub struct McpStdioSession {
        pub session_uuid: Uuid,
        child: Option<ManagedChild>,
        policy: SessionPolicy,
        capabilities: CapabilityPolicy,
        audit: RuntimeAudit,
    }

    impl McpStdioSession {
        pub async fn spawn(
            spec: &ProcessSpec,
            security: &ProcessSecurityPolicy,
            policy: SessionPolicy,
            capabilities: CapabilityPolicy,
            audit: RuntimeAudit,
        ) -> Result<Self, RuntimeError> {
            let validated = security.validate(spec)?;
            let session_uuid = new_uuid_v7();
            let child = controlled(
                ManagedChild::spawn(&validated),
                Duration::from_millis(policy.startup_timeout_ms),
                Cancellation::default(),
            ).await?;
            let this = Self { session_uuid, child: Some(child), policy, capabilities, audit };
            let _ = this.audit.record(this.session_uuid, "mcp.session", "mcp.process.spawned", EvidenceOutcome::Succeeded, json!({"executable": validated.executable}), json!({}))?;
            Ok(this)
        }

        fn child(&mut self) -> Result<&mut ManagedChild, RuntimeError> {
            self.child.as_mut().ok_or(RuntimeError::SessionClosed)
        }

        pub async fn request(&mut self, request: JsonRpcRequest, cancellation: Cancellation) -> Result<JsonRpcExchange, RuntimeError> {
            self.capabilities.allow_method(&request.method)?;
            if request.method == "tools/call" {
                if let Some(name) = request.params.as_ref().and_then(|p| p.get("name")).and_then(Value::as_str) {
                    self.capabilities.allow_tool(name)?;
                }
            }
            let request_id = request.id.clone();
            let method = request.method.clone();
            let wire = serde_json::to_value(&request)?;
            self.child()?.write_line(&wire).await?;
            let timeout_duration = Duration::from_millis(self.policy.request_timeout_ms);
            let max = self.policy.max_frame_bytes;
            let max_unsolicited = self.policy.max_unsolicited_messages;
            let is_initialize = method == "initialize";
            let mut notifications = Vec::new();
            loop {
                let read = self.child()?.read_line(max);
                enum ReadOutcome { Cancelled, Value(Result<Result<Value, RuntimeError>, tokio::time::error::Elapsed>) }
                let outcome = if is_initialize {
                    ReadOutcome::Value(timeout(timeout_duration, read).await)
                } else {
                    tokio::select! {
                        _ = cancellation.cancelled() => ReadOutcome::Cancelled,
                        result = timeout(timeout_duration, read) => ReadOutcome::Value(result),
                    }
                };
                let value = match outcome {
                    ReadOutcome::Cancelled => {
                        let cancel = json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":request_id,"reason":"client_cancelled"}});
                        let _ = self.child()?.write_line(&cancel).await;
                        let _ = self.audit.record(self.session_uuid, "mcp.request", "mcp.request.cancelled", EvidenceOutcome::Cancelled, json!({"method":method}), json!({}))?;
                        return Err(RuntimeError::Cancelled);
                    }
                    ReadOutcome::Value(Ok(v)) => v?,
                    ReadOutcome::Value(Err(_)) => return Err(RuntimeError::Timeout),
                };
                match classify_jsonrpc_message(&value).map_err(|e| RuntimeError::Protocol(e.to_string()))? {
                    JsonRpcMessageClass::Success | JsonRpcMessageClass::Error => {
                        let response: JsonRpcResponse = serde_json::from_value(value)?;
                        if response.id == request_id {
                            let outcome = if response.error.is_some() { EvidenceOutcome::Failed } else { EvidenceOutcome::Succeeded };
                            let _ = self.audit.record(self.session_uuid, "mcp.request", "mcp.request.completed", outcome, json!({"method":method}), json!({"notification_count":notifications.len()}))?;
                            return Ok(JsonRpcExchange { response, notifications });
                        }
                    }
                    _ => {
                        notifications.push(value);
                        if notifications.len() > max_unsolicited { return Err(RuntimeError::TooManyUnsolicited); }
                    }
                }
            }
        }

        pub async fn initialize_legacy(&mut self, client_name: &str, client_version: &str) -> Result<JsonRpcExchange, RuntimeError> {
            let req = JsonRpcRequest::new("initialize", json!({
                "protocolVersion": MCP_DEFAULT_LEGACY_PROTOCOL,
                "capabilities": {},
                "clientInfo": {"name":client_name,"version":client_version}
            }));
            let exchange = self.request(req, Cancellation::default()).await?;
            self.child()?.write_line(&json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}})).await?;
            Ok(exchange)
        }

        pub async fn discover_modern(&mut self, client_name: &str, client_version: &str) -> Result<JsonRpcExchange, RuntimeError> {
            let req = JsonRpcRequest::new("server/discover", json!({"_meta":{
                "io.modelcontextprotocol/protocolVersion":MCP_DEFAULT_MODERN_PROTOCOL,
                "io.modelcontextprotocol/clientInfo":{"name":client_name,"version":client_version},
                "io.modelcontextprotocol/clientCapabilities":{}
            }}));
            self.request(req, Cancellation::default()).await
        }

        pub async fn close(mut self) -> Result<Vec<u8>, RuntimeError> {
            let child = self.child.take().ok_or(RuntimeError::SessionClosed)?;
            child.terminate(Duration::from_millis(self.policy.shutdown_timeout_ms)).await
        }
    }

    pub struct LspStdioSession {
        pub session_uuid: Uuid,
        child: Option<ManagedChild>,
        policy: SessionPolicy,
        capabilities: CapabilityPolicy,
        audit: RuntimeAudit,
    }

    impl LspStdioSession {
        pub async fn spawn(
            spec: &ProcessSpec,
            security: &ProcessSecurityPolicy,
            policy: SessionPolicy,
            capabilities: CapabilityPolicy,
            audit: RuntimeAudit,
        ) -> Result<Self, RuntimeError> {
            let validated = security.validate(spec)?;
            let session_uuid = new_uuid_v7();
            let child = controlled(ManagedChild::spawn(&validated), Duration::from_millis(policy.startup_timeout_ms), Cancellation::default()).await?;
            Ok(Self { session_uuid, child: Some(child), policy, capabilities, audit })
        }

        fn child(&mut self) -> Result<&mut ManagedChild, RuntimeError> { self.child.as_mut().ok_or(RuntimeError::SessionClosed) }

        pub async fn notify(&mut self, method: &str, params: Value) -> Result<(), RuntimeError> {
            self.capabilities.allow_method(method)?;
            let value = json!({"jsonrpc":"2.0","method":method,"params":params});
            self.child()?.write_content_length(&value).await
        }

        pub async fn request(&mut self, request: JsonRpcRequest, cancellation: Cancellation) -> Result<JsonRpcExchange, RuntimeError> {
            self.capabilities.allow_method(&request.method)?;
            let request_id = request.id.clone();
            let method = request.method.clone();
            self.child()?.write_content_length(&serde_json::to_value(&request)?).await?;
            let duration = Duration::from_millis(self.policy.request_timeout_ms);
            let max = self.policy.max_frame_bytes;
            let mut notifications = Vec::new();
            loop {
                let read = self.child()?.read_content_length(max);
                enum ReadOutcome { Cancelled, Value(Result<Result<Value, RuntimeError>, tokio::time::error::Elapsed>) }
                let outcome = tokio::select! {
                    _ = cancellation.cancelled(), if method != "initialize" => ReadOutcome::Cancelled,
                    result = timeout(duration, read) => ReadOutcome::Value(result),
                };
                let value = match outcome {
                    ReadOutcome::Cancelled => {
                        let cancel = json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":request_id}});
                        let _ = self.child()?.write_content_length(&cancel).await;
                        return Err(RuntimeError::Cancelled);
                    }
                    ReadOutcome::Value(Ok(v)) => v?,
                    ReadOutcome::Value(Err(_)) => return Err(RuntimeError::Timeout),
                };
                match classify_jsonrpc_message(&value).map_err(|e| RuntimeError::Protocol(e.to_string()))? {
                    JsonRpcMessageClass::Success | JsonRpcMessageClass::Error => {
                        let response: JsonRpcResponse = serde_json::from_value(value)?;
                        if response.id == request_id {
                            let outcome=if response.error.is_some(){EvidenceOutcome::Failed}else{EvidenceOutcome::Succeeded};
                            let _=self.audit.record(self.session_uuid,"lsp.request","lsp.request.completed",outcome,json!({"method":method}),json!({"notification_count":notifications.len()}))?;
                            return Ok(JsonRpcExchange{response,notifications});
                        }
                    }
                    _ => {
                        notifications.push(value);
                        if notifications.len()>self.policy.max_unsolicited_messages { return Err(RuntimeError::TooManyUnsolicited); }
                    }
                }
            }
        }

        pub async fn initialize(&mut self, root_uri: Option<&str>, initialization_options: Value) -> Result<JsonRpcExchange, RuntimeError> {
            let req=JsonRpcRequest::new("initialize",json!({
                "processId":std::process::id(),"rootUri":root_uri,"capabilities":{},"initializationOptions":initialization_options
            }));
            let exchange=self.request(req,Cancellation::default()).await?;
            self.notify("initialized",json!({})).await?;
            Ok(exchange)
        }

        pub async fn shutdown(mut self) -> Result<Vec<u8>, RuntimeError> {
            if self.child.is_none() { return Err(RuntimeError::SessionClosed); }
            if self.capabilities.allowed_methods.contains("shutdown") {
                let req=JsonRpcRequest::new("shutdown",Value::Null);
                let _=self.request(req,Cancellation::default()).await;
            }
            if self.capabilities.allowed_methods.contains("exit") {
                let _=self.notify("exit",Value::Null).await;
            }
            let child=self.child.take().ok_or(RuntimeError::SessionClosed)?;
            child.terminate(Duration::from_millis(self.policy.shutdown_timeout_ms)).await
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct HttpEndpointPolicy {
        pub allowed_origins: BTreeSet<String>,
        pub allow_loopback_http: bool,
        pub max_response_bytes: usize,
        pub connect_timeout_ms: u64,
    }

    impl Default for HttpEndpointPolicy {
        fn default() -> Self {
            Self { allowed_origins:BTreeSet::new(),allow_loopback_http:true,max_response_bytes:DEFAULT_MAX_FRAME_BYTES,connect_timeout_ms:10_000 }
        }
    }

    impl HttpEndpointPolicy {
        fn validate(&self, endpoint: &Url) -> Result<(), RuntimeError> {
            let host=endpoint.host_str().ok_or_else(||RuntimeError::PolicyDenied("endpoint has no host".into()))?;
            let loopback=matches!(host,"localhost"|"127.0.0.1"|"::1");
            if endpoint.scheme()!="https" && !(self.allow_loopback_http && loopback && endpoint.scheme()=="http") {
                return Err(RuntimeError::PolicyDenied("remote MCP requires HTTPS; HTTP is loopback-only".into()));
            }
            let origin=endpoint.origin().ascii_serialization();
            if !self.allowed_origins.contains(&origin) {
                return Err(RuntimeError::PolicyDenied(format!("origin not allowlisted: {origin}")));
            }
            Ok(())
        }
    }

    pub struct McpStreamableHttpClient {
        pub session_uuid: Uuid,
        endpoint: Url,
        client: reqwest::Client,
        policy: SessionPolicy,
        endpoint_policy: HttpEndpointPolicy,
        capabilities: CapabilityPolicy,
        audit: RuntimeAudit,
    }

    impl McpStreamableHttpClient {
        pub fn new(endpoint: Url, policy:SessionPolicy, endpoint_policy:HttpEndpointPolicy, capabilities:CapabilityPolicy, audit:RuntimeAudit) -> Result<Self,RuntimeError> {
            endpoint_policy.validate(&endpoint)?;
            let client=reqwest::Client::builder()
                .connect_timeout(Duration::from_millis(endpoint_policy.connect_timeout_ms))
                .build()?;
            Ok(Self{session_uuid:new_uuid_v7(),endpoint,client,policy,endpoint_policy,capabilities,audit})
        }

        pub async fn request(&self, request:JsonRpcRequest, protocol_version:&str, cancellation:Cancellation) -> Result<JsonRpcExchange,RuntimeError> {
            self.capabilities.allow_method(&request.method)?;
            if request.method=="tools/call" {
                if let Some(name)=request.params.as_ref().and_then(|p|p.get("name")).and_then(Value::as_str) { self.capabilities.allow_tool(name)?; }
            }
            let request_id=request.id.clone();
            let method=request.method.clone();
            let mut builder=self.client.post(self.endpoint.clone())
                .header("Accept","application/json, text/event-stream")
                .header("Content-Type","application/json")
                .header("MCP-Protocol-Version",protocol_version)
                .json(&request);
            if protocol_version==MCP_DEFAULT_MODERN_PROTOCOL {
                builder=builder.header("Mcp-Method",&method);
                if let Some(name)=request.params.as_ref().and_then(|p|p.get("name")).and_then(Value::as_str) { builder=builder.header("Mcp-Name",name); }
            }
            let response=controlled(async { Ok(builder.send().await?) },Duration::from_millis(self.policy.request_timeout_ms),cancellation.clone()).await?;
            let status=response.status();
            if !status.is_success() { return Err(RuntimeError::HttpStatus(status.as_u16())); }
            let content_type=response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v|v.to_str().ok()).unwrap_or("").to_owned();
            let exchange=if content_type.contains("text/event-stream") {
                self.read_sse_response(response,request_id.clone(),cancellation).await?
            } else {
                let bytes=response.bytes().await?;
                if bytes.len()>self.endpoint_policy.max_response_bytes { return Err(RuntimeError::FrameTooLarge(bytes.len())); }
                let value:Value=serde_json::from_slice(&bytes)?;
                let response:JsonRpcResponse=serde_json::from_value(value)?;
                if response.id!=request_id { return Err(RuntimeError::ResponseIdMismatch); }
                JsonRpcExchange{response,notifications:Vec::new()}
            };
            let outcome=if exchange.response.error.is_some(){EvidenceOutcome::Failed}else{EvidenceOutcome::Succeeded};
            let _=self.audit.record(self.session_uuid,"mcp.request","mcp.http.completed",outcome,json!({"method":method,"endpoint":self.endpoint}),json!({"notifications":exchange.notifications.len()}))?;
            Ok(exchange)
        }

        async fn read_sse_response(&self, response:reqwest::Response, request_id:JsonRpcId, cancellation:Cancellation) -> Result<JsonRpcExchange,RuntimeError> {
            let mut stream=response.bytes_stream();
            let mut pending=Vec::<u8>::new();
            let mut notifications=Vec::new();
            loop {
                let next=tokio::select! {
                    _=cancellation.cancelled()=>return Err(RuntimeError::Cancelled),
                    result=timeout(Duration::from_millis(self.policy.request_timeout_ms),stream.next())=>match result{Ok(v)=>v,Err(_)=>return Err(RuntimeError::Timeout)}
                };
                let Some(chunk)=next else{return Err(RuntimeError::ProcessClosed)};
                pending.extend_from_slice(&chunk?);
                if pending.len()>self.endpoint_policy.max_response_bytes{return Err(RuntimeError::FrameTooLarge(pending.len()));}
                while let Some(pos)=pending.windows(2).position(|w|w==b"\n\n") {
                    let event=pending.drain(..pos+2).collect::<Vec<_>>();
                    let text=String::from_utf8_lossy(&event);
                    for line in text.lines() {
                        if let Some(data)=line.strip_prefix("data:") {
                            let value:Value=serde_json::from_str(data.trim())?;
                            match classify_jsonrpc_message(&value).map_err(|e|RuntimeError::Protocol(e.to_string()))? {
                                JsonRpcMessageClass::Success|JsonRpcMessageClass::Error=>{
                                    let response:JsonRpcResponse=serde_json::from_value(value)?;
                                    if response.id==request_id{return Ok(JsonRpcExchange{response,notifications});}
                                }
                                _=>{
                                    notifications.push(value);
                                    if notifications.len()>self.policy.max_unsolicited_messages{return Err(RuntimeError::TooManyUnsolicited);}
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ReconnectPolicy {
        pub max_attempts:u32,
        pub initial_backoff_ms:u64,
        pub max_backoff_ms:u64,
    }

    impl ReconnectPolicy {
        pub async fn run<T,F,Fut>(&self, mut operation:F)->Result<T,RuntimeError>
        where F:FnMut(u32)->Fut, Fut:Future<Output=Result<T,RuntimeError>> {
            let mut delay=self.initial_backoff_ms.max(1);
            for attempt in 0..=self.max_attempts {
                match operation(attempt).await {
                    Ok(v)=>return Ok(v),
                    Err(e) if attempt<self.max_attempts && e.is_reconnectable()=>{
                        sleep(Duration::from_millis(delay)).await;
                        delay=(delay.saturating_mul(2)).min(self.max_backoff_ms.max(delay));
                    }
                    Err(e)=>return Err(e),
                }
            }
            Err(RuntimeError::ReconnectExhausted)
        }
    }

    #[derive(Debug, Error)]
    pub enum RuntimeError {
        #[error("policy denied: {0}")]
        PolicyDenied(String),
        #[error("capability denied: {0}")]
        CapabilityDenied(String),
        #[error("invalid process spec: {0}")]
        ProcessSpec(String),
        #[error("missing child pipe: {0}")]
        MissingPipe(&'static str),
        #[error("session is closed")]
        SessionClosed,
        #[error("process/stream closed")]
        ProcessClosed,
        #[error("request cancelled")]
        Cancelled,
        #[error("request timeout")]
        Timeout,
        #[error("frame too large: {0} bytes")]
        FrameTooLarge(usize),
        #[error("too many unsolicited messages")]
        TooManyUnsolicited,
        #[error("response id mismatch")]
        ResponseIdMismatch,
        #[error("HTTP status {0}")]
        HttpStatus(u16),
        #[error("reconnect attempts exhausted")]
        ReconnectExhausted,
        #[error("protocol error: {0}")]
        Protocol(String),
        #[error(transparent)]
        Io(#[from] std::io::Error),
        #[error(transparent)]
        Json(#[from] serde_json::Error),
        #[error(transparent)]
        Http(#[from] reqwest::Error),
        #[error(transparent)]
        Url(#[from] url::ParseError),
        #[error(transparent)]
        Ledger(#[from] LedgerError),
    }

    impl RuntimeError {
        pub fn is_reconnectable(&self)->bool {
            matches!(self,RuntimeError::ProcessClosed|RuntimeError::Timeout|RuntimeError::Http(_)|RuntimeError::HttpStatus(502|503|504))
        }
    }

    pub fn websocket_is_extension_transport() -> bool { true }
    pub fn mcp_standard_remote_transport_note() -> &'static str {
        "MCP standard remote transport is Streamable HTTP; WebSocket remains an explicitly non-standard extension in PhxClaw."
    }

    #[cfg(test)]
    mod managed_tests {
        use super::*;

        #[test]
        fn deny_by_default_process_policy() {
            let policy=ProcessSecurityPolicy::default();
            let spec=ProcessSpec{executable:"/definitely/not/allowed".into(),args:vec![],cwd:"/".into(),env:BTreeMap::new()};
            assert!(policy.validate(&spec).is_err());
        }

        #[test]
        fn capability_policy_is_deny_by_default() {
            assert!(CapabilityPolicy::deny_all().allow_method("tools/list").is_err());
        }

        #[tokio::test]
        async fn cancellation_wakes_waiter() {
            let token=Cancellation::default();
            let other=token.clone();
            tokio::spawn(async move{sleep(Duration::from_millis(5)).await;other.cancel();});
            token.cancelled().await;
            assert!(token.is_cancelled());
        }

        #[tokio::test]
        async fn reconnect_policy_retries_only_reconnectable_errors() {
            let policy=ReconnectPolicy{max_attempts:2,initial_backoff_ms:1,max_backoff_ms:2};
            let mut n=0;
            let value=policy.run(|_|{n+=1;let now=n;async move{if now<2{Err(RuntimeError::Timeout)}else{Ok(7)}}}).await.unwrap();
            assert_eq!(value,7);
        }
    }
}

pub use managed_runtime::*;

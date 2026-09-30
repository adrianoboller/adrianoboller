#![forbid(unsafe_code)]

//! RustClaw 0.5.0 MIT-derived compatibility primitives for PhxClaw.
//!
//! Upstream concepts adapted from:
//! - `src/gateway/protocol.rs`
//! - `src/session/memory.rs`
//! - `src/session/store.rs`
//! - `src/cron/mod.rs`
//! - `src/tools/mcp.rs`
//!
//! The original MIT license is preserved in
//! `LICENSE-UPSTREAM-RUSTCLAW.md`, neste crate.

use chrono::{DateTime, Utc};
use phxclaw_mcp_lsp_runtime::{
    ProtocolError, decode_content_length_message, decode_mcp_line, encode_content_length_message,
    encode_mcp_line, qualified_tool_name,
};
use phxclaw_memory_context::MemoryScope;
use phxclaw_types::new_uuid_v7;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

pub const RUSTCLAW_UPSTREAM_VERSION: &str = "0.5.0";
pub const RUSTCLAW_UPSTREAM_LICENSE: &str = "MIT";

// ── Gateway protocol ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GatewayInboundFrame {
    Connect(ConnectRequest),
    Auth(AuthProof),
    Req(GatewayRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectRequest {
    pub params: ConnectParams,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectParams {
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub min_protocol: Option<u32>,
    #[serde(default)]
    pub max_protocol: Option<u32>,
    /// Opaque PhxClaw credential handle. Never a plaintext token.
    #[serde(default)]
    pub credential_handle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthProof {
    pub nonce: String,
    pub credential_lease_uuid: Uuid,
    pub proof_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayRequest {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayOutboundFrame {
    pub r#type: String,
    pub message_uuid: Uuid,
    pub correlation_uuid: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
}

impl GatewayOutboundFrame {
    pub fn challenge(correlation_uuid: Uuid, nonce: &str, timestamp_ms: i64) -> Self {
        Self {
            r#type: "event".into(),
            message_uuid: new_uuid_v7(),
            correlation_uuid,
            id: None,
            event: Some("connect.challenge".into()),
            status: None,
            code: None,
            message: None,
            payload: Some(json!({"nonce": nonce, "ts": timestamp_ms})),
        }
    }

    pub fn hello_ok(correlation_uuid: Uuid, device_binding_uuid: Uuid) -> Self {
        Self {
            r#type: "res".into(),
            message_uuid: new_uuid_v7(),
            correlation_uuid,
            id: None,
            event: None,
            status: Some("ok".into()),
            code: None,
            message: None,
            payload: Some(json!({
                "hello": "ok",
                "deviceBindingUuid": device_binding_uuid,
                "snapshot": {}
            })),
        }
    }

    pub fn error(correlation_uuid: Uuid, code: u32, message: impl Into<String>) -> Self {
        Self {
            r#type: "error".into(),
            message_uuid: new_uuid_v7(),
            correlation_uuid,
            id: None,
            event: None,
            status: Some("error".into()),
            code: Some(code),
            message: Some(message.into()),
            payload: None,
        }
    }
}

pub fn negotiate_gateway_protocol(
    client_min: Option<u32>,
    client_max: Option<u32>,
    supported: &[u32],
) -> Result<u32, RustClawNativeError> {
    let min = client_min.unwrap_or(1);
    let max = client_max.unwrap_or(u32::MAX);
    supported
        .iter()
        .copied()
        .filter(|version| *version >= min && *version <= max)
        .max()
        .ok_or(RustClawNativeError::NoCompatibleProtocol)
}

// ── Session / memory compatibility ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMessage {
    pub uuid: Uuid,
    pub session_uuid: Uuid,
    pub role: String,
    pub content: String,
    pub content_sha256: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeSession {
    pub uuid: Uuid,
    pub principal_uuid: Option<Uuid>,
    pub channel: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<SessionMessage>,
}

#[derive(Debug, Default)]
pub struct NativeSessionStore {
    sessions: BTreeMap<Uuid, NativeSession>,
}

impl NativeSessionStore {
    pub fn create(
        &mut self,
        principal_uuid: Option<Uuid>,
        channel: Option<String>,
    ) -> NativeSession {
        let now = Utc::now();
        let session = NativeSession {
            uuid: new_uuid_v7(),
            principal_uuid,
            channel,
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        };
        self.sessions.insert(session.uuid, session.clone());
        session
    }

    pub fn push(
        &mut self,
        session_uuid: Uuid,
        role: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<SessionMessage, RustClawNativeError> {
        let content = content.into();
        let message = SessionMessage {
            uuid: new_uuid_v7(),
            session_uuid,
            role: role.into(),
            content_sha256: sha256_hex(content.as_bytes()),
            content,
            created_at: Utc::now(),
        };
        let session = self
            .sessions
            .get_mut(&session_uuid)
            .ok_or(RustClawNativeError::UnknownSession(session_uuid))?;
        session.messages.push(message.clone());
        session.updated_at = Utc::now();
        Ok(message)
    }

    pub fn history(&self, session_uuid: Uuid) -> Result<&[SessionMessage], RustClawNativeError> {
        self.sessions
            .get(&session_uuid)
            .map(|session| session.messages.as_slice())
            .ok_or(RustClawNativeError::UnknownSession(session_uuid))
    }
}

pub fn phoenix_memory_scope(session_uuid: Uuid) -> MemoryScope {
    MemoryScope::Session(session_uuid)
}

pub fn external_user_scope(channel: &str, external_user_id: &str) -> String {
    format!("channel:{channel}:user:{external_user_id}")
}

// ── Cron / scheduled job model ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum ScheduleSpec {
    EverySeconds(u64),
    CronExpression(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScheduledJobState {
    Disabled,
    Ready,
    Running,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeScheduledJob {
    pub uuid: Uuid,
    pub name: String,
    pub capability: String,
    pub payload: Value,
    pub schedule: ScheduleSpec,
    pub state: ScheduledJobState,
    pub created_at: DateTime<Utc>,
}

impl NativeScheduledJob {
    pub fn new(
        name: impl Into<String>,
        capability: impl Into<String>,
        payload: Value,
        schedule: ScheduleSpec,
    ) -> Result<Self, RustClawNativeError> {
        validate_schedule(&schedule)?;
        Ok(Self {
            uuid: new_uuid_v7(),
            name: name.into(),
            capability: capability.into(),
            payload,
            schedule,
            state: ScheduledJobState::Disabled,
            created_at: Utc::now(),
        })
    }
}

pub fn validate_schedule(schedule: &ScheduleSpec) -> Result<(), RustClawNativeError> {
    match schedule {
        ScheduleSpec::EverySeconds(seconds) if *seconds >= 60 => Ok(()),
        ScheduleSpec::EverySeconds(_) => Err(RustClawNativeError::ScheduleTooFrequent),
        ScheduleSpec::CronExpression(expr) => {
            if expr.len() > 256 || expr.chars().any(|ch| matches!(ch, '\n' | '\r' | '\0')) {
                return Err(RustClawNativeError::InvalidCronExpression);
            }
            let fields = expr.split_whitespace().count();
            if matches!(fields, 5..=7) {
                Ok(())
            } else {
                Err(RustClawNativeError::InvalidCronExpression)
            }
        }
    }
}

/// Proxima execucao de um agendamento depois de `after` (UTC). Antes disto o modelo so
/// VALIDAVA a expressao: nenhum agendamento chegava a disparar.
///
/// Cron de 5 campos (minuto hora dia mes dia-da-semana) com `*`, listas, faixas e passos
/// (`*/15`, `1-5`, `0,30`). Dia-da-semana 0 ou 7 = domingo. Quando dia e dia-da-semana
/// estao restritos os dois, basta um casar (regra do cron classico). Busca de minuto em
/// minuto por ate 366 dias: expressao que nunca casa devolve None em vez de girar para sempre.
pub fn next_fire(schedule: &ScheduleSpec, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
    use chrono::{Datelike, Duration, Timelike};
    match schedule {
        ScheduleSpec::EverySeconds(s) => Some(after + Duration::seconds(*s as i64)),
        ScheduleSpec::CronExpression(expr) => {
            let f: Vec<&str> = expr.split_whitespace().collect();
            if f.len() != 5 {
                return None;
            }
            let minutos = cron_field(f[0], 0, 59)?;
            let horas = cron_field(f[1], 0, 23)?;
            let dias = cron_field(f[2], 1, 31)?;
            let meses = cron_field(f[3], 1, 12)?;
            let mut semana = cron_field(f[4], 0, 7)?;
            if semana[7] {
                semana[0] = true;
            }
            let dia_restrito = f[2] != "*";
            let semana_restrita = f[4] != "*";
            let mut t = after.with_second(0)?.with_nanosecond(0)? + Duration::minutes(1);
            for _ in 0..(366 * 24 * 60) {
                let dia_ok = dias[t.day() as usize];
                let sem_ok = semana[t.weekday().num_days_from_sunday() as usize];
                let data_ok = match (dia_restrito, semana_restrita) {
                    (true, true) => dia_ok || sem_ok,
                    (true, false) => dia_ok,
                    (false, true) => sem_ok,
                    (false, false) => true,
                };
                if data_ok
                    && meses[t.month() as usize]
                    && horas[t.hour() as usize]
                    && minutos[t.minute() as usize]
                {
                    return Some(t);
                }
                t += Duration::minutes(1);
            }
            None
        }
    }
}

fn cron_field(campo: &str, min: u32, max: u32) -> Option<Vec<bool>> {
    let mut v = vec![false; max as usize + 1];
    for parte in campo.split(',') {
        let (faixa, passo) = match parte.split_once('/') {
            Some((f, p)) => (f, p.parse::<u32>().ok().filter(|p| *p > 0)?),
            None => (parte, 1),
        };
        let (a, b) = if faixa == "*" {
            (min, max)
        } else if let Some((a, b)) = faixa.split_once('-') {
            (a.parse().ok()?, b.parse().ok()?)
        } else {
            let n: u32 = faixa.parse().ok()?;
            (n, if parte.contains('/') { max } else { n })
        };
        if a < min || b > max || a > b {
            return None;
        }
        let mut x = a;
        while x <= b {
            v[x as usize] = true;
            x += passo;
        }
    }
    Some(v)
}

// ── MCP compatibility ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpWireMode {
    ContentLength,
    Ndjson,
}

pub fn encode_mcp_compat(mode: McpWireMode, value: &Value) -> Vec<u8> {
    match mode {
        McpWireMode::ContentLength => encode_content_length_message(value),
        McpWireMode::Ndjson => encode_mcp_line(value),
    }
}

pub fn decode_mcp_compat(mode: McpWireMode, input: &[u8]) -> Result<(Value, usize), ProtocolError> {
    match mode {
        McpWireMode::ContentLength => decode_content_length_message(input),
        McpWireMode::Ndjson => decode_mcp_line(input),
    }
}

pub fn rustclaw_legacy_tool_name(server_name: &str, tool_name: &str) -> String {
    format!(
        "mcp_{}_{}",
        normalize_legacy_name(server_name),
        normalize_legacy_name(tool_name)
    )
}

pub fn phoenix_tool_name(server_name: &str, tool_name: &str) -> String {
    qualified_tool_name(server_name, tool_name)
}

fn normalize_legacy_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeFeatureMap {
    pub gateway_protocol: bool,
    pub session_memory: bool,
    pub cron_model: bool,
    pub mcp_ndjson: bool,
    pub mcp_content_length: bool,
    pub secret_plaintext_in_protocol: bool,
}

impl Default for NativeFeatureMap {
    fn default() -> Self {
        Self {
            gateway_protocol: true,
            session_memory: true,
            cron_model: true,
            mcp_ndjson: true,
            mcp_content_length: true,
            secret_plaintext_in_protocol: false,
        }
    }
}

#[derive(Debug, Error)]
pub enum RustClawNativeError {
    #[error("no compatible gateway protocol version")]
    NoCompatibleProtocol,
    #[error("unknown session {0}")]
    UnknownSession(Uuid),
    #[error("schedule frequency below PhxClaw minimum")]
    ScheduleTooFrequent,
    #[error("invalid cron expression")]
    InvalidCronExpression,
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_never_contains_plaintext_token_field() {
        let frame = ConnectParams {
            device_id: Some("dev-1".into()),
            role: Some("operator".into()),
            min_protocol: Some(1),
            max_protocol: Some(3),
            credential_handle: Some("secret://lease/opaque".into()),
        };
        let text = serde_json::to_string(&frame).unwrap();
        assert!(!text.contains("\"token\""));
        assert!(text.contains("credentialHandle"));
    }

    #[test]
    fn session_ids_are_uuid_v7() {
        let mut store = NativeSessionStore::default();
        let session = store.create(None, Some("webchat".into()));
        assert_eq!(session.uuid.get_version_num(), 7);
        let msg = store.push(session.uuid, "user", "hello").unwrap();
        assert_eq!(msg.uuid.get_version_num(), 7);
        assert_eq!(store.history(session.uuid).unwrap().len(), 1);
    }

    #[test]
    fn cron_minimum_is_one_minute() {
        assert!(validate_schedule(&ScheduleSpec::EverySeconds(60)).is_ok());
        assert!(validate_schedule(&ScheduleSpec::EverySeconds(10)).is_err());
    }

    #[test]
    fn mcp_supports_both_framings() {
        let value = json!({"jsonrpc":"2.0","id":1,"result":{"ok":true}});
        for mode in [McpWireMode::ContentLength, McpWireMode::Ndjson] {
            let encoded = encode_mcp_compat(mode, &value);
            let (decoded, used) = decode_mcp_compat(mode, &encoded).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(used, encoded.len());
        }
    }

    #[test]
    fn canonical_mcp_name_is_collision_resistant() {
        assert_eq!(
            phoenix_tool_name("server.one", "weather tool"),
            "mcp__server_one__weather_tool"
        );
        assert_eq!(
            rustclaw_legacy_tool_name("server.one", "weather tool"),
            "mcp_server_one_weather_tool"
        );
    }

    fn t(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    #[test]
    fn cron_calcula_a_proxima_execucao() {
        let c = |e: &str| ScheduleSpec::CronExpression(e.into());
        // a cada 15 min
        assert_eq!(
            next_fire(&c("*/15 * * * *"), t("2026-09-30T10:07:30Z")),
            Some(t("2026-09-30T10:15:00Z"))
        );
        // dias uteis as 08:45 -- 2026-10-03 e sabado, pula para segunda 05/10
        assert_eq!(
            next_fire(&c("45 8 * * 1-5"), t("2026-10-02T09:00:00Z")),
            Some(t("2026-10-05T08:45:00Z"))
        );
        // domingo como 7
        assert_eq!(
            next_fire(&c("0 12 * * 7"), t("2026-09-30T00:00:00Z")),
            Some(t("2026-10-04T12:00:00Z"))
        );
        // estritamente depois: no proprio minuto nao dispara de novo
        assert_eq!(
            next_fire(&c("0 9 * * *"), t("2026-09-30T09:00:00Z")),
            Some(t("2026-10-01T09:00:00Z"))
        );
        // 31 de fevereiro nunca existe: None, sem laco infinito
        assert_eq!(next_fire(&c("0 0 31 2 *"), t("2026-01-01T00:00:00Z")), None);
        assert_eq!(next_fire(&c("61 * * * *"), t("2026-01-01T00:00:00Z")), None);
    }
}

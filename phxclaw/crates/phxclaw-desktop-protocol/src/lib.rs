use chrono::{DateTime, Utc};
use phxclaw_system_automation::{CommandRequest, InputAction};
use phxclaw_types::new_uuid_v7;
use phxclaw_webview_control::WebViewCommand;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use uuid::Uuid;

pub const DESKTOP_PROTOCOL: &str = "phxclaw-desktop-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopActionRequest {
    pub protocol: String,
    pub uuid: Uuid,
    pub correlation_uuid: Option<Uuid>,
    pub actor: String,
    pub capability: String,
    pub action: DesktopAction,
    pub requested_at: DateTime<Utc>,
}

impl DesktopActionRequest {
    pub fn new(actor: impl Into<String>, capability: impl Into<String>, action: DesktopAction) -> Self {
        Self {
            protocol: DESKTOP_PROTOCOL.into(),
            uuid: new_uuid_v7(),
            correlation_uuid: None,
            actor: actor.into(),
            capability: capability.into(),
            action,
            requested_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DesktopAction {
    LaunchApplication {
        program: String,
        args: Vec<String>,
        cwd: Option<PathBuf>,
    },
    ExecuteCommand {
        request: CommandRequest,
    },
    Input {
        action: InputAction,
    },
    CaptureScreen {
        format: String,
        target: String,
    },
    WebView {
        view_label: String,
        command: WebViewCommand,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DesktopActionStatus {
    Accepted,
    Succeeded,
    Failed,
    Denied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopActionResult {
    pub protocol: String,
    pub uuid: Uuid,
    pub request_uuid: Uuid,
    pub correlation_uuid: Option<Uuid>,
    pub status: DesktopActionStatus,
    pub output: Value,
    pub evidence_uuid: Option<Uuid>,
    pub finished_at: DateTime<Utc>,
}

impl DesktopActionResult {
    pub fn finished(request: &DesktopActionRequest, status: DesktopActionStatus, output: Value) -> Self {
        Self {
            protocol: DESKTOP_PROTOCOL.into(),
            uuid: new_uuid_v7(),
            request_uuid: request.uuid,
            correlation_uuid: request.correlation_uuid,
            status,
            output,
            evidence_uuid: None,
            finished_at: Utc::now(),
        }
    }
}

use chrono::{DateTime, Utc};
use phxclaw_types::{new_uuid_v7, PermissionClaim};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const PLUGIN_SDK_VERSION: &str = "0.20.0";
pub const EXTENSION_PROTOCOL_V1: &str = "phxclaw-extension-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionRuntimeKind {
    Process,
    Wasm,
    InProcess,
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionHook {
    BeforeMessage,
    AfterMessage,
    BeforeToolCall,
    AfterToolCall,
    SessionStart,
    SessionEnd,
    AgentResponse,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookInvocation {
    pub uuid: Uuid,
    pub correlation_uuid: Uuid,
    pub hook: ExtensionHook,
    pub actor: String,
    pub payload: Value,
    pub invoked_at: DateTime<Utc>,
}

impl HookInvocation {
    pub fn new(correlation_uuid: Uuid, hook: ExtensionHook, actor: impl Into<String>, payload: Value) -> Self {
        Self {
            uuid: new_uuid_v7(),
            correlation_uuid,
            hook,
            actor: actor.into(),
            payload,
            invoked_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionPointKind {
    Capability,
    EventSubscriber,
    UiPanel,
    CompilerFrontend,
    StorageProvider,
    DataProvider,
    ModelProvider,
    Channel,
    DeviceNode,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionPointDescriptor {
    pub name: String,
    pub kind: ExtensionPointKind,
    pub contract_version: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHello {
    pub protocol: String,
    pub plugin_uuid: Uuid,
    pub plugin_name: String,
    pub plugin_version: String,
    pub sdk_version: String,
    pub capabilities: Vec<String>,
    pub extension_points: Vec<ExtensionPointDescriptor>,
    pub emitted_at: DateTime<Utc>,
}

impl PluginHello {
    pub fn new(
        plugin_uuid: Uuid,
        plugin_name: impl Into<String>,
        plugin_version: impl Into<String>,
        capabilities: Vec<String>,
        extension_points: Vec<ExtensionPointDescriptor>,
    ) -> Self {
        Self {
            protocol: EXTENSION_PROTOCOL_V1.into(),
            plugin_uuid,
            plugin_name: plugin_name.into(),
            plugin_version: plugin_version.into(),
            sdk_version: PLUGIN_SDK_VERSION.into(),
            capabilities,
            extension_points,
            emitted_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostHello {
    pub protocol: String,
    pub core_version: String,
    pub core_api_version: String,
    pub session_uuid: Uuid,
    pub allowed_capabilities: Vec<String>,
    pub emitted_at: DateTime<Utc>,
}

impl HostHello {
    pub fn new(
        core_version: impl Into<String>,
        core_api_version: impl Into<String>,
        allowed_capabilities: Vec<String>,
    ) -> Self {
        Self {
            protocol: EXTENSION_PROTOCOL_V1.into(),
            core_version: core_version.into(),
            core_api_version: core_api_version.into(),
            session_uuid: new_uuid_v7(),
            allowed_capabilities,
            emitted_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityInvocation {
    pub uuid: Uuid,
    pub correlation_uuid: Uuid,
    pub causation_uuid: Option<Uuid>,
    pub actor: String,
    pub capability: String,
    pub requested_permissions: Vec<PermissionClaim>,
    pub payload: Value,
    pub requested_at: DateTime<Utc>,
}

impl CapabilityInvocation {
    pub fn new(
        correlation_uuid: Uuid,
        actor: impl Into<String>,
        capability: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            uuid: new_uuid_v7(),
            correlation_uuid,
            causation_uuid: None,
            actor: actor.into(),
            capability: capability.into(),
            requested_permissions: Vec::new(),
            payload,
            requested_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InvocationStatus {
    Succeeded,
    Failed,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityResult {
    pub uuid: Uuid,
    pub invocation_uuid: Uuid,
    pub plugin_uuid: Uuid,
    pub status: InvocationStatus,
    pub payload: Value,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub emitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventSubscription {
    pub topic: String,
    pub event_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginHealth {
    pub healthy: bool,
    pub message: String,
    pub checked_at: DateTime<Utc>,
}

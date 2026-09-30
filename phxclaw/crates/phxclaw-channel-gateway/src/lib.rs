#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use phxclaw_evidence_ledger::{EvidenceDraft, EvidenceLedger, EvidenceOutcome, LedgerError};
use phxclaw_live_bus::{LiveBusError, LiveEventHub};
use phxclaw_types::new_uuid_v7;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::{BTreeMap, BTreeSet}, sync::{Arc, Mutex}};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentityState { Pending, Active, Blocked }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageDirection { Inbound, Outbound }

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ChannelCapabilities {
    pub text: bool,
    pub images: bool,
    pub videos: bool,
    pub voice: bool,
    pub files: bool,
    pub threads: bool,
    pub reactions: bool,
    pub editing: bool,
    pub deletion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelProbe {
    pub connected: bool,
    pub account_id: Option<String>,
    pub display_name: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelAllowlistEntry {
    pub channel: String,
    pub external_user_id: String,
    pub label: Option<String>,
}

impl ChannelAllowlistEntry {
    pub fn matches(&self, channel: &str, external_user_id: &str) -> bool {
        (self.channel == "*" || self.channel == channel)
            && (self.external_user_id == "*" || self.external_user_id == external_user_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelAllowlist {
    pub entries: Vec<ChannelAllowlistEntry>,
    pub default_allow: bool,
}

impl ChannelAllowlist {
    pub fn allows(&self, channel: &str, external_user_id: &str) -> bool {
        self.entries.iter().any(|entry| entry.matches(channel, external_user_id)) || self.default_allow
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelRouteRule {
    pub channel: Option<String>,
    pub external_user_id: Option<String>,
    pub agent_name: String,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelAgentRouter {
    pub rules: Vec<ChannelRouteRule>,
    pub default_agent: String,
}

impl ChannelAgentRouter {
    pub fn new(default_agent: impl Into<String>) -> Self {
        Self { rules: Vec::new(), default_agent: default_agent.into() }
    }

    pub fn add_rule(&mut self, rule: ChannelRouteRule) {
        self.rules.push(rule);
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.agent_name.cmp(&b.agent_name)));
    }

    pub fn route(&self, channel: &str, external_user_id: &str) -> &str {
        for rule in &self.rules {
            let channel_ok = rule.channel.as_deref().is_none_or(|value| value == "*" || value == channel);
            let user_ok = rule.external_user_id.as_deref().is_none_or(|value| value == "*" || value == external_user_id);
            if channel_ok && user_ok { return &rule.agent_name; }
        }
        &self.default_agent
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityBinding {
    pub uuid: Uuid,
    pub channel: String,
    pub account_id: String,
    pub external_user_id: String,
    pub principal_uuid: Uuid,
    pub state: IdentityState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelSession {
    pub uuid: Uuid,
    pub principal_uuid: Uuid,
    pub channel: String,
    pub account_id: String,
    pub conversation_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelAttachment {
    pub media_type: String,
    pub uri: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub uuid: Uuid,
    pub external_message_id: String,
    pub channel: String,
    pub account_id: String,
    pub external_user_id: String,
    pub conversation_id: String,
    pub text: String,
    #[serde(default)]
    pub attachments: Vec<ChannelAttachment>,
    #[serde(default)]
    pub metadata: Value,
    pub received_at: DateTime<Utc>,
}

impl InboundMessage {
    pub fn new(channel: impl Into<String>, account_id: impl Into<String>, external_user_id: impl Into<String>, conversation_id: impl Into<String>, external_message_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            uuid: new_uuid_v7(), external_message_id: external_message_id.into(), channel: channel.into(), account_id: account_id.into(),
            external_user_id: external_user_id.into(), conversation_id: conversation_id.into(), text: text.into(), attachments: vec![], metadata: json!({}), received_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutedInbound {
    pub message_uuid: Uuid,
    pub principal_uuid: Uuid,
    pub session_uuid: Uuid,
    pub channel: String,
    pub conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    pub uuid: Uuid,
    pub session_uuid: Uuid,
    pub principal_uuid: Uuid,
    pub channel: String,
    pub account_id: String,
    pub conversation_id: String,
    pub text: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderReceipt {
    pub provider_message_id: String,
    pub metadata: Value,
}

pub trait ChannelProvider: Send + Sync {
    fn channel(&self) -> &str;
    fn send(&self, message: &OutboundMessage) -> Result<ProviderReceipt, String>;
}

pub trait ChannelProviderV2: ChannelProvider {
    fn provider_id(&self) -> &str;
    fn capabilities(&self) -> ChannelCapabilities;
    fn probe(&self) -> Result<ChannelProbe, String>;
}

#[derive(Debug, Error)]
pub enum ChannelGatewayError {
    #[error("channel gateway lock poisoned")]
    Poisoned,
    #[error("identity is not bound")]
    IdentityMissing,
    #[error("identity is pending approval")]
    IdentityPending,
    #[error("identity is blocked")]
    IdentityBlocked,
    #[error("duplicate external message")]
    DuplicateMessage,
    #[error("unknown session {0}")]
    UnknownSession(Uuid),
    #[error("provider channel mismatch: expected {expected}, got {actual}")]
    ProviderMismatch { expected: String, actual: String },
    #[error("provider failed: {0}")]
    Provider(String),
    #[error(transparent)]
    LiveBus(#[from] LiveBusError),
    #[error(transparent)]
    Evidence(#[from] LedgerError),
}

#[derive(Default)]
struct ChannelState {
    identities: BTreeMap<String, IdentityBinding>,
    sessions: BTreeMap<String, ChannelSession>,
    sessions_by_uuid: BTreeMap<Uuid, String>,
    seen_messages: BTreeSet<String>,
}

#[derive(Clone)]
pub struct ChannelGateway {
    state: Arc<Mutex<ChannelState>>,
    live_bus: LiveEventHub,
    evidence: EvidenceLedger,
}

impl ChannelGateway {
    pub fn new(live_bus: LiveEventHub, evidence: EvidenceLedger) -> Self {
        Self { state: Arc::new(Mutex::new(ChannelState::default())), live_bus, evidence }
    }

    pub fn bind_identity(&self, channel: &str, account_id: &str, external_user_id: &str, principal_uuid: Uuid, state: IdentityState) -> Result<IdentityBinding, ChannelGatewayError> {
        let now = Utc::now();
        let key = identity_key(channel, account_id, external_user_id);
        let mut guard = self.state.lock().map_err(|_| ChannelGatewayError::Poisoned)?;
        let binding = IdentityBinding { uuid: new_uuid_v7(), channel: channel.into(), account_id: account_id.into(), external_user_id: external_user_id.into(), principal_uuid, state, created_at: now, updated_at: now };
        guard.identities.insert(key, binding.clone());
        drop(guard);
        self.live_bus.publish_json("channel.identity", "bound", json!({"binding_uuid":binding.uuid,"channel":channel,"principal_uuid":principal_uuid}), Some(binding.uuid), None)?;
        self.evidence.append(EvidenceDraft { action_uuid: binding.uuid, correlation_uuid: Some(binding.uuid), actor: "Channel Gateway".into(), capability: "channel.identity.bind".into(), action: "identity_bound".into(), outcome: EvidenceOutcome::Succeeded, request_summary: json!({"channel":channel,"account_id":account_id}), result_summary: json!({"principal_uuid":principal_uuid}), artifact_uris: vec![] })?;
        Ok(binding)
    }

    pub fn set_identity_state(&self, channel: &str, account_id: &str, external_user_id: &str, state: IdentityState) -> Result<(), ChannelGatewayError> {
        let key = identity_key(channel, account_id, external_user_id);
        let mut guard = self.state.lock().map_err(|_| ChannelGatewayError::Poisoned)?;
        let binding = guard.identities.get_mut(&key).ok_or(ChannelGatewayError::IdentityMissing)?;
        binding.state = state;
        binding.updated_at = Utc::now();
        Ok(())
    }

    pub fn accept_inbound(&self, message: InboundMessage) -> Result<RoutedInbound, ChannelGatewayError> {
        let identity_key_value = identity_key(&message.channel, &message.account_id, &message.external_user_id);
        let dedup = message_key(&message.channel, &message.account_id, &message.external_message_id);
        let session_key_value = session_key(&message.channel, &message.account_id, &message.external_user_id, &message.conversation_id);
        let mut guard = self.state.lock().map_err(|_| ChannelGatewayError::Poisoned)?;
        if guard.seen_messages.contains(&dedup) { return Err(ChannelGatewayError::DuplicateMessage); }
        let binding = guard.identities.get(&identity_key_value).cloned().ok_or(ChannelGatewayError::IdentityMissing)?;
        match binding.state { IdentityState::Pending => return Err(ChannelGatewayError::IdentityPending), IdentityState::Blocked => return Err(ChannelGatewayError::IdentityBlocked), IdentityState::Active => {} }
        let now = Utc::now();
        let session = if let Some(existing) = guard.sessions.get_mut(&session_key_value) {
            existing.updated_at = now; existing.clone()
        } else {
            let created = ChannelSession { uuid: new_uuid_v7(), principal_uuid: binding.principal_uuid, channel: message.channel.clone(), account_id: message.account_id.clone(), conversation_id: message.conversation_id.clone(), created_at: now, updated_at: now };
            guard.sessions_by_uuid.insert(created.uuid, session_key_value.clone());
            guard.sessions.insert(session_key_value.clone(), created.clone());
            created
        };
        guard.seen_messages.insert(dedup);
        drop(guard);
        let routed = RoutedInbound { message_uuid: message.uuid, principal_uuid: binding.principal_uuid, session_uuid: session.uuid, channel: message.channel.clone(), conversation_id: message.conversation_id.clone() };
        self.live_bus.publish_json("channel.inbound", "accepted", json!({"message_uuid":message.uuid,"session_uuid":session.uuid,"principal_uuid":binding.principal_uuid,"channel":message.channel}), Some(session.uuid), None)?;
        self.evidence.append(EvidenceDraft { action_uuid: message.uuid, correlation_uuid: Some(session.uuid), actor: "Channel Gateway".into(), capability: "channel.inbound.accept".into(), action: "inbound_routed".into(), outcome: EvidenceOutcome::Succeeded, request_summary: json!({"channel":message.channel,"account_id":message.account_id,"external_message_id":message.external_message_id}), result_summary: json!({"session_uuid":session.uuid,"principal_uuid":binding.principal_uuid}), artifact_uris: vec![] })?;
        Ok(routed)
    }

    pub fn prepare_outbound(&self, session_uuid: Uuid, text: impl Into<String>) -> Result<OutboundMessage, ChannelGatewayError> {
        let guard = self.state.lock().map_err(|_| ChannelGatewayError::Poisoned)?;
        let key = guard.sessions_by_uuid.get(&session_uuid).ok_or(ChannelGatewayError::UnknownSession(session_uuid))?;
        let session = guard.sessions.get(key).cloned().ok_or(ChannelGatewayError::UnknownSession(session_uuid))?;
        let binding = guard.identities.values().find(|b| b.principal_uuid == session.principal_uuid && b.channel == session.channel && b.account_id == session.account_id).cloned().ok_or(ChannelGatewayError::IdentityMissing)?;
        match binding.state { IdentityState::Pending => return Err(ChannelGatewayError::IdentityPending), IdentityState::Blocked => return Err(ChannelGatewayError::IdentityBlocked), IdentityState::Active => {} }
        drop(guard);
        let outbound = OutboundMessage { uuid: new_uuid_v7(), session_uuid, principal_uuid: session.principal_uuid, channel: session.channel.clone(), account_id: session.account_id.clone(), conversation_id: session.conversation_id.clone(), text: text.into(), created_at: Utc::now() };
        self.live_bus.publish_json("channel.outbound", "requested", json!({"message_uuid":outbound.uuid,"session_uuid":session_uuid,"channel":outbound.channel}), Some(session_uuid), None)?;
        Ok(outbound)
    }

    pub fn send_with(&self, outbound: &OutboundMessage, provider: &dyn ChannelProvider) -> Result<ProviderReceipt, ChannelGatewayError> {
        if provider.channel() != outbound.channel { return Err(ChannelGatewayError::ProviderMismatch { expected: outbound.channel.clone(), actual: provider.channel().into() }); }
        let receipt = provider.send(outbound).map_err(ChannelGatewayError::Provider)?;
        self.live_bus.publish_json("channel.outbound", "sent", json!({"message_uuid":outbound.uuid,"session_uuid":outbound.session_uuid,"provider_message_id":receipt.provider_message_id}), Some(outbound.session_uuid), None)?;
        self.evidence.append(EvidenceDraft { action_uuid: outbound.uuid, correlation_uuid: Some(outbound.session_uuid), actor: "Channel Gateway".into(), capability: "channel.outbound.send".into(), action: "outbound_sent".into(), outcome: EvidenceOutcome::Succeeded, request_summary: json!({"channel":outbound.channel,"account_id":outbound.account_id}), result_summary: json!({"provider_message_id":receipt.provider_message_id}), artifact_uris: vec![] })?;
        Ok(receipt)
    }

    pub fn session(&self, session_uuid: Uuid) -> Result<ChannelSession, ChannelGatewayError> {
        let guard = self.state.lock().map_err(|_| ChannelGatewayError::Poisoned)?;
        let key = guard.sessions_by_uuid.get(&session_uuid).ok_or(ChannelGatewayError::UnknownSession(session_uuid))?;
        guard.sessions.get(key).cloned().ok_or(ChannelGatewayError::UnknownSession(session_uuid))
    }
}

fn identity_key(channel: &str, account_id: &str, external_user_id: &str) -> String { format!("{channel}\u{1f}{account_id}\u{1f}{external_user_id}") }
fn session_key(channel: &str, account_id: &str, external_user_id: &str, conversation_id: &str) -> String { format!("{channel}\u{1f}{account_id}\u{1f}{external_user_id}\u{1f}{conversation_id}") }
fn message_key(channel: &str, account_id: &str, external_message_id: &str) -> String { format!("{channel}\u{1f}{account_id}\u{1f}{external_message_id}") }

//! F22 Device Nodes domain/runtime core.
//! Durable truth is PostgreSQL; this crate contains deterministic validation and policy logic.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use subtle::ConstantTimeEq;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum DeviceError {
    #[error("node is not allowed to execute")]
    NodeBlocked,
    #[error("capability is not declared by node")]
    CapabilityDenied,
    #[error("approval is required")]
    ApprovalRequired,
    #[error("command is expired or not active yet")]
    InvalidCommandWindow,
    #[error("command ttl exceeds policy")]
    CommandTtlTooLong,
    #[error("raw secret-like argument rejected; use Secret Broker handles")]
    RawSecretArgument,
    #[error("invalid public key")]
    InvalidPublicKey,
    #[error("invalid signature")]
    InvalidSignature,
    #[error("invalid base64 payload")]
    InvalidBody,
    #[error("body hash mismatch")]
    BodyHashMismatch,
    #[error("invalid nonce")]
    InvalidNonce,
    #[error("clock skew exceeds policy")]
    ClockSkew,
    #[error("replay detected")]
    Replay,
    #[error("fencing token mismatch")]
    FencingMismatch,
    #[error("repository error: {0}")]
    Repository(String),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceState {
    Pending,
    Enrolled,
    Active,
    Degraded,
    Quarantined,
    Revoked,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceCapability {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceNode {
    pub node_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub display_name: String,
    pub state: DeviceState,
    pub public_key_ed25519_b64: String,
    pub capabilities: Vec<DeviceCapability>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

impl DeviceNode {
    pub fn has_capability(&self, name: &str) -> bool {
        self.capabilities.iter().any(|c| c.name == name)
    }

    pub fn verifying_key(&self) -> Result<VerifyingKey, DeviceError> {
        let bytes = B64
            .decode(&self.public_key_ed25519_b64)
            .map_err(|_| DeviceError::InvalidPublicKey)?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| DeviceError::InvalidPublicKey)?;
        VerifyingKey::from_bytes(&bytes).map_err(|_| DeviceError::InvalidPublicKey)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceEnvelope {
    pub protocol_version: u16,
    pub node_uuid: Uuid,
    pub session_uuid: Uuid,
    pub message_uuid: Uuid,
    pub sequence: u64,
    pub sent_at: DateTime<Utc>,
    pub nonce_hex: String,
    pub kind: String,
    pub body_b64: String,
    pub body_sha256_hex: String,
    pub signature_b64: String,
}

impl DeviceEnvelope {
    pub fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.protocol_version,
            self.node_uuid,
            self.session_uuid,
            self.message_uuid,
            self.sequence,
            self.sent_at.to_rfc3339(),
            self.nonce_hex,
            self.kind,
            self.body_sha256_hex,
        )
        .into_bytes()
    }

    pub fn decode_and_verify_body(&self) -> Result<Vec<u8>, DeviceError> {
        let body = B64
            .decode(&self.body_b64)
            .map_err(|_| DeviceError::InvalidBody)?;
        let expected =
            hex::decode(&self.body_sha256_hex).map_err(|_| DeviceError::BodyHashMismatch)?;
        if expected.len() != 32 {
            return Err(DeviceError::BodyHashMismatch);
        }
        let actual = Sha256::digest(&body);
        if expected.as_slice().ct_eq(actual.as_slice()).unwrap_u8() != 1 {
            return Err(DeviceError::BodyHashMismatch);
        }
        Ok(body)
    }
}

pub trait ReplayGuard {
    /// Must atomically reserve `(node, session, sequence, nonce)` in authoritative storage.
    fn reserve(
        &mut self,
        node_uuid: Uuid,
        session_uuid: Uuid,
        sequence: u64,
        nonce_hex: &str,
    ) -> Result<(), DeviceError>;
}

pub fn verify_envelope(
    node: &DeviceNode,
    envelope: &DeviceEnvelope,
    replay_guard: &mut dyn ReplayGuard,
    now: DateTime<Utc>,
    max_clock_skew: Duration,
) -> Result<Vec<u8>, DeviceError> {
    if envelope.node_uuid != node.node_uuid {
        return Err(DeviceError::InvalidSignature);
    }
    if matches!(node.state, DeviceState::Quarantined | DeviceState::Revoked) {
        return Err(DeviceError::NodeBlocked);
    }
    let nonce = hex::decode(&envelope.nonce_hex).map_err(|_| DeviceError::InvalidNonce)?;
    if nonce.len() != 16 {
        return Err(DeviceError::InvalidNonce);
    }
    let skew_ms = (now.timestamp_millis() - envelope.sent_at.timestamp_millis()).abs();
    if skew_ms > max_clock_skew.num_milliseconds().abs() {
        return Err(DeviceError::ClockSkew);
    }
    let body = envelope.decode_and_verify_body()?;
    let signature_bytes = B64
        .decode(&envelope.signature_b64)
        .map_err(|_| DeviceError::InvalidSignature)?;
    let signature_bytes: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| DeviceError::InvalidSignature)?;
    let signature = Signature::from_bytes(&signature_bytes);
    node.verifying_key()?
        .verify_strict(&envelope.signing_bytes(), &signature)
        .map_err(|_| DeviceError::InvalidSignature)?;
    replay_guard.reserve(
        envelope.node_uuid,
        envelope.session_uuid,
        envelope.sequence,
        &envelope.nonce_hex,
    )?;
    Ok(body)
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecretHandle {
    pub secret_uuid: Uuid,
    pub lease_uuid: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovalRef {
    pub approval_uuid: Uuid,
    pub approved_by_uuid: Uuid,
    pub approved_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceCommand {
    pub command_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub node_uuid: Uuid,
    pub capability: String,
    pub arguments: Value,
    pub secret_handles: Vec<SecretHandle>,
    pub idempotency_key: String,
    pub risk: RiskLevel,
    pub approval: Option<ApprovalRef>,
    pub submitted_at: DateTime<Utc>,
    pub not_before: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub fencing_token: i64,
}

#[derive(Clone, Debug)]
pub struct CommandPolicy {
    pub protected_capabilities: BTreeSet<String>,
    pub max_ttl: Duration,
}

impl Default for CommandPolicy {
    fn default() -> Self {
        let protected_capabilities = [
            "device.system.exec".to_string(),
            "device.system.power".to_string(),
            "device.files.write".to_string(),
            "device.network.change".to_string(),
            "device.secret.rotate".to_string(),
        ]
        .into_iter()
        .collect();
        Self {
            protected_capabilities,
            max_ttl: Duration::minutes(10),
        }
    }
}

pub fn authorize_command(
    node: &DeviceNode,
    command: &DeviceCommand,
    policy: &CommandPolicy,
    now: DateTime<Utc>,
) -> Result<(), DeviceError> {
    if command.node_uuid != node.node_uuid || command.tenant_uuid != node.tenant_uuid {
        return Err(DeviceError::NodeBlocked);
    }
    if !matches!(node.state, DeviceState::Active | DeviceState::Degraded) {
        return Err(DeviceError::NodeBlocked);
    }
    if !node.has_capability(&command.capability) {
        return Err(DeviceError::CapabilityDenied);
    }
    if now < command.not_before || now >= command.expires_at {
        return Err(DeviceError::InvalidCommandWindow);
    }
    if command.expires_at - command.submitted_at > policy.max_ttl {
        return Err(DeviceError::CommandTtlTooLong);
    }
    let approval_required = policy.protected_capabilities.contains(&command.capability)
        || matches!(command.risk, RiskLevel::High | RiskLevel::Critical);
    if approval_required && command.approval.is_none() {
        return Err(DeviceError::ApprovalRequired);
    }
    if contains_raw_secret_like_value(&command.arguments) {
        return Err(DeviceError::RawSecretArgument);
    }
    Ok(())
}

pub fn validate_fencing(expected: i64, received: i64) -> Result<(), DeviceError> {
    if expected == received {
        Ok(())
    } else {
        Err(DeviceError::FencingMismatch)
    }
}

fn contains_raw_secret_like_value(value: &Value) -> bool {
    const FORBIDDEN: &[&str] = &[
        "password",
        "passwd",
        "token",
        "api_key",
        "apikey",
        "secret",
        "private_key",
    ];
    match value {
        Value::Object(map) => map.iter().any(|(k, v)| {
            let normalized = k.to_ascii_lowercase();
            let allowed_handle =
                normalized.ends_with("_secret_uuid") || normalized.ends_with("_lease_uuid");
            (!allowed_handle && FORBIDDEN.iter().any(|f| normalized == *f))
                || contains_raw_secret_like_value(v)
        }),
        Value::Array(items) => items.iter().any(contains_raw_secret_like_value),
        _ => false,
    }
}

pub trait DeviceRepository {
    fn get_node(&self, tenant_uuid: Uuid, node_uuid: Uuid) -> Result<DeviceNode, DeviceError>;
    fn persist_heartbeat(
        &mut self,
        tenant_uuid: Uuid,
        node_uuid: Uuid,
        at: DateTime<Utc>,
    ) -> Result<(), DeviceError>;
    fn next_fencing_token(
        &mut self,
        tenant_uuid: Uuid,
        node_uuid: Uuid,
    ) -> Result<i64, DeviceError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_token_argument_is_rejected() {
        let v = serde_json::json!({"token": "do-not-place-secrets-here"});
        assert!(contains_raw_secret_like_value(&v));
    }

    #[test]
    fn secret_handle_fields_are_allowed() {
        let v = serde_json::json!({"credential_secret_uuid": Uuid::now_v7().to_string()});
        assert!(!contains_raw_secret_like_value(&v));
    }

    #[test]
    fn stale_fencing_is_rejected() {
        assert!(matches!(
            validate_fencing(8, 7),
            Err(DeviceError::FencingMismatch)
        ));
    }
}

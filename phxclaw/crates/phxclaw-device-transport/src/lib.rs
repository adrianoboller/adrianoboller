#![forbid(unsafe_code)]

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signer, SigningKey};
use futures_util::{SinkExt, StreamExt};
use phxclaw_device_nodes::{DeviceCapability, DeviceEnvelope};
use phxclaw_key_provider::{KeyMaterial, KeyProvider, KeyProviderError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use url::Url;
use uuid::Uuid;

pub mod servidor;

const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Error)]
pub enum DeviceTransportError {
    #[error(transparent)]
    KeyProvider(#[from] KeyProviderError),
    #[error("random source failed: {0}")]
    Random(String),
    #[error("invalid signing key material")]
    InvalidSigningKey,
    #[error("invalid websocket url")]
    InvalidUrl,
    #[error("secure websocket (wss) required")]
    InsecureTransport,
    #[error("websocket error: {0}")]
    WebSocket(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("transport closed")]
    Closed,
    #[error("pairing rejected: {0}")]
    PairingRejected(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DevicePlatform {
    Windows,
    Linux,
    Macos,
    Android,
    Ios,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentRequest {
    pub tenant_uuid: Uuid,
    pub node_uuid: Uuid,
    pub display_name: String,
    pub platform: DevicePlatform,
    pub agent_version: String,
    pub public_key_ed25519_b64: String,
    pub enrollment_token: String,
    pub capabilities: Vec<DeviceCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentAcceptance {
    pub session_uuid: Uuid,
    pub fencing_token: i64,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentRecord {
    pub tenant_uuid: Uuid,
    pub node_uuid: Uuid,
    pub token_sha256_hex: String,
    pub display_name: String,
    pub platform: DevicePlatform,
    pub agent_version: String,
    pub public_key_ed25519_b64: String,
    pub capabilities: Vec<DeviceCapability>,
}

pub trait EnrollmentRepository: Send + Sync {
    fn consume(
        &self,
        record: EnrollmentRecord,
    ) -> Result<EnrollmentAcceptance, DeviceTransportError>;
}

pub fn enrollment_token_sha256_hex(token: &str) -> String {
    hex_lower(&Sha256::digest(token.as_bytes()))
}

pub fn consume_enrollment(
    repo: &dyn EnrollmentRepository,
    request: EnrollmentRequest,
) -> Result<EnrollmentAcceptance, DeviceTransportError> {
    if request.enrollment_token.len() < 24 {
        return Err(DeviceTransportError::PairingRejected(
            "enrollment token too short".into(),
        ));
    }
    let record = EnrollmentRecord {
        tenant_uuid: request.tenant_uuid,
        node_uuid: request.node_uuid,
        token_sha256_hex: enrollment_token_sha256_hex(&request.enrollment_token),
        display_name: request.display_name,
        platform: request.platform,
        agent_version: request.agent_version,
        public_key_ed25519_b64: request.public_key_ed25519_b64,
        capabilities: request.capabilities,
    };
    repo.consume(record)
}

pub struct NodeIdentity {
    pub node_uuid: Uuid,
    pub public_key_ed25519_b64: String,
    signing_key: SigningKey,
}

impl NodeIdentity {
    pub fn generate_and_store(
        provider: &dyn KeyProvider,
        node_uuid: Uuid,
    ) -> Result<Self, DeviceTransportError> {
        let mut seed = [0u8; 32];
        getrandom::fill(&mut seed).map_err(|e| DeviceTransportError::Random(e.to_string()))?;
        let signing_key = SigningKey::from_bytes(&seed);
        let material = KeyMaterial::new(seed.to_vec())?;
        provider.store(&key_id(node_uuid), &material)?;
        seed.fill(0);
        Ok(Self {
            node_uuid,
            public_key_ed25519_b64: B64.encode(signing_key.verifying_key().as_bytes()),
            signing_key,
        })
    }

    /// Identidade que nao vai para o chaveiro: a do servidor, que assina as respostas
    /// para a trilha de auditoria (quem autentica o servidor para o no e o TLS).
    pub fn efemera(node_uuid: Uuid) -> Result<Self, DeviceTransportError> {
        let mut seed = [0u8; 32];
        getrandom::fill(&mut seed).map_err(|e| DeviceTransportError::Random(e.to_string()))?;
        let signing_key = SigningKey::from_bytes(&seed);
        seed.fill(0);
        Ok(Self {
            node_uuid,
            public_key_ed25519_b64: B64.encode(signing_key.verifying_key().as_bytes()),
            signing_key,
        })
    }

    pub fn load(provider: &dyn KeyProvider, node_uuid: Uuid) -> Result<Self, DeviceTransportError> {
        let material = provider.load(&key_id(node_uuid))?;
        let bytes: [u8; 32] = material
            .expose()
            .try_into()
            .map_err(|_| DeviceTransportError::InvalidSigningKey)?;
        let signing_key = SigningKey::from_bytes(&bytes);
        Ok(Self {
            node_uuid,
            public_key_ed25519_b64: B64.encode(signing_key.verifying_key().as_bytes()),
            signing_key,
        })
    }

    pub fn sign_envelope(
        &self,
        session_uuid: Uuid,
        sequence: u64,
        kind: impl Into<String>,
        body: &[u8],
    ) -> Result<DeviceEnvelope, DeviceTransportError> {
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).map_err(|e| DeviceTransportError::Random(e.to_string()))?;
        let body_hash = Sha256::digest(body);
        let mut envelope = DeviceEnvelope {
            protocol_version: PROTOCOL_VERSION,
            node_uuid: self.node_uuid,
            session_uuid,
            message_uuid: Uuid::now_v7(),
            sequence,
            sent_at: Utc::now(),
            nonce_hex: hex_lower(&nonce),
            kind: kind.into(),
            body_b64: B64.encode(body),
            body_sha256_hex: hex_lower(&body_hash),
            signature_b64: String::new(),
        };
        envelope.signature_b64 =
            B64.encode(self.signing_key.sign(&envelope.signing_bytes()).to_bytes());
        Ok(envelope)
    }
}

fn key_id(node_uuid: Uuid) -> String {
    format!("device/{node_uuid}/ed25519")
}
fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub struct WssDeviceClient {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
}
impl WssDeviceClient {
    pub async fn connect(endpoint: &str) -> Result<Self, DeviceTransportError> {
        let url = Url::parse(endpoint).map_err(|_| DeviceTransportError::InvalidUrl)?;
        if url.scheme() != "wss" {
            return Err(DeviceTransportError::InsecureTransport);
        }
        let (socket, _) = connect_async(endpoint)
            .await
            .map_err(|e| DeviceTransportError::WebSocket(e.to_string()))?;
        Ok(Self { socket })
    }
    /// Conecta confiando SO na autoridade dada (PEM): servidor de dispositivos em rede
    /// local tem certificado proprio, e aceitar as raizes publicas ali abriria a porta
    /// para qualquer certificado valido na internet responder no lugar dele.
    pub async fn connect_with_ca(
        endpoint: &str,
        ca_pem: &[u8],
    ) -> Result<Self, DeviceTransportError> {
        use rustls_pki_types::pem::PemObject;
        let url = Url::parse(endpoint).map_err(|_| DeviceTransportError::InvalidUrl)?;
        if url.scheme() != "wss" {
            return Err(DeviceTransportError::InsecureTransport);
        }
        let mut raizes = rustls::RootCertStore::empty();
        for c in rustls_pki_types::CertificateDer::pem_slice_iter(ca_pem) {
            let c = c.map_err(|e| DeviceTransportError::WebSocket(format!("CA: {e}")))?;
            raizes
                .add(c)
                .map_err(|e| DeviceTransportError::WebSocket(format!("CA: {e}")))?;
        }
        let config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|e| DeviceTransportError::WebSocket(e.to_string()))?
        .with_root_certificates(raizes)
        .with_no_client_auth();
        let (socket, _) = tokio_tungstenite::connect_async_tls_with_config(
            endpoint,
            None,
            false,
            Some(tokio_tungstenite::Connector::Rustls(std::sync::Arc::new(
                config,
            ))),
        )
        .await
        .map_err(|e| DeviceTransportError::WebSocket(e.to_string()))?;
        Ok(Self { socket })
    }

    pub async fn send(&mut self, envelope: &DeviceEnvelope) -> Result<(), DeviceTransportError> {
        let text = serde_json::to_string(envelope)
            .map_err(|e| DeviceTransportError::Serialization(e.to_string()))?;
        self.socket
            .send(Message::Text(text.into()))
            .await
            .map_err(|e| DeviceTransportError::WebSocket(e.to_string()))
    }
    pub async fn receive(&mut self) -> Result<DeviceEnvelope, DeviceTransportError> {
        while let Some(message) = self.socket.next().await {
            let message = message.map_err(|e| DeviceTransportError::WebSocket(e.to_string()))?;
            match message {
                Message::Text(text) => {
                    return serde_json::from_str(&text)
                        .map_err(|e| DeviceTransportError::Serialization(e.to_string()));
                }
                Message::Binary(bytes) => {
                    return serde_json::from_slice(&bytes)
                        .map_err(|e| DeviceTransportError::Serialization(e.to_string()));
                }
                Message::Ping(payload) => {
                    self.socket
                        .send(Message::Pong(payload))
                        .await
                        .map_err(|e| DeviceTransportError::WebSocket(e.to_string()))?;
                }
                Message::Close(_) => return Err(DeviceTransportError::Closed),
                _ => {}
            }
        }
        Err(DeviceTransportError::Closed)
    }
    pub async fn close(mut self) -> Result<(), DeviceTransportError> {
        self.socket
            .close(None)
            .await
            .map_err(|e| DeviceTransportError::WebSocket(e.to_string()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHello {
    pub tenant_uuid: Uuid,
    pub platform: DevicePlatform,
    pub agent_version: String,
    pub capabilities: Vec<DeviceCapability>,
}

pub fn platform_default_capabilities(platform: DevicePlatform) -> Vec<DeviceCapability> {
    let mut names = vec![
        "device.heartbeat",
        "device.system.info",
        "device.files.read",
    ];
    match platform {
        DevicePlatform::Windows | DevicePlatform::Linux | DevicePlatform::Macos => names.extend([
            "device.screen.capture",
            "device.input.control",
            "device.process.launch",
        ]),
        DevicePlatform::Android | DevicePlatform::Ios => {
            names.extend(["device.mobile.notify", "device.mobile.share"])
        }
    }
    names
        .into_iter()
        .map(|name| DeviceCapability {
            name: name.into(),
            version: "1".into(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn token_hash_is_stable() {
        assert_eq!(
            enrollment_token_sha256_hex("abcdefghijklmnopqrstuvwxyz"),
            enrollment_token_sha256_hex("abcdefghijklmnopqrstuvwxyz")
        );
    }
    #[test]
    fn platform_capabilities_are_explicit() {
        assert!(
            platform_default_capabilities(DevicePlatform::Linux)
                .iter()
                .any(|x| x.name == "device.screen.capture")
        );
    }
}

#![forbid(unsafe_code)]
//! PhxClaw F23 Secret & Credential Broker.
//!
//! Design notes:
//! - secrets are never serialized in evidence/events/log metadata;
//! - all values are encrypted at rest using AES-256-GCM;
//! - access happens through short-lived UUIDv7 leases with scoped audiences;
//! - revocation and rotation are explicit;
//! - a bootstrap file key provider is supplied for local/private deployments;
//!   production deployments should inject a platform key provider / HSM / keyring.
//!
//! Portions of the storage/redaction design are informed by MIT-licensed openclaw-rs.

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::{DateTime, Duration, Utc};
use phxclaw_evidence_ledger::{EvidenceDraft, EvidenceLedger, EvidenceOutcome, LedgerError};
use phxclaw_live_bus::{LiveBusError, LiveEventHub};
use phxclaw_types::new_uuid_v7;
use secrecy::{ExposeSecret, SecretBox};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::Zeroize;

const FILE_MAGIC: &[u8] = b"PHXSECRET1";
const NONCE_BYTES: usize = 12;

#[derive(Clone)]
pub struct SecretValue(SecretBox<str>);

impl SecretValue {
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(SecretBox::new(value.into_boxed_str()))
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretValue([REDACTED])")
    }
}

impl std::fmt::Display for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretDescriptor {
    pub uuid: Uuid,
    pub name: String,
    pub namespace: String,
    pub version: u64,
    pub scopes: Vec<String>,
    pub sha256: String,
    pub created_at: DateTime<Utc>,
    pub rotated_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretLease {
    pub uuid: Uuid,
    pub secret_uuid: Uuid,
    pub secret_version: u64,
    pub consumer: String,
    pub scope: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl SecretLease {
    #[must_use]
    pub fn active_at(&self, now: DateTime<Utc>) -> bool {
        self.revoked_at.is_none() && now < self.expires_at
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecretFileEnvelope {
    descriptor: SecretDescriptor,
    nonce_base64: String,
    ciphertext_base64: String,
}

pub trait MasterKeyProvider: Send + Sync {
    fn key(&self) -> Result<[u8; 32], SecretBrokerError>;
    fn provider_name(&self) -> &'static str;
}

#[derive(Clone)]
pub struct FileMasterKeyProvider {
    path: PathBuf,
}

impl FileMasterKeyProvider {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn ensure(&self) -> Result<(), SecretBrokerError> {
        if self.path.exists() {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes)
            .map_err(|error| SecretBrokerError::Crypto(error.to_string()))?;
        let mut encoded = BASE64.encode(bytes);
        let written = write_private_file(&self.path, encoded.as_bytes());
        encoded.zeroize();
        bytes.zeroize();
        written?;
        Ok(())
    }
}

impl MasterKeyProvider for FileMasterKeyProvider {
    fn key(&self) -> Result<[u8; 32], SecretBrokerError> {
        self.ensure()?;
        let text = fs::read_to_string(&self.path)?;
        let bytes = BASE64.decode(text.trim()).map_err(|error| {
            SecretBrokerError::Crypto(format!("invalid master key encoding: {error}"))
        })?;
        bytes
            .try_into()
            .map_err(|_| SecretBrokerError::Crypto("master key must decode to 32 bytes".into()))
    }

    fn provider_name(&self) -> &'static str {
        "bootstrap-file-key"
    }
}

#[derive(Debug, Error)]
pub enum SecretBrokerError {
    #[error("secret broker state lock poisoned")]
    Poisoned,
    #[error("secret not found: {0}")]
    SecretNotFound(Uuid),
    #[error("secret lease not found: {0}")]
    LeaseNotFound(Uuid),
    #[error("secret lease expired or revoked: {0}")]
    LeaseInactive(Uuid),
    #[error("secret is revoked: {0}")]
    SecretRevoked(Uuid),
    #[error("scope denied: requested={requested} allowed={allowed:?}")]
    ScopeDenied {
        requested: String,
        allowed: Vec<String>,
    },
    #[error("invalid secret name")]
    InvalidName,
    #[error("invalid lease duration")]
    InvalidLeaseDuration,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error(transparent)]
    Event(#[from] LiveBusError),
    #[error(transparent)]
    Evidence(#[from] LedgerError),
}

#[derive(Default)]
struct BrokerState {
    descriptors: BTreeMap<Uuid, SecretDescriptor>,
    leases: BTreeMap<Uuid, SecretLease>,
}

#[derive(Clone)]
pub struct SecretBroker {
    root: PathBuf,
    key_provider: Arc<dyn MasterKeyProvider>,
    state: Arc<Mutex<BrokerState>>,
    live_bus: LiveEventHub,
    evidence: EvidenceLedger,
}

impl SecretBroker {
    pub fn new(
        root: PathBuf,
        key_provider: Arc<dyn MasterKeyProvider>,
        live_bus: LiveEventHub,
        evidence: EvidenceLedger,
    ) -> Result<Self, SecretBrokerError> {
        fs::create_dir_all(&root)?;
        let broker = Self {
            root,
            key_provider,
            state: Arc::new(Mutex::new(BrokerState::default())),
            live_bus,
            evidence,
        };
        broker.reload()?;
        Ok(broker)
    }

    pub fn store(
        &self,
        name: impl Into<String>,
        namespace: impl Into<String>,
        scopes: Vec<String>,
        value: SecretValue,
    ) -> Result<SecretDescriptor, SecretBrokerError> {
        let name = name.into();
        validate_name(&name)?;
        let namespace = namespace.into();
        validate_name(&namespace)?;
        let descriptor = SecretDescriptor {
            uuid: new_uuid_v7(),
            name,
            namespace,
            version: 1,
            scopes: normalize_scopes(scopes),
            sha256: sha256_hex(value.expose().as_bytes()),
            created_at: Utc::now(),
            rotated_at: None,
            revoked_at: None,
        };
        self.write_secret(&descriptor, &value)?;
        self.state
            .lock()
            .map_err(|_| SecretBrokerError::Poisoned)?
            .descriptors
            .insert(descriptor.uuid, descriptor.clone());
        self.record("stored", &descriptor, None, EvidenceOutcome::Succeeded)?;
        Ok(descriptor)
    }

    pub fn rotate(
        &self,
        secret_uuid: Uuid,
        value: SecretValue,
    ) -> Result<SecretDescriptor, SecretBrokerError> {
        let current = self.descriptor(secret_uuid)?;
        if current.revoked_at.is_some() {
            return Err(SecretBrokerError::SecretRevoked(secret_uuid));
        }
        let mut next = current.clone();
        next.version = next.version.saturating_add(1);
        next.rotated_at = Some(Utc::now());
        next.sha256 = sha256_hex(value.expose().as_bytes());
        self.write_secret(&next, &value)?;
        self.state
            .lock()
            .map_err(|_| SecretBrokerError::Poisoned)?
            .descriptors
            .insert(secret_uuid, next.clone());
        self.record("rotated", &next, None, EvidenceOutcome::Succeeded)?;
        Ok(next)
    }

    pub fn revoke_secret(&self, secret_uuid: Uuid) -> Result<SecretDescriptor, SecretBrokerError> {
        let mut guard = self.state.lock().map_err(|_| SecretBrokerError::Poisoned)?;
        let descriptor = guard
            .descriptors
            .get_mut(&secret_uuid)
            .ok_or(SecretBrokerError::SecretNotFound(secret_uuid))?;
        descriptor.revoked_at = Some(Utc::now());
        let updated = descriptor.clone();
        drop(guard);
        self.persist_descriptor_only(&updated)?;
        self.record("revoked", &updated, None, EvidenceOutcome::Succeeded)?;
        Ok(updated)
    }

    pub fn issue_lease(
        &self,
        secret_uuid: Uuid,
        consumer: impl Into<String>,
        scope: impl Into<String>,
        ttl_seconds: i64,
    ) -> Result<SecretLease, SecretBrokerError> {
        if !(1..=3600).contains(&ttl_seconds) {
            return Err(SecretBrokerError::InvalidLeaseDuration);
        }
        let descriptor = self.descriptor(secret_uuid)?;
        if descriptor.revoked_at.is_some() {
            return Err(SecretBrokerError::SecretRevoked(secret_uuid));
        }
        let scope = scope.into();
        if !scope_allowed(&descriptor.scopes, &scope) {
            return Err(SecretBrokerError::ScopeDenied {
                requested: scope,
                allowed: descriptor.scopes,
            });
        }
        let now = Utc::now();
        let lease = SecretLease {
            uuid: new_uuid_v7(),
            secret_uuid,
            secret_version: descriptor.version,
            consumer: consumer.into(),
            scope,
            issued_at: now,
            expires_at: now + Duration::seconds(ttl_seconds),
            revoked_at: None,
        };
        self.state
            .lock()
            .map_err(|_| SecretBrokerError::Poisoned)?
            .leases
            .insert(lease.uuid, lease.clone());
        self.record(
            "lease_issued",
            &descriptor,
            Some(&lease),
            EvidenceOutcome::Succeeded,
        )?;
        Ok(lease)
    }

    pub fn resolve(
        &self,
        lease_uuid: Uuid,
        requested_scope: &str,
    ) -> Result<SecretValue, SecretBrokerError> {
        let lease = self
            .state
            .lock()
            .map_err(|_| SecretBrokerError::Poisoned)?
            .leases
            .get(&lease_uuid)
            .cloned()
            .ok_or(SecretBrokerError::LeaseNotFound(lease_uuid))?;
        if !lease.active_at(Utc::now()) {
            return Err(SecretBrokerError::LeaseInactive(lease_uuid));
        }
        if lease.scope != requested_scope && lease.scope != "*" {
            return Err(SecretBrokerError::ScopeDenied {
                requested: requested_scope.into(),
                allowed: vec![lease.scope],
            });
        }
        let descriptor = self.descriptor(lease.secret_uuid)?;
        if descriptor.revoked_at.is_some() {
            return Err(SecretBrokerError::SecretRevoked(descriptor.uuid));
        }
        if descriptor.version != lease.secret_version {
            return Err(SecretBrokerError::LeaseInactive(lease_uuid));
        }
        let value = self.read_secret(&descriptor)?;
        self.record(
            "resolved",
            &descriptor,
            Some(&lease),
            EvidenceOutcome::Succeeded,
        )?;
        Ok(value)
    }

    pub fn revoke_lease(&self, lease_uuid: Uuid) -> Result<SecretLease, SecretBrokerError> {
        let mut guard = self.state.lock().map_err(|_| SecretBrokerError::Poisoned)?;
        let lease = guard
            .leases
            .get_mut(&lease_uuid)
            .ok_or(SecretBrokerError::LeaseNotFound(lease_uuid))?;
        lease.revoked_at = Some(Utc::now());
        Ok(lease.clone())
    }

    pub fn descriptor(&self, secret_uuid: Uuid) -> Result<SecretDescriptor, SecretBrokerError> {
        self.state
            .lock()
            .map_err(|_| SecretBrokerError::Poisoned)?
            .descriptors
            .get(&secret_uuid)
            .cloned()
            .ok_or(SecretBrokerError::SecretNotFound(secret_uuid))
    }

    pub fn descriptors(&self) -> Result<Vec<SecretDescriptor>, SecretBrokerError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| SecretBrokerError::Poisoned)?
            .descriptors
            .values()
            .cloned()
            .collect())
    }

    pub fn reload(&self) -> Result<(), SecretBrokerError> {
        let mut loaded = BTreeMap::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.path().is_file()
                || entry.path().extension().and_then(|x| x.to_str()) != Some("phxsecret")
            {
                continue;
            }
            // Mesmo leitor do `read_envelope`: ler o arquivo como JSON cru tropecava no
            // prefixo magico que `write_secret` grava, e todo broker com um segredo guardado
            // deixava de abrir no reinicio.
            let envelope = parse_envelope(&fs::read(entry.path())?)?;
            loaded.insert(envelope.descriptor.uuid, envelope.descriptor);
        }
        self.state
            .lock()
            .map_err(|_| SecretBrokerError::Poisoned)?
            .descriptors = loaded;
        Ok(())
    }

    fn path_for(&self, descriptor: &SecretDescriptor) -> PathBuf {
        self.root.join(format!("{}.phxsecret", descriptor.uuid))
    }

    fn write_secret(
        &self,
        descriptor: &SecretDescriptor,
        value: &SecretValue,
    ) -> Result<(), SecretBrokerError> {
        let key = self.key_provider.key()?;
        let cipher = Aes256Gcm::new((&key).into());
        let mut nonce_bytes = [0u8; NONCE_BYTES];
        getrandom::fill(&mut nonce_bytes)
            .map_err(|error| SecretBrokerError::Crypto(error.to_string()))?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, value.expose().as_bytes())
            .map_err(|error| SecretBrokerError::Crypto(error.to_string()))?;
        let envelope = SecretFileEnvelope {
            descriptor: descriptor.clone(),
            nonce_base64: BASE64.encode(nonce_bytes),
            ciphertext_base64: BASE64.encode(ciphertext),
        };
        let mut bytes = FILE_MAGIC.to_vec();
        bytes.extend_from_slice(&serde_json::to_vec_pretty(&envelope)?);
        write_private_file(&self.path_for(descriptor), &bytes)?;
        Ok(())
    }

    fn persist_descriptor_only(
        &self,
        descriptor: &SecretDescriptor,
    ) -> Result<(), SecretBrokerError> {
        let old = self.read_envelope(descriptor.uuid)?;
        let envelope = SecretFileEnvelope {
            descriptor: descriptor.clone(),
            nonce_base64: old.nonce_base64,
            ciphertext_base64: old.ciphertext_base64,
        };
        let mut bytes = FILE_MAGIC.to_vec();
        bytes.extend_from_slice(&serde_json::to_vec_pretty(&envelope)?);
        write_private_file(&self.path_for(descriptor), &bytes)?;
        Ok(())
    }

    fn read_secret(&self, descriptor: &SecretDescriptor) -> Result<SecretValue, SecretBrokerError> {
        let envelope = self.read_envelope(descriptor.uuid)?;
        let nonce_bytes = BASE64
            .decode(envelope.nonce_base64)
            .map_err(|error| SecretBrokerError::Crypto(error.to_string()))?;
        let ciphertext = BASE64
            .decode(envelope.ciphertext_base64)
            .map_err(|error| SecretBrokerError::Crypto(error.to_string()))?;
        if nonce_bytes.len() != NONCE_BYTES {
            return Err(SecretBrokerError::Crypto("invalid nonce length".into()));
        }
        let key = self.key_provider.key()?;
        let cipher = Aes256Gcm::new((&key).into());
        let nonce = Nonce::from_slice(&nonce_bytes);
        let mut plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|error| SecretBrokerError::Crypto(error.to_string()))?;
        let text = String::from_utf8(plaintext.clone())
            .map_err(|error| SecretBrokerError::Crypto(error.to_string()))?;
        plaintext.zeroize();
        Ok(SecretValue::new(text))
    }

    fn read_envelope(&self, secret_uuid: Uuid) -> Result<SecretFileEnvelope, SecretBrokerError> {
        let descriptor = self.descriptor(secret_uuid)?;
        parse_envelope(&fs::read(self.path_for(&descriptor))?)
    }

    fn record(
        &self,
        event: &str,
        descriptor: &SecretDescriptor,
        lease: Option<&SecretLease>,
        outcome: EvidenceOutcome,
    ) -> Result<(), SecretBrokerError> {
        let correlation = lease.map(|item| item.uuid).unwrap_or(descriptor.uuid);
        self.live_bus.publish_json(
            "secret.broker",
            event,
            json!({
                "secret_uuid": descriptor.uuid,
                "secret_name": descriptor.name,
                "namespace": descriptor.namespace,
                "secret_version": descriptor.version,
                "lease_uuid": lease.map(|item| item.uuid),
                "consumer": lease.map(|item| item.consumer.clone()),
                "scope": lease.map(|item| item.scope.clone()),
                "master_key_provider": self.key_provider.provider_name(),
            }),
            Some(correlation),
            None,
        )?;
        self.evidence.append(EvidenceDraft {
            action_uuid: correlation,
            correlation_uuid: Some(correlation),
            actor: lease
                .map(|item| item.consumer.clone())
                .unwrap_or_else(|| "Secrets Manager".into()),
            capability: format!("secret.{event}"),
            action: format!("secret.{event}"),
            outcome,
            request_summary: json!({
                "secret_uuid": descriptor.uuid,
                "secret_name": descriptor.name,
                "namespace": descriptor.namespace,
                "secret_version": descriptor.version,
                "scope": lease.map(|item| item.scope.clone()),
            }),
            result_summary: json!({"value":"[REDACTED]"}),
            artifact_uris: vec![],
        })?;
        Ok(())
    }
}

fn validate_name(value: &str) -> Result<(), SecretBrokerError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(SecretBrokerError::InvalidName);
    }
    Ok(())
}

/// O unico leitor de envelope em disco: o formato e `FILE_MAGIC` seguido do JSON.
fn parse_envelope(bytes: &[u8]) -> Result<SecretFileEnvelope, SecretBrokerError> {
    let Some(json) = bytes.strip_prefix(FILE_MAGIC) else {
        return Err(SecretBrokerError::Crypto(
            "invalid secret file magic".into(),
        ));
    };
    Ok(serde_json::from_slice(json)?)
}

/// Unico jeito de gravar arquivo secreto nesta base (chave mestra, envelope de segredo,
/// token da API do desktop). O arquivo NASCE 0600: gravar e so depois ajustar a permissao
/// deixava uma janela em que ele existia com a umask (0644) e outro usuario o lia. Grava num
/// temporario exclusivo ao lado e renomeia por cima -- atomico, e troca tambem o arquivo
/// antigo que tivesse permissao frouxa.
pub fn write_private_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = parent.join(format!(".{name}.{}.tmp", new_uuid_v7().simple()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = options.open(&tmp).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    match result.and_then(|()| fs::rename(&tmp, path)) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&tmp);
            Err(error)
        }
    }
}

fn normalize_scopes(mut scopes: Vec<String>) -> Vec<String> {
    scopes.sort();
    scopes.dedup();
    scopes
}

fn scope_allowed(allowed: &[String], requested: &str) -> bool {
    allowed
        .iter()
        .any(|scope| scope == "*" || scope == requested)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn scrub_text(text: &str, known_values: &[SecretValue]) -> String {
    let mut output = text.to_owned();
    for value in known_values {
        let exposed = value.expose();
        if !exposed.is_empty() {
            output = output.replace(exposed, "[REDACTED]");
        }
    }
    for marker in [
        "api_key=",
        "apikey=",
        "token=",
        "secret=",
        "password=",
        "Authorization: Bearer ",
        "Authorization: Basic ",
        "x-api-key: ",
    ] {
        output = scrub_after_marker(&output, marker);
    }
    output
}

// Os prefixos de credencial que se reconhecem SEM marcador ao lado (quem cola uma chave num
// texto livre raramente escreve `token=` antes dela) e a regra da palavra moram em
// `phxclaw_types::segredo`: um motor so para o broker, o `config.json` e o motor de fluxo.

/// O que `scrub_text` faz sem segredo conhecido, mais o que tem FORMA de credencial: chave
/// privada PEM, palavra com prefixo de chave de provedor e JWT. Existe a parte de
/// `scrub_text` porque mudar o que ela tapa mudaria em silencio a saida de quem ja a chama;
/// quem quer a tarja mais larga pede esta.
pub fn scrub_secret_like(text: &str) -> String {
    // Basic/Bearer ANTES do `scrub_text`: o marcador dele (`auth_token=`, `password=`) engole a
    // palavra «Bearer» e deixa o valor orfao, que a entrada do fluxo recusa e a tarja soltaria.
    let output = phxclaw_types::segredo::tarjar_basic_bearer(text, "[REDACTED]");
    let mut output = scrub_text(&output, &[]);
    while let Some(ini) = output.find("-----BEGIN ") {
        let resto = &output[ini..];
        let fim = resto
            .find("-----END ")
            .and_then(|e| resto[e + 9..].find("-----").map(|f| e + 9 + f + 5))
            .unwrap_or(resto.len());
        output.replace_range(ini..ini + fim, "[REDACTED]");
    }
    let mut saida = String::with_capacity(output.len());
    let mut palavra = String::new();
    let fecha = |palavra: &mut String, saida: &mut String| {
        if parece_credencial(palavra) {
            saida.push_str("[REDACTED]");
        } else {
            saida.push_str(palavra);
        }
        palavra.clear();
    };
    // A senha de URL e o separador de palavras vem do motor unico (`phxclaw_types::segredo`):
    // o mesmo criterio que a entrada do motor de fluxo usa para recusar.
    let output = phxclaw_types::segredo::tarjar_url_com_senha(&output, "[REDACTED]");
    let output = phxclaw_types::segredo::tarjar_basic_bearer(&output, "[REDACTED]");
    for ch in output.chars() {
        if phxclaw_types::segredo::e_separador(ch) {
            fecha(&mut palavra, &mut saida);
            saida.push(ch);
        } else {
            palavra.push(ch);
        }
    }
    fecha(&mut palavra, &mut saida);
    saida
}

fn parece_credencial(palavra: &str) -> bool {
    phxclaw_types::segredo::palavra_parece_credencial(palavra)
}

fn scrub_after_marker(text: &str, marker: &str) -> String {
    let mut output = text.to_owned();
    let mut cursor = 0usize;
    while cursor < output.len() {
        let Some(relative) = output[cursor..].find(marker) else {
            break;
        };
        let start = cursor + relative + marker.len();
        let end = output[start..]
            .find(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'' | '&' | ','))
            .map_or(output.len(), |index| start + index);
        output.replace_range(start..end, "[REDACTED]");
        cursor = start + "[REDACTED]".len();
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_value_never_displays_contents() {
        let value = SecretValue::new("super-secret".into());
        assert_eq!(format!("{value}"), "[REDACTED]");
        assert_eq!(format!("{value:?}"), "SecretValue([REDACTED])");
    }

    #[test]
    fn scrub_known_values_and_markers() {
        let secret = SecretValue::new("abc123".into());
        let scrubbed = scrub_text("token=abc123 Authorization: Bearer abc123", &[secret]);
        assert!(!scrubbed.contains("abc123"));
    }

    #[test]
    fn tarja_o_que_tem_forma_de_credencial_e_deixa_o_texto_comum() {
        let t = scrub_secret_like(
            "use sk-proj-ABCdef0123456789xyz, ghp_0123456789abcdefABCDEF e \
             eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJh; password=abc123\n\
             -----BEGIN PRIVATE KEY-----\nMIIEv\n-----END PRIVATE KEY----- fim",
        );
        for vazou in ["sk-proj", "ghp_0123", "eyJhbG", "abc123", "MIIEv"] {
            assert!(!t.contains(vazou), "{vazou} vazou: {t}");
        }
        assert!(t.ends_with(" fim"), "{t}");
        // Texto comum, prefixo solto e versao nao sao credencial.
        let comum = "o sk- do prefixo, task-runner 1.2.3 e AKIA curto";
        assert_eq!(scrub_secret_like(comum), comum);
    }

    #[cfg(unix)]
    #[test]
    fn arquivo_privado_nasce_0600_mesmo_com_umask_frouxa_e_troca_o_antigo() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("phx-priv-{}", new_uuid_v7().simple()));
        fs::create_dir_all(&dir).unwrap();
        let alvo = dir.join("api.token");
        // Arquivo antigo com permissao frouxa: tem de sair 0600 depois da regravacao.
        fs::write(&alvo, b"velho").unwrap();
        fs::set_permissions(&alvo, fs::Permissions::from_mode(0o644)).unwrap();
        write_private_file(&alvo, b"novo").unwrap();
        let modo = fs::metadata(&alvo).unwrap().permissions().mode() & 0o777;
        assert_eq!(modo, 0o600, "modo {modo:o}");
        assert_eq!(fs::read(&alvo).unwrap(), b"novo");
        // Nenhum temporario sobra ao lado.
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    /// A tarja e a recusa da entrada do fluxo usam o MESMO motor (`phxclaw_types::segredo`):
    /// os corpos que o fluxo recusa nao podem passar em claro aqui. RED medido em 09/10: com o
    /// separador proprio de antes, `[ghp_…]`, `{ghp_…}`, `x=ghp_…&y=1` e as duas URLs com senha
    /// passavam; com a lista propria de 11 prefixos, `xapp-…` e `sk_live_…` passavam; sem o
    /// `tarjar_basic_bearer`, os quatro `Bearer`/`Basic` fora do marcador exato passavam.
    #[test]
    fn tarja_pelo_mesmo_motor_da_entrada_do_fluxo() {
        let ghp = "ghp_0123456789abcdefABCDEF";
        for sim in [
            format!("[{ghp}]"),
            format!("{{{ghp}}}"),
            format!("x={ghp}&y=1"),
            format!("token:{ghp}"),
            format!("https://x-access-token:{ghp}@github.com/a/b"),
            "https://eu:senha123@busca.local:443/x".to_string(),
            "xapp-1-A0123456789-0123456789".to_string(),
            "sk_live_0123456789abcdefgh".to_string(),
            "Bearer abcdefghijklmnopqrstuvwx".to_string(),
            r#"{"auth":"Bearer abcdefghijklmnopqrstuvwx"}"#.to_string(),
            r#"{"Authorization":"Bearer abcdefghijklmnopqrstuvwx"}"#.to_string(),
            "curl -H 'authorization: bearer abcdefghijklmnopqrstuvwx'".to_string(),
            "Basic dXNlcjpwYXNzd29yZA==".to_string(),
            "use `Bearer a1b2c3d4e5f6g7h8i9j0` no header".to_string(),
            "<Bearer a1b2c3d4e5f6g7h8i9j0>".to_string(),
            "[Bearer a1b2c3d4e5f6g7h8i9j0]".to_string(),
            "headers=Bearer a1b2c3d4e5f6g7h8i9j0&x=1".to_string(),
            "Basic dXNlcjpwYXNzd29yZA==.".to_string(),
            "Bearer 7f3k-9QxZ_2mN8.pL4v~R6tY".to_string(),
            "auth_token=Bearer a1b2c3d4e5f6g7h8i9j0".to_string(),
            "x-api-key: Bearer a1b2c3d4e5f6g7h8i9j0".to_string(),
            "password=Basic dXNlcjpwYXNzd29yZA==".to_string(),
            "Authorization: Bearer  a1b2c3d4e5f6g7h8i9j0".to_string(),
            "Authorization: Basic  dXNlcjpwYXNzd29yZA==".to_string(),
        ] {
            let t = scrub_secret_like(&sim);
            assert!(t.contains("[REDACTED]"), "{sim} -> {t}");
            assert!(
                !t.contains("0123456789")
                    && !t.contains("senha123")
                    && !t.contains("abcdefghijklmnop")
                    && !t.contains("a1b2c3d4e5f6")
                    && !t.contains("dXNlcjpw")
                    && !t.contains("9QxZ_2mN8"),
                "{sim} -> {t}"
            );
            assert!(phxclaw_types::segredo::texto_tem_credencial(&sim), "{sim}");
        }
        // `token=` continua tarjado aqui pelo marcador do `scrub_text` (redigir erra para o lado
        // largo, de proposito); a ENTRADA do fluxo nao o recusa, e isso se prova no fluxo_onda3b.
        for nao in [
            "sk-SK",
            "sk-telecom",
            "PROJ-123",
            "Add basic validation to the form",
            "basic authentication is deprecated",
            "bearer responsibilities",
        ] {
            assert_eq!(scrub_secret_like(nao), nao);
        }
    }
}

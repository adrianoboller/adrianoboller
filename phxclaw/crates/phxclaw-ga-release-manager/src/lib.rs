//! PhxClaw v0.27 multi-platform qualification and GA release contracts.
//! Absence of proof is never converted into a successful release gate.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum GaError {
    #[error("invalid SHA-256 digest")]
    InvalidDigest,
    #[error("signer is not trusted")]
    SignerNotTrusted,
    #[error("signature is invalid")]
    InvalidSignature,
    #[error("platform evidence is incomplete")]
    PlatformIncomplete,
    #[error("source-state, RC or candidate mismatch")]
    IdentityMismatch,
    #[error("update sequence must increase")]
    SequenceNotMonotonic,
    #[error("update metadata is expired or exceeds policy lifetime")]
    InvalidMetadataLifetime,
    #[error("downgrade requires a matching signed rollback authorization")]
    RollbackAuthorizationRequired,
    #[error("invalid semantic version transition")]
    InvalidVersionTransition,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedSigner {
    pub key_id: String,
    pub public_key_b64: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}
fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Linux,
    Windows,
    Macos,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatformEvidence {
    pub schema_version: String,
    pub evidence_uuid: Uuid,
    pub platform: Platform,
    pub architecture: String,
    pub candidate_version: String,
    pub source_state_sha256: String,
    pub rc_archive_sha256: String,
    pub artifact_name: String,
    pub artifact_sha256: String,
    pub artifact_size: u64,
    pub evidence_bundle_sha256: String,
    pub fresh_install: String,
    pub upgrade_n_minus_1: String,
    pub rollback_or_restore: String,
    pub native_tests: String,
    pub code_signing: String,
    pub notarization: String,
    pub first_release: bool,
    pub previous_version: Option<String>,
    pub created_at: DateTime<Utc>,
    pub signer_key_id: String,
    pub signature_algorithm: String,
    pub signature_b64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateTarget {
    pub platform: Platform,
    pub architecture: String,
    pub name: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub media_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateManifest {
    pub schema_version: String,
    pub channel: String,
    pub sequence: u64,
    pub version: String,
    pub source_state_sha256: String,
    pub ga_release_uuid: Uuid,
    pub targets: Vec<UpdateTarget>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub signer_key_id: String,
    pub signature_algorithm: String,
    pub signature_b64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RollbackAuthorization {
    pub schema_version: String,
    pub authorization_uuid: Uuid,
    pub from_version: String,
    pub to_version: String,
    pub from_sequence: u64,
    pub to_sequence: u64,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub signer_key_id: String,
    pub signature_algorithm: String,
    pub signature_b64: String,
}

fn valid_sha256(v: &str) -> bool {
    v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit())
}
fn hash_json<T: Serialize>(v: &T) -> String {
    let bytes = serde_json::to_vec(v).expect("serializable release metadata");
    format!("{:x}", Sha256::digest(bytes))
}

pub fn platform_payload(o: &PlatformEvidence) -> Vec<u8> {
    format!(
        "phxclaw-platform-qualification-v027\nplatform={}\narchitecture={}\ncandidate_version={}\nsource_state_sha256={}\nrc_archive_sha256={}\nartifact_sha256={}\nartifact_size={}\nevidence_bundle_sha256={}\nfresh_install={}\nupgrade_n_minus_1={}\nrollback_or_restore={}\nnative_tests={}\ncode_signing={}\nnotarization={}\nfirst_release={}\nprevious_version={}\ncreated_at={}\n",
        match o.platform { Platform::Linux=>"linux", Platform::Windows=>"windows", Platform::Macos=>"macos" },
        o.architecture, o.candidate_version, o.source_state_sha256.to_ascii_lowercase(),
        o.rc_archive_sha256.to_ascii_lowercase(), o.artifact_sha256.to_ascii_lowercase(), o.artifact_size,
        o.evidence_bundle_sha256.to_ascii_lowercase(), o.fresh_install, o.upgrade_n_minus_1,
        o.rollback_or_restore, o.native_tests, o.code_signing, o.notarization,
        if o.first_release {1} else {0}, o.previous_version.as_deref().unwrap_or(""),
        o.created_at.to_rfc3339_opts(SecondsFormat::AutoSi, true)
    ).into_bytes()
}

pub fn update_payload(o: &UpdateManifest) -> Vec<u8> {
    format!(
        "phxclaw-update-manifest-v027\nchannel={}\nsequence={}\nversion={}\nsource_state_sha256={}\nga_release_uuid={}\ntargets_sha256={}\ncreated_at={}\nexpires_at={}\n",
        o.channel, o.sequence, o.version, o.source_state_sha256.to_ascii_lowercase(), o.ga_release_uuid,
        hash_json(&o.targets), o.created_at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        o.expires_at.to_rfc3339_opts(SecondsFormat::AutoSi, true)
    ).into_bytes()
}

pub fn rollback_payload(o: &RollbackAuthorization) -> Vec<u8> {
    format!(
        "phxclaw-rollback-authorization-v027\nauthorization_uuid={}\nfrom_version={}\nto_version={}\nfrom_sequence={}\nto_sequence={}\nreason_sha256={:x}\ncreated_at={}\nexpires_at={}\n",
        o.authorization_uuid, o.from_version, o.to_version, o.from_sequence, o.to_sequence,
        Sha256::digest(o.reason.as_bytes()), o.created_at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        o.expires_at.to_rfc3339_opts(SecondsFormat::AutoSi, true)
    ).into_bytes()
}

pub fn verify_ed25519(
    payload: &[u8],
    key_id: &str,
    signature_b64: &str,
    signers: &[TrustedSigner],
) -> Result<(), GaError> {
    let signer = signers
        .iter()
        .find(|s| s.enabled && s.key_id == key_id)
        .ok_or(GaError::SignerNotTrusted)?;
    let kb = STANDARD
        .decode(&signer.public_key_b64)
        .map_err(|_| GaError::InvalidSignature)?;
    let ka: [u8; 32] = kb.try_into().map_err(|_| GaError::InvalidSignature)?;
    let key = VerifyingKey::from_bytes(&ka).map_err(|_| GaError::InvalidSignature)?;
    let sb = STANDARD
        .decode(signature_b64)
        .map_err(|_| GaError::InvalidSignature)?;
    let sig = Signature::try_from(sb.as_slice()).map_err(|_| GaError::InvalidSignature)?;
    key.verify_strict(payload, &sig)
        .map_err(|_| GaError::InvalidSignature)
}

pub fn verify_platform_evidence(
    o: &PlatformEvidence,
    signers: &[TrustedSigner],
) -> Result<(), GaError> {
    if !valid_sha256(&o.source_state_sha256)
        || !valid_sha256(&o.rc_archive_sha256)
        || !valid_sha256(&o.artifact_sha256)
        || !valid_sha256(&o.evidence_bundle_sha256)
        || o.artifact_size == 0
    {
        return Err(GaError::InvalidDigest);
    }
    if o.signature_algorithm != "ed25519" {
        return Err(GaError::InvalidSignature);
    }
    if o.native_tests != "verified" || o.fresh_install != "verified" {
        return Err(GaError::PlatformIncomplete);
    }
    if !o.first_release
        && (o.upgrade_n_minus_1 != "verified"
            || o.rollback_or_restore != "verified"
            || o.previous_version.is_none())
    {
        return Err(GaError::PlatformIncomplete);
    }
    match o.platform {
        Platform::Windows if o.code_signing != "verified" => {
            return Err(GaError::PlatformIncomplete)
        }
        Platform::Macos if o.code_signing != "verified" || o.notarization != "verified" => {
            return Err(GaError::PlatformIncomplete)
        }
        _ => {}
    }
    verify_ed25519(
        &platform_payload(o),
        &o.signer_key_id,
        &o.signature_b64,
        signers,
    )
}

pub fn verify_update_transition(
    manifest: &UpdateManifest,
    current_version: &str,
    current_sequence: u64,
    rollback: Option<&RollbackAuthorization>,
    now: DateTime<Utc>,
    signers: &[TrustedSigner],
) -> Result<(), GaError> {
    if manifest.sequence <= current_sequence {
        return Err(GaError::SequenceNotMonotonic);
    }
    if manifest.expires_at <= now
        || manifest.created_at > now + Duration::minutes(5)
        || manifest.expires_at - manifest.created_at > Duration::hours(168)
    {
        return Err(GaError::InvalidMetadataLifetime);
    }
    if !valid_sha256(&manifest.source_state_sha256)
        || manifest
            .targets
            .iter()
            .any(|t| !valid_sha256(&t.sha256) || t.size == 0 || !t.url.starts_with("https://"))
    {
        return Err(GaError::InvalidDigest);
    }
    verify_ed25519(
        &update_payload(manifest),
        &manifest.signer_key_id,
        &manifest.signature_b64,
        signers,
    )?;
    let current = Version::parse(current_version).map_err(|_| GaError::InvalidVersionTransition)?;
    let target =
        Version::parse(&manifest.version).map_err(|_| GaError::InvalidVersionTransition)?;
    if target < current {
        let token = rollback.ok_or(GaError::RollbackAuthorizationRequired)?;
        if token.from_version != current_version
            || token.to_version != manifest.version
            || token.from_sequence != current_sequence
            || token.to_sequence != manifest.sequence
            || token.expires_at <= now
            || token.created_at > now + Duration::minutes(5)
            || token.expires_at - token.created_at > Duration::hours(72)
        {
            return Err(GaError::RollbackAuthorizationRequired);
        }
        let from =
            Version::parse(&token.from_version).map_err(|_| GaError::InvalidVersionTransition)?;
        let to =
            Version::parse(&token.to_version).map_err(|_| GaError::InvalidVersionTransition)?;
        if to >= from {
            return Err(GaError::InvalidVersionTransition);
        }
        verify_ed25519(
            &rollback_payload(token),
            &token.signer_key_id,
            &token.signature_b64,
            signers,
        )?;
    }
    Ok(())
}

//! PhxClaw v0.26 Release Candidate Factory contracts.
//!
//! A release candidate is not eligible because a file exists. Eligibility is derived from
//! the signed v0.25 qualification attestation, the exact source-state digest, required
//! artifacts and cryptographically verifiable RC metadata.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, SecondsFormat, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use phxclaw_release_qualification::{
    verify_attestation, QualificationRun, ReleaseAttestationV025, ReleaseSurface,
    TrustedReleaseSigner,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum RcFactoryError {
    #[error("candidate version must be a semver prerelease")]
    InvalidCandidateVersion,
    #[error("qualification attestation is not release-ready")]
    QualificationNotReady,
    #[error("source-state differs from qualified source-state")]
    SourceStateMismatch,
    #[error("release candidate requires at least one native artifact")]
    MissingNativeArtifact,
    #[error("rollback input is required unless explicitly marked first release")]
    RollbackRequired,
    #[error("invalid SHA-256 digest")]
    InvalidDigest,
    #[error("RC signer is not trusted")]
    SignerNotTrusted,
    #[error("RC signature is invalid")]
    InvalidSignature,
    #[error("qualification verification failed")]
    QualificationVerification,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactDigest {
    pub name: String,
    pub sha256: String,
    pub size: u64,
    pub media_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseCandidateManifest {
    pub schema_version: String,
    pub candidate_uuid: Uuid,
    pub candidate_version: String,
    pub source_state_sha256: String,
    pub source_bundle_sha256: String,
    pub qualification_run_sha256: String,
    pub qualification_attestation_sha256: String,
    pub artifact_manifest_sha256: String,
    pub sbom_sha256: String,
    pub provenance_sha256: String,
    pub compatibility_matrix_sha256: String,
    pub sprint_promotion_plan_sha256: String,
    pub rollback_bundle_sha256: String,
    pub artifacts: Vec<ArtifactDigest>,
    pub created_at: DateTime<Utc>,
    pub signer_key_id: Option<String>,
    pub signature_algorithm: Option<String>,
    pub signature_b64: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateInputs {
    pub current_source_state_sha256: String,
    pub source_bundle_sha256: String,
    pub qualification_run_sha256: String,
    pub qualification_attestation_sha256: String,
    pub artifact_manifest_sha256: String,
    pub sbom_sha256: String,
    pub provenance_sha256: String,
    pub compatibility_matrix_sha256: String,
    pub sprint_promotion_plan_sha256: String,
    pub rollback_bundle_sha256: String,
    pub artifacts: Vec<ArtifactDigest>,
    pub first_release: bool,
    pub previous_release_present: bool,
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn candidate_signing_payload(manifest: &ReleaseCandidateManifest) -> Vec<u8> {
    let artifacts_hash = {
        let encoded = serde_json::to_vec(&manifest.artifacts).expect("artifact serialization");
        format!("{:x}", Sha256::digest(encoded))
    };
    format!(
        "phxclaw-release-candidate-v026\n\
         candidate_uuid={}\n\
         candidate_version={}\n\
         source_state_sha256={}\n\
         source_bundle_sha256={}\n\
         qualification_run_sha256={}\n\
         qualification_attestation_sha256={}\n\
         artifact_manifest_sha256={}\n\
         sbom_sha256={}\n\
         provenance_sha256={}\n\
         compatibility_matrix_sha256={}\n\
         sprint_promotion_plan_sha256={}\n\
         rollback_bundle_sha256={}\n\
         artifacts_sha256={}\n\
         created_at={}\n",
        manifest.candidate_uuid,
        manifest.candidate_version,
        manifest.source_state_sha256.to_ascii_lowercase(),
        manifest.source_bundle_sha256.to_ascii_lowercase(),
        manifest.qualification_run_sha256.to_ascii_lowercase(),
        manifest.qualification_attestation_sha256.to_ascii_lowercase(),
        manifest.artifact_manifest_sha256.to_ascii_lowercase(),
        manifest.sbom_sha256.to_ascii_lowercase(),
        manifest.provenance_sha256.to_ascii_lowercase(),
        manifest.compatibility_matrix_sha256.to_ascii_lowercase(),
        manifest.sprint_promotion_plan_sha256.to_ascii_lowercase(),
        manifest.rollback_bundle_sha256.to_ascii_lowercase(),
        artifacts_hash,
        manifest.created_at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
    ).into_bytes()
}

pub fn build_candidate_manifest(
    run: &QualificationRun,
    attestation: &ReleaseAttestationV025,
    release_policy: &phxclaw_release_hardening::ReleasePolicy,
    trusted_qualification_signers: &[TrustedReleaseSigner],
    candidate_version: &str,
    inputs: CandidateInputs,
    now: DateTime<Utc>,
) -> Result<ReleaseCandidateManifest, RcFactoryError> {
    let parsed = Version::parse(candidate_version).map_err(|_| RcFactoryError::InvalidCandidateVersion)?;
    if parsed.pre.is_empty() {
        return Err(RcFactoryError::InvalidCandidateVersion);
    }
    verify_attestation(
        run,
        attestation,
        release_policy,
        ReleaseSurface::PublicRelease,
        trusted_qualification_signers,
        now,
    ).map_err(|_| RcFactoryError::QualificationVerification)?;
    if !attestation.assessment.release_ready {
        return Err(RcFactoryError::QualificationNotReady);
    }
    if !inputs.current_source_state_sha256.eq_ignore_ascii_case(&attestation.workspace_sha256_hex) {
        return Err(RcFactoryError::SourceStateMismatch);
    }
    if inputs.artifacts.is_empty() {
        return Err(RcFactoryError::MissingNativeArtifact);
    }
    if !inputs.first_release && !inputs.previous_release_present {
        return Err(RcFactoryError::RollbackRequired);
    }
    let digests = [
        &inputs.current_source_state_sha256,
        &inputs.source_bundle_sha256,
        &inputs.qualification_run_sha256,
        &inputs.qualification_attestation_sha256,
        &inputs.artifact_manifest_sha256,
        &inputs.sbom_sha256,
        &inputs.provenance_sha256,
        &inputs.compatibility_matrix_sha256,
        &inputs.sprint_promotion_plan_sha256,
        &inputs.rollback_bundle_sha256,
    ];
    if digests.into_iter().any(|d| !valid_sha256(d))
        || inputs.artifacts.iter().any(|a| !valid_sha256(&a.sha256))
    {
        return Err(RcFactoryError::InvalidDigest);
    }
    Ok(ReleaseCandidateManifest {
        schema_version: "0.26.0".into(),
        candidate_uuid: Uuid::now_v7(),
        candidate_version: candidate_version.into(),
        source_state_sha256: inputs.current_source_state_sha256.to_ascii_lowercase(),
        source_bundle_sha256: inputs.source_bundle_sha256.to_ascii_lowercase(),
        qualification_run_sha256: inputs.qualification_run_sha256.to_ascii_lowercase(),
        qualification_attestation_sha256: inputs.qualification_attestation_sha256.to_ascii_lowercase(),
        artifact_manifest_sha256: inputs.artifact_manifest_sha256.to_ascii_lowercase(),
        sbom_sha256: inputs.sbom_sha256.to_ascii_lowercase(),
        provenance_sha256: inputs.provenance_sha256.to_ascii_lowercase(),
        compatibility_matrix_sha256: inputs.compatibility_matrix_sha256.to_ascii_lowercase(),
        sprint_promotion_plan_sha256: inputs.sprint_promotion_plan_sha256.to_ascii_lowercase(),
        rollback_bundle_sha256: inputs.rollback_bundle_sha256.to_ascii_lowercase(),
        artifacts: inputs.artifacts,
        created_at: now,
        signer_key_id: None,
        signature_algorithm: None,
        signature_b64: None,
    })
}

pub fn verify_candidate_signature(
    manifest: &ReleaseCandidateManifest,
    trusted_signers: &[TrustedReleaseSigner],
) -> Result<(), RcFactoryError> {
    let key_id = manifest.signer_key_id.as_deref().ok_or(RcFactoryError::InvalidSignature)?;
    if manifest.signature_algorithm.as_deref() != Some("ed25519") {
        return Err(RcFactoryError::InvalidSignature);
    }
    let sig_b64 = manifest.signature_b64.as_deref().ok_or(RcFactoryError::InvalidSignature)?;
    let signer = trusted_signers.iter().find(|s| s.key_id == key_id).ok_or(RcFactoryError::SignerNotTrusted)?;
    let key_bytes = STANDARD.decode(&signer.public_key_b64).map_err(|_| RcFactoryError::InvalidSignature)?;
    let key_array: [u8; 32] = key_bytes.try_into().map_err(|_| RcFactoryError::InvalidSignature)?;
    let key = VerifyingKey::from_bytes(&key_array).map_err(|_| RcFactoryError::InvalidSignature)?;
    let sig_bytes = STANDARD.decode(sig_b64).map_err(|_| RcFactoryError::InvalidSignature)?;
    let signature = Signature::try_from(sig_bytes.as_slice()).map_err(|_| RcFactoryError::InvalidSignature)?;
    key.verify_strict(&candidate_signing_payload(manifest), &signature)
        .map_err(|_| RcFactoryError::InvalidSignature)
}

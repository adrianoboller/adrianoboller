//! PhxClaw v0.25 native release qualification contracts.
//! The qualification layer never upgrades missing/unavailable execution to success.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, SecondsFormat, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use phxclaw_release_hardening::{
    evaluate_release, GateProof, GateStatus, ReleaseAssessment, ReleaseError, ReleaseGate,
    ReleasePolicy, REQUIRED_GATES,
};
use serde::{Deserialize, Serialize};
use serde_json;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum QualificationError {
    #[error("release hardening rejected evidence: {0}")]
    Release(#[from] ReleaseError),
    #[error("invalid SHA-256 digest")]
    InvalidDigest,
    #[error("attestation source-state differs from qualification run")]
    SourceStateMismatch,
    #[error("attestation assessment differs from recomputed assessment")]
    AssessmentMismatch,
    #[error("public release attestation must be signed")]
    SignatureRequired,
    #[error("signer key id and signature must be present together")]
    PartialSignature,
    #[error("release signer is not trusted")]
    SignerNotTrusted,
    #[error("trusted signer public key is invalid")]
    InvalidPublicKey,
    #[error("attestation signature is invalid")]
    InvalidSignature,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct QualificationRun {
    pub run_uuid: Uuid,
    pub release_uuid: Uuid,
    pub version: String,
    #[serde(rename = "workspace_sha256")]
    pub workspace_sha256_hex: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub proofs: Vec<GateProof>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseAttestationV025 {
    pub attestation_uuid: Uuid,
    pub run_uuid: Uuid,
    pub release_uuid: Uuid,
    pub version: String,
    #[serde(rename = "workspace_sha256")]
    pub workspace_sha256_hex: String,
    pub assessment: ReleaseAssessment,
    #[serde(rename = "proof_bundle_sha256")]
    pub proof_bundle_sha256_hex: String,
    #[serde(rename = "signing_payload_sha256")]
    pub signing_payload_sha256_hex: String,
    pub created_at: DateTime<Utc>,
    pub signer_key_id: Option<String>,
    pub signature_b64: Option<String>,
    pub signature_algorithm: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseSurface {
    InternalQualification,
    PublicRelease,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedReleaseSigner {
    pub key_id: String,
    pub public_key_b64: String,
}

pub fn attestation_signing_payload(attestation: &ReleaseAttestationV025) -> Vec<u8> {
    let a = &attestation.assessment;
    format!(
        "phxclaw-release-attestation-v025\nattestation_uuid={}\nrun_uuid={}\nrelease_uuid={}\nversion={}\nworkspace_sha256={}\nproof_bundle_sha256={}\nsource_ready={}\nstatic_verified={}\nruntime_verified={}\ne2e_verified={}\nrelease_ready={}\ncreated_at={}\n",
        attestation.attestation_uuid,
        attestation.run_uuid,
        attestation.release_uuid,
        attestation.version,
        attestation.workspace_sha256_hex.to_ascii_lowercase(),
        attestation.proof_bundle_sha256_hex.to_ascii_lowercase(),
        u8::from(a.source_ready),
        u8::from(a.static_verified),
        u8::from(a.runtime_verified),
        u8::from(a.e2e_verified),
        u8::from(a.release_ready),
        attestation.created_at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
    ).into_bytes()
}

pub fn verify_attestation_signature(
    attestation: &ReleaseAttestationV025,
    trusted_signers: &[TrustedReleaseSigner],
) -> Result<(), QualificationError> {
    let key_id = attestation
        .signer_key_id
        .as_deref()
        .ok_or(QualificationError::SignatureRequired)?;
    let signature_b64 = attestation
        .signature_b64
        .as_deref()
        .ok_or(QualificationError::SignatureRequired)?;
    let signer = trusted_signers
        .iter()
        .find(|s| s.key_id == key_id)
        .ok_or(QualificationError::SignerNotTrusted)?;
    let key_bytes = STANDARD
        .decode(&signer.public_key_b64)
        .map_err(|_| QualificationError::InvalidPublicKey)?;
    let key_array: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| QualificationError::InvalidPublicKey)?;
    let key =
        VerifyingKey::from_bytes(&key_array).map_err(|_| QualificationError::InvalidPublicKey)?;
    let sig_bytes = STANDARD
        .decode(signature_b64)
        .map_err(|_| QualificationError::InvalidSignature)?;
    let signature = Signature::try_from(sig_bytes.as_slice())
        .map_err(|_| QualificationError::InvalidSignature)?;
    key.verify_strict(&attestation_signing_payload(attestation), &signature)
        .map_err(|_| QualificationError::InvalidSignature)
}

fn valid_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn proof_bundle_sha256_hex(proofs: &[GateProof]) -> String {
    #[derive(Serialize)]
    struct CanonicalProof<'a> {
        gate: &'static str,
        status: &'static str,
        source_state_sha256: String,
        evidence_sha256: Option<&'a str>,
        tool_name: &'a str,
        tool_version: Option<&'a str>,
        evidence_ref: Option<&'a str>,
        verified_at: String,
    }
    fn gate_name(gate: ReleaseGate) -> &'static str {
        match gate {
            ReleaseGate::WorkspaceStatic => "workspace_static",
            ReleaseGate::JsonSchemaStatic => "json_schema_static",
            ReleaseGate::MigrationStatic => "migration_static",
            ReleaseGate::LicenseSbom => "license_sbom",
            ReleaseGate::SupplyChain => "supply_chain",
            ReleaseGate::CargoFmt => "cargo_fmt",
            ReleaseGate::CargoCheck => "cargo_check",
            ReleaseGate::CargoTest => "cargo_test",
            ReleaseGate::CargoClippy => "cargo_clippy",
            ReleaseGate::PostgresMigration => "postgres_migration",
            ReleaseGate::RlsCrossTenant => "rls_cross_tenant",
            ReleaseGate::SkillKnowledgeLineage => "skill_knowledge_lineage",
            ReleaseGate::MissionE2e => "mission_e2e",
            ReleaseGate::TauriNativeE2e => "tauri_native_e2e",
            ReleaseGate::ProviderModelE2e => "provider_model_e2e",
        }
    }
    fn status_name(status: GateStatus) -> &'static str {
        match status {
            GateStatus::Verified => "verified",
            GateStatus::Failed => "failed",
            GateStatus::Unavailable => "unavailable",
        }
    }
    let mut rows: Vec<CanonicalProof<'_>> = proofs
        .iter()
        .map(|proof| CanonicalProof {
            gate: gate_name(proof.gate),
            status: status_name(proof.status),
            source_state_sha256: proof.source_state_sha256_hex.to_ascii_lowercase(),
            evidence_sha256: proof.evidence_sha256_hex.as_deref(),
            tool_name: &proof.tool_name,
            tool_version: proof.tool_version.as_deref(),
            evidence_ref: proof.evidence_ref.as_deref(),
            verified_at: proof
                .verified_at
                .to_rfc3339_opts(SecondsFormat::AutoSi, true),
        })
        .collect();
    rows.sort_by_key(|row| row.gate);
    let encoded = serde_json::to_vec(&rows).expect("canonical proof serialization cannot fail");
    format!("{:x}", Sha256::digest(encoded))
}

pub fn build_attestation(
    run: &QualificationRun,
    policy: &ReleasePolicy,
    now: DateTime<Utc>,
) -> Result<ReleaseAttestationV025, QualificationError> {
    if !valid_sha256_hex(&run.workspace_sha256_hex) {
        return Err(QualificationError::InvalidDigest);
    }
    let assessment = evaluate_release(&run.workspace_sha256_hex, &run.proofs, policy, now)?;
    let mut attestation = ReleaseAttestationV025 {
        attestation_uuid: Uuid::now_v7(),
        run_uuid: run.run_uuid,
        release_uuid: run.release_uuid,
        version: run.version.clone(),
        workspace_sha256_hex: run.workspace_sha256_hex.to_ascii_lowercase(),
        assessment,
        proof_bundle_sha256_hex: proof_bundle_sha256_hex(&run.proofs),
        signing_payload_sha256_hex: String::new(),
        created_at: now,
        signer_key_id: None,
        signature_b64: None,
        signature_algorithm: None,
    };
    attestation.signing_payload_sha256_hex = format!(
        "{:x}",
        Sha256::digest(attestation_signing_payload(&attestation))
    );
    Ok(attestation)
}

pub fn verify_attestation(
    run: &QualificationRun,
    attestation: &ReleaseAttestationV025,
    policy: &ReleasePolicy,
    surface: ReleaseSurface,
    trusted_signers: &[TrustedReleaseSigner],
    now: DateTime<Utc>,
) -> Result<(), QualificationError> {
    if !valid_sha256_hex(&attestation.workspace_sha256_hex)
        || !valid_sha256_hex(&attestation.proof_bundle_sha256_hex)
        || !valid_sha256_hex(&attestation.signing_payload_sha256_hex)
    {
        return Err(QualificationError::InvalidDigest);
    }
    if run.run_uuid != attestation.run_uuid
        || run.release_uuid != attestation.release_uuid
        || run.version != attestation.version
        || !run
            .workspace_sha256_hex
            .eq_ignore_ascii_case(&attestation.workspace_sha256_hex)
    {
        return Err(QualificationError::SourceStateMismatch);
    }
    let recomputed = evaluate_release(&run.workspace_sha256_hex, &run.proofs, policy, now)?;
    if recomputed != attestation.assessment
        || proof_bundle_sha256_hex(&run.proofs) != attestation.proof_bundle_sha256_hex
        || format!(
            "{:x}",
            Sha256::digest(attestation_signing_payload(attestation))
        ) != attestation.signing_payload_sha256_hex
    {
        return Err(QualificationError::AssessmentMismatch);
    }
    match (&attestation.signer_key_id, &attestation.signature_b64) {
        (Some(_), Some(_)) => {
            if attestation.signature_algorithm.as_deref() != Some("ed25519") {
                return Err(QualificationError::InvalidSignature);
            }
            verify_attestation_signature(attestation, trusted_signers)?
        }
        (None, None) if surface == ReleaseSurface::InternalQualification => {}
        (None, None) => return Err(QualificationError::SignatureRequired),
        _ => return Err(QualificationError::PartialSignature),
    }
    Ok(())
}

pub fn all_verified_proofs(workspace_sha256_hex: &str, now: DateTime<Utc>) -> Vec<GateProof> {
    REQUIRED_GATES
        .iter()
        .map(|gate| GateProof {
            gate: *gate,
            status: GateStatus::Verified,
            source_state_sha256_hex: workspace_sha256_hex.to_string(),
            evidence_sha256_hex: Some("b".repeat(64)),
            tool_name: "qualification-test".into(),
            tool_version: Some("0.25.0".into()),
            evidence_ref: None,
            verified_at: now,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    fn digest(ch: char) -> String {
        ch.to_string().repeat(64)
    }

    #[test]
    fn all_verified_proofs_can_be_release_ready() {
        let now = Utc::now();
        let workspace = digest('a');
        let run = QualificationRun {
            run_uuid: Uuid::now_v7(),
            release_uuid: Uuid::now_v7(),
            version: "0.25.0".into(),
            workspace_sha256_hex: workspace.clone(),
            started_at: now,
            finished_at: now,
            proofs: all_verified_proofs(&workspace, now),
        };
        let attestation = build_attestation(&run, &ReleasePolicy::default(), now).unwrap();
        assert!(attestation.assessment.release_ready);
        verify_attestation(
            &run,
            &attestation,
            &ReleasePolicy::default(),
            ReleaseSurface::InternalQualification,
            &[],
            now,
        )
        .unwrap();
    }

    #[test]
    fn unavailable_gate_blocks_release() {
        let now = Utc::now();
        let workspace = digest('a');
        let mut proofs = all_verified_proofs(&workspace, now);
        let cargo_check = proofs
            .iter_mut()
            .find(|p| p.gate == ReleaseGate::CargoCheck)
            .unwrap();
        cargo_check.status = GateStatus::Unavailable;
        cargo_check.evidence_sha256_hex = None;
        let run = QualificationRun {
            run_uuid: Uuid::now_v7(),
            release_uuid: Uuid::now_v7(),
            version: "0.25.0".into(),
            workspace_sha256_hex: workspace,
            started_at: now,
            finished_at: now,
            proofs,
        };
        let attestation = build_attestation(&run, &ReleasePolicy::default(), now).unwrap();
        assert!(!attestation.assessment.release_ready);
    }

    #[test]
    fn proof_bundle_hash_is_cross_language_canonical() {
        let when = DateTime::parse_from_rfc3339("2026-09-28T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let proofs = vec![
            GateProof {
                gate: ReleaseGate::WorkspaceStatic,
                status: GateStatus::Verified,
                source_state_sha256_hex: "a".repeat(64),
                evidence_sha256_hex: Some("c".repeat(64)),
                tool_name: "verify_v025".into(),
                tool_version: Some("0.25.0".into()),
                evidence_ref: Some("workspace_static.log".into()),
                verified_at: when,
            },
            GateProof {
                gate: ReleaseGate::CargoCheck,
                status: GateStatus::Verified,
                source_state_sha256_hex: "a".repeat(64),
                evidence_sha256_hex: Some("b".repeat(64)),
                tool_name: "cargo".into(),
                tool_version: Some("1.90.0".into()),
                evidence_ref: Some("cargo_check.log".into()),
                verified_at: when,
            },
        ];
        assert_eq!(
            proof_bundle_sha256_hex(&proofs),
            "8026ca26cea13700bba5f5f7fea2e9370768120d97b44999dab08b0a2d5eceb6"
        );
    }

    #[test]
    fn public_release_requires_and_verifies_trusted_ed25519_signature() {
        use ed25519_dalek::{Signer, SigningKey};
        let now = Utc::now();
        let workspace = digest('a');
        let run = QualificationRun {
            run_uuid: Uuid::now_v7(),
            release_uuid: Uuid::now_v7(),
            version: "0.25.0".into(),
            workspace_sha256_hex: workspace.clone(),
            started_at: now,
            finished_at: now,
            proofs: all_verified_proofs(&workspace, now),
        };
        let mut attestation = build_attestation(&run, &ReleasePolicy::default(), now).unwrap();
        assert_eq!(
            verify_attestation(
                &run,
                &attestation,
                &ReleasePolicy::default(),
                ReleaseSurface::PublicRelease,
                &[],
                now
            ),
            Err(QualificationError::SignatureRequired)
        );
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let verifying = signing.verifying_key();
        attestation.signer_key_id = Some("release-test".into());
        let signature = signing.sign(&attestation_signing_payload(&attestation));
        attestation.signature_b64 = Some(STANDARD.encode(signature.to_bytes()));
        attestation.signature_algorithm = Some("ed25519".into());
        let trusted = vec![TrustedReleaseSigner {
            key_id: "release-test".into(),
            public_key_b64: STANDARD.encode(verifying.to_bytes()),
        }];
        verify_attestation(
            &run,
            &attestation,
            &ReleasePolicy::default(),
            ReleaseSurface::PublicRelease,
            &trusted,
            now,
        )
        .unwrap();
        attestation.signature_b64 = Some(STANDARD.encode([0u8; 64]));
        assert_eq!(
            verify_attestation(
                &run,
                &attestation,
                &ReleasePolicy::default(),
                ReleaseSurface::PublicRelease,
                &trusted,
                now
            ),
            Err(QualificationError::InvalidSignature)
        );
    }
}

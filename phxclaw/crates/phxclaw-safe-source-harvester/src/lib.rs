#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use phxclaw_evidence_ledger::{EvidenceDraft, EvidenceLedger, EvidenceOutcome, LedgerError};
use phxclaw_knowledge_evidence_graph::{EpistemicState, KnowledgeNode, NodeKind};
use phxclaw_provenance_core::{evaluate, IngestionDecision, ProvenanceClaim, ReasonCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestPolicy {
    pub max_files: usize,
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
    pub max_excerpt_bytes: usize,
    pub reject_symlinks: bool,
    pub allowed_extensions: BTreeSet<String>,
    pub secret_markers: Vec<String>,
}

impl Default for HarvestPolicy {
    fn default() -> Self {
        Self {
            max_files: 20_000,
            max_file_bytes: 4 * 1024 * 1024,
            max_total_bytes: 256 * 1024 * 1024,
            max_excerpt_bytes: 2_048,
            reject_symlinks: true,
            allowed_extensions: [
                "rs", "toml", "json", "jsonl", "md", "txt", "yaml", "yml", "sql", "py", "js",
                "jsx", "ts", "tsx", "c", "h", "cpp", "hpp", "cc", "java", "go", "cs", "php", "rb",
                "swift", "kt", "kts", "xml", "html", "css", "scss", "sh", "ps1", "bat", "ini",
                "conf", "proto", "graphql", "wl", "cob", "cbl", "rpg", "cl",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            secret_markers: vec![
                "-----BEGIN PRIVATE KEY-----".into(),
                "-----BEGIN OPENSSH PRIVATE KEY-----".into(),
                "AWS_SECRET_ACCESS_KEY=".into(),
                "ANTHROPIC_API_KEY=".into(),
                "OPENAI_API_KEY=".into(),
                "GITHUB_TOKEN=".into(),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HarvestDecision {
    Allow,
    Quarantine,
    Deny,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HarvestFileState {
    Accepted,
    QuarantinedSecret,
    QuarantinedOversize,
    SkippedBinary,
    SkippedExtension,
    SkippedSymlink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestedFile {
    pub relative_path: String,
    pub byte_len: u64,
    pub sha256_hex: Option<String>,
    pub state: HarvestFileState,
    pub content_address_uri: Option<String>,
    pub detected_markers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeCandidate {
    pub candidate_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub source_artifact_uuid: Uuid,
    pub source_path: String,
    pub content_sha256_hex: String,
    pub source_state_sha256_hex: String,
    pub mechanism: String,
    pub excerpt: String,
    pub collected_at: DateTime<Utc>,
}

impl KnowledgeCandidate {
    pub fn to_raw_observation_node(&self) -> KnowledgeNode {
        KnowledgeNode {
            node_uuid: Uuid::now_v7(),
            tenant_uuid: self.tenant_uuid,
            kind: NodeKind::Evidence,
            state: EpistemicState::RawObservation,
            content_sha256_hex: self.content_sha256_hex.clone(),
            source_state_sha256_hex: self.source_state_sha256_hex.clone(),
            subject_key: Some(self.source_path.clone()),
            predicate_key: Some("source.harvest.observation".into()),
            value_sha256_hex: None,
            confidence_ppm: 500_000,
            created_at: self.collected_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestReceipt {
    pub run_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub source_artifact_uuid: Uuid,
    pub source_name: String,
    pub artifact_sha256_hex: String,
    pub provenance_decision: HarvestDecision,
    pub final_decision: HarvestDecision,
    pub provenance_reasons: Vec<String>,
    pub security_reasons: Vec<String>,
    pub files_seen: usize,
    pub bytes_seen: u64,
    pub files: Vec<HarvestedFile>,
    pub candidates: Vec<KnowledgeCandidate>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

impl HarvestReceipt {
    pub fn reusable_knowledge_allowed(&self) -> bool {
        self.final_decision == HarvestDecision::Allow
    }
}

pub struct HarvestRequest<'a> {
    pub tenant_uuid: Uuid,
    pub source_artifact_uuid: Uuid,
    pub artifact_sha256_hex: &'a str,
    pub source_root: &'a Path,
    pub store_root: &'a Path,
    pub actor: &'a str,
    pub claim: ProvenanceClaim<'a>,
}

#[derive(Debug, Error)]
pub enum HarvestError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("evidence ledger error: {0}")]
    Ledger(#[from] LedgerError),
    #[error("source root is not a directory")]
    RootNotDirectory,
    #[error("invalid artifact sha256")]
    InvalidArtifactSha256,
}

#[derive(Debug, Clone)]
pub struct SafeSourceHarvester {
    policy: HarvestPolicy,
}

impl SafeSourceHarvester {
    pub fn new(policy: HarvestPolicy) -> Self {
        Self { policy }
    }
    pub fn policy(&self) -> &HarvestPolicy {
        &self.policy
    }

    pub fn harvest(
        &self,
        request: HarvestRequest<'_>,
        ledger: Option<&EvidenceLedger>,
    ) -> Result<HarvestReceipt, HarvestError> {
        if !request.source_root.is_dir() {
            return Err(HarvestError::RootNotDirectory);
        }
        if !valid_sha256(request.artifact_sha256_hex) {
            return Err(HarvestError::InvalidArtifactSha256);
        }

        let started_at = Utc::now();
        let run_uuid = Uuid::now_v7();
        let evaluation = evaluate(&request.claim);
        let provenance_decision = map_decision(evaluation.decision);
        let provenance_reasons = evaluation
            .reasons
            .iter()
            .map(reason_name)
            .map(str::to_string)
            .collect::<Vec<_>>();

        if provenance_decision == HarvestDecision::Deny {
            let receipt = HarvestReceipt {
                run_uuid,
                tenant_uuid: request.tenant_uuid,
                source_artifact_uuid: request.source_artifact_uuid,
                source_name: request.claim.source_name.to_string(),
                artifact_sha256_hex: request.artifact_sha256_hex.to_ascii_lowercase(),
                provenance_decision,
                final_decision: HarvestDecision::Deny,
                provenance_reasons,
                security_reasons: vec![],
                files_seen: 0,
                bytes_seen: 0,
                files: vec![],
                candidates: vec![],
                started_at,
                completed_at: Utc::now(),
            };
            self.append_evidence(&receipt, request.actor, ledger)?;
            return Ok(receipt);
        }

        let mut paths = Vec::new();
        collect_paths(request.source_root, request.source_root, &mut paths)?;
        paths.sort();

        let mut files = Vec::new();
        let mut candidates = Vec::new();
        let mut security_reasons = Vec::new();
        let mut files_seen = 0usize;
        let mut bytes_seen = 0u64;
        let mut security_quarantine = false;

        for path in paths {
            if files_seen >= self.policy.max_files {
                security_reasons.push("max_files_exceeded".into());
                security_quarantine = true;
                break;
            }
            let rel = normalized_relative(request.source_root, &path);
            let meta = fs::symlink_metadata(&path)?;
            if meta.file_type().is_symlink() {
                if self.policy.reject_symlinks {
                    files.push(HarvestedFile {
                        relative_path: rel,
                        byte_len: 0,
                        sha256_hex: None,
                        state: HarvestFileState::SkippedSymlink,
                        content_address_uri: None,
                        detected_markers: vec![],
                    });
                    security_reasons.push("symlink_rejected".into());
                    security_quarantine = true;
                }
                continue;
            }
            if !meta.is_file() {
                continue;
            }
            files_seen += 1;
            let len = meta.len();
            bytes_seen = bytes_seen.saturating_add(len);
            if bytes_seen > self.policy.max_total_bytes {
                security_reasons.push("max_total_bytes_exceeded".into());
                security_quarantine = true;
                break;
            }
            let ext = path
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !self.policy.allowed_extensions.contains(&ext) {
                files.push(HarvestedFile {
                    relative_path: rel,
                    byte_len: len,
                    sha256_hex: None,
                    state: HarvestFileState::SkippedExtension,
                    content_address_uri: None,
                    detected_markers: vec![],
                });
                continue;
            }
            if len > self.policy.max_file_bytes {
                files.push(HarvestedFile {
                    relative_path: rel,
                    byte_len: len,
                    sha256_hex: None,
                    state: HarvestFileState::QuarantinedOversize,
                    content_address_uri: None,
                    detected_markers: vec![],
                });
                security_reasons.push("max_file_bytes_exceeded".into());
                security_quarantine = true;
                continue;
            }
            let bytes = fs::read(&path)?;
            if bytes.contains(&0) {
                files.push(HarvestedFile {
                    relative_path: rel,
                    byte_len: len,
                    sha256_hex: None,
                    state: HarvestFileState::SkippedBinary,
                    content_address_uri: None,
                    detected_markers: vec![],
                });
                continue;
            }
            let digest = sha256_hex(&bytes);
            let text = String::from_utf8_lossy(&bytes);
            let markers = self
                .policy
                .secret_markers
                .iter()
                .filter(|m| text.contains(m.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            let has_secret = !markers.is_empty();
            if has_secret {
                security_reasons.push(format!("secret_marker:{}", rel));
                security_quarantine = true;
            }

            let bucket = if provenance_decision == HarvestDecision::Allow && !has_secret {
                "allowed"
            } else {
                "quarantine"
            };
            let stored = store_content_addressed(request.store_root, bucket, &digest, &bytes)?;
            let state = if has_secret {
                HarvestFileState::QuarantinedSecret
            } else {
                HarvestFileState::Accepted
            };
            files.push(HarvestedFile {
                relative_path: rel.clone(),
                byte_len: len,
                sha256_hex: Some(digest.clone()),
                state,
                content_address_uri: Some(stored),
                detected_markers: markers,
            });

            if provenance_decision == HarvestDecision::Allow && !has_secret {
                candidates.push(KnowledgeCandidate {
                    candidate_uuid: Uuid::now_v7(),
                    tenant_uuid: request.tenant_uuid,
                    source_artifact_uuid: request.source_artifact_uuid,
                    source_path: rel,
                    content_sha256_hex: digest,
                    source_state_sha256_hex: request.artifact_sha256_hex.to_ascii_lowercase(),
                    mechanism: "safe_source_harvester:v0.61:deterministic_text".into(),
                    excerpt: bounded_excerpt(&text, self.policy.max_excerpt_bytes),
                    collected_at: Utc::now(),
                });
            }
        }

        let final_decision =
            if provenance_decision == HarvestDecision::Quarantine || security_quarantine {
                HarvestDecision::Quarantine
            } else {
                HarvestDecision::Allow
            };
        if final_decision != HarvestDecision::Allow {
            candidates.clear();
        }
        security_reasons.sort();
        security_reasons.dedup();
        let receipt = HarvestReceipt {
            run_uuid,
            tenant_uuid: request.tenant_uuid,
            source_artifact_uuid: request.source_artifact_uuid,
            source_name: request.claim.source_name.to_string(),
            artifact_sha256_hex: request.artifact_sha256_hex.to_ascii_lowercase(),
            provenance_decision,
            final_decision,
            provenance_reasons,
            security_reasons,
            files_seen,
            bytes_seen,
            files,
            candidates,
            started_at,
            completed_at: Utc::now(),
        };
        self.append_evidence(&receipt, request.actor, ledger)?;
        Ok(receipt)
    }

    fn append_evidence(
        &self,
        receipt: &HarvestReceipt,
        actor: &str,
        ledger: Option<&EvidenceLedger>,
    ) -> Result<(), HarvestError> {
        let Some(ledger) = ledger else { return Ok(()) };
        let outcome = match receipt.final_decision {
            HarvestDecision::Deny => EvidenceOutcome::Denied,
            _ => EvidenceOutcome::Succeeded,
        };
        ledger.append(EvidenceDraft{
            action_uuid:receipt.run_uuid, correlation_uuid:None, actor:actor.into(), capability:"source.harvest.safe".into(), action:"harvest".into(), outcome,
            request_summary:json!({"source":receipt.source_name,"artifact_uuid":receipt.source_artifact_uuid,"artifact_sha256":receipt.artifact_sha256_hex}),
            result_summary:json!({"provenance_decision":receipt.provenance_decision,"final_decision":receipt.final_decision,"files_seen":receipt.files_seen,"bytes_seen":receipt.bytes_seen,"candidate_count":receipt.candidates.len(),"security_reasons":receipt.security_reasons}),
            artifact_uris:receipt.files.iter().filter_map(|f|f.content_address_uri.clone()).collect(),
        })?;
        Ok(())
    }
}

fn collect_paths(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            out.push(path);
            continue;
        }
        if ft.is_dir() {
            collect_paths(root, &path, out)?;
        } else if ft.is_file() {
            out.push(path);
        }
    }
    let _ = root;
    Ok(())
}

fn normalized_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
fn valid_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn bounded_excerpt(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}
fn map_decision(d: IngestionDecision) -> HarvestDecision {
    match d {
        IngestionDecision::Allow => HarvestDecision::Allow,
        IngestionDecision::Quarantine => HarvestDecision::Quarantine,
        IngestionDecision::Deny => HarvestDecision::Deny,
    }
}
fn reason_name(r: &ReasonCode) -> &'static str {
    match r {
        ReasonCode::VerifiedPermissiveLicense => "verified_permissive_license",
        ReasonCode::LicenseEvidenceMissing => "license_evidence_missing",
        ReasonCode::UnknownProvenance => "unknown_provenance",
        ReasonCode::CopyleftNeedsCompatibilityReview => "copyleft_needs_compatibility_review",
        ReasonCode::ProprietarySource => "proprietary_source",
        ReasonCode::DeclaredLeak => "declared_leak",
        ReasonCode::RedistributionProhibited => "redistribution_prohibited",
        ReasonCode::HashEvidenceMissing => "hash_evidence_missing",
    }
}

fn store_content_addressed(
    root: &Path,
    bucket: &str,
    digest: &str,
    bytes: &[u8],
) -> Result<String, std::io::Error> {
    let dir = root.join(bucket).join("sha256").join(&digest[..2]);
    fs::create_dir_all(&dir)?;
    let path = dir.join(digest);
    if !path.exists() {
        fs::write(&path, bytes)?;
        let mut p = fs::metadata(&path)?.permissions();
        p.set_readonly(true);
        fs::set_permissions(&path, p)?;
    }
    Ok(format!(
        "phxclaw://source-vault/{bucket}/sha256/{}/{}",
        &digest[..2],
        digest
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn claim<'a>(name: &'a str) -> ProvenanceClaim<'a> {
        ProvenanceClaim {
            source_name: name,
            license_class: phxclaw_provenance_core::LicenseClass::Permissive,
            local_license_evidence: true,
            archive_hash_verified: true,
            origin_known: true,
            declared_leak: false,
            redistribution_prohibited: false,
        }
    }
    fn dirs(tag: &str) -> (PathBuf, PathBuf) {
        let id = Uuid::now_v7();
        let root = std::env::temp_dir().join(format!("phx-harvest-{tag}-{id}"));
        let store = std::env::temp_dir().join(format!("phx-store-{tag}-{id}"));
        fs::create_dir_all(&root).unwrap();
        (root, store)
    }
    #[test]
    fn allow_creates_candidate_and_content_address() {
        let (root, store) = dirs("allow");
        fs::write(root.join("README.md"), "safe documentation").unwrap();
        let h = SafeSourceHarvester::new(HarvestPolicy::default());
        let r = h
            .harvest(
                HarvestRequest {
                    tenant_uuid: Uuid::now_v7(),
                    source_artifact_uuid: Uuid::now_v7(),
                    artifact_sha256_hex: &"a".repeat(64),
                    source_root: &root,
                    store_root: &store,
                    actor: "test",
                    claim: claim("fixture"),
                },
                None,
            )
            .unwrap();
        assert_eq!(r.final_decision, HarvestDecision::Allow);
        assert_eq!(r.candidates.len(), 1);
        assert!(store.join("allowed/sha256").exists());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(store);
    }
    #[test]
    fn secret_forces_quarantine_and_zero_candidates() {
        let (root, store) = dirs("secret");
        fs::write(root.join("x.txt"), "OPENAI_API_KEY=secret").unwrap();
        let h = SafeSourceHarvester::new(HarvestPolicy::default());
        let r = h
            .harvest(
                HarvestRequest {
                    tenant_uuid: Uuid::now_v7(),
                    source_artifact_uuid: Uuid::now_v7(),
                    artifact_sha256_hex: &"b".repeat(64),
                    source_root: &root,
                    store_root: &store,
                    actor: "test",
                    claim: claim("fixture"),
                },
                None,
            )
            .unwrap();
        assert_eq!(r.final_decision, HarvestDecision::Quarantine);
        assert!(r.candidates.is_empty());
        assert!(store.join("quarantine/sha256").exists());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(store);
    }
    #[test]
    fn proprietary_is_denied_without_copying() {
        let (root, store) = dirs("deny");
        fs::write(root.join("x.rs"), "fn main(){}").unwrap();
        let mut c = claim("blocked");
        c.license_class = phxclaw_provenance_core::LicenseClass::Proprietary;
        let h = SafeSourceHarvester::new(HarvestPolicy::default());
        let r = h
            .harvest(
                HarvestRequest {
                    tenant_uuid: Uuid::now_v7(),
                    source_artifact_uuid: Uuid::now_v7(),
                    artifact_sha256_hex: &"c".repeat(64),
                    source_root: &root,
                    store_root: &store,
                    actor: "test",
                    claim: c,
                },
                None,
            )
            .unwrap();
        assert_eq!(r.final_decision, HarvestDecision::Deny);
        assert_eq!(r.files_seen, 0);
        assert!(!store.exists());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn quarantine_provenance_never_yields_candidates() {
        let (root, store) = dirs("q");
        fs::write(root.join("x.md"), "text").unwrap();
        let mut c = claim("unknown");
        c.origin_known = false;
        let h = SafeSourceHarvester::new(HarvestPolicy::default());
        let r = h
            .harvest(
                HarvestRequest {
                    tenant_uuid: Uuid::now_v7(),
                    source_artifact_uuid: Uuid::now_v7(),
                    artifact_sha256_hex: &"d".repeat(64),
                    source_root: &root,
                    store_root: &store,
                    actor: "test",
                    claim: c,
                },
                None,
            )
            .unwrap();
        assert_eq!(r.final_decision, HarvestDecision::Quarantine);
        assert!(r.candidates.is_empty());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(store);
    }
    #[test]
    fn invalid_hash_fails_closed() {
        let (root, store) = dirs("bad-hash");
        let h = SafeSourceHarvester::new(HarvestPolicy::default());
        let e = h
            .harvest(
                HarvestRequest {
                    tenant_uuid: Uuid::now_v7(),
                    source_artifact_uuid: Uuid::now_v7(),
                    artifact_sha256_hex: "bad",
                    source_root: &root,
                    store_root: &store,
                    actor: "test",
                    claim: claim("fixture"),
                },
                None,
            )
            .unwrap_err();
        assert!(matches!(e, HarvestError::InvalidArtifactSha256));
        let _ = fs::remove_dir_all(root);
    }
}

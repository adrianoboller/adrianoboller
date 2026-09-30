//! Reconstructed PhxClaw v0.34 provider capacity contracts.
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapacitySpec {
    pub tenant_uuid: Uuid,
    pub snapshot_uuid: Uuid,
    pub provider_uuid: Uuid,
    pub requests_limit: u64,
    pub requests_available: u64,
    pub tokens_limit: u64,
    pub tokens_available: u64,
    pub concurrency_limit: u32,
    pub concurrency_available: u32,
    pub observed_at_unix: i64,
    pub ttl_seconds: u64,
    pub reset_at_unix: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedCapacitySnapshot {
    pub document_sha256: String,
    pub signer_id: String,
    pub signature_hex: String,
    pub spec: CapacitySpec,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifiedCapacitySnapshot {
    pub document_sha256: String,
    pub signer_id: String,
    pub spec: CapacitySpec,
}
#[derive(Error, Debug, PartialEq, Eq)]
pub enum CapacityError {
    #[error("stale or invalid capacity evidence")]
    InvalidEvidence,
    #[error("untrusted signer")]
    UntrustedSigner,
    #[error("invalid signature")]
    InvalidSignature,
    #[error("insufficient capacity")]
    InsufficientCapacity,
}
fn valid_sha(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn hash(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}
pub fn verify_capacity(
    now: i64,
    doc: &SignedCapacitySnapshot,
    keys: &BTreeMap<String, String>,
) -> Result<VerifiedCapacitySnapshot, CapacityError> {
    let s = &doc.spec;
    if s.observed_at_unix > now
        || now.saturating_sub(s.observed_at_unix) > s.ttl_seconds as i64
        || now >= s.reset_at_unix
    {
        return Err(CapacityError::InvalidEvidence);
    }
    let body = serde_json::to_vec(s).map_err(|_| CapacityError::InvalidEvidence)?;
    if hash(&body) != doc.document_sha256 || !valid_sha(&doc.document_sha256) {
        return Err(CapacityError::InvalidEvidence);
    }
    let kb = hex::decode(
        keys.get(&doc.signer_id)
            .ok_or(CapacityError::UntrustedSigner)?,
    )
    .map_err(|_| CapacityError::InvalidSignature)?;
    let ka: [u8; 32] = kb.try_into().map_err(|_| CapacityError::InvalidSignature)?;
    let vk = VerifyingKey::from_bytes(&ka).map_err(|_| CapacityError::InvalidSignature)?;
    let sb = hex::decode(&doc.signature_hex).map_err(|_| CapacityError::InvalidSignature)?;
    let sa: [u8; 64] = sb.try_into().map_err(|_| CapacityError::InvalidSignature)?;
    vk.verify(&body, &Signature::from_bytes(&sa))
        .map_err(|_| CapacityError::InvalidSignature)?;
    Ok(VerifiedCapacitySnapshot {
        document_sha256: doc.document_sha256.clone(),
        signer_id: doc.signer_id.clone(),
        spec: s.clone(),
    })
}
pub fn admit(
    snapshot: &VerifiedCapacitySnapshot,
    estimated_tokens: u64,
) -> Result<(), CapacityError> {
    let s = &snapshot.spec;
    if s.requests_available < 1
        || s.tokens_available < estimated_tokens
        || s.concurrency_available < 1
    {
        Err(CapacityError::InsufficientCapacity)
    } else {
        Ok(())
    }
}

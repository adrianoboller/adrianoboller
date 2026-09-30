//! Reconstructed PhxClaw v0.34 AI portfolio contracts.
//! Source reconstruction is based on the surviving v0.35 prerequisite/API contract and session evidence.
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct PortfolioCandidate {
    pub provider_uuid: Uuid,
    pub model_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PortfolioMember {
    pub candidate: PortfolioCandidate,
    pub weight_basis_points: u16,
    pub promoted_profile_sha256: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DiversityClass {
    Normal,
    High,
    Extreme,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PortfolioSlot {
    pub slot_uuid: Uuid,
    pub task_family: String,
    pub complexity: String,
    pub diversity: DiversityClass,
    pub members: Vec<PortfolioMember>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PortfolioSpec {
    pub tenant_uuid: Uuid,
    pub portfolio_uuid: Uuid,
    pub policy_sha256: String,
    pub source_arena_sha256: String,
    pub starts_at_unix: i64,
    pub expires_at_unix: i64,
    pub max_provider_concentration_basis_points: u16,
    pub min_high_diversity_providers: u16,
    pub min_extreme_diversity_providers: u16,
    pub slots: Vec<PortfolioSlot>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedPortfolioDocument {
    pub document_sha256: String,
    pub signer_id: String,
    pub signature_hex: String,
    pub spec: PortfolioSpec,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifiedPortfolioDocument {
    pub document_sha256: String,
    pub signer_id: String,
    pub spec: PortfolioSpec,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum PortfolioError {
    #[error("invalid portfolio")]
    InvalidPortfolio,
    #[error("invalid hash")]
    InvalidHash,
    #[error("untrusted signer")]
    UntrustedSigner,
    #[error("invalid signature")]
    InvalidSignature,
    #[error("provider concentration exceeds signed policy")]
    ProviderConcentration,
    #[error("diversity requirement not met")]
    DiversityRequirement,
    #[error("no eligible portfolio member")]
    NoEligibleMember,
}
fn valid_sha(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}
fn canonical(spec: &PortfolioSpec) -> Vec<u8> {
    serde_json::to_vec(spec).expect("portfolio serialization")
}

fn validate_slot(spec: &PortfolioSpec, slot: &PortfolioSlot) -> Result<(), PortfolioError> {
    if slot.members.is_empty() {
        return Err(PortfolioError::InvalidPortfolio);
    }
    let total: u32 = slot
        .members
        .iter()
        .map(|m| m.weight_basis_points as u32)
        .sum();
    if total != 10_000 {
        return Err(PortfolioError::InvalidPortfolio);
    }
    if slot
        .members
        .iter()
        .any(|m| m.candidate.model_id.trim().is_empty() || !valid_sha(&m.promoted_profile_sha256))
    {
        return Err(PortfolioError::InvalidPortfolio);
    }
    let mut per_provider: BTreeMap<Uuid, u32> = BTreeMap::new();
    for m in &slot.members {
        *per_provider.entry(m.candidate.provider_uuid).or_default() += m.weight_basis_points as u32;
    }
    if per_provider
        .values()
        .any(|v| *v > spec.max_provider_concentration_basis_points as u32)
    {
        return Err(PortfolioError::ProviderConcentration);
    }
    let distinct = per_provider.len() as u16;
    match slot.diversity {
        DiversityClass::High if distinct < spec.min_high_diversity_providers => {
            Err(PortfolioError::DiversityRequirement)
        }
        DiversityClass::Extreme if distinct < spec.min_extreme_diversity_providers => {
            Err(PortfolioError::DiversityRequirement)
        }
        _ => Ok(()),
    }
}

pub fn verify_signed_portfolio(
    now_unix: i64,
    doc: &SignedPortfolioDocument,
    trusted_keys: &BTreeMap<String, String>,
) -> Result<VerifiedPortfolioDocument, PortfolioError> {
    let s = &doc.spec;
    if !valid_sha(&s.policy_sha256)
        || !valid_sha(&s.source_arena_sha256)
        || s.starts_at_unix >= s.expires_at_unix
        || now_unix < s.starts_at_unix
        || now_unix > s.expires_at_unix
        || s.slots.is_empty()
        || s.max_provider_concentration_basis_points == 0
        || s.max_provider_concentration_basis_points > 10_000
    {
        return Err(PortfolioError::InvalidPortfolio);
    }
    let mut ids = BTreeSet::new();
    for slot in &s.slots {
        if !ids.insert(slot.slot_uuid) {
            return Err(PortfolioError::InvalidPortfolio);
        }
        validate_slot(s, slot)?;
    }
    let body = canonical(s);
    if sha(&body) != doc.document_sha256 || !valid_sha(&doc.document_sha256) {
        return Err(PortfolioError::InvalidHash);
    }
    let key_hex = trusted_keys
        .get(&doc.signer_id)
        .ok_or(PortfolioError::UntrustedSigner)?;
    let kb = hex::decode(key_hex).map_err(|_| PortfolioError::InvalidSignature)?;
    let ka: [u8; 32] = kb
        .try_into()
        .map_err(|_| PortfolioError::InvalidSignature)?;
    let vk = VerifyingKey::from_bytes(&ka).map_err(|_| PortfolioError::InvalidSignature)?;
    let sb = hex::decode(&doc.signature_hex).map_err(|_| PortfolioError::InvalidSignature)?;
    let sa: [u8; 64] = sb
        .try_into()
        .map_err(|_| PortfolioError::InvalidSignature)?;
    vk.verify(&body, &Signature::from_bytes(&sa))
        .map_err(|_| PortfolioError::InvalidSignature)?;
    Ok(VerifiedPortfolioDocument {
        document_sha256: doc.document_sha256.clone(),
        signer_id: doc.signer_id.clone(),
        spec: s.clone(),
    })
}

fn bucket(slot_uuid: Uuid, request_uuid: Uuid) -> u16 {
    let mut h = Sha256::new();
    h.update(slot_uuid.as_bytes());
    h.update(request_uuid.as_bytes());
    let d = h.finalize();
    u16::from_be_bytes([d[0], d[1]]) % 10_000
}
pub fn select_member(
    portfolio: &VerifiedPortfolioDocument,
    slot_uuid: Uuid,
    request_uuid: Uuid,
    excluded: &BTreeSet<Uuid>,
) -> Result<PortfolioCandidate, PortfolioError> {
    let slot = portfolio
        .spec
        .slots
        .iter()
        .find(|s| s.slot_uuid == slot_uuid)
        .ok_or(PortfolioError::NoEligibleMember)?;
    let live: Vec<&PortfolioMember> = slot
        .members
        .iter()
        .filter(|m| !excluded.contains(&m.candidate.provider_uuid))
        .collect();
    if live.is_empty() {
        return Err(PortfolioError::NoEligibleMember);
    }
    let total: u32 = live.iter().map(|m| m.weight_basis_points as u32).sum();
    let target = (bucket(slot_uuid, request_uuid) as u32 * total) / 10_000;
    let mut acc = 0u32;
    for m in &live {
        acc += m.weight_basis_points as u32;
        if target < acc {
            return Ok(m.candidate.clone());
        }
    }
    Ok(live.last().unwrap().candidate.clone())
}

pub fn renormalized_weights(
    slot: &PortfolioSlot,
    failed_provider: Uuid,
) -> Result<Vec<(PortfolioCandidate, u16)>, PortfolioError> {
    let live: Vec<&PortfolioMember> = slot
        .members
        .iter()
        .filter(|m| m.candidate.provider_uuid != failed_provider)
        .collect();
    if live.is_empty() {
        return Err(PortfolioError::NoEligibleMember);
    }
    let sum: u32 = live.iter().map(|m| m.weight_basis_points as u32).sum();
    let mut out = Vec::new();
    let mut assigned = 0u32;
    for (i, m) in live.iter().enumerate() {
        let w = if i + 1 == live.len() {
            10_000 - assigned
        } else {
            ((m.weight_basis_points as u32 * 10_000) / sum).min(10_000 - assigned)
        };
        assigned += w;
        out.push((m.candidate.clone(), w as u16));
    }
    Ok(out)
}

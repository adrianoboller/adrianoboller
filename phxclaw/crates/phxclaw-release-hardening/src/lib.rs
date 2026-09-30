//! PhxClaw v0.24 release hardening.
//! A release gate is evidence-driven: unavailable or stale proof is never treated as success.

use chrono::{DateTime, Duration, Utc};
use phxclaw_knowledge_evidence_graph::{
    EdgeKind, EpistemicState as GraphState, EvidenceBinding, GraphSnapshot, KnowledgeEdge,
    KnowledgeGraph, KnowledgeNode, MutationAuthority, NodeKind, PromotionPolicy as GraphPromotionPolicy,
};
use phxclaw_skill_evolution::{SkillCandidate, SkillRelease};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ReleaseError {
    #[error("invalid sha256 digest")]
    InvalidDigest,
    #[error("duplicate proof for release gate")]
    DuplicateGateProof,
    #[error("gate proof is bound to a different workspace/source state")]
    SourceStateMismatch,
    #[error("verified or failed gate requires evidence digest")]
    MissingEvidenceDigest,
    #[error("gate evidence is stale")]
    StaleEvidence,
    #[error("skill candidate/release lineage mismatch")]
    LineageMismatch,
    #[error("lineage evidence is bound to a different source state")]
    LineageEvidenceMismatch,
    #[error("knowledge graph rejected lineage: {0}")]
    KnowledgeGraph(String),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseGate {
    WorkspaceStatic,
    JsonSchemaStatic,
    MigrationStatic,
    LicenseSbom,
    SupplyChain,
    CargoFmt,
    CargoCheck,
    CargoTest,
    CargoClippy,
    PostgresMigration,
    RlsCrossTenant,
    SkillKnowledgeLineage,
    MissionE2e,
    TauriNativeE2e,
    ProviderModelE2e,
}

pub const REQUIRED_GATES: [ReleaseGate; 15] = [
    ReleaseGate::WorkspaceStatic,
    ReleaseGate::JsonSchemaStatic,
    ReleaseGate::MigrationStatic,
    ReleaseGate::LicenseSbom,
    ReleaseGate::SupplyChain,
    ReleaseGate::CargoFmt,
    ReleaseGate::CargoCheck,
    ReleaseGate::CargoTest,
    ReleaseGate::CargoClippy,
    ReleaseGate::PostgresMigration,
    ReleaseGate::RlsCrossTenant,
    ReleaseGate::SkillKnowledgeLineage,
    ReleaseGate::MissionE2e,
    ReleaseGate::TauriNativeE2e,
    ReleaseGate::ProviderModelE2e,
];

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Verified,
    Failed,
    Unavailable,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GateProof {
    pub gate: ReleaseGate,
    pub status: GateStatus,
    pub source_state_sha256_hex: String,
    pub evidence_sha256_hex: Option<String>,
    pub tool_name: String,
    pub tool_version: Option<String>,
    pub evidence_ref: Option<String>,
    pub verified_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct ReleasePolicy {
    pub max_proof_age: Duration,
}

impl Default for ReleasePolicy {
    fn default() -> Self {
        Self { max_proof_age: Duration::days(7) }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseAssessment {
    pub workspace_sha256_hex: String,
    pub source_ready: bool,
    pub static_verified: bool,
    pub runtime_verified: bool,
    pub e2e_verified: bool,
    pub release_ready: bool,
    pub verified_gates: Vec<ReleaseGate>,
    pub blockers: Vec<String>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn valid_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn evaluate_release(
    workspace_sha256_hex: &str,
    proofs: &[GateProof],
    policy: &ReleasePolicy,
    now: DateTime<Utc>,
) -> Result<ReleaseAssessment, ReleaseError> {
    if !valid_sha256_hex(workspace_sha256_hex) {
        return Err(ReleaseError::InvalidDigest);
    }
    let mut by_gate = BTreeMap::new();
    for proof in proofs {
        if !valid_sha256_hex(&proof.source_state_sha256_hex) {
            return Err(ReleaseError::InvalidDigest);
        }
        if !proof.source_state_sha256_hex.eq_ignore_ascii_case(workspace_sha256_hex) {
            return Err(ReleaseError::SourceStateMismatch);
        }
        match proof.status {
            GateStatus::Verified | GateStatus::Failed => {
                let digest = proof.evidence_sha256_hex.as_deref().ok_or(ReleaseError::MissingEvidenceDigest)?;
                if !valid_sha256_hex(digest) {
                    return Err(ReleaseError::InvalidDigest);
                }
            }
            GateStatus::Unavailable => {
                if let Some(digest) = proof.evidence_sha256_hex.as_deref() {
                    if !valid_sha256_hex(digest) {
                        return Err(ReleaseError::InvalidDigest);
                    }
                }
            }
        }
        if now.signed_duration_since(proof.verified_at) > policy.max_proof_age || now < proof.verified_at {
            return Err(ReleaseError::StaleEvidence);
        }
        if by_gate.insert(proof.gate, proof).is_some() {
            return Err(ReleaseError::DuplicateGateProof);
        }
    }

    let static_gates = [
        ReleaseGate::WorkspaceStatic,
        ReleaseGate::JsonSchemaStatic,
        ReleaseGate::MigrationStatic,
        ReleaseGate::LicenseSbom,
        ReleaseGate::SupplyChain,
    ];
    let runtime_gates = [
        ReleaseGate::CargoFmt,
        ReleaseGate::CargoCheck,
        ReleaseGate::CargoTest,
        ReleaseGate::CargoClippy,
        ReleaseGate::PostgresMigration,
        ReleaseGate::RlsCrossTenant,
        ReleaseGate::SkillKnowledgeLineage,
    ];
    let e2e_gates = [
        ReleaseGate::MissionE2e,
        ReleaseGate::TauriNativeE2e,
        ReleaseGate::ProviderModelE2e,
    ];

    let verified = |gate: &ReleaseGate| {
        by_gate.get(gate).is_some_and(|proof| proof.status == GateStatus::Verified)
    };
    let source_ready = [
        ReleaseGate::WorkspaceStatic,
        ReleaseGate::JsonSchemaStatic,
        ReleaseGate::MigrationStatic,
    ].iter().all(verified);
    let static_verified = source_ready && static_gates.iter().all(verified);
    let runtime_verified = static_verified && runtime_gates.iter().all(verified);
    let e2e_verified = runtime_verified && e2e_gates.iter().all(verified);
    let release_ready = e2e_verified && REQUIRED_GATES.iter().all(verified);

    let mut blockers = Vec::new();
    let mut verified_gates = Vec::new();
    for gate in REQUIRED_GATES {
        match by_gate.get(&gate).map(|p| p.status) {
            Some(GateStatus::Verified) => verified_gates.push(gate),
            Some(GateStatus::Failed) => blockers.push(format!("{:?}: failed", gate)),
            Some(GateStatus::Unavailable) => blockers.push(format!("{:?}: unavailable", gate)),
            None => blockers.push(format!("{:?}: missing proof", gate)),
        }
    }
    Ok(ReleaseAssessment {
        workspace_sha256_hex: workspace_sha256_hex.to_ascii_lowercase(),
        source_ready,
        static_verified,
        runtime_verified,
        e2e_verified,
        release_ready,
        verified_gates,
        blockers,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LineageEvidence {
    pub evidence_uuid: Uuid,
    pub evidence_sha256_hex: String,
    pub source_state_sha256_hex: String,
    pub mechanism: String,
    pub collected_at: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillLineageReceipt {
    pub tenant_uuid: Uuid,
    pub skill_node_uuid: Uuid,
    pub artifact_node_uuid: Uuid,
    pub release_node_uuid: Uuid,
    pub claim_node_uuid: Uuid,
    pub promoted_claim_node_uuid: Uuid,
    pub snapshot: GraphSnapshot,
}

pub fn record_skill_release_lineage(
    graph: &mut KnowledgeGraph,
    candidate: &SkillCandidate,
    release: &SkillRelease,
    evidence: &[LineageEvidence],
    human_approved: bool,
    now: DateTime<Utc>,
) -> Result<SkillLineageReceipt, ReleaseError> {
    if release.candidate_uuid != candidate.candidate_uuid
        || release.skill_uuid != candidate.skill_uuid
        || release.previous_release_uuid != candidate.base_release_uuid
        || !release.artifact_sha256_hex.eq_ignore_ascii_case(&candidate.artifact_sha256_hex)
        || !release.manifest_sha256_hex.eq_ignore_ascii_case(&candidate.manifest_sha256_hex)
    {
        return Err(ReleaseError::LineageMismatch);
    }
    for digest in [
        &candidate.artifact_sha256_hex,
        &candidate.manifest_sha256_hex,
        &candidate.source_state_sha256_hex,
    ] {
        if !valid_sha256_hex(digest) {
            return Err(ReleaseError::InvalidDigest);
        }
    }
    if evidence.len() < 2 {
        return Err(ReleaseError::LineageEvidenceMismatch);
    }
    let mechanisms: BTreeSet<&str> = evidence.iter().map(|e| e.mechanism.as_str()).collect();
    if mechanisms.len() < 2 {
        return Err(ReleaseError::LineageEvidenceMismatch);
    }
    for item in evidence {
        if !valid_sha256_hex(&item.evidence_sha256_hex)
            || !valid_sha256_hex(&item.source_state_sha256_hex)
            || !item.source_state_sha256_hex.eq_ignore_ascii_case(&candidate.source_state_sha256_hex)
            || now < item.collected_at
            || item.valid_until.as_ref().is_some_and(|until| &now >= until)
        {
            return Err(ReleaseError::LineageEvidenceMismatch);
        }
    }

    let tenant_uuid = candidate.tenant_uuid;
    let artifact_node = KnowledgeNode {
        node_uuid: Uuid::now_v7(), tenant_uuid, kind: NodeKind::Artifact, state: GraphState::Accepted,
        content_sha256_hex: candidate.artifact_sha256_hex.clone(), source_state_sha256_hex: candidate.source_state_sha256_hex.clone(),
        subject_key: Some(format!("skill:{}", candidate.skill_uuid)), predicate_key: Some("candidate_artifact".into()),
        value_sha256_hex: Some(candidate.artifact_sha256_hex.clone()), confidence_ppm: 1_000_000, created_at: now,
    };
    let skill_node = KnowledgeNode {
        node_uuid: Uuid::now_v7(), tenant_uuid, kind: NodeKind::Skill, state: GraphState::Accepted,
        content_sha256_hex: candidate.manifest_sha256_hex.clone(), source_state_sha256_hex: candidate.source_state_sha256_hex.clone(),
        subject_key: Some(format!("skill:{}", candidate.skill_uuid)), predicate_key: Some("manifest".into()),
        value_sha256_hex: Some(candidate.manifest_sha256_hex.clone()), confidence_ppm: 1_000_000, created_at: now,
    };
    let release_content = sha256_hex(format!(
        "{}|{}|{}|{}|{}", release.release_uuid, release.candidate_uuid, release.skill_uuid,
        release.artifact_sha256_hex.to_ascii_lowercase(), release.manifest_sha256_hex.to_ascii_lowercase()
    ).as_bytes());
    let release_node = KnowledgeNode {
        node_uuid: Uuid::now_v7(), tenant_uuid, kind: NodeKind::Release, state: GraphState::Accepted,
        content_sha256_hex: release_content, source_state_sha256_hex: candidate.source_state_sha256_hex.clone(),
        subject_key: Some(format!("release:{}", release.release_uuid)), predicate_key: Some("skill_release".into()),
        value_sha256_hex: Some(candidate.artifact_sha256_hex.clone()), confidence_ppm: 1_000_000, created_at: now,
    };
    let claim_content = sha256_hex(format!(
        "promoted|{}|{}|{}", release.release_uuid, candidate.skill_uuid, candidate.artifact_sha256_hex.to_ascii_lowercase()
    ).as_bytes());
    let claim_node = KnowledgeNode {
        node_uuid: Uuid::now_v7(), tenant_uuid, kind: NodeKind::Claim, state: GraphState::Unverified,
        content_sha256_hex: claim_content, source_state_sha256_hex: candidate.source_state_sha256_hex.clone(),
        subject_key: Some(format!("skill:{}", candidate.skill_uuid)), predicate_key: Some("release_promoted".into()),
        value_sha256_hex: Some(candidate.artifact_sha256_hex.clone()), confidence_ppm: 1_000_000, created_at: now,
    };

    graph.add_node(MutationAuthority::System, artifact_node.clone()).map_err(|e| ReleaseError::KnowledgeGraph(e.to_string()))?;
    graph.add_node(MutationAuthority::System, skill_node.clone()).map_err(|e| ReleaseError::KnowledgeGraph(e.to_string()))?;
    graph.add_node(MutationAuthority::System, release_node.clone()).map_err(|e| ReleaseError::KnowledgeGraph(e.to_string()))?;
    graph.add_node(MutationAuthority::System, claim_node.clone()).map_err(|e| ReleaseError::KnowledgeGraph(e.to_string()))?;

    for (from, to, kind) in [
        (release_node.node_uuid, skill_node.node_uuid, EdgeKind::Implements),
        (release_node.node_uuid, artifact_node.node_uuid, EdgeKind::PromotedFrom),
        (claim_node.node_uuid, release_node.node_uuid, EdgeKind::RelatesTo),
    ] {
        graph.add_edge(KnowledgeEdge {
            edge_uuid: Uuid::now_v7(), tenant_uuid, from_node_uuid: from, to_node_uuid: to, kind,
            evidence_sha256_hex: None, created_at: now,
        }).map_err(|e| ReleaseError::KnowledgeGraph(e.to_string()))?;
    }

    for item in evidence {
        let evidence_node = KnowledgeNode {
            node_uuid: item.evidence_uuid, tenant_uuid, kind: NodeKind::Evidence, state: GraphState::Accepted,
            content_sha256_hex: item.evidence_sha256_hex.clone(), source_state_sha256_hex: item.source_state_sha256_hex.clone(),
            subject_key: Some(format!("candidate:{}", candidate.candidate_uuid)), predicate_key: Some("promotion_evidence".into()),
            value_sha256_hex: Some(item.evidence_sha256_hex.clone()), confidence_ppm: 1_000_000, created_at: item.collected_at,
        };
        graph.add_node(MutationAuthority::System, evidence_node).map_err(|e| ReleaseError::KnowledgeGraph(e.to_string()))?;
        graph.bind_evidence(EvidenceBinding {
            binding_uuid: Uuid::now_v7(), tenant_uuid, claim_node_uuid: claim_node.node_uuid,
            evidence_node_uuid: item.evidence_uuid, relation: EdgeKind::Supports,
            evidence_sha256_hex: item.evidence_sha256_hex.clone(), source_state_sha256_hex: item.source_state_sha256_hex.clone(),
            mechanism: item.mechanism.clone(), collected_at: item.collected_at, valid_until: item.valid_until,
        }, now).map_err(|e| ReleaseError::KnowledgeGraph(e.to_string()))?;
    }

    let target_state = if human_approved { GraphState::Governed } else { GraphState::Accepted };
    let decision = graph.evaluate_claim_promotion(
        claim_node.node_uuid,
        target_state,
        &GraphPromotionPolicy { min_supporting_evidence: 2, min_independent_mechanisms: 2, allow_governed_without_human: false },
        now,
    ).map_err(|e| ReleaseError::KnowledgeGraph(e.to_string()))?;
    let (promoted_claim, supersedes) = graph.promoted_claim_version(claim_node.node_uuid, &decision, human_approved, now)
        .map_err(|e| ReleaseError::KnowledgeGraph(e.to_string()))?;
    graph.add_node(MutationAuthority::System, promoted_claim.clone()).map_err(|e| ReleaseError::KnowledgeGraph(e.to_string()))?;
    graph.add_edge(supersedes).map_err(|e| ReleaseError::KnowledgeGraph(e.to_string()))?;

    let snapshot = graph.snapshot(tenant_uuid, now);
    Ok(SkillLineageReceipt {
        tenant_uuid,
        skill_node_uuid: skill_node.node_uuid,
        artifact_node_uuid: artifact_node.node_uuid,
        release_node_uuid: release_node.node_uuid,
        claim_node_uuid: claim_node.node_uuid,
        promoted_claim_node_uuid: promoted_claim.node_uuid,
        snapshot,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use phxclaw_skill_evolution::{ChangeBoundary, RiskLevel};

    fn digest(ch: char) -> String { ch.to_string().repeat(64) }

    #[test]
    fn unavailable_is_never_release_ready() {
        let now = Utc::now();
        let workspace = digest('a');
        let proofs: Vec<GateProof> = REQUIRED_GATES.iter().map(|gate| GateProof {
            gate: *gate,
            status: if *gate == ReleaseGate::CargoCheck { GateStatus::Unavailable } else { GateStatus::Verified },
            source_state_sha256_hex: workspace.clone(),
            evidence_sha256_hex: if *gate == ReleaseGate::CargoCheck { None } else { Some(digest('b')) },
            tool_name: "test".into(), tool_version: None, evidence_ref: None, verified_at: now,
        }).collect();
        let result = evaluate_release(&workspace, &proofs, &ReleasePolicy::default(), now).unwrap();
        assert!(!result.release_ready);
        assert!(result.blockers.iter().any(|b| b.contains("CargoCheck") && b.contains("unavailable")));
    }

    #[test]
    fn source_hash_mismatch_is_rejected() {
        let now = Utc::now();
        let proof = GateProof { gate: ReleaseGate::WorkspaceStatic, status: GateStatus::Verified,
            source_state_sha256_hex: digest('b'), evidence_sha256_hex: Some(digest('c')),
            tool_name: "test".into(), tool_version: None, evidence_ref: None, verified_at: now };
        assert_eq!(evaluate_release(&digest('a'), &[proof], &ReleasePolicy::default(), now), Err(ReleaseError::SourceStateMismatch));
    }

    #[test]
    fn skill_release_lineage_rejects_mismatched_release() {
        let now = Utc::now();
        let tenant = Uuid::now_v7();
        let candidate = SkillCandidate {
            candidate_uuid: Uuid::now_v7(), tenant_uuid: tenant, skill_uuid: Uuid::now_v7(), base_release_uuid: None,
            target_namespace: "skills.demo".into(), boundary: ChangeBoundary::SkillPlugin,
            artifact_sha256_hex: digest('a'), manifest_sha256_hex: digest('b'), source_state_sha256_hex: digest('c'),
            risk: RiskLevel::Low, behavior_change: false, reversible: true, created_at: now,
        };
        let release = SkillRelease { release_uuid: Uuid::now_v7(), candidate_uuid: Uuid::now_v7(), skill_uuid: candidate.skill_uuid,
            previous_release_uuid: None, artifact_sha256_hex: candidate.artifact_sha256_hex.clone(),
            manifest_sha256_hex: candidate.manifest_sha256_hex.clone(), promoted_at: now };
        let mut graph = KnowledgeGraph::default();
        assert_eq!(record_skill_release_lineage(&mut graph, &candidate, &release, &[], false, now), Err(ReleaseError::LineageMismatch));
    }
}

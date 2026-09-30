//! F25 Knowledge / Evidence Graph.
//! The graph records provenance and engineering traceability; it does not turn memory,
//! repeated statements, model confidence, or graph centrality into truth or policy.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum GraphError {
    #[error("invalid sha256 digest")]
    InvalidDigest,
    #[error("cross-tenant graph mutation is forbidden")]
    CrossTenant,
    #[error("self edge is forbidden")]
    SelfEdge,
    #[error("learning authority cannot create governed or policy-bearing nodes")]
    ForbiddenAuthority,
    #[error("learning output must remain unverified")]
    LearningMustRemainUnverified,
    #[error("claim and evidence source-state binding mismatch")]
    SourceStateMismatch,
    #[error("evidence is stale or outside its validity interval")]
    StaleEvidence,
    #[error("evidence set is insufficient")]
    InsufficientEvidence,
    #[error("active refuting evidence blocks promotion")]
    RefutingEvidence,
    #[error("unresolved contradiction blocks promotion")]
    UnresolvedContradiction,
    #[error("graph traversal bounds are invalid")]
    InvalidTraversalBounds,
    #[error("graph node not found")]
    NodeNotFound,
    #[error("durable identity collision with different content")]
    IdentityCollision,
    #[error("claim promotion target must be accepted or governed")]
    InvalidPromotionTarget,
    #[error("human approval is required for governed knowledge")]
    ApprovalRequired,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    RawSource,
    Artifact,
    Claim,
    Evidence,
    Hypothesis,
    Decision,
    Requirement,
    Task,
    Agent,
    Skill,
    Release,
    Test,
    Policy,
    Constraint,
    ExternalFact,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Supports,
    Refutes,
    DerivedFrom,
    ProducedBy,
    Validates,
    Tests,
    DependsOn,
    Implements,
    Supersedes,
    Contradicts,
    ApprovedBy,
    PromotedFrom,
    CausedBy,
    RelatesTo,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum EpistemicState {
    RawObservation,
    Unverified,
    Accepted,
    Governed,
    Rejected,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MutationAuthority {
    Learning,
    Agent,
    System,
    Human,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnowledgeNode {
    pub node_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub kind: NodeKind,
    pub state: EpistemicState,
    pub content_sha256_hex: String,
    pub source_state_sha256_hex: String,
    pub subject_key: Option<String>,
    pub predicate_key: Option<String>,
    pub value_sha256_hex: Option<String>,
    pub confidence_ppm: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnowledgeEdge {
    pub edge_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub from_node_uuid: Uuid,
    pub to_node_uuid: Uuid,
    pub kind: EdgeKind,
    pub evidence_sha256_hex: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceBinding {
    pub binding_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub claim_node_uuid: Uuid,
    pub evidence_node_uuid: Uuid,
    pub relation: EdgeKind,
    pub evidence_sha256_hex: String,
    pub source_state_sha256_hex: String,
    pub mechanism: String,
    pub collected_at: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Contradiction {
    pub contradiction_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub left_claim_uuid: Uuid,
    pub right_claim_uuid: Uuid,
    pub detected_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContradictionResolution {
    pub resolution_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub contradiction_uuid: Uuid,
    pub resolution_node_uuid: Uuid,
    pub resolved_by_uuid: Uuid,
    pub resolution_sha256_hex: String,
    pub resolved_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphSnapshot {
    pub snapshot_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub node_count: usize,
    pub edge_count: usize,
    pub root_sha256_hex: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct PromotionPolicy {
    pub min_supporting_evidence: usize,
    pub min_independent_mechanisms: usize,
    pub allow_governed_without_human: bool,
}

impl Default for PromotionPolicy {
    fn default() -> Self {
        Self {
            min_supporting_evidence: 2,
            min_independent_mechanisms: 2,
            allow_governed_without_human: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromotionDecision {
    pub claim_node_uuid: Uuid,
    pub target_state: EpistemicState,
    pub eligible: bool,
    pub requires_human_approval: bool,
    pub supporting_evidence: usize,
    pub independent_mechanisms: usize,
}

#[derive(Clone, Debug, Default)]
pub struct KnowledgeGraph {
    nodes: BTreeMap<Uuid, KnowledgeNode>,
    edges: BTreeMap<Uuid, KnowledgeEdge>,
    bindings: BTreeMap<Uuid, EvidenceBinding>,
    contradictions: BTreeMap<Uuid, Contradiction>,
    resolutions: BTreeMap<Uuid, ContradictionResolution>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn valid_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn validate_node_digests(node: &KnowledgeNode) -> Result<(), GraphError> {
    if !valid_sha256_hex(&node.content_sha256_hex)
        || !valid_sha256_hex(&node.source_state_sha256_hex)
        || node
            .value_sha256_hex
            .as_deref()
            .is_some_and(|value| !valid_sha256_hex(value))
    {
        return Err(GraphError::InvalidDigest);
    }
    Ok(())
}

pub fn authorize_new_node(
    authority: MutationAuthority,
    node: &KnowledgeNode,
) -> Result<(), GraphError> {
    validate_node_digests(node)?;
    if authority == MutationAuthority::Learning {
        if matches!(node.kind, NodeKind::Policy | NodeKind::Constraint | NodeKind::Decision) {
            return Err(GraphError::ForbiddenAuthority);
        }
        if !matches!(node.state, EpistemicState::RawObservation | EpistemicState::Unverified) {
            return Err(GraphError::LearningMustRemainUnverified);
        }
    }
    if matches!(node.state, EpistemicState::Governed)
        && !matches!(authority, MutationAuthority::Human | MutationAuthority::System)
    {
        return Err(GraphError::ForbiddenAuthority);
    }
    Ok(())
}

impl KnowledgeGraph {
    pub fn add_node(
        &mut self,
        authority: MutationAuthority,
        node: KnowledgeNode,
    ) -> Result<(), GraphError> {
        authorize_new_node(authority, &node)?;
        if let Some(existing) = self.nodes.get(&node.node_uuid) {
            if existing != &node {
                return Err(GraphError::IdentityCollision);
            }
            return Ok(());
        }
        self.nodes.insert(node.node_uuid, node);
        Ok(())
    }

    pub fn add_edge(&mut self, edge: KnowledgeEdge) -> Result<(), GraphError> {
        if edge.from_node_uuid == edge.to_node_uuid {
            return Err(GraphError::SelfEdge);
        }
        if let Some(digest) = &edge.evidence_sha256_hex {
            if !valid_sha256_hex(digest) {
                return Err(GraphError::InvalidDigest);
            }
        }
        let from = self.nodes.get(&edge.from_node_uuid).ok_or(GraphError::NodeNotFound)?;
        let to = self.nodes.get(&edge.to_node_uuid).ok_or(GraphError::NodeNotFound)?;
        if from.tenant_uuid != edge.tenant_uuid || to.tenant_uuid != edge.tenant_uuid {
            return Err(GraphError::CrossTenant);
        }
        if let Some(existing) = self.edges.get(&edge.edge_uuid) {
            if existing != &edge {
                return Err(GraphError::IdentityCollision);
            }
            return Ok(());
        }
        self.edges.insert(edge.edge_uuid, edge);
        Ok(())
    }

    pub fn bind_evidence(&mut self, binding: EvidenceBinding, now: DateTime<Utc>) -> Result<(), GraphError> {
        if !valid_sha256_hex(&binding.evidence_sha256_hex)
            || !valid_sha256_hex(&binding.source_state_sha256_hex)
        {
            return Err(GraphError::InvalidDigest);
        }
        let claim = self.nodes.get(&binding.claim_node_uuid).ok_or(GraphError::NodeNotFound)?;
        let evidence = self.nodes.get(&binding.evidence_node_uuid).ok_or(GraphError::NodeNotFound)?;
        if claim.tenant_uuid != binding.tenant_uuid || evidence.tenant_uuid != binding.tenant_uuid {
            return Err(GraphError::CrossTenant);
        }
        if claim.kind != NodeKind::Claim || evidence.kind != NodeKind::Evidence {
            return Err(GraphError::InsufficientEvidence);
        }
        if !claim
            .source_state_sha256_hex
            .eq_ignore_ascii_case(&binding.source_state_sha256_hex)
        {
            return Err(GraphError::SourceStateMismatch);
        }
        if !evidence
            .content_sha256_hex
            .eq_ignore_ascii_case(&binding.evidence_sha256_hex)
        {
            return Err(GraphError::SourceStateMismatch);
        }
        if binding.valid_until.as_ref().is_some_and(|until| &now >= until) || now < binding.collected_at {
            return Err(GraphError::StaleEvidence);
        }
        if let Some(existing) = self.bindings.get(&binding.binding_uuid) {
            if existing != &binding {
                return Err(GraphError::IdentityCollision);
            }
            return Ok(());
        }
        self.bindings.insert(binding.binding_uuid, binding);
        Ok(())
    }

    pub fn add_contradiction(&mut self, contradiction: Contradiction) -> Result<(), GraphError> {
        let left = self.nodes.get(&contradiction.left_claim_uuid).ok_or(GraphError::NodeNotFound)?;
        let right = self.nodes.get(&contradiction.right_claim_uuid).ok_or(GraphError::NodeNotFound)?;
        if left.tenant_uuid != contradiction.tenant_uuid || right.tenant_uuid != contradiction.tenant_uuid {
            return Err(GraphError::CrossTenant);
        }
        if let Some(existing) = self.contradictions.get(&contradiction.contradiction_uuid) {
            if existing != &contradiction {
                return Err(GraphError::IdentityCollision);
            }
            return Ok(());
        }
        self.contradictions
            .insert(contradiction.contradiction_uuid, contradiction);
        Ok(())
    }

    pub fn add_contradiction_resolution(
        &mut self,
        resolution: ContradictionResolution,
    ) -> Result<(), GraphError> {
        if !valid_sha256_hex(&resolution.resolution_sha256_hex) {
            return Err(GraphError::InvalidDigest);
        }
        let contradiction = self
            .contradictions
            .get(&resolution.contradiction_uuid)
            .ok_or(GraphError::NodeNotFound)?;
        let resolution_node = self
            .nodes
            .get(&resolution.resolution_node_uuid)
            .ok_or(GraphError::NodeNotFound)?;
        if contradiction.tenant_uuid != resolution.tenant_uuid
            || resolution_node.tenant_uuid != resolution.tenant_uuid
        {
            return Err(GraphError::CrossTenant);
        }
        if self
            .resolutions
            .values()
            .any(|existing| existing.contradiction_uuid == resolution.contradiction_uuid)
        {
            return Err(GraphError::IdentityCollision);
        }
        self.resolutions.insert(resolution.resolution_uuid, resolution);
        Ok(())
    }

    pub fn detect_claim_contradictions(&self, tenant_uuid: Uuid) -> Vec<(Uuid, Uuid)> {
        let claims: Vec<&KnowledgeNode> = self
            .nodes
            .values()
            .filter(|node| {
                node.tenant_uuid == tenant_uuid
                    && node.kind == NodeKind::Claim
                    && matches!(node.state, EpistemicState::Accepted | EpistemicState::Governed)
                    && node.subject_key.is_some()
                    && node.predicate_key.is_some()
                    && node.value_sha256_hex.is_some()
            })
            .collect();
        let mut pairs = Vec::new();
        for (idx, left) in claims.iter().enumerate() {
            for right in claims.iter().skip(idx + 1) {
                if left.subject_key == right.subject_key
                    && left.predicate_key == right.predicate_key
                    && left.value_sha256_hex != right.value_sha256_hex
                {
                    pairs.push((left.node_uuid, right.node_uuid));
                }
            }
        }
        pairs
    }

    pub fn evaluate_claim_promotion(
        &self,
        claim_uuid: Uuid,
        target_state: EpistemicState,
        policy: &PromotionPolicy,
        now: DateTime<Utc>,
    ) -> Result<PromotionDecision, GraphError> {
        let claim = self.nodes.get(&claim_uuid).ok_or(GraphError::NodeNotFound)?;
        if !matches!(target_state, EpistemicState::Accepted | EpistemicState::Governed) {
            return Err(GraphError::InvalidPromotionTarget);
        }
        if claim.kind != NodeKind::Claim {
            return Err(GraphError::InsufficientEvidence);
        }
        if self.contradictions.values().any(|item| {
            item.tenant_uuid == claim.tenant_uuid
                && (item.left_claim_uuid == claim_uuid || item.right_claim_uuid == claim_uuid)
                && !self
                    .resolutions
                    .values()
                    .any(|resolution| resolution.contradiction_uuid == item.contradiction_uuid)
        }) {
            return Err(GraphError::UnresolvedContradiction);
        }

        let active: Vec<&EvidenceBinding> = self
            .bindings
            .values()
            .filter(|binding| {
                binding.tenant_uuid == claim.tenant_uuid
                    && binding.claim_node_uuid == claim_uuid
                    && !binding.valid_until.as_ref().is_some_and(|until| &now >= until)
                    && now >= binding.collected_at
            })
            .collect();
        if active.iter().any(|binding| binding.relation == EdgeKind::Refutes) {
            return Err(GraphError::RefutingEvidence);
        }
        let supporting: Vec<&EvidenceBinding> = active
            .into_iter()
            .filter(|binding| binding.relation == EdgeKind::Supports)
            .collect();
        let mechanisms: BTreeSet<&str> = supporting.iter().map(|binding| binding.mechanism.as_str()).collect();
        if supporting.len() < policy.min_supporting_evidence
            || mechanisms.len() < policy.min_independent_mechanisms
        {
            return Err(GraphError::InsufficientEvidence);
        }
        let requires_human_approval = target_state == EpistemicState::Governed
            && !policy.allow_governed_without_human;
        Ok(PromotionDecision {
            claim_node_uuid: claim_uuid,
            target_state,
            eligible: true,
            requires_human_approval,
            supporting_evidence: supporting.len(),
            independent_mechanisms: mechanisms.len(),
        })
    }

    pub fn promoted_claim_version(
        &self,
        claim_uuid: Uuid,
        decision: &PromotionDecision,
        human_approved: bool,
        now: DateTime<Utc>,
    ) -> Result<(KnowledgeNode, KnowledgeEdge), GraphError> {
        let claim = self.nodes.get(&claim_uuid).ok_or(GraphError::NodeNotFound)?;
        if !decision.eligible || decision.claim_node_uuid != claim_uuid {
            return Err(GraphError::InsufficientEvidence);
        }
        if decision.requires_human_approval && !human_approved {
            return Err(GraphError::ApprovalRequired);
        }
        let mut promoted = claim.clone();
        promoted.node_uuid = Uuid::now_v7();
        promoted.state = decision.target_state;
        promoted.created_at = now.clone();
        let edge = KnowledgeEdge {
            edge_uuid: Uuid::now_v7(),
            tenant_uuid: claim.tenant_uuid,
            from_node_uuid: promoted.node_uuid,
            to_node_uuid: claim.node_uuid,
            kind: EdgeKind::Supersedes,
            evidence_sha256_hex: None,
            created_at: now,
        };
        Ok((promoted, edge))
    }


    pub fn node(&self, node_uuid: Uuid) -> Option<&KnowledgeNode> {
        self.nodes.get(&node_uuid)
    }

    pub fn unresolved_contradiction_count(&self, claim_uuid: Uuid) -> usize {
        self.contradictions.values().filter(|item| {
            (item.left_claim_uuid == claim_uuid || item.right_claim_uuid == claim_uuid)
                && !self.resolutions.values().any(|resolution| resolution.contradiction_uuid == item.contradiction_uuid)
        }).count()
    }

    pub fn rejected_claim_version(
        &self,
        claim_uuid: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(KnowledgeNode, KnowledgeEdge), GraphError> {
        let claim = self.nodes.get(&claim_uuid).ok_or(GraphError::NodeNotFound)?;
        if claim.kind != NodeKind::Claim {
            return Err(GraphError::InvalidPromotionTarget);
        }
        let mut rejected = claim.clone();
        rejected.node_uuid = Uuid::now_v7();
        rejected.state = EpistemicState::Rejected;
        rejected.created_at = now.clone();
        let edge = KnowledgeEdge {
            edge_uuid: Uuid::now_v7(),
            tenant_uuid: claim.tenant_uuid,
            from_node_uuid: rejected.node_uuid,
            to_node_uuid: claim.node_uuid,
            kind: EdgeKind::Supersedes,
            evidence_sha256_hex: None,
            created_at: now,
        };
        Ok((rejected, edge))
    }

    pub fn traverse(
        &self,
        start: Uuid,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<Vec<Uuid>, GraphError> {
        if max_depth == 0 || max_depth > 32 || max_nodes == 0 || max_nodes > 10_000 {
            return Err(GraphError::InvalidTraversalBounds);
        }
        let start_node = self.nodes.get(&start).ok_or(GraphError::NodeNotFound)?;
        let tenant_uuid = start_node.tenant_uuid;
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((start, 0usize));
        while let Some((node_uuid, depth)) = queue.pop_front() {
            if seen.len() >= max_nodes || !seen.insert(node_uuid) || depth >= max_depth {
                continue;
            }
            for edge in self.edges.values().filter(|edge| {
                edge.tenant_uuid == tenant_uuid && edge.from_node_uuid == node_uuid
            }) {
                if !seen.contains(&edge.to_node_uuid) {
                    queue.push_back((edge.to_node_uuid, depth + 1));
                }
            }
        }
        Ok(seen.into_iter().collect())
    }

    pub fn snapshot(&self, tenant_uuid: Uuid, now: DateTime<Utc>) -> GraphSnapshot {
        let mut records = Vec::new();
        let mut node_count = 0usize;
        let mut edge_count = 0usize;
        for node in self.nodes.values().filter(|node| node.tenant_uuid == tenant_uuid) {
            node_count += 1;
            records.push(format!(
                "N|{}|{:?}|{:?}|{}|{}",
                node.node_uuid,
                node.kind,
                node.state,
                node.content_sha256_hex.to_ascii_lowercase(),
                node.source_state_sha256_hex.to_ascii_lowercase()
            ));
        }
        for edge in self.edges.values().filter(|edge| edge.tenant_uuid == tenant_uuid) {
            edge_count += 1;
            records.push(format!(
                "E|{}|{}|{}|{:?}|{}",
                edge.edge_uuid,
                edge.from_node_uuid,
                edge.to_node_uuid,
                edge.kind,
                edge.evidence_sha256_hex.as_deref().unwrap_or("-")
            ));
        }
        for binding in self.bindings.values().filter(|binding| binding.tenant_uuid == tenant_uuid) {
            records.push(format!(
                "B|{}|{}|{}|{:?}|{}|{}|{}",
                binding.binding_uuid,
                binding.claim_node_uuid,
                binding.evidence_node_uuid,
                binding.relation,
                binding.evidence_sha256_hex.to_ascii_lowercase(),
                binding.source_state_sha256_hex.to_ascii_lowercase(),
                binding.mechanism
            ));
        }
        for contradiction in self.contradictions.values().filter(|item| item.tenant_uuid == tenant_uuid) {
            records.push(format!(
                "C|{}|{}|{}",
                contradiction.contradiction_uuid,
                contradiction.left_claim_uuid,
                contradiction.right_claim_uuid
            ));
        }
        for resolution in self.resolutions.values().filter(|item| item.tenant_uuid == tenant_uuid) {
            records.push(format!(
                "R|{}|{}|{}|{}|{}",
                resolution.resolution_uuid,
                resolution.contradiction_uuid,
                resolution.resolution_node_uuid,
                resolution.resolved_by_uuid,
                resolution.resolution_sha256_hex.to_ascii_lowercase()
            ));
        }
        records.sort();
        GraphSnapshot {
            snapshot_uuid: Uuid::now_v7(),
            tenant_uuid,
            node_count,
            edge_count,
            root_sha256_hex: sha256_hex(records.join("\n").as_bytes()),
            created_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(ch: char) -> String {
        ch.to_string().repeat(64)
    }

    fn node(tenant_uuid: Uuid, kind: NodeKind, state: EpistemicState, ch: char) -> KnowledgeNode {
        KnowledgeNode {
            node_uuid: Uuid::now_v7(),
            tenant_uuid,
            kind,
            state,
            content_sha256_hex: digest(ch),
            source_state_sha256_hex: digest('f'),
            subject_key: None,
            predicate_key: None,
            value_sha256_hex: None,
            confidence_ppm: 800_000,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn learning_cannot_create_policy() {
        let tenant = Uuid::now_v7();
        let policy = node(tenant, NodeKind::Policy, EpistemicState::Unverified, 'a');
        assert_eq!(authorize_new_node(MutationAuthority::Learning, &policy), Err(GraphError::ForbiddenAuthority));
    }

    #[test]
    fn learning_cannot_mark_claim_accepted() {
        let tenant = Uuid::now_v7();
        let claim = node(tenant, NodeKind::Claim, EpistemicState::Accepted, 'a');
        assert_eq!(authorize_new_node(MutationAuthority::Learning, &claim), Err(GraphError::LearningMustRemainUnverified));
    }

    #[test]
    fn evidence_must_bind_same_source_state() {
        let tenant = Uuid::now_v7();
        let mut graph = KnowledgeGraph::default();
        let claim = node(tenant, NodeKind::Claim, EpistemicState::Unverified, 'a');
        let evidence = node(tenant, NodeKind::Evidence, EpistemicState::Accepted, 'b');
        graph.add_node(MutationAuthority::Agent, claim.clone()).unwrap();
        graph.add_node(MutationAuthority::System, evidence.clone()).unwrap();
        let binding = EvidenceBinding {
            binding_uuid: Uuid::now_v7(),
            tenant_uuid: tenant,
            claim_node_uuid: claim.node_uuid,
            evidence_node_uuid: evidence.node_uuid,
            relation: EdgeKind::Supports,
            evidence_sha256_hex: evidence.content_sha256_hex.clone(),
            source_state_sha256_hex: digest('e'),
            mechanism: "test-suite".into(),
            collected_at: Utc::now(),
            valid_until: None,
        };
        assert_eq!(graph.bind_evidence(binding, Utc::now()), Err(GraphError::SourceStateMismatch));
    }

    #[test]
    fn contradiction_is_detected_for_same_subject_predicate() {
        let tenant = Uuid::now_v7();
        let mut graph = KnowledgeGraph::default();
        let mut a = node(tenant, NodeKind::Claim, EpistemicState::Accepted, 'a');
        a.subject_key = Some("runtime.postgresql".into());
        a.predicate_key = Some("major".into());
        a.value_sha256_hex = Some(digest('1'));
        let mut b = node(tenant, NodeKind::Claim, EpistemicState::Accepted, 'b');
        b.subject_key = a.subject_key.clone();
        b.predicate_key = a.predicate_key.clone();
        b.value_sha256_hex = Some(digest('2'));
        graph.add_node(MutationAuthority::System, a).unwrap();
        graph.add_node(MutationAuthority::System, b).unwrap();
        assert_eq!(graph.detect_claim_contradictions(tenant).len(), 1);
    }

    #[test]
    fn unresolved_contradiction_blocks_promotion() {
        let tenant = Uuid::now_v7();
        let mut graph = KnowledgeGraph::default();
        let claim = node(tenant, NodeKind::Claim, EpistemicState::Unverified, 'a');
        let other = node(tenant, NodeKind::Claim, EpistemicState::Accepted, 'b');
        graph.add_node(MutationAuthority::Agent, claim.clone()).unwrap();
        graph.add_node(MutationAuthority::System, other.clone()).unwrap();
        graph.add_contradiction(Contradiction {
            contradiction_uuid: Uuid::now_v7(),
            tenant_uuid: tenant,
            left_claim_uuid: claim.node_uuid,
            right_claim_uuid: other.node_uuid,
            detected_at: Utc::now(),
        }).unwrap();
        assert_eq!(
            graph.evaluate_claim_promotion(claim.node_uuid, EpistemicState::Accepted, &PromotionPolicy::default(), Utc::now()),
            Err(GraphError::UnresolvedContradiction)
        );
    }

    #[test]
    fn graph_snapshot_is_deterministic_for_same_state() {
        let tenant = Uuid::now_v7();
        let mut graph = KnowledgeGraph::default();
        let a = node(tenant, NodeKind::Artifact, EpistemicState::Accepted, 'a');
        graph.add_node(MutationAuthority::System, a).unwrap();
        let s1 = graph.snapshot(tenant, Utc::now());
        let s2 = graph.snapshot(tenant, Utc::now());
        assert_eq!(s1.root_sha256_hex, s2.root_sha256_hex);
    }
}

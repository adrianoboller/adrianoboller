#![forbid(unsafe_code)]

use chrono::{DateTime, Duration, Utc};
use phxclaw_evidence_ledger::{EvidenceDraft, EvidenceLedger, EvidenceOutcome, LedgerError};
use phxclaw_knowledge_evidence_graph::{
    EpistemicState, GraphError, KnowledgeGraph, MutationAuthority,
    PromotionPolicy as GraphPromotionPolicy,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewerAuthority {
    Learning,
    Agent,
    System,
    Human,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRelation {
    Supports,
    Refutes,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromotionEvidence {
    pub evidence_uuid: Uuid,
    pub evidence_sha256_hex: String,
    pub source_state_sha256_hex: String,
    pub mechanism: String,
    pub relation: EvidenceRelation,
    pub collected_at: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromotionRequest {
    pub request_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub candidate_uuid: Option<Uuid>,
    pub claim_node_uuid: Uuid,
    pub target_state: EpistemicState,
    pub expected_source_state_sha256_hex: String,
    pub requested_by: String,
    pub requested_at: DateTime<Utc>,
    pub evidence: Vec<PromotionEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromotionGatePolicy {
    pub min_supporting_evidence: usize,
    pub min_independent_mechanisms: usize,
    pub max_evidence_age_seconds: i64,
    pub require_human_for_governed: bool,
}

impl Default for PromotionGatePolicy {
    fn default() -> Self {
        Self {
            min_supporting_evidence: 2,
            min_independent_mechanisms: 2,
            max_evidence_age_seconds: 30 * 24 * 60 * 60,
            require_human_for_governed: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromotionAssessment {
    pub request_uuid: Uuid,
    pub eligible: bool,
    pub blockers: Vec<String>,
    pub supporting_evidence: usize,
    pub independent_mechanisms: usize,
    pub requires_human_approval: bool,
    pub assessed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromotionReceipt {
    pub promotion_uuid: Uuid,
    pub request_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub previous_node_uuid: Uuid,
    pub promoted_node_uuid: Uuid,
    pub target_state: EpistemicState,
    pub reviewer_authority: ReviewerAuthority,
    pub reviewer: String,
    pub decision_sha256_hex: String,
    pub promoted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RevocationReceipt {
    pub revocation_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub previous_node_uuid: Uuid,
    pub rejected_node_uuid: Uuid,
    pub reviewer: String,
    pub reason_sha256_hex: String,
    pub revoked_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum PromotionError {
    #[error("claim node not found")]
    ClaimNotFound,
    #[error("cross-tenant promotion is forbidden")]
    CrossTenant,
    #[error("target state must be accepted or governed")]
    InvalidTarget,
    #[error("source-state hash is invalid or does not match the claim")]
    SourceStateMismatch,
    #[error("promotion is blocked: {0:?}")]
    Blocked(Vec<String>),
    #[error("human approval is required")]
    HumanApprovalRequired,
    #[error("learning/agent authority cannot approve knowledge")]
    AuthorityDenied,
    #[error("graph error: {0}")]
    Graph(#[from] GraphError),
    #[error("evidence ledger error: {0}")]
    Ledger(#[from] LedgerError),
}

#[derive(Debug, Clone)]
pub struct KnowledgePromotionGate {
    policy: PromotionGatePolicy,
}

impl KnowledgePromotionGate {
    pub fn new(policy: PromotionGatePolicy) -> Self {
        Self { policy }
    }
    pub fn policy(&self) -> &PromotionGatePolicy {
        &self.policy
    }

    pub fn assess(
        &self,
        graph: &KnowledgeGraph,
        request: &PromotionRequest,
        now: DateTime<Utc>,
    ) -> Result<PromotionAssessment, PromotionError> {
        if !matches!(
            request.target_state,
            EpistemicState::Accepted | EpistemicState::Governed
        ) {
            return Err(PromotionError::InvalidTarget);
        }
        let claim = graph
            .node(request.claim_node_uuid)
            .ok_or(PromotionError::ClaimNotFound)?;
        if claim.tenant_uuid != request.tenant_uuid {
            return Err(PromotionError::CrossTenant);
        }
        if !valid_sha256(&request.expected_source_state_sha256_hex)
            || !claim
                .source_state_sha256_hex
                .eq_ignore_ascii_case(&request.expected_source_state_sha256_hex)
        {
            return Err(PromotionError::SourceStateMismatch);
        }

        let mut blockers = Vec::new();
        let max_age = Duration::seconds(self.policy.max_evidence_age_seconds.max(0));
        let mut supporting = 0usize;
        let mut mechanisms = BTreeSet::<String>::new();
        for ev in &request.evidence {
            if !valid_sha256(&ev.evidence_sha256_hex)
                || !valid_sha256(&ev.source_state_sha256_hex)
                || !ev
                    .source_state_sha256_hex
                    .eq_ignore_ascii_case(&request.expected_source_state_sha256_hex)
            {
                blockers.push(format!("source_state_mismatch:{}", ev.evidence_uuid));
                continue;
            }
            if ev.collected_at > now {
                blockers.push(format!("future_evidence:{}", ev.evidence_uuid));
                continue;
            }
            if now.signed_duration_since(ev.collected_at) > max_age {
                blockers.push(format!("stale_evidence:{}", ev.evidence_uuid));
                continue;
            }
            if ev.valid_until.as_ref().is_some_and(|until| now >= *until) {
                blockers.push(format!("expired_evidence:{}", ev.evidence_uuid));
                continue;
            }
            if ev.mechanism.trim().is_empty() {
                blockers.push(format!("empty_mechanism:{}", ev.evidence_uuid));
                continue;
            }
            match ev.relation {
                EvidenceRelation::Refutes => {
                    blockers.push(format!("active_refutation:{}", ev.evidence_uuid))
                }
                EvidenceRelation::Supports => {
                    supporting += 1;
                    mechanisms.insert(ev.mechanism.clone());
                }
            }
        }
        let unresolved = graph.unresolved_contradiction_count(request.claim_node_uuid);
        if unresolved > 0 {
            blockers.push(format!("unresolved_contradictions:{unresolved}"));
        }
        if supporting < self.policy.min_supporting_evidence {
            blockers.push(format!(
                "supporting_evidence:{supporting}<{}",
                self.policy.min_supporting_evidence
            ));
        }
        if mechanisms.len() < self.policy.min_independent_mechanisms {
            blockers.push(format!(
                "independent_mechanisms:{}<{}",
                mechanisms.len(),
                self.policy.min_independent_mechanisms
            ));
        }
        Ok(PromotionAssessment {
            request_uuid: request.request_uuid,
            eligible: blockers.is_empty(),
            blockers,
            supporting_evidence: supporting,
            independent_mechanisms: mechanisms.len(),
            requires_human_approval: request.target_state == EpistemicState::Governed
                && self.policy.require_human_for_governed,
            assessed_at: now,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn promote(
        &self,
        graph: &mut KnowledgeGraph,
        request: &PromotionRequest,
        reviewer_authority: ReviewerAuthority,
        reviewer: &str,
        decision_payload: &[u8],
        ledger: Option<&EvidenceLedger>,
        now: DateTime<Utc>,
    ) -> Result<PromotionReceipt, PromotionError> {
        let assessment = self.assess(graph, request, now)?;
        if !assessment.eligible {
            return Err(PromotionError::Blocked(assessment.blockers));
        }
        if matches!(
            reviewer_authority,
            ReviewerAuthority::Learning | ReviewerAuthority::Agent
        ) {
            return Err(PromotionError::AuthorityDenied);
        }
        if assessment.requires_human_approval && reviewer_authority != ReviewerAuthority::Human {
            return Err(PromotionError::HumanApprovalRequired);
        }

        // Reuse graph-level proof rules as a second independent gate. The caller must have bound
        // evidence into the KnowledgeGraph before promotion.
        let graph_decision = graph.evaluate_claim_promotion(
            request.claim_node_uuid,
            request.target_state,
            &GraphPromotionPolicy {
                min_supporting_evidence: self.policy.min_supporting_evidence,
                min_independent_mechanisms: self.policy.min_independent_mechanisms,
                allow_governed_without_human: !self.policy.require_human_for_governed,
            },
            now,
        )?;
        let human_approved = reviewer_authority == ReviewerAuthority::Human;
        let (promoted, edge) = graph.promoted_claim_version(
            request.claim_node_uuid,
            &graph_decision,
            human_approved,
            now,
        )?;
        let promoted_node_uuid = promoted.node_uuid;
        graph.add_node(
            match reviewer_authority {
                ReviewerAuthority::Human => MutationAuthority::Human,
                _ => MutationAuthority::System,
            },
            promoted,
        )?;
        graph.add_edge(edge)?;

        let receipt = PromotionReceipt {
            promotion_uuid: Uuid::now_v7(),
            request_uuid: request.request_uuid,
            tenant_uuid: request.tenant_uuid,
            previous_node_uuid: request.claim_node_uuid,
            promoted_node_uuid,
            target_state: request.target_state,
            reviewer_authority,
            reviewer: reviewer.to_string(),
            decision_sha256_hex: sha256_hex(decision_payload),
            promoted_at: now,
        };
        append_promotion_evidence(&receipt, request, &assessment, ledger)?;
        Ok(receipt)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn revoke(
        &self,
        graph: &mut KnowledgeGraph,
        tenant_uuid: Uuid,
        promoted_node_uuid: Uuid,
        reviewer_authority: ReviewerAuthority,
        reviewer: &str,
        reason: &[u8],
        ledger: Option<&EvidenceLedger>,
        now: DateTime<Utc>,
    ) -> Result<RevocationReceipt, PromotionError> {
        if reviewer_authority != ReviewerAuthority::Human {
            return Err(PromotionError::HumanApprovalRequired);
        }
        let node = graph
            .node(promoted_node_uuid)
            .ok_or(PromotionError::ClaimNotFound)?;
        if node.tenant_uuid != tenant_uuid {
            return Err(PromotionError::CrossTenant);
        }
        let (rejected, edge) = graph.rejected_claim_version(promoted_node_uuid, now)?;
        let rejected_node_uuid = rejected.node_uuid;
        graph.add_node(MutationAuthority::Human, rejected)?;
        graph.add_edge(edge)?;
        let receipt = RevocationReceipt {
            revocation_uuid: Uuid::now_v7(),
            tenant_uuid,
            previous_node_uuid: promoted_node_uuid,
            rejected_node_uuid,
            reviewer: reviewer.to_string(),
            reason_sha256_hex: sha256_hex(reason),
            revoked_at: now,
        };
        if let Some(ledger) = ledger {
            ledger.append(EvidenceDraft {
                action_uuid: receipt.revocation_uuid,
                correlation_uuid: None,
                actor: reviewer.to_string(), capability: "knowledge.revoke".into(), action: "revoke".into(),
                outcome: EvidenceOutcome::Succeeded,
                request_summary: json!({"tenant_uuid": tenant_uuid, "promoted_node_uuid": promoted_node_uuid}),
                result_summary: json!({"rejected_node_uuid": rejected_node_uuid, "reason_sha256": receipt.reason_sha256_hex}),
                artifact_uris: vec![],
            })?;
        }
        Ok(receipt)
    }
}

fn append_promotion_evidence(
    receipt: &PromotionReceipt,
    request: &PromotionRequest,
    assessment: &PromotionAssessment,
    ledger: Option<&EvidenceLedger>,
) -> Result<(), LedgerError> {
    if let Some(ledger) = ledger {
        ledger.append(EvidenceDraft {
            action_uuid: receipt.promotion_uuid,
            correlation_uuid: Some(request.request_uuid),
            actor: receipt.reviewer.clone(), capability: "knowledge.promote".into(), action: "promote".into(),
            outcome: EvidenceOutcome::Succeeded,
            request_summary: json!({
                "tenant_uuid": request.tenant_uuid, "claim_node_uuid": request.claim_node_uuid,
                "candidate_uuid": request.candidate_uuid, "target_state": request.target_state,
                "source_state_sha256": request.expected_source_state_sha256_hex
            }),
            result_summary: json!({
                "promoted_node_uuid": receipt.promoted_node_uuid, "supporting_evidence": assessment.supporting_evidence,
                "independent_mechanisms": assessment.independent_mechanisms, "decision_sha256": receipt.decision_sha256_hex
            }), artifact_uris: vec![],
        })?;
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use phxclaw_knowledge_evidence_graph::{EdgeKind, EvidenceBinding, KnowledgeNode, NodeKind};

    fn digest(ch: char) -> String {
        ch.to_string().repeat(64)
    }
    fn base_graph() -> (Uuid, KnowledgeGraph, KnowledgeNode, Vec<PromotionEvidence>) {
        let tenant = Uuid::now_v7();
        let now = Utc::now();
        let mut graph = KnowledgeGraph::default();
        let claim = KnowledgeNode {
            node_uuid: Uuid::now_v7(),
            tenant_uuid: tenant,
            kind: NodeKind::Claim,
            state: EpistemicState::Unverified,
            content_sha256_hex: digest('a'),
            source_state_sha256_hex: digest('f'),
            subject_key: Some("runtime.postgresql".into()),
            predicate_key: Some("major".into()),
            value_sha256_hex: Some(digest('1')),
            confidence_ppm: 700_000,
            created_at: now,
        };
        graph
            .add_node(MutationAuthority::Agent, claim.clone())
            .unwrap();
        let mut evs = Vec::new();
        for (ch, mech) in [('b', "test-suite"), ('c', "documentation")] {
            let evidence = KnowledgeNode {
                node_uuid: Uuid::now_v7(),
                tenant_uuid: tenant,
                kind: NodeKind::Evidence,
                state: EpistemicState::Accepted,
                content_sha256_hex: digest(ch),
                source_state_sha256_hex: digest('f'),
                subject_key: None,
                predicate_key: None,
                value_sha256_hex: None,
                confidence_ppm: 900_000,
                created_at: now,
            };
            graph
                .add_node(MutationAuthority::System, evidence.clone())
                .unwrap();
            graph
                .bind_evidence(
                    EvidenceBinding {
                        binding_uuid: Uuid::now_v7(),
                        tenant_uuid: tenant,
                        claim_node_uuid: claim.node_uuid,
                        evidence_node_uuid: evidence.node_uuid,
                        relation: EdgeKind::Supports,
                        evidence_sha256_hex: evidence.content_sha256_hex.clone(),
                        source_state_sha256_hex: digest('f'),
                        mechanism: mech.into(),
                        collected_at: now,
                        valid_until: None,
                    },
                    now,
                )
                .unwrap();
            evs.push(PromotionEvidence {
                evidence_uuid: evidence.node_uuid,
                evidence_sha256_hex: evidence.content_sha256_hex,
                source_state_sha256_hex: digest('f'),
                mechanism: mech.into(),
                relation: EvidenceRelation::Supports,
                collected_at: now,
                valid_until: None,
            });
        }
        (tenant, graph, claim, evs)
    }

    #[test]
    fn accepted_can_be_system_approved_with_two_mechanisms() {
        let (tenant, mut graph, claim, evidence) = base_graph();
        let now = Utc::now();
        let gate = KnowledgePromotionGate::new(PromotionGatePolicy::default());
        let req = PromotionRequest {
            request_uuid: Uuid::now_v7(),
            tenant_uuid: tenant,
            candidate_uuid: None,
            claim_node_uuid: claim.node_uuid,
            target_state: EpistemicState::Accepted,
            expected_source_state_sha256_hex: digest('f'),
            requested_by: "test".into(),
            requested_at: now,
            evidence,
        };
        let r = gate
            .promote(
                &mut graph,
                &req,
                ReviewerAuthority::System,
                "gate",
                b"approved",
                None,
                now,
            )
            .unwrap();
        assert_eq!(
            graph.node(r.promoted_node_uuid).unwrap().state,
            EpistemicState::Accepted
        );
    }

    #[test]
    fn governed_requires_human() {
        let (tenant, mut graph, claim, evidence) = base_graph();
        let now = Utc::now();
        let gate = KnowledgePromotionGate::new(PromotionGatePolicy::default());
        let req = PromotionRequest {
            request_uuid: Uuid::now_v7(),
            tenant_uuid: tenant,
            candidate_uuid: None,
            claim_node_uuid: claim.node_uuid,
            target_state: EpistemicState::Governed,
            expected_source_state_sha256_hex: digest('f'),
            requested_by: "test".into(),
            requested_at: now,
            evidence,
        };
        assert!(matches!(
            gate.promote(
                &mut graph,
                &req,
                ReviewerAuthority::System,
                "gate",
                b"approved",
                None,
                now
            ),
            Err(PromotionError::HumanApprovalRequired)
        ));
    }

    #[test]
    fn refuting_evidence_blocks_before_graph_mutation() {
        let (tenant, graph, claim, mut evidence) = base_graph();
        let now = Utc::now();
        evidence.push(PromotionEvidence {
            evidence_uuid: Uuid::now_v7(),
            evidence_sha256_hex: digest('d'),
            source_state_sha256_hex: digest('f'),
            mechanism: "audit".into(),
            relation: EvidenceRelation::Refutes,
            collected_at: now,
            valid_until: None,
        });
        let gate = KnowledgePromotionGate::new(PromotionGatePolicy::default());
        let req = PromotionRequest {
            request_uuid: Uuid::now_v7(),
            tenant_uuid: tenant,
            candidate_uuid: None,
            claim_node_uuid: claim.node_uuid,
            target_state: EpistemicState::Accepted,
            expected_source_state_sha256_hex: digest('f'),
            requested_by: "test".into(),
            requested_at: now,
            evidence,
        };
        let a = gate.assess(&graph, &req, now).unwrap();
        assert!(!a.eligible);
        assert!(a
            .blockers
            .iter()
            .any(|x| x.starts_with("active_refutation:")));
    }
}

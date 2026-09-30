#![forbid(unsafe_code)]

use chrono::Utc;
use phxclaw_types::{EvidenceRef, ResearchRecord, new_uuid_v7};
use postgres::{Client, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ResearchError {
    #[error("postgres error: {0}")]
    Postgres(#[from] postgres::Error),
    #[error("invalid sha256 digest")]
    InvalidDigest,
    #[error("research record not found")]
    NotFound,
    #[error("cross-tenant access denied")]
    CrossTenant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchPersistenceContext {
    pub tenant_uuid: Uuid,
    pub project_uuid: Option<Uuid>,
    pub correlation_uuid: Uuid,
    pub source_state_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchGraphLink {
    pub research_uuid: Uuid,
    pub knowledge_node_uuid: Uuid,
    pub role: String,
}

#[derive(Debug, Default)]
pub struct ResearchCore;

impl ResearchCore {
    pub fn create_record(
        &self,
        topic: impl Into<String>,
        question: impl Into<String>,
        evidence: Vec<EvidenceRef>,
    ) -> ResearchRecord {
        ResearchRecord {
            uuid: new_uuid_v7(),
            topic: topic.into(),
            question: question.into(),
            evidence,
            findings: vec![],
            suggested_hypotheses: vec![],
            created_at: Utc::now(),
        }
    }

    pub fn persist(
        &self,
        client: &mut Client,
        context: &ResearchPersistenceContext,
        record: &ResearchRecord,
    ) -> Result<(), ResearchError> {
        validate_digest(&context.source_state_sha256)?;
        let mut tx = client.transaction()?;
        persist_record(&mut tx, context, record)?;
        tx.commit()?;
        Ok(())
    }

    pub fn load(
        &self,
        client: &mut Client,
        tenant_uuid: Uuid,
        research_uuid: Uuid,
    ) -> Result<ResearchRecord, ResearchError> {
        set_tenant(client, tenant_uuid)?;
        let row=client.query_opt(
            "SELECT topic,question,findings,suggested_hypotheses,created_at FROM phxclaw.research_records WHERE research_uuid=$1 AND tenant_uuid=$2",
            &[&research_uuid,&tenant_uuid])?.ok_or(ResearchError::NotFound)?;
        let evrows=client.query(
            "SELECT evidence_uuid,uri,source_type,retrieved_at,sha256,notes FROM phxclaw.research_record_evidence WHERE research_uuid=$1 ORDER BY retrieved_at,evidence_uuid",
            &[&research_uuid])?;
        let evidence = evrows
            .into_iter()
            .map(|r| EvidenceRef {
                uuid: r.get(0),
                uri: r.get(1),
                source_type: r.get(2),
                retrieved_at: r.get(3),
                sha256: r.get(4),
                notes: r.get(5),
            })
            .collect();
        let findings: Value = row.get(2);
        let suggested: Value = row.get(3);
        Ok(ResearchRecord {
            uuid: research_uuid,
            topic: row.get(0),
            question: row.get(1),
            evidence,
            findings: serde_json::from_value(findings).unwrap_or_default(),
            suggested_hypotheses: serde_json::from_value(suggested).unwrap_or_default(),
            created_at: row.get(4),
        })
    }

    pub fn link_knowledge_node(
        &self,
        client: &mut Client,
        tenant_uuid: Uuid,
        link: &ResearchGraphLink,
    ) -> Result<(), ResearchError> {
        set_tenant(client, tenant_uuid)?;
        client.execute("INSERT INTO phxclaw.research_graph_links(research_uuid,knowledge_node_uuid,role) VALUES($1,$2,$3) ON CONFLICT DO NOTHING", &[&link.research_uuid,&link.knowledge_node_uuid,&link.role])?;
        Ok(())
    }
}

fn persist_record(
    tx: &mut Transaction<'_>,
    ctx: &ResearchPersistenceContext,
    r: &ResearchRecord,
) -> Result<(), ResearchError> {
    tx.query_one(
        "SELECT set_config('phxclaw.tenant_uuid',$1,true)",
        &[&ctx.tenant_uuid.to_string()],
    )?;
    tx.execute(r#"INSERT INTO phxclaw.research_records(research_uuid,tenant_uuid,project_uuid,correlation_uuid,topic,question,findings,suggested_hypotheses,source_state_sha256,created_at)
      VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT(research_uuid) DO NOTHING"#,
      &[&r.uuid,&ctx.tenant_uuid,&ctx.project_uuid,&ctx.correlation_uuid,&r.topic,&r.question,&serde_json::to_value(&r.findings).unwrap_or(Value::Array(vec![])),&serde_json::to_value(&r.suggested_hypotheses).unwrap_or(Value::Array(vec![])),&ctx.source_state_sha256,&r.created_at])?;
    for e in &r.evidence {
        validate_digest(&e.sha256)?;
        tx.execute(r#"INSERT INTO phxclaw.research_record_evidence(research_uuid,evidence_uuid,uri,source_type,retrieved_at,sha256,notes)
          VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(research_uuid,evidence_uuid) DO NOTHING"#,
          &[&r.uuid,&e.uuid,&e.uri,&e.source_type,&e.retrieved_at,&e.sha256,&e.notes])?;
    }
    Ok(())
}
fn validate_digest(v: &str) -> Result<(), ResearchError> {
    if v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(ResearchError::InvalidDigest)
    }
}
fn set_tenant(client: &mut Client, tenant_uuid: Uuid) -> Result<(), ResearchError> {
    client.query_one(
        "SELECT set_config('phxclaw.tenant_uuid',$1,false)",
        &[&tenant_uuid.to_string()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use phxclaw_types::is_uuid_v7;
    #[test]
    fn record_v7() {
        let r = ResearchCore.create_record("plugins", "como validar?", vec![]);
        assert!(is_uuid_v7(&r.uuid));
    }
    #[test]
    fn digest_guard() {
        assert!(validate_digest(&"a".repeat(64)).is_ok());
        assert!(validate_digest("bad").is_err());
    }
}

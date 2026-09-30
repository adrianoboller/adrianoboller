#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use phxclaw_types::{EvidenceRef, ExperimentPlan, Hypothesis, HypothesisStatus, new_uuid_v7};
use postgres::{Client, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum HypothesisError {
    #[error("postgres error: {0}")]
    Postgres(#[from] postgres::Error),
    #[error("invalid sha256 digest")]
    InvalidDigest,
    #[error("hypothesis not found")]
    NotFound,
    #[error("invalid status transition from {0:?} to {1:?}")]
    InvalidTransition(HypothesisStatus, HypothesisStatus),
    #[error("hypothesis is no longer {0:?}: another decision won")]
    StaleDecision(HypothesisStatus),
    #[error("experiment run is not running (already finished or unknown)")]
    RunNotRunning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypothesisPersistenceContext {
    pub tenant_uuid: Uuid,
    pub research_uuid: Option<Uuid>,
    pub source_state_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentRunStart {
    pub run_uuid: Uuid,
    pub hypothesis_uuid: Uuid,
    pub started_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentRunResult {
    pub run_uuid: Uuid,
    pub success: Option<bool>,
    pub result_summary: Value,
    pub evidence: Vec<EvidenceRef>,
    pub completed_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypothesisGraphLink {
    pub hypothesis_uuid: Uuid,
    pub knowledge_node_uuid: Uuid,
    pub role: String,
}

#[derive(Debug, Default)]
pub struct HypothesisCore;
impl HypothesisCore {
    pub fn propose(
        &self,
        statement: impl Into<String>,
        rationale: impl Into<String>,
        steps: Vec<String>,
        success_criteria: Vec<String>,
        failure_criteria: Vec<String>,
    ) -> Hypothesis {
        let now = Utc::now();
        Hypothesis {
            uuid: new_uuid_v7(),
            statement: statement.into(),
            rationale: rationale.into(),
            experiment: ExperimentPlan {
                uuid: new_uuid_v7(),
                steps,
                success_criteria,
                failure_criteria,
            },
            evidence: vec![],
            status: HypothesisStatus::Proposed,
            decision_notes: vec![],
            created_at: now,
            updated_at: now,
        }
    }
    pub fn decide(
        &self,
        h: &mut Hypothesis,
        status: HypothesisStatus,
        note: impl Into<String>,
    ) -> Result<(), HypothesisError> {
        if !valid_transition(&h.status, &status) {
            return Err(HypothesisError::InvalidTransition(h.status.clone(), status));
        }
        h.status = status;
        h.decision_notes.push(note.into());
        h.updated_at = Utc::now();
        Ok(())
    }
    pub fn persist(
        &self,
        client: &mut Client,
        ctx: &HypothesisPersistenceContext,
        h: &Hypothesis,
    ) -> Result<(), HypothesisError> {
        validate_digest(&ctx.source_state_sha256)?;
        let mut tx = client.transaction()?;
        persist_hypothesis(&mut tx, ctx, h)?;
        tx.commit()?;
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn record_decision(
        &self,
        client: &mut Client,
        tenant_uuid: Uuid,
        hypothesis_uuid: Uuid,
        from: HypothesisStatus,
        to: HypothesisStatus,
        note: &str,
        actor: &str,
    ) -> Result<(), HypothesisError> {
        if !valid_transition(&from, &to) {
            return Err(HypothesisError::InvalidTransition(from, to));
        }
        set_tenant(client, tenant_uuid)?;
        let mut tx = client.transaction()?;
        tx.query_one(
            "SELECT set_config('phxclaw.tenant_uuid',$1,true)",
            &[&tenant_uuid.to_string()],
        )?;
        // O UPDATE confere o estado de origem e vem ANTES do diario: duas decisoes
        // concorrentes a partir do mesmo estado nao podem as duas acontecer. A segunda
        // espera a trava de linha da primeira, reavalia o WHERE e acha 0 linhas.
        let mudou = tx.execute("UPDATE phxclaw.hypothesis_records SET current_status=$2,updated_at=clock_timestamp() WHERE hypothesis_uuid=$1 AND current_status=$3",&[&hypothesis_uuid,&status_str(&to),&status_str(&from)])?;
        if mudou != 1 {
            return Err(HypothesisError::StaleDecision(from));
        }
        tx.execute("INSERT INTO phxclaw.hypothesis_decisions(decision_uuid,hypothesis_uuid,from_status,to_status,note,actor,decided_at) VALUES($1,$2,$3,$4,$5,$6,clock_timestamp())",&[&new_uuid_v7(),&hypothesis_uuid,&status_str(&from),&status_str(&to),&note,&actor])?;
        tx.commit()?;
        Ok(())
    }
    pub fn start_experiment(
        &self,
        client: &mut Client,
        tenant_uuid: Uuid,
        hypothesis_uuid: Uuid,
    ) -> Result<ExperimentRunStart, HypothesisError> {
        set_tenant(client, tenant_uuid)?;
        let run = ExperimentRunStart {
            run_uuid: new_uuid_v7(),
            hypothesis_uuid,
            started_at: Utc::now(),
        };
        client.execute("INSERT INTO phxclaw.experiment_runs(run_uuid,tenant_uuid,hypothesis_uuid,state,started_at) VALUES($1,$2,$3,'running',$4)",&[&run.run_uuid,&tenant_uuid,&hypothesis_uuid,&run.started_at])?;
        Ok(run)
    }
    pub fn finish_experiment(
        &self,
        client: &mut Client,
        tenant_uuid: Uuid,
        result: &ExperimentRunResult,
    ) -> Result<(), HypothesisError> {
        set_tenant(client, tenant_uuid)?;
        let mut tx = client.transaction()?;
        tx.query_one(
            "SELECT set_config('phxclaw.tenant_uuid',$1,true)",
            &[&tenant_uuid.to_string()],
        )?;
        // Zero linhas aqui quer dizer que o experimento ja foi fechado (ou nao existe):
        // ignorar deixaria o segundo resultado sumir sem ninguem saber.
        let fechou = tx.execute("UPDATE phxclaw.experiment_runs SET state='completed',success=$2,result_summary=$3,completed_at=$4 WHERE run_uuid=$1 AND tenant_uuid=$5 AND state='running'",&[&result.run_uuid,&result.success,&result.result_summary,&result.completed_at,&tenant_uuid])?;
        if fechou != 1 {
            return Err(HypothesisError::RunNotRunning);
        }
        for e in &result.evidence {
            validate_digest(&e.sha256)?;
            tx.execute("INSERT INTO phxclaw.experiment_run_evidence(run_uuid,evidence_uuid,uri,source_type,retrieved_at,sha256,notes) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING",&[&result.run_uuid,&e.uuid,&e.uri,&e.source_type,&e.retrieved_at,&e.sha256,&e.notes])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn link_knowledge_node(
        &self,
        client: &mut Client,
        tenant_uuid: Uuid,
        link: &HypothesisGraphLink,
    ) -> Result<(), HypothesisError> {
        set_tenant(client, tenant_uuid)?;
        client.execute("INSERT INTO phxclaw.hypothesis_graph_links(hypothesis_uuid,knowledge_node_uuid,role) VALUES($1,$2,$3) ON CONFLICT DO NOTHING",&[&link.hypothesis_uuid,&link.knowledge_node_uuid,&link.role])?;
        Ok(())
    }
}
fn persist_hypothesis(
    tx: &mut Transaction<'_>,
    ctx: &HypothesisPersistenceContext,
    h: &Hypothesis,
) -> Result<(), HypothesisError> {
    tx.query_one(
        "SELECT set_config('phxclaw.tenant_uuid',$1,true)",
        &[&ctx.tenant_uuid.to_string()],
    )?;
    tx.execute(r#"INSERT INTO phxclaw.hypothesis_records(hypothesis_uuid,tenant_uuid,research_uuid,statement,rationale,experiment_uuid,current_status,source_state_sha256,created_at,updated_at)
 VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT(hypothesis_uuid) DO NOTHING"#,&[&h.uuid,&ctx.tenant_uuid,&ctx.research_uuid,&h.statement,&h.rationale,&h.experiment.uuid,&status_str(&h.status),&ctx.source_state_sha256,&h.created_at,&h.updated_at])?;
    tx.execute("INSERT INTO phxclaw.experiment_plans(experiment_uuid,hypothesis_uuid,steps,success_criteria,failure_criteria) VALUES($1,$2,$3,$4,$5) ON CONFLICT(experiment_uuid) DO NOTHING",&[&h.experiment.uuid,&h.uuid,&serde_json::to_value(&h.experiment.steps).unwrap_or(Value::Array(vec![])),&serde_json::to_value(&h.experiment.success_criteria).unwrap_or(Value::Array(vec![])),&serde_json::to_value(&h.experiment.failure_criteria).unwrap_or(Value::Array(vec![]))])?;
    for e in &h.evidence {
        validate_digest(&e.sha256)?;
        tx.execute("INSERT INTO phxclaw.hypothesis_evidence(hypothesis_uuid,evidence_uuid,uri,source_type,retrieved_at,sha256,notes) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING",&[&h.uuid,&e.uuid,&e.uri,&e.source_type,&e.retrieved_at,&e.sha256,&e.notes])?;
    }
    for (i, note) in h.decision_notes.iter().enumerate() {
        tx.execute("INSERT INTO phxclaw.hypothesis_decisions(decision_uuid,hypothesis_uuid,from_status,to_status,note,actor,decided_at,import_order) VALUES($1,$2,NULL,$3,$4,'import', $5,$6) ON CONFLICT(hypothesis_uuid,import_order) DO NOTHING",&[&new_uuid_v7(),&h.uuid,&status_str(&h.status),note,&h.updated_at,&((i+1) as i32)])?;
    }
    Ok(())
}
fn valid_transition(from: &HypothesisStatus, to: &HypothesisStatus) -> bool {
    use HypothesisStatus::*;
    matches!(
        (from, to),
        (Proposed, Testing)
            | (Proposed, Rejected)
            | (Testing, Supported)
            | (Testing, Rejected)
            | (Testing, Inconclusive)
            | (Inconclusive, Testing)
            | (Inconclusive, Rejected)
    ) || from == to
}
fn status_str(s: &HypothesisStatus) -> &'static str {
    match s {
        HypothesisStatus::Proposed => "proposed",
        HypothesisStatus::Testing => "testing",
        HypothesisStatus::Supported => "supported",
        HypothesisStatus::Rejected => "rejected",
        HypothesisStatus::Inconclusive => "inconclusive",
    }
}
fn validate_digest(v: &str) -> Result<(), HypothesisError> {
    if v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(HypothesisError::InvalidDigest)
    }
}
fn set_tenant(client: &mut Client, tenant_uuid: Uuid) -> Result<(), HypothesisError> {
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
    fn proposed_has_experiment() {
        let h = HypothesisCore.propose(
            "x",
            "r",
            vec!["s".into()],
            vec!["ok".into()],
            vec!["bad".into()],
        );
        assert!(is_uuid_v7(&h.uuid));
        assert!(is_uuid_v7(&h.experiment.uuid));
    }
    #[test]
    fn transition_guard() {
        assert!(valid_transition(
            &HypothesisStatus::Proposed,
            &HypothesisStatus::Testing
        ));
        assert!(!valid_transition(
            &HypothesisStatus::Proposed,
            &HypothesisStatus::Supported
        ));
    }
}

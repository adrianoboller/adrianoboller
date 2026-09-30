//! PhxClaw v0.55 — Real-Time Portfolio Reconciliation & Execution Ledger.
//! Compares planned vs executed state and reconstructs verified reasons for every change.
//! This crate is read/reconcile/audit oriented: it never mutates project plans directly.
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::collections::{BTreeMap,BTreeSet};
use thiserror::Error;
use uuid::Uuid;

pub const DIRECT_MUTATION_ALLOWED: bool = false;
pub const UNAVAILABLE_IS_PASS: bool = false;

#[derive(Debug,Error,PartialEq,Eq)]
pub enum LedgerError {
 #[error("source state mismatch")] SourceStateMismatch,
 #[error("invalid hash")] InvalidHash,
 #[error("non-monotonic sequence")] NonMonotonicSequence,
 #[error("stale fencing token")] StaleFencing,
 #[error("idempotency conflict")] IdempotencyConflict,
 #[error("unproven reason")] UnprovenReason,
 #[error("direct mutation forbidden")] DirectMutationForbidden,
}

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
pub struct PlannedWorkItem {
 pub tenant_uuid:Uuid,pub project_uuid:Uuid,pub work_item_uuid:Uuid,pub plan_revision_uuid:Uuid,
 pub source_state_sha256:String,pub plan_sha256:String,pub planned_start_epoch:i64,pub planned_finish_epoch:i64,
 pub planned_cost:f64,pub planned_agent_uuid:Option<Uuid>,pub planned_model_profile_uuid:Option<Uuid>,
 pub planned_skills:Vec<String>,pub planned_dependency_hash:String,pub planned_quality_floor:f64,
}

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub enum FactKind { Started, Finished, CostSettled, AgentChanged, ModelChanged, SkillsChanged, DependencyChanged, QualityMeasured, BudgetChanged, DecisionApplied, SupervisorIntervention }

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub enum ReasonStatus { Verified, Unverified, Missing }

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct ExecutionFact {
 pub tenant_uuid:Uuid,pub project_uuid:Uuid,pub work_item_uuid:Uuid,pub fact_uuid:Uuid,pub sequence_no:u64,
 pub controller_epoch:u64,pub fencing_token:u64,pub observed_at_epoch:i64,pub source_state_sha256:String,
 pub fact_kind:FactKind,pub idempotency_key:String,pub payload_sha256:String,pub evidence_sha256:String,
 pub reason_code:Option<String>,pub reason_status:ReasonStatus,pub causation_uuid:Option<Uuid>,pub correlation_uuid:Uuid,
 pub payload:serde_json::Value,
}

#[derive(Clone,Debug,Serialize,Deserialize,Default)]
pub struct LedgerCursor { pub last_sequence_no:u64,pub last_controller_epoch:u64,pub last_fencing_token:u64,pub idempotency:BTreeMap<String,String> }

pub fn append_fact(cursor:&mut LedgerCursor,f:&ExecutionFact)->Result<(),LedgerError>{
 validate_hash(&f.source_state_sha256)?;validate_hash(&f.payload_sha256)?;validate_hash(&f.evidence_sha256)?;
 if f.sequence_no<=cursor.last_sequence_no{return Err(LedgerError::NonMonotonicSequence)}
 if f.controller_epoch<cursor.last_controller_epoch || (f.controller_epoch==cursor.last_controller_epoch && f.fencing_token<=cursor.last_fencing_token){return Err(LedgerError::StaleFencing)}
 if let Some(prev)=cursor.idempotency.get(&f.idempotency_key){if prev!=&f.payload_sha256{return Err(LedgerError::IdempotencyConflict)}}
 cursor.idempotency.insert(f.idempotency_key.clone(),f.payload_sha256.clone());cursor.last_sequence_no=f.sequence_no;cursor.last_controller_epoch=f.controller_epoch;cursor.last_fencing_token=f.fencing_token;Ok(())
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct ActualSnapshot {
 pub actual_start_epoch:Option<i64>,pub actual_finish_epoch:Option<i64>,pub actual_cost:f64,
 pub agent_uuid:Option<Uuid>,pub model_profile_uuid:Option<Uuid>,pub skills:Vec<String>,
 pub dependency_hash:String,pub quality_score:Option<f64>,
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct VarianceRecord {
 pub variance_uuid:Uuid,pub project_uuid:Uuid,pub work_item_uuid:Uuid,pub plan_revision_uuid:Uuid,
 pub source_state_sha256:String,pub schedule_finish_delta_seconds:Option<i64>,pub cost_delta:f64,
 pub agent_changed:bool,pub model_changed:bool,pub skills_changed:bool,pub dependency_changed:bool,
 pub quality_floor_missed:bool,pub reason_status:ReasonStatus,pub reason_code:Option<String>,pub reason_evidence_sha256:Option<String>,pub variance_sha256:String,
}

pub fn reconcile(plan:&PlannedWorkItem,actual:&ActualSnapshot,last_reason:Option<&ExecutionFact>)->Result<VarianceRecord,LedgerError>{
 validate_hash(&plan.source_state_sha256)?;validate_hash(&plan.plan_sha256)?;validate_hash(&plan.planned_dependency_hash)?;
 let schedule_finish_delta_seconds=actual.actual_finish_epoch.map(|x|x-plan.planned_finish_epoch);
 let cost_delta=actual.actual_cost-plan.planned_cost;
 let agent_changed=actual.agent_uuid!=plan.planned_agent_uuid;
 let model_changed=actual.model_profile_uuid!=plan.planned_model_profile_uuid;
 let mut a=actual.skills.clone();let mut p=plan.planned_skills.clone();a.sort();p.sort();let skills_changed=a!=p;
 let dependency_changed=actual.dependency_hash!=plan.planned_dependency_hash;
 let quality_floor_missed=actual.quality_score.map(|q|q<plan.planned_quality_floor).unwrap_or(false);
 let changed=schedule_finish_delta_seconds.unwrap_or(0)!=0 || cost_delta.abs()>f64::EPSILON || agent_changed||model_changed||skills_changed||dependency_changed||quality_floor_missed;
 let (reason_status,reason_code,reason_evidence_sha256)=if !changed {(ReasonStatus::Verified,Some("no_variance".into()),None)} else if let Some(f)=last_reason {
   match f.reason_status { ReasonStatus::Verified => (ReasonStatus::Verified,f.reason_code.clone(),Some(f.evidence_sha256.clone())), ReasonStatus::Unverified => (ReasonStatus::Unverified,f.reason_code.clone(),Some(f.evidence_sha256.clone())), ReasonStatus::Missing => (ReasonStatus::Missing,None,None) }
 } else {(ReasonStatus::Missing,None,None)};
 #[derive(Serialize)] struct H<'a>{p:Uuid,w:Uuid,r:Uuid,s:&'a str,sd:Option<i64>,cd:f64,a:bool,m:bool,sk:bool,d:bool,q:bool,rc:&'a Option<String>}
 let variance_sha256=canonical_sha256(&H{p:plan.project_uuid,w:plan.work_item_uuid,r:plan.plan_revision_uuid,s:&plan.source_state_sha256,sd:schedule_finish_delta_seconds,cd:cost_delta,a:agent_changed,m:model_changed,sk:skills_changed,d:dependency_changed,q:quality_floor_missed,rc:&reason_code});
 Ok(VarianceRecord{variance_uuid:Uuid::now_v7(),project_uuid:plan.project_uuid,work_item_uuid:plan.work_item_uuid,plan_revision_uuid:plan.plan_revision_uuid,source_state_sha256:plan.source_state_sha256.clone(),schedule_finish_delta_seconds,cost_delta,agent_changed,model_changed,skills_changed,dependency_changed,quality_floor_missed,reason_status,reason_code,reason_evidence_sha256,variance_sha256})
}

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub enum GateResult { Pass, Fail, Unavailable }
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct SprintGateEvidence { pub sprint_id:String,pub gate_id:String,pub source_state_sha256:String,pub result:GateResult,pub evidence_sha256:String,pub executed_at_epoch:i64,pub tool_version:String }
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub enum SprintColor { Green, Yellow, Red }
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct SprintGateStatus { pub sprint_id:String,pub color:SprintColor,pub missing_gates:Vec<String>,pub failing_gates:Vec<String>,pub unavailable_gates:Vec<String>,pub evidence_sha256:String }

pub fn reconcile_sprint_gates(sprint_id:&str,required:&[String],source_state:&str,evidence:&[SprintGateEvidence])->Result<SprintGateStatus,LedgerError>{
 validate_hash(source_state)?;let req:BTreeSet<_>=required.iter().cloned().collect();let mut pass=BTreeSet::new();let mut fail=Vec::new();let mut unavailable=Vec::new();
 for e in evidence.iter().filter(|e|e.sprint_id==sprint_id){if e.source_state_sha256!=source_state{continue}validate_hash(&e.evidence_sha256)?;match e.result{GateResult::Pass=>{pass.insert(e.gate_id.clone());},GateResult::Fail=>fail.push(e.gate_id.clone()),GateResult::Unavailable=>unavailable.push(e.gate_id.clone())}}
 let missing=req.difference(&pass).filter(|g|!fail.contains(*g)&&!unavailable.contains(*g)).cloned().collect::<Vec<_>>();
 let color=if fail.is_empty()&&unavailable.is_empty()&&missing.is_empty()&&req.iter().all(|g|pass.contains(g)){SprintColor::Green}else{SprintColor::Yellow};
 #[derive(Serialize)]struct H<'a>{s:&'a str,c:&'a SprintColor,m:&'a [String],f:&'a [String],u:&'a [String],state:&'a str}
 let evidence_sha256=canonical_sha256(&H{s:sprint_id,c:&color,m:&missing,f:&fail,u:&unavailable,state:source_state});
 Ok(SprintGateStatus{sprint_id:sprint_id.into(),color,missing_gates:missing,failing_gates:fail,unavailable_gates:unavailable,evidence_sha256})
}

pub fn assert_no_direct_mutation()->Result<(),LedgerError>{if DIRECT_MUTATION_ALLOWED{Err(LedgerError::DirectMutationForbidden)}else{Ok(())}
}
fn validate_hash(s:&str)->Result<(),LedgerError>{if s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit()&&(!b.is_ascii_alphabetic()||b.is_ascii_lowercase())){Ok(())}else{Err(LedgerError::InvalidHash)}}
pub fn canonical_sha256<T:Serialize>(x:&T)->String{let v=serde_json::to_value(x).unwrap();let b=serde_json::to_vec(&sort_json(v)).unwrap();let mut h=Sha256::new();h.update(b);format!("{:x}",h.finalize())}
fn sort_json(v:serde_json::Value)->serde_json::Value{match v{serde_json::Value::Object(m)=>{let mut b=BTreeMap::new();for(k,v)in m{b.insert(k,sort_json(v));}serde_json::Value::Object(b.into_iter().collect())},serde_json::Value::Array(a)=>serde_json::Value::Array(a.into_iter().map(sort_json).collect()),x=>x}}

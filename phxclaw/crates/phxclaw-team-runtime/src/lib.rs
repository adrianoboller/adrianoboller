#![forbid(unsafe_code)]
use chrono::{DateTime, Duration, Utc};
use phxclaw_evidence_ledger::{EvidenceDraft, EvidenceLedger, EvidenceOutcome, LedgerError};
use phxclaw_live_bus::{LiveBusError, LiveEventHub};
use phxclaw_types::new_uuid_v7;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::{BTreeMap, BTreeSet}, sync::{Arc, Mutex}};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamState { Planned, Running, Draining, Succeeded, Failed, Cancelled }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamTaskState { Pending, Running, Succeeded, Failed, Cancelled }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamTask {
    pub uuid: Uuid,
    pub name: String,
    pub capability: String,
    pub depends_on: Vec<Uuid>,
    pub priority: i32,
    pub state: TeamTaskState,
    pub worker_id: Option<String>,
    pub fencing_token: u64,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub heartbeat_at: Option<DateTime<Utc>>,
    pub attempts: u32,
    pub cancel_requested: bool,
    pub output: Option<Value>,
}

impl TeamTask {
    pub fn new(name: impl Into<String>, capability: impl Into<String>, depends_on: Vec<Uuid>) -> Self {
        Self { uuid: new_uuid_v7(), name: name.into(), capability: capability.into(), depends_on, priority: 0,
            state: TeamTaskState::Pending, worker_id: None, fencing_token: 0, lease_expires_at: None,
            heartbeat_at: None, attempts: 0, cancel_requested: false, output: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamSession {
    pub uuid: Uuid,
    pub correlation_uuid: Uuid,
    pub name: String,
    pub state: TeamState,
    pub max_parallelism: usize,
    pub tasks: BTreeMap<Uuid, TeamTask>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TeamSession {
    pub fn new(name: impl Into<String>, max_parallelism: usize) -> Self {
        Self { uuid: new_uuid_v7(), correlation_uuid: new_uuid_v7(), name: name.into(), state: TeamState::Planned,
            max_parallelism: max_parallelism.max(1), tasks: BTreeMap::new(), created_at: Utc::now(), updated_at: Utc::now() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseGrant {
    pub team_uuid: Uuid,
    pub task_uuid: Uuid,
    pub worker_id: String,
    pub fencing_token: u64,
    pub lease_expires_at: DateTime<Utc>,
    pub capability: String,
}

#[derive(Debug, Error)]
pub enum TeamRuntimeError {
    #[error("team runtime lock poisoned")] Poisoned,
    #[error("unknown team {0}")] UnknownTeam(Uuid),
    #[error("unknown task {0}")] UnknownTask(Uuid),
    #[error("task is not owned by worker/token")] StaleFence,
    #[error("task is cancelled")] Cancelled,
    #[error("dependency graph contains missing task {0}")] MissingDependency(Uuid),
    #[error(transparent)] LiveBus(#[from] LiveBusError),
    #[error(transparent)] Evidence(#[from] LedgerError),
}

#[derive(Clone)]
pub struct TeamRuntime {
    teams: Arc<Mutex<BTreeMap<Uuid, TeamSession>>>,
    live_bus: LiveEventHub,
    evidence: EvidenceLedger,
}

impl TeamRuntime {
    pub fn new(live_bus: LiveEventHub, evidence: EvidenceLedger) -> Self {
        Self { teams: Arc::new(Mutex::new(BTreeMap::new())), live_bus, evidence }
    }

    pub fn create(&self, mut team: TeamSession) -> Result<Uuid, TeamRuntimeError> {
        validate_dependencies(&team)?;
        team.state = TeamState::Running;
        team.updated_at = Utc::now();
        let uuid = team.uuid; let correlation = team.correlation_uuid;
        self.teams.lock().map_err(|_| TeamRuntimeError::Poisoned)?.insert(uuid, team);
        self.live_bus.publish_json("team.runtime", "created", json!({"team_uuid": uuid}), Some(correlation), None)?;
        self.evidence.append(EvidenceDraft { action_uuid: uuid, correlation_uuid: Some(correlation), actor: "Master Orchestrator".into(),
            capability: "team.session.create".into(), action: "team_created".into(), outcome: EvidenceOutcome::Succeeded,
            request_summary: json!({"team_uuid": uuid}), result_summary: json!({"state":"running"}), artifact_uris: vec![] })?;
        Ok(uuid)
    }

    pub fn claim_ready(&self, team_uuid: Uuid, worker_id: &str, lease_seconds: i64) -> Result<Option<LeaseGrant>, TeamRuntimeError> {
        let mut teams = self.teams.lock().map_err(|_| TeamRuntimeError::Poisoned)?;
        let team = teams.get_mut(&team_uuid).ok_or(TeamRuntimeError::UnknownTeam(team_uuid))?;
        if matches!(team.state, TeamState::Cancelled | TeamState::Failed | TeamState::Succeeded) { return Ok(None); }
        let succeeded: BTreeSet<Uuid> = team.tasks.iter().filter_map(|(id,t)| (t.state==TeamTaskState::Succeeded).then_some(*id)).collect();
        let running = team.tasks.values().filter(|t| t.state==TeamTaskState::Running).count();
        if running >= team.max_parallelism { return Ok(None); }
        let mut ready = team.tasks.values().filter(|t| t.state==TeamTaskState::Pending && !t.cancel_requested && t.depends_on.iter().all(|d| succeeded.contains(d))).map(|t| (t.priority,t.uuid)).collect::<Vec<_>>();
        ready.sort_by(|a,b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        let Some((_,task_uuid))=ready.first().copied() else { return Ok(None); };
        let task=team.tasks.get_mut(&task_uuid).ok_or(TeamRuntimeError::UnknownTask(task_uuid))?;
        task.state=TeamTaskState::Running; task.worker_id=Some(worker_id.to_string()); task.fencing_token=task.fencing_token.saturating_add(1);
        task.attempts=task.attempts.saturating_add(1); task.heartbeat_at=Some(Utc::now()); task.lease_expires_at=Some(Utc::now()+Duration::seconds(lease_seconds.max(1))); team.updated_at=Utc::now();
        let grant=LeaseGrant{team_uuid,task_uuid,worker_id:worker_id.to_string(),fencing_token:task.fencing_token,lease_expires_at:task.lease_expires_at.unwrap(),capability:task.capability.clone()};
        self.live_bus.publish_json("team.task", "claimed", serde_json::to_value(&grant).unwrap_or_else(|_|json!({})), Some(team.correlation_uuid), None)?;
        Ok(Some(grant))
    }

    pub fn heartbeat(&self, grant:&LeaseGrant, extend_seconds:i64)->Result<(),TeamRuntimeError>{
        let mut teams=self.teams.lock().map_err(|_|TeamRuntimeError::Poisoned)?; let team=teams.get_mut(&grant.team_uuid).ok_or(TeamRuntimeError::UnknownTeam(grant.team_uuid))?;
        let task=team.tasks.get_mut(&grant.task_uuid).ok_or(TeamRuntimeError::UnknownTask(grant.task_uuid))?; check_fence(task,grant)?;
        if task.cancel_requested { return Err(TeamRuntimeError::Cancelled); }
        task.heartbeat_at=Some(Utc::now()); task.lease_expires_at=Some(Utc::now()+Duration::seconds(extend_seconds.max(1))); team.updated_at=Utc::now(); Ok(())
    }

    pub fn succeed(&self, grant:&LeaseGrant, output:Value)->Result<(),TeamRuntimeError>{ self.finish(grant,TeamTaskState::Succeeded,Some(output)) }
    pub fn fail(&self, grant:&LeaseGrant, output:Value)->Result<(),TeamRuntimeError>{ self.finish(grant,TeamTaskState::Failed,Some(output)) }

    fn finish(&self,grant:&LeaseGrant,state:TeamTaskState,output:Option<Value>)->Result<(),TeamRuntimeError>{
        let mut teams=self.teams.lock().map_err(|_|TeamRuntimeError::Poisoned)?; let team=teams.get_mut(&grant.team_uuid).ok_or(TeamRuntimeError::UnknownTeam(grant.team_uuid))?;
        let task=team.tasks.get_mut(&grant.task_uuid).ok_or(TeamRuntimeError::UnknownTask(grant.task_uuid))?; check_fence(task,grant)?;
        if task.cancel_requested { task.state=TeamTaskState::Cancelled; return Err(TeamRuntimeError::Cancelled); }
        task.state=state.clone(); task.output=output; task.worker_id=None; task.lease_expires_at=None; team.updated_at=Utc::now();
        if team.tasks.values().all(|t| t.state==TeamTaskState::Succeeded){team.state=TeamState::Succeeded;}
        else if team.tasks.values().any(|t| t.state==TeamTaskState::Failed){team.state=TeamState::Failed;}
        self.live_bus.publish_json("team.task", if state==TeamTaskState::Succeeded{"succeeded"}else{"failed"}, json!({"team_uuid":grant.team_uuid,"task_uuid":grant.task_uuid,"fencing_token":grant.fencing_token}), Some(team.correlation_uuid), None)?; Ok(())
    }

    pub fn cancel(&self, team_uuid:Uuid)->Result<(),TeamRuntimeError>{
        let mut teams=self.teams.lock().map_err(|_|TeamRuntimeError::Poisoned)?; let team=teams.get_mut(&team_uuid).ok_or(TeamRuntimeError::UnknownTeam(team_uuid))?;
        team.state=TeamState::Cancelled; for task in team.tasks.values_mut(){ if task.state==TeamTaskState::Pending {task.state=TeamTaskState::Cancelled;} else if task.state==TeamTaskState::Running {task.cancel_requested=true;} } team.updated_at=Utc::now();
        self.live_bus.publish_json("team.runtime","cancelled",json!({"team_uuid":team_uuid}),Some(team.correlation_uuid),None)?; Ok(())
    }

    pub fn recover_expired(&self, now:DateTime<Utc>)->Result<Vec<Uuid>,TeamRuntimeError>{
        let mut teams=self.teams.lock().map_err(|_|TeamRuntimeError::Poisoned)?; let mut recovered=Vec::new();
        for team in teams.values_mut(){ if !matches!(team.state,TeamState::Running|TeamState::Draining){continue;} for task in team.tasks.values_mut(){ if task.state==TeamTaskState::Running && task.lease_expires_at.is_some_and(|x|x<=now){ task.state=TeamTaskState::Pending; task.worker_id=None; task.lease_expires_at=None; task.heartbeat_at=None; task.fencing_token=task.fencing_token.saturating_add(1); recovered.push(task.uuid); } } team.updated_at=Utc::now(); }
        Ok(recovered)
    }

    pub fn snapshot(&self, team_uuid:Uuid)->Result<TeamSession,TeamRuntimeError>{ self.teams.lock().map_err(|_|TeamRuntimeError::Poisoned)?.get(&team_uuid).cloned().ok_or(TeamRuntimeError::UnknownTeam(team_uuid)) }
}

fn check_fence(task:&TeamTask, grant:&LeaseGrant)->Result<(),TeamRuntimeError>{ if task.state!=TeamTaskState::Running || task.worker_id.as_deref()!=Some(grant.worker_id.as_str()) || task.fencing_token!=grant.fencing_token {Err(TeamRuntimeError::StaleFence)} else {Ok(())} }
fn validate_dependencies(team:&TeamSession)->Result<(),TeamRuntimeError>{ for task in team.tasks.values(){ for dep in &task.depends_on { if !team.tasks.contains_key(dep){ return Err(TeamRuntimeError::MissingDependency(*dep)); } } } Ok(()) }

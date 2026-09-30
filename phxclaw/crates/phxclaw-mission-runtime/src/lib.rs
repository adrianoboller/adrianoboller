use chrono::{DateTime, Utc};
use phxclaw_agent_catalog::{AgentCatalog, AgentManifest};
use phxclaw_code_workspace::{CodeWorkspace, GateResult, GateSpec, WorktreeHandle, WorkspaceError};
use phxclaw_evidence_ledger::{EvidenceDraft, EvidenceLedger, EvidenceOutcome, LedgerError};
use phxclaw_live_bus::{LiveBusError, LiveEventHub};
use phxclaw_types::{new_uuid_v7, PermissionClaim};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{path::{Path, PathBuf}, sync::Arc};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MissionState {
    Planned,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionSpec {
    pub uuid: Uuid,
    pub correlation_uuid: Uuid,
    pub objective: String,
    pub project_root: PathBuf,
    pub base_ref: String,
    pub isolate_worktree: bool,
    pub require_clean_start: bool,
    pub steps: Vec<MissionStep>,
    pub created_at: DateTime<Utc>,
}

impl MissionSpec {
    pub fn new(objective: impl Into<String>, project_root: impl Into<PathBuf>) -> Self {
        Self {
            uuid: new_uuid_v7(),
            correlation_uuid: new_uuid_v7(),
            objective: objective.into(),
            project_root: project_root.into(),
            base_ref: "HEAD".into(),
            isolate_worktree: true,
            require_clean_start: true,
            steps: Vec::new(),
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionStep {
    pub uuid: Uuid,
    pub name: String,
    pub agent_capability: String,
    pub action: MissionAction,
    pub required: bool,
}

impl MissionStep {
    pub fn new(name: impl Into<String>, agent_capability: impl Into<String>, action: MissionAction) -> Self {
        Self {
            uuid: new_uuid_v7(),
            name: name.into(),
            agent_capability: agent_capability.into(),
            action,
            required: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MissionAction {
    Snapshot,
    WriteFile { path: PathBuf, contents: String },
    CommandGate { gate: GateSpec },
    Diff,
    ModelSynthesis { capability: String, input: Value },
    ExtensionCall { capability: String, input: Value },
    TeamDispatch { plan: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_uuid: Uuid,
    pub agent_uuid: Uuid,
    pub agent_name: String,
    pub success: bool,
    pub output: Value,
    pub evidence_uuid: Uuid,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionReport {
    pub mission_uuid: Uuid,
    pub correlation_uuid: Uuid,
    pub objective: String,
    pub state: MissionState,
    pub worktree: Option<WorktreeHandle>,
    pub steps: Vec<StepResult>,
    pub final_diff: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum MissionError {
    #[error("mission contains no steps")]
    EmptyMission,
    #[error("project must start clean, but git status is dirty")]
    DirtyWorkspace,
    #[error("no agent provides required capability: {0}")]
    NoAgent(String),
    #[error("real model execution is required for capability {0}; no model executor is attached")]
    ModelExecutorRequired(String),
    #[error("real extension execution is required for capability {0}; no extension executor is attached")]
    ExtensionExecutorRequired(String),
    #[error("model execution failed: {0}")]
    ModelExecution(String),
    #[error("extension execution failed: {0}")]
    ExtensionExecution(String),
    #[error("real team execution is required; no team executor is attached")]
    TeamExecutorRequired,
    #[error("team execution failed: {0}")]
    TeamExecution(String),
    #[error("required mission step failed: {0}")]
    RequiredStepFailed(String),
    #[error("agent permission denied: {0}")]
    PermissionDenied(String),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Evidence(#[from] LedgerError),
    #[error(transparent)]
    LiveBus(#[from] LiveBusError),
}

pub trait ModelExecutor: Send + Sync {
    fn execute_model(&self, capability: &str, input: &Value, correlation_uuid: Uuid, actor: &str) -> Result<Value, String>;
}

pub trait ExtensionExecutor: Send + Sync {
    fn execute_extension(&self, capability: &str, input: &Value, correlation_uuid: Uuid, actor: &str) -> Result<Value, String>;
}

pub trait TeamExecutor: Send + Sync {
    fn execute_team(&self, plan: &Value, correlation_uuid: Uuid, actor: &str) -> Result<Value, String>;
}

pub struct MissionRuntime {
    agents: AgentCatalog,
    live_bus: LiveEventHub,
    evidence: EvidenceLedger,
    model_executor: Option<Arc<dyn ModelExecutor>>,
    extension_executor: Option<Arc<dyn ExtensionExecutor>>,
    team_executor: Option<Arc<dyn TeamExecutor>>,
}

impl MissionRuntime {
    pub fn new(agents: AgentCatalog, live_bus: LiveEventHub, evidence: EvidenceLedger) -> Self {
        Self { agents, live_bus, evidence, model_executor: None, extension_executor: None, team_executor: None }
    }

    pub fn with_model_executor(mut self, executor: Arc<dyn ModelExecutor>) -> Self { self.model_executor = Some(executor); self }

    pub fn with_extension_executor(mut self, executor: Arc<dyn ExtensionExecutor>) -> Self { self.extension_executor = Some(executor); self }

    pub fn with_team_executor(mut self, executor: Arc<dyn TeamExecutor>) -> Self { self.team_executor = Some(executor); self }

    pub fn run(&self, workspace: &CodeWorkspace, mission: &MissionSpec) -> Result<MissionReport, MissionError> {
        if mission.steps.is_empty() { return Err(MissionError::EmptyMission); }
        let started_at = Utc::now();
        let start_snapshot = workspace.snapshot()?;
        if mission.require_clean_start && !start_snapshot.status_porcelain.trim().is_empty() {
            return Err(MissionError::DirtyWorkspace);
        }
        let started_event = self.live_bus.publish_json(
            "mission.runtime", "started",
            json!({"mission_uuid": mission.uuid, "objective_sha256": sha256_text(&mission.objective)}),
            Some(mission.correlation_uuid), None,
        )?;
        self.evidence.append(EvidenceDraft {
            action_uuid: mission.uuid,
            correlation_uuid: Some(mission.correlation_uuid),
            actor: "Master Orchestrator".into(),
            capability: "mission.run".into(),
            action: "mission_started".into(),
            outcome: EvidenceOutcome::Succeeded,
            request_summary: json!({"objective_sha256": sha256_text(&mission.objective), "steps": mission.steps.len()}),
            result_summary: json!({"head": start_snapshot.head, "branch": start_snapshot.branch}),
            artifact_uris: vec![],
        })?;

        let worktree = if mission.isolate_worktree {
            Some(workspace.create_worktree(mission.uuid, &mission.base_ref)?)
        } else { None };
        let execution_root = worktree.as_ref().map(|w| w.path.as_path()).unwrap_or(workspace.root());
        let mut results = Vec::new();

        for step in &mission.steps {
            let agent = self.resolve_agent(&step.agent_capability)?;
            let step_event = self.live_bus.publish_json(
                "mission.step", "started",
                json!({"mission_uuid": mission.uuid, "step_uuid": step.uuid, "agent_uuid": agent.uuid, "agent": agent.name}),
                Some(mission.correlation_uuid), Some(started_event.uuid),
            )?;
            let permission_name = action_capability(&step.action);
            let outcome = agent.authorize(&[PermissionClaim {
                name: permission_name.to_string(),
                scope: execution_root.display().to_string(),
            }])
            .map_err(|error| MissionError::PermissionDenied(error.to_string()))
            .and_then(|_| self.execute_step(workspace, execution_root, step, mission.correlation_uuid, &agent.name));
            match outcome {
                Ok(output) => {
                    let evidence = self.evidence.append(EvidenceDraft {
                        action_uuid: step.uuid,
                        correlation_uuid: Some(mission.correlation_uuid),
                        actor: agent.name.clone(),
                        capability: step.agent_capability.clone(),
                        action: action_name(&step.action).into(),
                        outcome: EvidenceOutcome::Succeeded,
                        request_summary: safe_action_summary(&step.action),
                        result_summary: output.clone(),
                        artifact_uris: vec![],
                    })?;
                    self.live_bus.publish_json(
                        "mission.step", "succeeded",
                        json!({"mission_uuid": mission.uuid, "step_uuid": step.uuid, "evidence_uuid": evidence.uuid}),
                        Some(mission.correlation_uuid), Some(step_event.uuid),
                    )?;
                    results.push(StepResult {
                        step_uuid: step.uuid,
                        agent_uuid: agent.uuid,
                        agent_name: agent.name.clone(),
                        success: true,
                        output,
                        evidence_uuid: evidence.uuid,
                        finished_at: Utc::now(),
                    });
                }
                Err(error) => {
                    let evidence = self.evidence.append(EvidenceDraft {
                        action_uuid: step.uuid,
                        correlation_uuid: Some(mission.correlation_uuid),
                        actor: agent.name.clone(),
                        capability: step.agent_capability.clone(),
                        action: action_name(&step.action).into(),
                        outcome: EvidenceOutcome::Failed,
                        request_summary: safe_action_summary(&step.action),
                        result_summary: json!({"error": error.to_string()}),
                        artifact_uris: vec![],
                    })?;
                    self.live_bus.publish_json(
                        "mission.step", "failed",
                        json!({"mission_uuid": mission.uuid, "step_uuid": step.uuid, "evidence_uuid": evidence.uuid, "error": error.to_string()}),
                        Some(mission.correlation_uuid), Some(step_event.uuid),
                    )?;
                    results.push(StepResult {
                        step_uuid: step.uuid,
                        agent_uuid: agent.uuid,
                        agent_name: agent.name.clone(),
                        success: false,
                        output: json!({"error": error.to_string()}),
                        evidence_uuid: evidence.uuid,
                        finished_at: Utc::now(),
                    });
                    if step.required {
                        self.live_bus.publish_json(
                            "mission.runtime", "failed",
                            json!({"mission_uuid": mission.uuid, "step_uuid": step.uuid}),
                            Some(mission.correlation_uuid), Some(step_event.uuid),
                        )?;
                        return Err(MissionError::RequiredStepFailed(step.name.clone()));
                    }
                }
            }
        }

        let final_diff = workspace.diff(Some(execution_root))?;
        let finished_at = Utc::now();
        self.live_bus.publish_json(
            "mission.runtime", "succeeded",
            json!({"mission_uuid": mission.uuid, "steps": results.len(), "diff_sha256": sha256_text(&final_diff)}),
            Some(mission.correlation_uuid), Some(started_event.uuid),
        )?;
        Ok(MissionReport {
            mission_uuid: mission.uuid,
            correlation_uuid: mission.correlation_uuid,
            objective: mission.objective.clone(),
            state: MissionState::Succeeded,
            worktree,
            steps: results,
            final_diff,
            started_at,
            finished_at,
        })
    }

    fn resolve_agent(&self, capability: &str) -> Result<&AgentManifest, MissionError> {
        self.agents.candidates_for_capability(capability).into_iter().next()
            .ok_or_else(|| MissionError::NoAgent(capability.to_string()))
    }

    fn execute_step(
        &self,
        workspace: &CodeWorkspace,
        execution_root: &Path,
        step: &MissionStep,
        correlation_uuid: Uuid,
        actor: &str,
    ) -> Result<Value, MissionError> {
        match &step.action {
            MissionAction::Snapshot => Ok(serde_json::to_value(workspace.snapshot()?).expect("snapshot serializes")),
            MissionAction::WriteFile { path, contents } => {
                let result = workspace.write_file(execution_root, path, contents.as_bytes())?;
                Ok(serde_json::to_value(result).expect("file write serializes"))
            }
            MissionAction::CommandGate { gate } => {
                let result: GateResult = workspace.run_gate(gate, execution_root)?;
                if !result.passed {
                    return Err(MissionError::RequiredStepFailed(format!("gate {}", gate.name)));
                }
                Ok(serde_json::to_value(result).expect("gate serializes"))
            }
            MissionAction::Diff => Ok(json!({"diff": workspace.diff(Some(execution_root))?})),
            MissionAction::ModelSynthesis { capability, input } => {
                let executor = self.model_executor.as_ref().ok_or_else(|| MissionError::ModelExecutorRequired(capability.clone()))?;
                executor.execute_model(capability, input, correlation_uuid, actor).map_err(MissionError::ModelExecution)
            }
            MissionAction::ExtensionCall { capability, input } => {
                let executor = self.extension_executor.as_ref().ok_or_else(|| MissionError::ExtensionExecutorRequired(capability.clone()))?;
                executor.execute_extension(capability, input, correlation_uuid, actor).map_err(MissionError::ExtensionExecution)
            },
            MissionAction::TeamDispatch { plan } => {
                let executor = self.team_executor.as_ref().ok_or(MissionError::TeamExecutorRequired)?;
                executor.execute_team(plan, correlation_uuid, actor).map_err(MissionError::TeamExecution)
            },
        }
    }
}

fn action_capability(action: &MissionAction) -> &str {
    match action {
        MissionAction::Snapshot => "workspace.snapshot",
        MissionAction::WriteFile { .. } => "workspace.file.write",
        MissionAction::CommandGate { .. } => "workspace.gate.run",
        MissionAction::Diff => "workspace.diff",
        MissionAction::ModelSynthesis { capability, .. } => capability,
        MissionAction::ExtensionCall { capability, .. } => capability,
        MissionAction::TeamDispatch { .. } => "team.dispatch",
    }
}

fn action_name(action: &MissionAction) -> &'static str {
    match action {
        MissionAction::Snapshot => "workspace_snapshot",
        MissionAction::WriteFile { .. } => "workspace_file_write",
        MissionAction::CommandGate { .. } => "workspace_command_gate",
        MissionAction::Diff => "workspace_diff",
        MissionAction::ModelSynthesis { .. } => "model_synthesis",
        MissionAction::ExtensionCall { .. } => "extension_call",
        MissionAction::TeamDispatch { .. } => "team_dispatch",
    }
}

fn safe_action_summary(action: &MissionAction) -> Value {
    match action {
        MissionAction::Snapshot => json!({"kind": "snapshot"}),
        MissionAction::WriteFile { path, contents } => json!({
            "kind": "write_file", "path": path, "content_sha256": sha256_text(contents), "bytes": contents.len()
        }),
        MissionAction::CommandGate { gate } => json!({"kind": "command_gate", "name": gate.name, "program": gate.program, "args": gate.args}),
        MissionAction::Diff => json!({"kind": "diff"}),
        MissionAction::ModelSynthesis { capability, input } => json!({"kind": "model_synthesis", "capability": capability, "input_sha256": sha256_text(&input.to_string())}),
        MissionAction::ExtensionCall { capability, input } => json!({"kind": "extension_call", "capability": capability, "input_sha256": sha256_text(&input.to_string())}),
        MissionAction::TeamDispatch { plan } => json!({"kind":"team_dispatch","plan_sha256":sha256_text(&plan.to_string())}),
    }
}

fn sha256_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

pub struct PostgresMissionJournal;

impl PostgresMissionJournal {
    pub fn persist_report(
        client: &mut postgres::Client,
        mission: &MissionSpec,
        report: &MissionReport,
    ) -> Result<(), postgres::Error> {
        let mut tx = client.transaction()?;
        tx.execute(
            "INSERT INTO phoenix_missions \
             (uuid, correlation_uuid, objective_sha256, project_root, base_ref, state, created_at, started_at, finished_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) \
             ON CONFLICT (uuid) DO UPDATE SET state=EXCLUDED.state, started_at=EXCLUDED.started_at, finished_at=EXCLUDED.finished_at",
            &[
                &mission.uuid,
                &mission.correlation_uuid,
                &sha256_text(&mission.objective),
                &mission.project_root.to_string_lossy().to_string(),
                &mission.base_ref,
                &format!("{:?}", report.state).to_ascii_lowercase(),
                &mission.created_at,
                &report.started_at,
                &report.finished_at,
            ],
        )?;
        for step in &report.steps {
            let spec = mission.steps.iter().find(|item| item.uuid == step.step_uuid).expect("report step belongs to mission");
            tx.execute(
                "INSERT INTO phoenix_mission_steps \
                 (uuid, mission_uuid, name, agent_capability, action, required, status, agent_uuid, agent_name, evidence_uuid, output, finished_at) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) \
                 ON CONFLICT (uuid) DO UPDATE SET status=EXCLUDED.status, agent_uuid=EXCLUDED.agent_uuid, agent_name=EXCLUDED.agent_name, evidence_uuid=EXCLUDED.evidence_uuid, output=EXCLUDED.output, finished_at=EXCLUDED.finished_at",
                &[
                    &step.step_uuid,
                    &mission.uuid,
                    &spec.name,
                    &spec.agent_capability,
                    &serde_json::to_value(&spec.action).expect("action serializes"),
                    &spec.required,
                    &if step.success { "succeeded" } else { "failed" },
                    &step.agent_uuid,
                    &step.agent_name,
                    &step.evidence_uuid,
                    &step.output,
                    &step.finished_at,
                ],
            )?;
        }
        tx.commit()
    }
}

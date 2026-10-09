/// Troca atomica, trava entre processos e cauda cortada: a escrita em disco de toda a base.
pub mod arquivo;
/// O nome com cara de segredo: a lista unica da base.
pub mod segredo;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub fn new_uuid_v7() -> Uuid {
    Uuid::now_v7()
}

pub fn is_uuid_v7(value: &Uuid) -> bool {
    (value.as_bytes()[6] >> 4) == 7
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceRef {
    pub uuid: Uuid,
    pub uri: String,
    pub source_type: String,
    pub retrieved_at: DateTime<Utc>,
    pub sha256: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchRecord {
    pub uuid: Uuid,
    pub topic: String,
    pub question: String,
    pub evidence: Vec<EvidenceRef>,
    pub findings: Vec<String>,
    pub suggested_hypotheses: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisStatus {
    Proposed,
    Testing,
    Supported,
    Rejected,
    Inconclusive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentPlan {
    pub uuid: Uuid,
    pub steps: Vec<String>,
    pub success_criteria: Vec<String>,
    pub failure_criteria: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    pub uuid: Uuid,
    pub statement: String,
    pub rationale: String,
    pub experiment: ExperimentPlan,
    pub evidence: Vec<EvidenceRef>,
    pub status: HypothesisStatus,
    pub decision_notes: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallAction {
    Install,
    Update,
    Verify,
    Repair,
    Migrate,
    Rollback,
    Backup,
    Restore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPlan {
    pub uuid: Uuid,
    pub action: InstallAction,
    pub target: String,
    pub preflight_checks: Vec<String>,
    pub steps: Vec<String>,
    pub rollback_steps: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginEntrypoint {
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginDependency {
    pub uuid: Uuid,
    pub version: String,
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginPermission {
    pub name: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginLifecycle {
    pub install: String,
    pub enable: String,
    pub disable: String,
    pub uninstall: String,
    pub health: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginContracts {
    pub input_schema: String,
    pub output_schema: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginIntegrity {
    pub artifact: String,
    pub hash_algorithm: String,
    pub digest: String,
    pub signature_algorithm: String,
    pub signature: String,
    pub signer: String,
    pub provenance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginTestSpec {
    pub name: String,
    pub command: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginRollback {
    pub strategy: String,
    pub steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SandboxNetworkMode {
    Deny,
    Allowlist,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginSandboxPolicy {
    pub network: SandboxNetworkMode,
    pub network_allowlist: Vec<String>,
    pub read_only_paths: Vec<String>,
    pub write_paths: Vec<String>,
    pub environment_allowlist: Vec<String>,
    pub timeout_ms: u64,
    pub memory_mb: u64,
    pub cpu_quota_percent: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentProfile {
    pub display_name: String,
    pub role: String,
    pub priority: u16,
    pub accepts: Vec<String>,
    pub emits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelDescriptor {
    pub id: String,
    pub capabilities: Vec<String>,
    pub context_window: u64,
    pub input_microunits_per_million_tokens: u64,
    pub output_microunits_per_million_tokens: u64,
    pub local: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelProviderProfile {
    pub display_name: String,
    pub priority: u16,
    pub transport: String,
    pub models: Vec<ModelDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginExtensionBinding {
    pub point: String,
    pub contract_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    pub manifest_version: String,
    pub uuid: Uuid,
    pub name: String,
    pub version: String,
    pub kind: String,
    pub description: String,
    pub core_api: String,
    pub entrypoint: PluginEntrypoint,
    pub dependencies: Vec<PluginDependency>,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub extension_points: Vec<PluginExtensionBinding>,
    pub permissions: Vec<PluginPermission>,
    pub lifecycle: PluginLifecycle,
    pub contracts: PluginContracts,
    pub sandbox: PluginSandboxPolicy,
    pub integrity: PluginIntegrity,
    pub tests: Vec<PluginTestSpec>,
    pub rollback: PluginRollback,
    #[serde(default)]
    pub agent: Option<AgentProfile>,
    #[serde(default)]
    pub model_provider: Option<ModelProviderProfile>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    Discovered,
    Validated,
    Enabled,
    Disabled,
    Quarantined,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Registered,
    Starting,
    Ready,
    Busy,
    Draining,
    Stopped,
    Failed,
    Quarantined,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionClaim {
    pub name: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub uuid: Uuid,
    pub capability: String,
    pub payload: Value,
    pub requested_permissions: Vec<PermissionClaim>,
    pub created_at: DateTime<Utc>,
}

impl AgentRequest {
    pub fn new(capability: impl Into<String>, payload: Value) -> Self {
        Self {
            uuid: new_uuid_v7(),
            capability: capability.into(),
            payload,
            requested_permissions: Vec::new(),
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub uuid: Uuid,
    pub request_uuid: Uuid,
    pub agent_uuid: Uuid,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_id_is_v7() {
        let id = new_uuid_v7();
        assert!(is_uuid_v7(&id));
    }

    #[test]
    fn agent_request_uses_uuid_v7() {
        let request = AgentRequest::new("research.query", Value::Null);
        assert!(is_uuid_v7(&request.uuid));
    }
}

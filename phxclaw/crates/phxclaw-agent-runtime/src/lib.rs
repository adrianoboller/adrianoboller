use phxclaw_sandbox::{SandboxBackend, SandboxError, SandboxPlan};
use phxclaw_types::{AgentRequest, AgentState, PermissionClaim, PluginManifest};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AgentInstance {
    pub manifest: PluginManifest,
    pub state: AgentState,
}

#[derive(Debug, Clone)]
pub struct RouteDecision {
    pub request_uuid: Uuid,
    pub agent_uuid: Uuid,
    pub agent_name: String,
    pub capability: String,
}

#[derive(Debug, Clone)]
pub struct DispatchPlan {
    pub route: RouteDecision,
    pub sandbox: SandboxPlan,
}

#[derive(Debug, Error)]
pub enum AgentRuntimeError {
    #[error("plugin {0} is not an agent")]
    NotAgent(String),
    #[error("agent already registered: {0}")]
    Duplicate(Uuid),
    #[error("agent not found: {0}")]
    UnknownAgent(Uuid),
    #[error("no ready agent provides capability: {0}")]
    NoRoute(String),
    #[error("permission denied for agent {agent}: {permission}:{scope}")]
    PermissionDenied {
        agent: String,
        permission: String,
        scope: String,
    },
    #[error("sandbox failure: {0}")]
    Sandbox(#[from] SandboxError),
}

pub struct AgentRuntime<B: SandboxBackend> {
    sandbox: B,
    agents: BTreeMap<Uuid, AgentInstance>,
}

impl<B: SandboxBackend> AgentRuntime<B> {
    pub fn new(sandbox: B) -> Self {
        Self {
            sandbox,
            agents: BTreeMap::new(),
        }
    }

    pub fn agents(&self) -> impl Iterator<Item = (&Uuid, &AgentInstance)> {
        self.agents.iter()
    }

    pub fn agent(&self, agent_uuid: &Uuid) -> Option<&AgentInstance> {
        self.agents.get(agent_uuid)
    }

    pub fn register(&mut self, manifest: PluginManifest) -> Result<Uuid, AgentRuntimeError> {
        if manifest.kind != "agent" || manifest.agent.is_none() {
            return Err(AgentRuntimeError::NotAgent(manifest.name));
        }
        if self.agents.contains_key(&manifest.uuid) {
            return Err(AgentRuntimeError::Duplicate(manifest.uuid));
        }
        let id = manifest.uuid;
        self.agents.insert(
            id,
            AgentInstance {
                manifest,
                state: AgentState::Registered,
            },
        );
        Ok(id)
    }

    pub fn start(&mut self, agent_uuid: Uuid) -> Result<(), AgentRuntimeError> {
        self.sandbox.probe()?;
        let instance = self
            .agents
            .get_mut(&agent_uuid)
            .ok_or(AgentRuntimeError::UnknownAgent(agent_uuid))?;
        instance.state = AgentState::Starting;
        if let Err(error) = self.sandbox.plan(&instance.manifest) {
            instance.state = AgentState::Quarantined;
            return Err(error.into());
        }
        instance.state = AgentState::Ready;
        Ok(())
    }

    pub fn stop(&mut self, agent_uuid: Uuid) -> Result<(), AgentRuntimeError> {
        let instance = self
            .agents
            .get_mut(&agent_uuid)
            .ok_or(AgentRuntimeError::UnknownAgent(agent_uuid))?;
        instance.state = AgentState::Draining;
        instance.state = AgentState::Stopped;
        Ok(())
    }

    pub fn route(&self, request: &AgentRequest) -> Result<RouteDecision, AgentRuntimeError> {
        let mut candidates = self
            .agents
            .values()
            .filter(|instance| {
                instance.state == AgentState::Ready
                    && instance
                        .manifest
                        .capabilities
                        .iter()
                        .any(|capability| capability == &request.capability)
            })
            .collect::<Vec<_>>();

        candidates.sort_by(|left, right| {
            let left_priority = left
                .manifest
                .agent
                .as_ref()
                .map(|agent| agent.priority)
                .unwrap_or(0);
            let right_priority = right
                .manifest
                .agent
                .as_ref()
                .map(|agent| agent.priority)
                .unwrap_or(0);
            right_priority
                .cmp(&left_priority)
                .then_with(|| left.manifest.uuid.cmp(&right.manifest.uuid))
        });

        let selected = candidates
            .first()
            .ok_or_else(|| AgentRuntimeError::NoRoute(request.capability.clone()))?;
        self.authorize(&selected.manifest, &request.requested_permissions)?;

        Ok(RouteDecision {
            request_uuid: request.uuid,
            agent_uuid: selected.manifest.uuid,
            agent_name: selected.manifest.name.clone(),
            capability: request.capability.clone(),
        })
    }

    pub fn prepare_dispatch(
        &self,
        request: &AgentRequest,
    ) -> Result<DispatchPlan, AgentRuntimeError> {
        let route = self.route(request)?;
        let instance = self
            .agents
            .get(&route.agent_uuid)
            .ok_or(AgentRuntimeError::UnknownAgent(route.agent_uuid))?;
        let sandbox = self.sandbox.plan(&instance.manifest)?;
        Ok(DispatchPlan { route, sandbox })
    }

    pub fn execute_lifecycle_smoke(
        &mut self,
        request: &AgentRequest,
    ) -> Result<bool, AgentRuntimeError> {
        let route = self.route(request)?;
        let instance = self
            .agents
            .get_mut(&route.agent_uuid)
            .ok_or(AgentRuntimeError::UnknownAgent(route.agent_uuid))?;
        instance.state = AgentState::Busy;
        let result = self.sandbox.execute(&instance.manifest);
        match result {
            Ok(status) => {
                instance.state = AgentState::Ready;
                Ok(status.success())
            }
            Err(error) => {
                instance.state = AgentState::Failed;
                Err(error.into())
            }
        }
    }

    fn authorize(
        &self,
        manifest: &PluginManifest,
        requested: &[PermissionClaim],
    ) -> Result<(), AgentRuntimeError> {
        for claim in requested {
            let allowed = manifest.permissions.iter().any(|permission| {
                permission.name == claim.name
                    && permission
                        .scopes
                        .iter()
                        .any(|scope| scope_matches(scope, &claim.scope))
            });
            if !allowed {
                return Err(AgentRuntimeError::PermissionDenied {
                    agent: manifest.name.clone(),
                    permission: claim.name.clone(),
                    scope: claim.scope.clone(),
                });
            }
        }
        Ok(())
    }
}

fn scope_matches(granted: &str, requested: &str) -> bool {
    if granted == "*" || granted == requested {
        return true;
    }
    granted
        .strip_suffix("/*")
        .is_some_and(|prefix| requested.starts_with(&format!("{prefix}/")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use phxclaw_sandbox::{SandboxError, SandboxPlan};
    use std::{path::PathBuf, process::ExitStatus, time::Duration};

    #[derive(Clone)]
    struct FakeSandbox;

    impl SandboxBackend for FakeSandbox {
        fn name(&self) -> &'static str {
            "fake"
        }

        fn probe(&self) -> Result<(), SandboxError> {
            Ok(())
        }

        fn plan(&self, _manifest: &PluginManifest) -> Result<SandboxPlan, SandboxError> {
            Ok(SandboxPlan {
                program: PathBuf::from("fake"),
                args: vec![],
                timeout: Duration::from_secs(1),
            })
        }

        fn execute(&self, _manifest: &PluginManifest) -> Result<ExitStatus, SandboxError> {
            Err(SandboxError::BackendUnavailable(
                "fake backend does not execute".into(),
            ))
        }
    }

    fn manifest(name: &str) -> PluginManifest {
        let input = match name {
            "morpheus" => {
                include_str!("../../../plugins/builtin/manifests/morpheus.agent.plugin.json")
            }
            "oracle" => include_str!("../../../plugins/builtin/manifests/oracle.agent.plugin.json"),
            _ => include_str!(
                "../../../plugins/builtin/manifests/master-orchestrator.agent.plugin.json"
            ),
        };
        serde_json::from_str(input).unwrap()
    }

    #[test]
    fn routes_deterministically_by_priority() {
        let mut runtime = AgentRuntime::new(FakeSandbox);
        let morpheus = manifest("morpheus");
        let oracle = manifest("oracle");
        let morpheus_id = runtime.register(morpheus).unwrap();
        let oracle_id = runtime.register(oracle).unwrap();
        runtime.start(morpheus_id).unwrap();
        runtime.start(oracle_id).unwrap();

        let request = AgentRequest::new("decision.technical", serde_json::Value::Null);
        let decision = runtime.route(&request).unwrap();
        assert_eq!(decision.agent_uuid, oracle_id);
    }

    #[test]
    fn permissions_are_deny_by_default() {
        let mut runtime = AgentRuntime::new(FakeSandbox);
        let manifest = manifest("morpheus");
        let id = runtime.register(manifest).unwrap();
        runtime.start(id).unwrap();

        let mut request = AgentRequest::new("strategy.assess", serde_json::Value::Null);
        request.requested_permissions.push(PermissionClaim {
            name: "filesystem.write".into(),
            scope: "/etc/passwd".into(),
        });
        let error = runtime.route(&request).unwrap_err();
        assert!(error.to_string().contains("permission denied"));
    }
}

// -----------------------------------------------------------------------------
// Logical Agent Runtime
// -----------------------------------------------------------------------------
// The 110 spreadsheet-derived agents are logical roles, not executable process
// plugins. They are routed and authorized here, while concrete execution still
// happens through model providers/tools under the Capability Broker.

use phxclaw_agent_catalog::{AgentCatalog, AgentManifest as CatalogAgentManifest};

#[derive(Debug, Clone)]
pub struct LogicalAgentInstance {
    pub manifest: CatalogAgentManifest,
    pub state: AgentState,
}

#[derive(Debug, Clone)]
pub struct LogicalRouteDecision {
    pub request_uuid: Uuid,
    pub agent_uuid: Uuid,
    pub agent_name: String,
    pub capability: String,
    pub models_allowed: Vec<String>,
    pub knowledge_sources: Vec<String>,
}

#[derive(Debug, Default)]
pub struct LogicalAgentRuntime {
    agents: BTreeMap<Uuid, LogicalAgentInstance>,
}

impl LogicalAgentRuntime {
    pub fn from_catalog(catalog: AgentCatalog) -> Result<Self, AgentRuntimeError> {
        let mut runtime = Self::default();
        for (_, manifest) in catalog.iter() {
            runtime.register(manifest.clone())?;
        }
        Ok(runtime)
    }

    pub fn register(&mut self, manifest: CatalogAgentManifest) -> Result<Uuid, AgentRuntimeError> {
        if self.agents.contains_key(&manifest.uuid) {
            return Err(AgentRuntimeError::Duplicate(manifest.uuid));
        }
        let uuid = manifest.uuid;
        self.agents.insert(
            uuid,
            LogicalAgentInstance {
                manifest,
                state: AgentState::Registered,
            },
        );
        Ok(uuid)
    }

    pub fn start(&mut self, agent_uuid: Uuid) -> Result<(), AgentRuntimeError> {
        let instance = self
            .agents
            .get_mut(&agent_uuid)
            .ok_or(AgentRuntimeError::UnknownAgent(agent_uuid))?;
        instance.state = AgentState::Starting;
        instance.state = AgentState::Ready;
        Ok(())
    }

    pub fn start_all(&mut self) {
        for instance in self.agents.values_mut() {
            instance.state = AgentState::Ready;
        }
    }

    pub fn stop(&mut self, agent_uuid: Uuid) -> Result<(), AgentRuntimeError> {
        let instance = self
            .agents
            .get_mut(&agent_uuid)
            .ok_or(AgentRuntimeError::UnknownAgent(agent_uuid))?;
        instance.state = AgentState::Draining;
        instance.state = AgentState::Stopped;
        Ok(())
    }

    pub fn agent(&self, agent_uuid: &Uuid) -> Option<&LogicalAgentInstance> {
        self.agents.get(agent_uuid)
    }

    pub fn agent_by_name(&self, name: &str) -> Option<&LogicalAgentInstance> {
        self.agents
            .values()
            .find(|instance| instance.manifest.name == name)
    }

    pub fn route(&self, request: &AgentRequest) -> Result<LogicalRouteDecision, AgentRuntimeError> {
        let mut candidates = self
            .agents
            .values()
            .filter(|instance| {
                instance.state == AgentState::Ready
                    && instance.manifest.provides(&request.capability)
            })
            .collect::<Vec<_>>();

        candidates.sort_by(|left, right| {
            logical_criticality_rank(&right.manifest.criticality)
                .cmp(&logical_criticality_rank(&left.manifest.criticality))
                .then_with(|| left.manifest.agent_id.cmp(&right.manifest.agent_id))
                .then_with(|| left.manifest.uuid.cmp(&right.manifest.uuid))
        });

        let selected = candidates
            .first()
            .ok_or_else(|| AgentRuntimeError::NoRoute(request.capability.clone()))?;
        selected
            .manifest
            .authorize(&request.requested_permissions)
            .map_err(|_| AgentRuntimeError::PermissionDenied {
                agent: selected.manifest.name.clone(),
                permission: request
                    .requested_permissions
                    .first()
                    .map(|claim| claim.name.clone())
                    .unwrap_or_else(|| "unknown".into()),
                scope: request
                    .requested_permissions
                    .first()
                    .map(|claim| claim.scope.clone())
                    .unwrap_or_else(|| "unknown".into()),
            })?;

        Ok(LogicalRouteDecision {
            request_uuid: request.uuid,
            agent_uuid: selected.manifest.uuid,
            agent_name: selected.manifest.name.clone(),
            capability: request.capability.clone(),
            models_allowed: selected.manifest.models_allowed.clone(),
            knowledge_sources: selected.manifest.knowledge_sources.clone(),
        })
    }

    pub fn route_named(
        &self,
        agent_name: &str,
        request: &AgentRequest,
    ) -> Result<LogicalRouteDecision, AgentRuntimeError> {
        let selected = self.agent_by_name(agent_name).ok_or_else(|| {
            AgentRuntimeError::NoRoute(format!("{} via {}", request.capability, agent_name))
        })?;
        if selected.state != AgentState::Ready || !selected.manifest.provides(&request.capability) {
            return Err(AgentRuntimeError::NoRoute(format!(
                "{} via {}",
                request.capability, agent_name
            )));
        }
        selected
            .manifest
            .authorize(&request.requested_permissions)
            .map_err(|_| AgentRuntimeError::PermissionDenied {
                agent: selected.manifest.name.clone(),
                permission: request
                    .requested_permissions
                    .first()
                    .map(|claim| claim.name.clone())
                    .unwrap_or_else(|| "unknown".into()),
                scope: request
                    .requested_permissions
                    .first()
                    .map(|claim| claim.scope.clone())
                    .unwrap_or_else(|| "unknown".into()),
            })?;
        Ok(LogicalRouteDecision {
            request_uuid: request.uuid,
            agent_uuid: selected.manifest.uuid,
            agent_name: selected.manifest.name.clone(),
            capability: request.capability.clone(),
            models_allowed: selected.manifest.models_allowed.clone(),
            knowledge_sources: selected.manifest.knowledge_sources.clone(),
        })
    }
}

fn logical_criticality_rank(value: &str) -> u8 {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.contains("crít") || normalized.contains("crit") {
        3
    } else if normalized.contains("alta") || normalized.contains("high") {
        2
    } else if normalized.contains("média")
        || normalized.contains("media")
        || normalized.contains("medium")
    {
        1
    } else {
        0
    }
}

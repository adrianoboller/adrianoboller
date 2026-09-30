use chrono::{DateTime, Utc};
use phxclaw_types::{ModelDescriptor, PluginManifest, new_uuid_v7};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DataClassification {
    Public,
    Internal,
    Confidential,
    Restricted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub uuid: Uuid,
    pub capability: String,
    pub input: Value,
    pub input_tokens_estimate: u64,
    pub output_tokens_limit: u64,
    pub max_cost_microunits: u64,
    pub preferred_models: Vec<String>,
    pub requires_local: bool,
    pub classification: DataClassification,
    pub budget_account: String,
    pub created_at: DateTime<Utc>,
}

impl ModelRequest {
    pub fn new(capability: impl Into<String>, input: Value) -> Self {
        Self {
            uuid: new_uuid_v7(),
            capability: capability.into(),
            input,
            input_tokens_estimate: 1_000,
            output_tokens_limit: 2_000,
            max_cost_microunits: u64::MAX,
            preferred_models: Vec::new(),
            requires_local: false,
            classification: DataClassification::Internal,
            budget_account: "default".into(),
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRouteCandidate {
    pub provider_uuid: Uuid,
    pub provider_name: String,
    pub model_id: String,
    pub estimated_cost_microunits: u64,
    pub provider_priority: u16,
    pub preference_rank: usize,
    pub local: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRouteDecision {
    pub request_uuid: Uuid,
    pub selected: ModelRouteCandidate,
    pub fallbacks: Vec<ModelRouteCandidate>,
    pub routed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetAccount {
    pub name: String,
    pub ceiling_microunits: u64,
    pub reserved_microunits: u64,
    pub spent_microunits: u64,
}

impl BudgetAccount {
    pub fn available(&self) -> u64 {
        self.ceiling_microunits
            .saturating_sub(self.reserved_microunits)
            .saturating_sub(self.spent_microunits)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTelemetry {
    pub uuid: Uuid,
    pub request_uuid: Uuid,
    pub provider_uuid: Uuid,
    pub model_id: String,
    pub latency_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_microunits: u64,
    pub success: bool,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum ModelGatewayError {
    #[error("plugin {0} is not a model provider")]
    NotProvider(String),
    #[error("model provider already registered: {0}")]
    DuplicateProvider(Uuid),
    #[error("no model route satisfies capability/policy/budget for request {0}")]
    NoRoute(Uuid),
    #[error("budget account not found: {0}")]
    UnknownBudget(String),
    #[error("budget exceeded for account {account}: requested {requested}, available {available}")]
    BudgetExceeded {
        account: String,
        requested: u64,
        available: u64,
    },
    #[error("budget reservation underflow for account {0}")]
    ReservationUnderflow(String),
}

#[derive(Debug, Clone)]
struct ProviderRegistration {
    manifest: PluginManifest,
}

#[derive(Debug, Default)]
pub struct ModelGateway {
    providers: BTreeMap<Uuid, ProviderRegistration>,
    budgets: BTreeMap<String, BudgetAccount>,
    telemetry: Vec<ModelTelemetry>,
}

impl ModelGateway {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_provider(
        &mut self,
        manifest: PluginManifest,
    ) -> Result<Uuid, ModelGatewayError> {
        if manifest.kind != "model_provider" || manifest.model_provider.is_none() {
            return Err(ModelGatewayError::NotProvider(manifest.name));
        }
        if self.providers.contains_key(&manifest.uuid) {
            return Err(ModelGatewayError::DuplicateProvider(manifest.uuid));
        }
        let uuid = manifest.uuid;
        self.providers
            .insert(uuid, ProviderRegistration { manifest });
        Ok(uuid)
    }

    pub fn set_budget(&mut self, name: impl Into<String>, ceiling_microunits: u64) {
        let name = name.into();
        self.budgets.insert(
            name.clone(),
            BudgetAccount {
                name,
                ceiling_microunits,
                reserved_microunits: 0,
                spent_microunits: 0,
            },
        );
    }

    pub fn budget(&self, name: &str) -> Option<&BudgetAccount> {
        self.budgets.get(name)
    }

    pub fn telemetry(&self) -> &[ModelTelemetry] {
        &self.telemetry
    }

    pub fn route(&self, request: &ModelRequest) -> Result<ModelRouteDecision, ModelGatewayError> {
        let account = self
            .budgets
            .get(&request.budget_account)
            .ok_or_else(|| ModelGatewayError::UnknownBudget(request.budget_account.clone()))?;
        let account_limit = account.available().min(request.max_cost_microunits);
        let mut candidates = Vec::new();

        for registration in self.providers.values() {
            let manifest = &registration.manifest;
            let profile = manifest
                .model_provider
                .as_ref()
                .expect("validated provider profile");
            for model in &profile.models {
                if !model
                    .capabilities
                    .iter()
                    .any(|capability| capability == &request.capability)
                {
                    continue;
                }
                if request.requires_local && !model.local {
                    continue;
                }
                if request.classification == DataClassification::Restricted && !model.local {
                    continue;
                }
                if request
                    .input_tokens_estimate
                    .saturating_add(request.output_tokens_limit)
                    > model.context_window
                {
                    continue;
                }
                let cost = estimate_cost(
                    model,
                    request.input_tokens_estimate,
                    request.output_tokens_limit,
                );
                if cost > account_limit {
                    continue;
                }
                let preference_rank = request
                    .preferred_models
                    .iter()
                    .position(|preferred| preferred == &model.id)
                    .unwrap_or(usize::MAX);
                candidates.push(ModelRouteCandidate {
                    provider_uuid: manifest.uuid,
                    provider_name: manifest.name.clone(),
                    model_id: model.id.clone(),
                    estimated_cost_microunits: cost,
                    provider_priority: profile.priority,
                    preference_rank,
                    local: model.local,
                });
            }
        }

        candidates.sort_by(|left, right| {
            left.preference_rank
                .cmp(&right.preference_rank)
                .then_with(|| right.provider_priority.cmp(&left.provider_priority))
                .then_with(|| {
                    left.estimated_cost_microunits
                        .cmp(&right.estimated_cost_microunits)
                })
                .then_with(|| left.provider_uuid.cmp(&right.provider_uuid))
                .then_with(|| left.model_id.cmp(&right.model_id))
        });

        let selected = candidates
            .first()
            .cloned()
            .ok_or(ModelGatewayError::NoRoute(request.uuid))?;
        let fallbacks = candidates.into_iter().skip(1).collect();
        Ok(ModelRouteDecision {
            request_uuid: request.uuid,
            selected,
            fallbacks,
            routed_at: Utc::now(),
        })
    }

    pub fn reserve_route(
        &mut self,
        request: &ModelRequest,
        decision: &ModelRouteDecision,
    ) -> Result<(), ModelGatewayError> {
        let account = self
            .budgets
            .get_mut(&request.budget_account)
            .ok_or_else(|| ModelGatewayError::UnknownBudget(request.budget_account.clone()))?;
        let requested = decision.selected.estimated_cost_microunits;
        let available = account.available().min(request.max_cost_microunits);
        if requested > available {
            return Err(ModelGatewayError::BudgetExceeded {
                account: request.budget_account.clone(),
                requested,
                available,
            });
        }
        account.reserved_microunits = account.reserved_microunits.saturating_add(requested);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn settle(
        &mut self,
        request: &ModelRequest,
        decision: &ModelRouteDecision,
        actual_cost_microunits: u64,
        latency_ms: u64,
        input_tokens: u64,
        output_tokens: u64,
        success: bool,
    ) -> Result<(), ModelGatewayError> {
        let account = self
            .budgets
            .get_mut(&request.budget_account)
            .ok_or_else(|| ModelGatewayError::UnknownBudget(request.budget_account.clone()))?;
        let reserved = decision.selected.estimated_cost_microunits;
        if account.reserved_microunits < reserved {
            return Err(ModelGatewayError::ReservationUnderflow(
                request.budget_account.clone(),
            ));
        }
        account.reserved_microunits -= reserved;
        account.spent_microunits = account
            .spent_microunits
            .saturating_add(actual_cost_microunits);
        self.telemetry.push(ModelTelemetry {
            uuid: new_uuid_v7(),
            request_uuid: request.uuid,
            provider_uuid: decision.selected.provider_uuid,
            model_id: decision.selected.model_id.clone(),
            latency_ms,
            input_tokens,
            output_tokens,
            cost_microunits: actual_cost_microunits,
            success,
            observed_at: Utc::now(),
        });
        Ok(())
    }
}

pub fn estimate_cost(model: &ModelDescriptor, input_tokens: u64, output_tokens: u64) -> u64 {
    let input = (input_tokens as u128)
        .saturating_mul(model.input_microunits_per_million_tokens as u128)
        / 1_000_000;
    let output = (output_tokens as u128)
        .saturating_mul(model.output_microunits_per_million_tokens as u128)
        / 1_000_000;
    input.saturating_add(output).min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use phxclaw_types::{
        ModelProviderProfile, PluginContracts, PluginEntrypoint, PluginIntegrity, PluginLifecycle,
        PluginRollback, PluginSandboxPolicy, PluginTestSpec, SandboxNetworkMode,
    };

    fn provider(name: &str, priority: u16, local: bool, model_id: &str) -> PluginManifest {
        PluginManifest {
            manifest_version: "1.2.0".into(),
            uuid: new_uuid_v7(),
            name: name.into(),
            version: "0.4.0".into(),
            kind: "model_provider".into(),
            description: "test provider".into(),
            core_api: ">=0.4.0, <0.5.0".into(),
            entrypoint: PluginEntrypoint {
                kind: "process".into(),
                value: "test".into(),
            },
            dependencies: vec![],
            capabilities: vec!["model.chat".into()],
            extension_points: vec![],
            permissions: vec![],
            lifecycle: PluginLifecycle {
                install: "install".into(),
                enable: "enable".into(),
                disable: "disable".into(),
                uninstall: "uninstall".into(),
                health: Some("health".into()),
            },
            contracts: PluginContracts {
                input_schema: "in".into(),
                output_schema: "out".into(),
            },
            sandbox: PluginSandboxPolicy {
                network: SandboxNetworkMode::Deny,
                network_allowlist: vec![],
                read_only_paths: vec![],
                write_paths: vec![],
                environment_allowlist: vec![],
                timeout_ms: 1000,
                memory_mb: 128,
                cpu_quota_percent: 20,
            },
            integrity: PluginIntegrity {
                artifact: "test".into(),
                hash_algorithm: "sha256".into(),
                digest: "0".repeat(64),
                signature_algorithm: "ed25519".into(),
                signature: "test".into(),
                signer: "test".into(),
                provenance: "test".into(),
            },
            tests: vec![PluginTestSpec {
                name: "test".into(),
                command: "test".into(),
                required: true,
            }],
            rollback: PluginRollback {
                strategy: "disable".into(),
                steps: vec!["disable".into()],
            },
            agent: None,
            model_provider: Some(ModelProviderProfile {
                display_name: name.into(),
                priority,
                transport: "process".into(),
                models: vec![ModelDescriptor {
                    id: model_id.into(),
                    capabilities: vec!["model.chat".into()],
                    context_window: 32_000,
                    input_microunits_per_million_tokens: 1000,
                    output_microunits_per_million_tokens: 2000,
                    local,
                }],
            }),
        }
    }

    #[test]
    fn routing_is_deterministic_and_honors_local_policy() {
        let mut gateway = ModelGateway::new();
        gateway
            .register_provider(provider(
                "com.phxclaw.provider.remote",
                100,
                false,
                "remote",
            ))
            .unwrap();
        gateway
            .register_provider(provider("com.phxclaw.provider.local", 50, true, "local"))
            .unwrap();
        gateway.set_budget("default", 1_000_000);
        let mut request = ModelRequest::new("model.chat", Value::Null);
        request.requires_local = true;
        let decision = gateway.route(&request).unwrap();
        assert_eq!(decision.selected.model_id, "local");
    }
}

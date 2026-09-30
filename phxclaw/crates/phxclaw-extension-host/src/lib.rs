use phxclaw_evidence_ledger::{EvidenceDraft, EvidenceLedger, EvidenceOutcome, LedgerError};
use phxclaw_live_bus::{LiveBusError, LiveEventHub};
use phxclaw_plugin_registry::PluginRegistry;
use phxclaw_plugin_sdk::{CapabilityInvocation, CapabilityResult, InvocationStatus};
use phxclaw_process_protocol::{
    ProcessEnvelope, ProcessMessageKind, ProcessProtocolError, ProcessReplyStatus, ProcessRunner,
};
use phxclaw_sandbox::SandboxBackend;
use phxclaw_types::{new_uuid_v7, PermissionClaim, PluginManifest, PluginState};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct CapabilityRoutes {
    pinned: BTreeMap<String, Uuid>,
}

impl CapabilityRoutes {
    pub fn pin(&mut self, capability: impl Into<String>, plugin_uuid: Uuid) {
        self.pinned.insert(capability.into(), plugin_uuid);
    }

    pub fn pinned(&self, capability: &str) -> Option<Uuid> {
        self.pinned.get(capability).copied()
    }
}

#[derive(Debug, Error)]
pub enum ExtensionHostError {
    #[error("no enabled plugin provides capability {0}")]
    NoProvider(String),
    #[error("capability {capability} is ambiguous across enabled plugins: {plugins:?}")]
    Ambiguous { capability: String, plugins: Vec<Uuid> },
    #[error("pinned plugin {plugin} does not provide enabled capability {capability}")]
    InvalidPin { capability: String, plugin: Uuid },
    #[error("permission denied for plugin {plugin}: {permission} scope={scope}")]
    PermissionDenied { plugin: String, permission: String, scope: String },
    #[error("plugin process failure: {0}")]
    Process(#[from] ProcessProtocolError),
    #[error("event bus failure: {0}")]
    Event(#[from] LiveBusError),
    #[error("evidence ledger failure: {0}")]
    Evidence(#[from] LedgerError),
}

pub struct ExtensionHost<B: SandboxBackend> {
    registry: PluginRegistry,
    runner: ProcessRunner<B>,
    live_bus: LiveEventHub,
    evidence: EvidenceLedger,
    routes: CapabilityRoutes,
}

impl<B: SandboxBackend> ExtensionHost<B> {
    pub fn new(
        registry: PluginRegistry,
        sandbox: B,
        live_bus: LiveEventHub,
        evidence: EvidenceLedger,
    ) -> Self {
        Self {
            registry,
            runner: ProcessRunner::new(sandbox),
            live_bus,
            evidence,
            routes: CapabilityRoutes::default(),
        }
    }

    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut PluginRegistry {
        &mut self.registry
    }

    pub fn routes_mut(&mut self) -> &mut CapabilityRoutes {
        &mut self.routes
    }

    pub fn route_capability(&self, capability: &str) -> Result<&PluginManifest, ExtensionHostError> {
        if let Some(plugin_uuid) = self.routes.pinned(capability) {
            let valid = self
                .registry
                .manifest(&plugin_uuid)
                .filter(|manifest| self.registry.state(&plugin_uuid) == Some(PluginState::Enabled))
                .filter(|manifest| manifest.capabilities.iter().any(|item| item == capability));
            return valid.ok_or_else(|| ExtensionHostError::InvalidPin {
                capability: capability.to_owned(),
                plugin: plugin_uuid,
            });
        }

        let candidates = self
            .registry
            .manifests()
            .filter(|manifest| self.registry.state(&manifest.uuid) == Some(PluginState::Enabled))
            .filter(|manifest| manifest.capabilities.iter().any(|item| item == capability))
            .collect::<Vec<_>>();

        match candidates.as_slice() {
            [] => Err(ExtensionHostError::NoProvider(capability.to_owned())),
            [only] => Ok(*only),
            many => Err(ExtensionHostError::Ambiguous {
                capability: capability.to_owned(),
                plugins: many.iter().map(|manifest| manifest.uuid).collect(),
            }),
        }
    }

    pub fn invoke(&self, request: &CapabilityInvocation) -> Result<CapabilityResult, ExtensionHostError> {
        let manifest = self.route_capability(&request.capability)?;
        authorize(manifest, &request.requested_permissions)?;

        let started = self.live_bus.publish_json(
            "plugin.capability",
            "started",
            json!({
                "invocation_uuid": request.uuid,
                "plugin_uuid": manifest.uuid,
                "plugin_name": manifest.name,
                "capability": request.capability,
            }),
            Some(request.correlation_uuid),
            request.causation_uuid,
        )?;

        let envelope = ProcessEnvelope::new(
            ProcessMessageKind::Execute,
            json!({
                "invocation_uuid": request.uuid,
                "actor": request.actor,
                "capability": request.capability,
                "payload": request.payload,
            }),
        )
        .correlate(request.correlation_uuid);

        let request_hash = sha256_json(&request.payload);
        match self.runner.request::<Value, Value>(manifest, &envelope) {
            Ok(reply) => {
                let success = reply.status == ProcessReplyStatus::Ok;
                let status = if success { InvocationStatus::Succeeded } else { InvocationStatus::Rejected };
                let result = CapabilityResult {
                    uuid: new_uuid_v7(),
                    invocation_uuid: request.uuid,
                    plugin_uuid: manifest.uuid,
                    status,
                    payload: reply.payload.clone(),
                    error_code: reply.error.as_ref().map(|error| error.code.clone()),
                    error_message: reply.error.as_ref().map(|error| error.message.clone()),
                    emitted_at: chrono::Utc::now(),
                };
                let outcome = if success { EvidenceOutcome::Succeeded } else { EvidenceOutcome::Denied };
                let evidence = self.evidence.append(EvidenceDraft {
                    action_uuid: request.uuid,
                    correlation_uuid: Some(request.correlation_uuid),
                    actor: request.actor.clone(),
                    capability: request.capability.clone(),
                    action: "plugin.capability.invoke".into(),
                    outcome,
                    request_summary: json!({
                        "plugin_uuid": manifest.uuid,
                        "plugin_name": manifest.name,
                        "payload_sha256": request_hash,
                    }),
                    result_summary: json!({
                        "reply_status": format!("{:?}", reply.status).to_lowercase(),
                        "result_uuid": result.uuid,
                    }),
                    artifact_uris: Vec::new(),
                })?;
                self.live_bus.publish_json(
                    "plugin.capability",
                    if success { "succeeded" } else { "rejected" },
                    json!({
                        "invocation_uuid": request.uuid,
                        "plugin_uuid": manifest.uuid,
                        "result_uuid": result.uuid,
                        "evidence_uuid": evidence.uuid,
                    }),
                    Some(request.correlation_uuid),
                    Some(started.uuid),
                )?;
                Ok(result)
            }
            Err(error) => {
                let evidence = self.evidence.append(EvidenceDraft {
                    action_uuid: request.uuid,
                    correlation_uuid: Some(request.correlation_uuid),
                    actor: request.actor.clone(),
                    capability: request.capability.clone(),
                    action: "plugin.capability.invoke".into(),
                    outcome: EvidenceOutcome::Failed,
                    request_summary: json!({
                        "plugin_uuid": manifest.uuid,
                        "plugin_name": manifest.name,
                        "payload_sha256": request_hash,
                    }),
                    result_summary: json!({ "error": error.to_string() }),
                    artifact_uris: Vec::new(),
                })?;
                self.live_bus.publish_json(
                    "plugin.capability",
                    "failed",
                    json!({
                        "invocation_uuid": request.uuid,
                        "plugin_uuid": manifest.uuid,
                        "evidence_uuid": evidence.uuid,
                        "error": error.to_string(),
                    }),
                    Some(request.correlation_uuid),
                    Some(started.uuid),
                )?;
                Err(error.into())
            }
        }
    }
}

fn authorize(manifest: &PluginManifest, claims: &[PermissionClaim]) -> Result<(), ExtensionHostError> {
    for claim in claims {
        let allowed = manifest.permissions.iter().any(|permission| {
            permission.name == claim.name
                && permission.scopes.iter().any(|scope| scope == "*" || scope == &claim.scope)
        });
        if !allowed {
            return Err(ExtensionHostError::PermissionDenied {
                plugin: manifest.name.clone(),
                permission: claim.name.clone(),
                scope: claim.scope.clone(),
            });
        }
    }
    Ok(())
}

fn sha256_json(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

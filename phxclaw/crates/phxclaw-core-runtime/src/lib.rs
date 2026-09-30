#![forbid(unsafe_code)]

//! PhxClaw unified product runtime.
//!
//! This crate is the public facade consumed by the user-facing PhxClaw
//! binary. Upstream MIT-derived building blocks stay behind Phoenix-native
//! contracts so the product behaves as one system rather than a collection
//! of bridges.

use chrono::{DateTime, Utc};
use phxclaw_channel_gateway::{ChannelAgentRouter, ChannelRouteRule};
use phxclaw_mcp_lsp_runtime::qualified_tool_name;
use phxclaw_rustclaw_native::{
    NativeScheduledJob, NativeSession, NativeSessionStore, RustClawNativeError, ScheduleSpec,
    negotiate_gateway_protocol,
};
use phxclaw_types::new_uuid_v7;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

pub const PRODUCT_NAME: &str = "PhxClaw";
pub const PRODUCT_CLI: &str = "phx";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CoreService {
    Kernel,
    AgentRegistry,
    MissionRuntime,
    TeamRuntime,
    ChannelGateway,
    SecretBroker,
    ModelGateway,
    McpRuntime,
    RepoIntelligence,
    Checkpoint,
    PostgreSql,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    Disabled,
    Ready,
    Degraded,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub service: CoreService,
    pub state: ServiceState,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub runtime_uuid: Uuid,
    pub product: String,
    pub version: String,
    pub started_at: DateTime<Utc>,
    pub services: Vec<ServiceStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixMessage {
    pub session_uuid: Uuid,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoenixScheduleRequest {
    pub name: String,
    pub capability: String,
    pub payload: Value,
    pub schedule: ScheduleSpec,
}

#[derive(Debug)]
pub struct PhoenixCoreRuntime {
    runtime_uuid: Uuid,
    started_at: DateTime<Utc>,
    version: String,
    sessions: NativeSessionStore,
    channel_router: ChannelAgentRouter,
    schedules: BTreeMap<Uuid, NativeScheduledJob>,
    services: BTreeMap<CoreService, ServiceStatus>,
}

impl PhoenixCoreRuntime {
    pub fn new(version: impl Into<String>, default_agent: impl Into<String>) -> Self {
        let mut services = BTreeMap::new();
        for service in [
            CoreService::Kernel,
            CoreService::AgentRegistry,
            CoreService::MissionRuntime,
            CoreService::TeamRuntime,
            CoreService::ChannelGateway,
            CoreService::SecretBroker,
            CoreService::ModelGateway,
            CoreService::McpRuntime,
            CoreService::RepoIntelligence,
            CoreService::Checkpoint,
            CoreService::PostgreSql,
        ] {
            services.insert(
                service.clone(),
                ServiceStatus {
                    service,
                    state: ServiceState::Disabled,
                    detail: "not initialized".into(),
                },
            );
        }
        Self {
            runtime_uuid: new_uuid_v7(),
            started_at: Utc::now(),
            version: version.into(),
            sessions: NativeSessionStore::default(),
            channel_router: ChannelAgentRouter::new(default_agent),
            schedules: BTreeMap::new(),
            services,
        }
    }

    pub fn set_service_state(
        &mut self,
        service: CoreService,
        state: ServiceState,
        detail: impl Into<String>,
    ) {
        self.services.insert(
            service.clone(),
            ServiceStatus {
                service,
                state,
                detail: detail.into(),
            },
        );
    }

    pub fn status(&self) -> RuntimeStatus {
        RuntimeStatus {
            runtime_uuid: self.runtime_uuid,
            product: PRODUCT_NAME.into(),
            version: self.version.clone(),
            started_at: self.started_at,
            services: self.services.values().cloned().collect(),
        }
    }

    pub fn create_session(
        &mut self,
        principal_uuid: Option<Uuid>,
        channel: Option<String>,
    ) -> NativeSession {
        self.sessions.create(principal_uuid, channel)
    }

    pub fn push_message(&mut self, message: PhoenixMessage) -> Result<Uuid, CoreRuntimeError> {
        Ok(self
            .sessions
            .push(message.session_uuid, message.role, message.content)?
            .uuid)
    }

    pub fn add_channel_route(&mut self, rule: ChannelRouteRule) {
        self.channel_router.add_rule(rule);
    }

    pub fn route_channel(&self, channel: &str, external_user_id: &str) -> &str {
        self.channel_router.route(channel, external_user_id)
    }

    pub fn create_schedule(
        &mut self,
        request: PhoenixScheduleRequest,
    ) -> Result<NativeScheduledJob, CoreRuntimeError> {
        let job = NativeScheduledJob::new(
            request.name,
            request.capability,
            request.payload,
            request.schedule,
        )?;
        self.schedules.insert(job.uuid, job.clone());
        Ok(job)
    }

    pub fn negotiate_device_protocol(
        &self,
        client_min: Option<u32>,
        client_max: Option<u32>,
        supported: &[u32],
    ) -> Result<u32, CoreRuntimeError> {
        Ok(negotiate_gateway_protocol(
            client_min, client_max, supported,
        )?)
    }

    pub fn mcp_tool_name(&self, server: &str, tool: &str) -> String {
        qualified_tool_name(server, tool)
    }
}

#[derive(Debug, Error)]
pub enum CoreRuntimeError {
    #[error(transparent)]
    Native(#[from] RustClawNativeError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use phxclaw_types::is_uuid_v7;

    #[test]
    fn facade_hides_upstream_names_from_user_contract() {
        let mut rt = PhoenixCoreRuntime::new("0.20.0", "Master Orchestrator");
        rt.set_service_state(CoreService::Kernel, ServiceState::Ready, "booted");
        let status = rt.status();
        assert_eq!(status.product, "PhxClaw");
        assert_eq!(status.version, "0.20.0");
    }

    #[test]
    fn session_schedule_and_routing_share_one_facade() {
        let mut rt = PhoenixCoreRuntime::new("0.20.0", "Master Orchestrator");
        let session = rt.create_session(None, Some("webchat".into()));
        let msg = rt
            .push_message(PhoenixMessage {
                session_uuid: session.uuid,
                role: "user".into(),
                content: "hello".into(),
            })
            .unwrap();
        assert!(is_uuid_v7(&msg));
        rt.add_channel_route(ChannelRouteRule {
            channel: Some("telegram".into()),
            external_user_id: Some("42".into()),
            agent_name: "Research Agent".into(),
            priority: 100,
        });
        assert_eq!(rt.route_channel("telegram", "42"), "Research Agent");
        let job = rt
            .create_schedule(PhoenixScheduleRequest {
                name: "health".into(),
                capability: "system.health".into(),
                payload: serde_json::json!({}),
                schedule: ScheduleSpec::EverySeconds(60),
            })
            .unwrap();
        assert!(is_uuid_v7(&job.uuid));
    }
}

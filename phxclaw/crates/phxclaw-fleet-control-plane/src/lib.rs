//! PhxClaw v0.29 Fleet Control Plane.
//! The control plane never bypasses F22 Device Node authorization or v0.28 rollout verification.

use chrono::{DateTime, Duration, Utc};
use phxclaw_device_nodes::{
    authorize_command, ApprovalRef, CommandPolicy, DeviceCommand, DeviceNode, DeviceState,
    RiskLevel, SecretHandle,
};
use phxclaw_fleet_updater::{
    selected_for_percent, verify_fleet_signature, FleetSigner, RolloutPlan,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ControlError {
    #[error("node is not eligible for selector")]
    SelectorDenied,
    #[error("node is offline, quarantined, revoked, or in maintenance")]
    NodeUnavailable,
    #[error("fanout exceeds signed policy")]
    FanoutExceeded,
    #[error("parallelism exceeds signed policy")]
    ParallelismExceeded,
    #[error("selector membership changed after snapshot")]
    MembershipDrift,
    #[error("node sequence regressed")]
    SequenceRegression,
    #[error("stale control fencing token")]
    FencingMismatch,
    #[error("remote command rejected by F22 Device Node policy")]
    DeviceCommandDenied,
    #[error("maintenance window invalid")]
    InvalidMaintenance,
    #[error("recovery evidence invalid")]
    RecoveryDenied,
    #[error("signed control plan is invalid or untrusted")]
    InvalidControlPlan,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ControlNodeState {
    Online,
    Stale,
    Offline,
    Recovering,
    Maintenance,
    Quarantined,
    Revoked,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryNode {
    pub tenant_uuid: Uuid,
    pub node_uuid: Uuid,
    pub fleet_uuid: Uuid,
    pub region: String,
    pub groups: BTreeSet<String>,
    pub tags: BTreeSet<String>,
    pub device_state: DeviceState,
    pub current_version: String,
    pub current_sequence: u64,
    pub session_fencing_token: i64,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub control_state: ControlNodeState,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FleetSelector {
    #[serde(default)]
    pub regions: BTreeSet<String>,
    #[serde(default)]
    pub groups_any: BTreeSet<String>,
    #[serde(default)]
    pub tags_all: BTreeSet<String>,
    #[serde(default)]
    pub exclude_tags: BTreeSet<String>,
}

impl FleetSelector {
    pub fn matches(&self, n: &InventoryNode) -> bool {
        if matches!(
            n.device_state,
            DeviceState::Quarantined | DeviceState::Revoked
        ) || matches!(
            n.control_state,
            ControlNodeState::Quarantined | ControlNodeState::Revoked
        ) {
            return false;
        }
        (self.regions.is_empty() || self.regions.contains(&n.region))
            && (self.groups_any.is_empty() || !self.groups_any.is_disjoint(&n.groups))
            && self.tags_all.is_subset(&n.tags)
            && self.exclude_tags.is_disjoint(&n.tags)
    }
    pub fn sha256(&self) -> String {
        let value = serde_json::to_value(self).expect("serializable selector");
        let bytes = serde_json::to_vec(&value).expect("serializable selector value");
        format!("{:x}", Sha256::digest(bytes))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlPolicy {
    pub max_fanout: usize,
    pub max_parallel: usize,
    pub offline_after_seconds: i64,
    pub stale_after_seconds: i64,
    pub recovery_grace_seconds: i64,
    pub protected_capabilities: BTreeSet<String>,
}

pub fn derive_control_state(
    n: &InventoryNode,
    now: DateTime<Utc>,
    p: &ControlPolicy,
    maintenance: bool,
) -> ControlNodeState {
    if matches!(n.device_state, DeviceState::Revoked) {
        return ControlNodeState::Revoked;
    }
    if matches!(n.device_state, DeviceState::Quarantined) {
        return ControlNodeState::Quarantined;
    }
    if maintenance {
        return ControlNodeState::Maintenance;
    }
    match n.last_heartbeat_at {
        None => ControlNodeState::Offline,
        Some(t) if now - t > Duration::seconds(p.offline_after_seconds) => {
            ControlNodeState::Offline
        }
        Some(t) if now - t > Duration::seconds(p.stale_after_seconds) => ControlNodeState::Stale,
        _ => ControlNodeState::Online,
    }
}

pub fn canonical_membership(nodes: &[InventoryNode], selector: &FleetSelector) -> Vec<Uuid> {
    let mut ids: Vec<_> = nodes
        .iter()
        .filter(|n| selector.matches(n))
        .map(|n| n.node_uuid)
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

pub fn membership_sha256(ids: &[Uuid]) -> String {
    let mut ids = ids.to_vec();
    ids.sort();
    ids.dedup();
    let mut h = Sha256::new();
    for id in ids {
        h.update(id.as_bytes());
    }
    format!("{:x}", h.finalize())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelectorSnapshot {
    pub snapshot_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub selector: FleetSelector,
    pub selector_sha256: String,
    pub membership_sha256: String,
    pub member_count: usize,
    pub created_at: DateTime<Utc>,
}

pub fn freeze_selector(
    tenant_uuid: Uuid,
    selector: FleetSelector,
    nodes: &[InventoryNode],
    now: DateTime<Utc>,
) -> SelectorSnapshot {
    let ids = canonical_membership(nodes, &selector);
    SelectorSnapshot {
        snapshot_uuid: Uuid::now_v7(),
        tenant_uuid,
        selector_sha256: selector.sha256(),
        membership_sha256: membership_sha256(&ids),
        member_count: ids.len(),
        selector,
        created_at: now,
    }
}

pub fn verify_snapshot(
    snapshot: &SelectorSnapshot,
    nodes: &[InventoryNode],
) -> Result<Vec<Uuid>, ControlError> {
    if snapshot.selector_sha256 != snapshot.selector.sha256() {
        return Err(ControlError::MembershipDrift);
    }
    let ids = canonical_membership(nodes, &snapshot.selector);
    if ids.len() != snapshot.member_count || membership_sha256(&ids) != snapshot.membership_sha256 {
        return Err(ControlError::MembershipDrift);
    }
    Ok(ids)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MaintenanceWindow {
    pub maintenance_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub snapshot_uuid: Uuid,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub reason: String,
    pub approval_uuid: Option<Uuid>,
}

pub fn validate_maintenance(m: &MaintenanceWindow, now: DateTime<Utc>) -> Result<(), ControlError> {
    if m.ends_at <= m.starts_at
        || m.ends_at - m.starts_at > Duration::hours(24)
        || m.starts_at < now - Duration::minutes(5)
    {
        return Err(ControlError::InvalidMaintenance);
    }
    if m.reason.trim().is_empty() {
        return Err(ControlError::InvalidMaintenance);
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedControlPlanDocument {
    pub schema_version: String,
    pub plan_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub snapshot: SelectorSnapshot,
    pub capability: String,
    pub arguments: Value,
    pub arguments_sha256: String,
    pub secret_handles: Vec<SecretHandle>,
    pub risk: RiskLevel,
    pub approval_uuid: Option<Uuid>,
    pub max_parallel: usize,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub idempotency_seed: String,
    pub policy_sha256: String,
    pub signer_key_id: String,
    pub signature_algorithm: String,
    pub signature_b64: String,
}

pub fn signed_control_plan_payload(plan: &SignedControlPlanDocument) -> Vec<u8> {
    let mut value = serde_json::to_value(plan).expect("serializable signed control plan");
    let map = value.as_object_mut().expect("control plan is an object");
    map.remove("signature_b64");
    map.remove("signature_algorithm");
    map.remove("signer_key_id");
    serde_json::to_vec(&value).expect("serializable control plan payload")
}

pub fn verify_signed_control_plan(
    plan: &SignedControlPlanDocument,
    expected_policy_sha256: &str,
    signers: &[FleetSigner],
    now: DateTime<Utc>,
) -> Result<(), ControlError> {
    if plan.signature_algorithm != "ed25519"
        || plan.policy_sha256 != expected_policy_sha256
        || plan.expires_at <= now
    {
        return Err(ControlError::InvalidControlPlan);
    }
    if plan.snapshot.selector_sha256 != plan.snapshot.selector.sha256() {
        return Err(ControlError::InvalidControlPlan);
    }
    let arg_value =
        serde_json::to_value(&plan.arguments).map_err(|_| ControlError::InvalidControlPlan)?;
    let arg_bytes = serde_json::to_vec(&arg_value).map_err(|_| ControlError::InvalidControlPlan)?;
    if format!("{:x}", Sha256::digest(arg_bytes)) != plan.arguments_sha256 {
        return Err(ControlError::InvalidControlPlan);
    }
    verify_fleet_signature(
        &signed_control_plan_payload(plan),
        &plan.signer_key_id,
        "fleet.control.plan",
        &plan.signature_b64,
        signers,
    )
    .map_err(|_| ControlError::InvalidControlPlan)
}

pub fn plan_fanout(
    plan: &SignedControlPlanDocument,
    resolved_approval: Option<ApprovalRef>,
    inventory: &[InventoryNode],
    device_nodes: &BTreeMap<Uuid, DeviceNode>,
    policy: &ControlPolicy,
    expected_policy_sha256: &str,
    signers: &[FleetSigner],
    now: DateTime<Utc>,
) -> Result<Vec<DeviceCommand>, ControlError> {
    verify_signed_control_plan(plan, expected_policy_sha256, signers, now)?;
    if plan.approval_uuid != resolved_approval.as_ref().map(|a| a.approval_uuid) {
        return Err(ControlError::DeviceCommandDenied);
    }
    if plan.max_parallel == 0 || plan.max_parallel > policy.max_parallel {
        return Err(ControlError::ParallelismExceeded);
    }
    let ids = verify_snapshot(&plan.snapshot, inventory)?;
    if ids.len() > policy.max_fanout {
        return Err(ControlError::FanoutExceeded);
    }
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let inv = inventory
            .iter()
            .find(|n| n.node_uuid == id)
            .ok_or(ControlError::SelectorDenied)?;
        if !matches!(
            inv.control_state,
            ControlNodeState::Online | ControlNodeState::Stale
        ) {
            return Err(ControlError::NodeUnavailable);
        }
        let node = device_nodes
            .get(&id)
            .ok_or(ControlError::DeviceCommandDenied)?;
        let cmd = DeviceCommand {
            command_uuid: Uuid::now_v7(),
            tenant_uuid: plan.tenant_uuid,
            node_uuid: id,
            capability: plan.capability.clone(),
            arguments: plan.arguments.clone(),
            secret_handles: plan.secret_handles.clone(),
            idempotency_key: format!("{}:{}", plan.idempotency_seed, id),
            risk: plan.risk.clone(),
            approval: resolved_approval.clone(),
            submitted_at: now,
            not_before: now,
            expires_at: plan.expires_at,
            fencing_token: inv.session_fencing_token,
        };
        authorize_command(
            node,
            &cmd,
            &CommandPolicy {
                protected_capabilities: policy.protected_capabilities.clone(),
                max_ttl: plan.expires_at - plan.created_at,
            },
            now,
        )
        .map_err(|_| ControlError::DeviceCommandDenied)?;
        out.push(cmd);
    }
    Ok(out)
}

pub fn scoped_rollout_members(
    rollout: &RolloutPlan,
    stage_percent: u8,
    snapshot: &SelectorSnapshot,
    inventory: &[InventoryNode],
) -> Result<Vec<Uuid>, ControlError> {
    let ids = verify_snapshot(snapshot, inventory)?;
    let mut selected: Vec<_> = ids
        .into_iter()
        .filter(|id| selected_for_percent(*id, rollout.rollout_uuid, stage_percent))
        .collect();
    selected.sort();
    Ok(selected)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryObservation {
    pub node_uuid: Uuid,
    pub previous_sequence: u64,
    pub claimed_sequence: u64,
    pub expected_fencing_token: u64,
    pub received_fencing_token: u64,
    pub heartbeat_at: DateTime<Utc>,
}

pub fn assess_recovery(
    o: &RecoveryObservation,
    now: DateTime<Utc>,
    p: &ControlPolicy,
) -> Result<ControlNodeState, ControlError> {
    if o.claimed_sequence < o.previous_sequence {
        return Err(ControlError::SequenceRegression);
    }
    if o.received_fencing_token != o.expected_fencing_token {
        return Err(ControlError::FencingMismatch);
    }
    if now - o.heartbeat_at > Duration::seconds(p.recovery_grace_seconds) {
        return Err(ControlError::RecoveryDenied);
    }
    Ok(ControlNodeState::Recovering)
}

pub fn validate_control_fencing(expected: u64, received: u64) -> Result<(), ControlError> {
    if expected == received {
        Ok(())
    } else {
        Err(ControlError::FencingMismatch)
    }
}

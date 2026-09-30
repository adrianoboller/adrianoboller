//! PhxClaw v0.28 secure auto-updater and fleet rollout contracts.
//! The library is policy/verification code. It never treats missing telemetry or signatures as success.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum FleetError {
    #[error("signer is not trusted for requested purpose")]
    SignerNotTrusted,
    #[error("invalid Ed25519 signature")]
    InvalidSignature,
    #[error("invalid SHA-256 digest")]
    InvalidDigest,
    #[error("rollout stage or state transition denied")]
    TransitionDenied,
    #[error("health evidence is stale, insufficient or failed")]
    HealthGateFailed,
    #[error("update sequence is not monotonic")]
    AntiRollback,
    #[error("device is not part of the selected rollout cohort")]
    DeviceNotSelected,
    #[error("component dependency graph is invalid")]
    InvalidComponentGraph,
    #[error("database contract is blocked until full compatibility and manual approval")]
    DatabaseContractBlocked,
    #[error("plugin/core compatibility mismatch")]
    CompatibilityMismatch,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FleetSigner {
    pub key_id: String,
    pub public_key_b64: String,
    pub purposes: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}
fn default_true() -> bool { true }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Channel { Canary, Beta, Stable }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RolloutState { Draft, Active, Paused, RollbackRequired, RollingBack, Completed, Aborted }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind { Preflight, Backup, DatabaseExpand, Core, Plugin, HealthCheck, DatabaseContract, Cleanup }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RolloutStage {
    pub index: u32,
    pub percent: u8,
    pub min_samples: u32,
    pub soak_seconds: u64,
}


#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthThresholds {
    pub min_success_rate: f64,
    pub max_crash_rate: f64,
    pub max_rollback_rate: f64,
    pub max_stale_heartbeat_rate: f64,
    pub max_install_error_rate: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CriticalThresholds {
    pub crash_rate_gte: f64,
    pub install_error_rate_gte: f64,
    pub rollback_rate_gte: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RolloutPlan {
    pub schema_version: String,
    pub rollout_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub channel: Channel,
    pub version: String,
    pub sequence: u64,
    pub source_state_sha256: String,
    pub update_manifest_sha256: String,
    pub policy_sha256: String,
    pub health_thresholds: HealthThresholds,
    pub critical_thresholds: CriticalThresholds,
    pub health_evidence_max_age_seconds: u64,
    pub stages: Vec<RolloutStage>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub signer_key_id: String,
    pub signature_algorithm: String,
    pub signature_b64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthEvidence {
    pub schema_version: String,
    pub evidence_uuid: Uuid,
    pub rollout_uuid: Uuid,
    pub stage_index: u32,
    pub samples: u32,
    pub success_rate: f64,
    pub crash_rate: f64,
    pub rollback_rate: f64,
    pub stale_heartbeat_rate: f64,
    pub install_error_rate: f64,
    pub window_started_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub signer_key_id: String,
    pub signature_algorithm: String,
    pub signature_b64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FleetState {
    pub schema_version: String,
    pub rollout_uuid: Uuid,
    pub generation: u64,
    pub fencing_token: u64,
    pub state: RolloutState,
    pub stage_index: u32,
    pub updated_at: DateTime<Utc>,
    pub reason: String,
    pub signer_key_id: String,
    pub signature_algorithm: String,
    pub signature_b64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateComponent {
    pub component_uuid: Uuid,
    pub kind: ComponentKind,
    pub name: String,
    pub version: String,
    pub sha256: String,
    pub reversible: bool,
    pub dependencies: Vec<Uuid>,
    pub core_min: Option<String>,
    pub core_max_exclusive: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComponentPlan {
    pub schema_version: String,
    pub target_core_version: String,
    pub policy_sha256: String,
    pub components: Vec<UpdateComponent>,
    pub signer_key_id: String,
    pub signature_algorithm: String,
    pub signature_b64: String,
}

pub fn component_plan_payload(o: &ComponentPlan) -> Vec<u8> {
    let components = serde_json::to_vec(&o.components).expect("serializable components");
    format!(
        "phxclaw-component-plan-v028\ntarget_core_version={}\npolicy_sha256={}\ncomponents_sha256={:x}\n",
        o.target_core_version, o.policy_sha256.to_ascii_lowercase(), Sha256::digest(components)
    ).into_bytes()
}

pub fn verify_component_plan_signature(o: &ComponentPlan, signers: &[FleetSigner]) -> Result<(), FleetError> {
    if !valid_sha256(&o.policy_sha256) || o.signature_algorithm != "ed25519" { return Err(FleetError::InvalidSignature); }
    verify_fleet_signature(&component_plan_payload(o), &o.signer_key_id, "fleet.component_plan", &o.signature_b64, signers)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeUpdateState {
    pub node_uuid: Uuid,
    pub current_version: String,
    pub current_sequence: u64,
    pub known_good_version: String,
    pub known_good_sequence: u64,
}

fn valid_sha256(v: &str) -> bool { v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()) }

pub fn verify_fleet_signature(payload: &[u8], key_id: &str, purpose: &str, sig_b64: &str, signers: &[FleetSigner]) -> Result<(), FleetError> {
    let signer = signers.iter().find(|s| s.enabled && s.key_id == key_id && s.purposes.iter().any(|p| p == purpose)).ok_or(FleetError::SignerNotTrusted)?;
    let kb = STANDARD.decode(&signer.public_key_b64).map_err(|_| FleetError::InvalidSignature)?;
    let ka: [u8; 32] = kb.try_into().map_err(|_| FleetError::InvalidSignature)?;
    let key = VerifyingKey::from_bytes(&ka).map_err(|_| FleetError::InvalidSignature)?;
    let sb = STANDARD.decode(sig_b64).map_err(|_| FleetError::InvalidSignature)?;
    let sig = Signature::try_from(sb.as_slice()).map_err(|_| FleetError::InvalidSignature)?;
    key.verify_strict(payload, &sig).map_err(|_| FleetError::InvalidSignature)
}

pub fn rollout_payload(o: &RolloutPlan) -> Vec<u8> {
    let stages = serde_json::to_vec(&o.stages).expect("serializable stages");
    let health = serde_json::to_vec(&o.health_thresholds).expect("serializable health thresholds");
    let critical = serde_json::to_vec(&o.critical_thresholds).expect("serializable critical thresholds");
    format!(
        "phxclaw-fleet-rollout-v028\nrollout_uuid={}\ntenant_uuid={}\nchannel={}\nversion={}\nsequence={}\nsource_state_sha256={}\nupdate_manifest_sha256={}\npolicy_sha256={}\nhealth_thresholds_sha256={:x}\ncritical_thresholds_sha256={:x}\nhealth_evidence_max_age_seconds={}\nstages_sha256={:x}\ncreated_at={}\nexpires_at={}\n",
        o.rollout_uuid, o.tenant_uuid,
        match o.channel { Channel::Canary => "canary", Channel::Beta => "beta", Channel::Stable => "stable" },
        o.version, o.sequence, o.source_state_sha256.to_ascii_lowercase(), o.update_manifest_sha256.to_ascii_lowercase(), o.policy_sha256.to_ascii_lowercase(),
        Sha256::digest(health), Sha256::digest(critical), o.health_evidence_max_age_seconds, Sha256::digest(stages),
        o.created_at.to_rfc3339_opts(SecondsFormat::AutoSi, true), o.expires_at.to_rfc3339_opts(SecondsFormat::AutoSi, true)
    ).into_bytes()
}

pub fn health_payload(o: &HealthEvidence) -> Vec<u8> {
    format!(
        "phxclaw-fleet-health-v028\nevidence_uuid={}\nrollout_uuid={}\nstage_index={}\nsamples={}\nsuccess_rate={:.9}\ncrash_rate={:.9}\nrollback_rate={:.9}\nstale_heartbeat_rate={:.9}\ninstall_error_rate={:.9}\nwindow_started_at={}\ncreated_at={}\n",
        o.evidence_uuid, o.rollout_uuid, o.stage_index, o.samples, o.success_rate, o.crash_rate, o.rollback_rate,
        o.stale_heartbeat_rate, o.install_error_rate, o.window_started_at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        o.created_at.to_rfc3339_opts(SecondsFormat::AutoSi, true)
    ).into_bytes()
}

pub fn state_payload(o: &FleetState) -> Vec<u8> {
    format!(
        "phxclaw-fleet-state-v028\nrollout_uuid={}\ngeneration={}\nfencing_token={}\nstate={}\nstage_index={}\nupdated_at={}\nreason_sha256={:x}\n",
        o.rollout_uuid, o.generation, o.fencing_token,
        match o.state { RolloutState::Draft=>"draft", RolloutState::Active=>"active", RolloutState::Paused=>"paused", RolloutState::RollbackRequired=>"rollback_required", RolloutState::RollingBack=>"rolling_back", RolloutState::Completed=>"completed", RolloutState::Aborted=>"aborted" },
        o.stage_index, o.updated_at.to_rfc3339_opts(SecondsFormat::AutoSi, true), Sha256::digest(o.reason.as_bytes())
    ).into_bytes()
}

pub fn verify_rollout_plan(o: &RolloutPlan, now: DateTime<Utc>, signers: &[FleetSigner]) -> Result<(), FleetError> {
    if !valid_sha256(&o.source_state_sha256) || !valid_sha256(&o.update_manifest_sha256) || !valid_sha256(&o.policy_sha256) { return Err(FleetError::InvalidDigest); }
    if o.sequence == 0 || o.stages.is_empty() || o.stages.iter().any(|s| s.percent == 0 || s.percent > 100 || s.min_samples == 0 || s.soak_seconds == 0) { return Err(FleetError::TransitionDenied); }
    if o.stages.windows(2).any(|w| w[1].index != w[0].index + 1 || w[1].percent <= w[0].percent) { return Err(FleetError::TransitionDenied); }
    if o.expires_at <= now || o.created_at > now + Duration::minutes(5) || o.expires_at - o.created_at > Duration::days(14) { return Err(FleetError::TransitionDenied); }
    if o.signature_algorithm != "ed25519" { return Err(FleetError::InvalidSignature); }
    Version::parse(&o.version).map_err(|_| FleetError::TransitionDenied)?;
    verify_fleet_signature(&rollout_payload(o), &o.signer_key_id, "fleet.rollout", &o.signature_b64, signers)
}

pub fn cohort_bucket(node_uuid: Uuid, rollout_uuid: Uuid) -> u16 {
    let mut h = Sha256::new(); h.update(node_uuid.as_bytes()); h.update(rollout_uuid.as_bytes());
    let d = h.finalize(); u16::from_be_bytes([d[0], d[1]]) % 10_000
}

pub fn selected_for_percent(node_uuid: Uuid, rollout_uuid: Uuid, percent: u8) -> bool {
    percent > 0 && percent <= 100 && cohort_bucket(node_uuid, rollout_uuid) < u16::from(percent) * 100
}

pub fn verify_health(o: &HealthEvidence, plan: &RolloutPlan, now: DateTime<Utc>, signers: &[FleetSigner]) -> Result<(), FleetError> {
    let stage = plan.stages.iter().find(|s| s.index == o.stage_index).ok_or(FleetError::HealthGateFailed)?;
    if o.rollout_uuid != plan.rollout_uuid || o.samples < stage.min_samples || o.created_at < o.window_started_at || now - o.created_at > Duration::seconds(plan.health_evidence_max_age_seconds as i64) || o.created_at - o.window_started_at < Duration::seconds(stage.soak_seconds as i64) { return Err(FleetError::HealthGateFailed); }
    if !(0.0..=1.0).contains(&o.success_rate) || !(0.0..=1.0).contains(&o.crash_rate) || !(0.0..=1.0).contains(&o.rollback_rate) || !(0.0..=1.0).contains(&o.stale_heartbeat_rate) || !(0.0..=1.0).contains(&o.install_error_rate) { return Err(FleetError::HealthGateFailed); }
    if o.signature_algorithm != "ed25519" { return Err(FleetError::InvalidSignature); }
    verify_fleet_signature(&health_payload(o), &o.signer_key_id, "fleet.health", &o.signature_b64, signers)?;
    if o.success_rate < plan.health_thresholds.min_success_rate || o.crash_rate > plan.health_thresholds.max_crash_rate || o.rollback_rate > plan.health_thresholds.max_rollback_rate || o.stale_heartbeat_rate > plan.health_thresholds.max_stale_heartbeat_rate || o.install_error_rate > plan.health_thresholds.max_install_error_rate { return Err(FleetError::HealthGateFailed); }
    Ok(())
}

pub fn verify_state(o: &FleetState, signers: &[FleetSigner]) -> Result<(), FleetError> {
    if o.generation == 0 || o.fencing_token == 0 || o.signature_algorithm != "ed25519" { return Err(FleetError::TransitionDenied); }
    verify_fleet_signature(&state_payload(o), &o.signer_key_id, "fleet.state", &o.signature_b64, signers)
}

pub fn verify_node_sequence(node: &NodeUpdateState, target_version: &str, target_sequence: u64) -> Result<(), FleetError> {
    Version::parse(&node.current_version).map_err(|_| FleetError::AntiRollback)?;
    Version::parse(target_version).map_err(|_| FleetError::AntiRollback)?;
    if target_sequence <= node.current_sequence { return Err(FleetError::AntiRollback); }
    Ok(())
}

pub fn component_order(components: &[UpdateComponent], target_core_version: &str) -> Result<Vec<Uuid>, FleetError> {
    Version::parse(target_core_version).map_err(|_| FleetError::CompatibilityMismatch)?;
    let ids: BTreeSet<Uuid> = components.iter().map(|c| c.component_uuid).collect();
    if ids.len() != components.len() || components.iter().any(|c| !valid_sha256(&c.sha256) || c.dependencies.iter().any(|d| !ids.contains(d))) { return Err(FleetError::InvalidComponentGraph); }
    let target = Version::parse(target_core_version).map_err(|_| FleetError::CompatibilityMismatch)?;
    for c in components.iter().filter(|c| c.kind == ComponentKind::Plugin) {
        if let Some(min) = &c.core_min { if target < Version::parse(min).map_err(|_| FleetError::CompatibilityMismatch)? { return Err(FleetError::CompatibilityMismatch); } }
        if let Some(max) = &c.core_max_exclusive { if target >= Version::parse(max).map_err(|_| FleetError::CompatibilityMismatch)? { return Err(FleetError::CompatibilityMismatch); } }
    }
    let mut indegree: BTreeMap<Uuid, usize> = components.iter().map(|c| (c.component_uuid, c.dependencies.len())).collect();
    let mut reverse: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    for c in components { for d in &c.dependencies { reverse.entry(*d).or_default().push(c.component_uuid); } }
    let mut q: VecDeque<Uuid> = indegree.iter().filter_map(|(id,n)| if *n == 0 { Some(*id) } else { None }).collect();
    let mut out = Vec::new();
    while let Some(id) = q.pop_front() {
        out.push(id);
        for dep in reverse.get(&id).into_iter().flatten() {
            let n = indegree.get_mut(dep).ok_or(FleetError::InvalidComponentGraph)?; *n -= 1; if *n == 0 { q.push_back(*dep); }
        }
    }
    if out.len() != components.len() { return Err(FleetError::InvalidComponentGraph); }
    Ok(out)
}

pub fn allow_database_contract(fleet_compatible_percent: u8, manual_approval: bool, prior_health_verified: bool) -> Result<(), FleetError> {
    if fleet_compatible_percent != 100 || !manual_approval || !prior_health_verified { return Err(FleetError::DatabaseContractBlocked); }
    Ok(())
}

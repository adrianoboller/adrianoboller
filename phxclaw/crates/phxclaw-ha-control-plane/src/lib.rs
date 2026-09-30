//! PhxClaw v0.30 distributed control-plane contracts.
//! PostgreSQL time and a monotonically increasing fencing epoch are authoritative.
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::time::Duration as StdDuration;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum HaError {
    #[error("controller is not healthy")]
    ControllerUnhealthy,
    #[error("leadership lease expired or not owned")]
    NotLeader,
    #[error("stale fencing epoch")]
    StaleEpoch,
    #[error("partition uncertainty requires self-fencing")]
    PartitionSelfFence,
    #[error("operation source state differs from reconciled state")]
    ReconcileMismatch,
    #[error("invalid sha256")]
    InvalidSha256,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ControllerState {
    Joining,
    Follower,
    Leader,
    SelfFenced,
    Draining,
    Offline,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControllerMember {
    pub tenant_uuid: Uuid,
    pub controller_uuid: Uuid,
    pub region: String,
    pub state: ControllerState,
    pub started_at: DateTime<Utc>,
    pub last_heartbeat_at: DateTime<Utc>,
    pub build_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaderLease {
    pub tenant_uuid: Uuid,
    pub lease_uuid: Uuid,
    pub holder_uuid: Uuid,
    pub epoch: u64,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
impl LeaderLease {
    pub fn authorizes(
        &self,
        controller: Uuid,
        epoch: u64,
        db_now: DateTime<Utc>,
    ) -> Result<(), HaError> {
        if self.holder_uuid != controller || self.expires_at <= db_now {
            return Err(HaError::NotLeader);
        }
        if epoch != self.epoch {
            return Err(HaError::StaleEpoch);
        }
        Ok(())
    }
    pub fn renew_deadline(&self, renew_before: ChronoDuration) -> DateTime<Utc> {
        self.expires_at - renew_before
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationFence {
    pub controller_uuid: Uuid,
    pub leader_epoch: u64,
    pub source_state_sha256: String,
}

fn valid_sha256(v: &str) -> bool {
    v.len() == 64
        && v.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

pub fn authorize_mutation(
    lease: &LeaderLease,
    fence: &OperationFence,
    current_state_sha256: &str,
    db_now: DateTime<Utc>,
) -> Result<(), HaError> {
    if !valid_sha256(&fence.source_state_sha256) || !valid_sha256(current_state_sha256) {
        return Err(HaError::InvalidSha256);
    }
    lease.authorizes(fence.controller_uuid, fence.leader_epoch, db_now)?;
    if fence.source_state_sha256 != current_state_sha256 {
        return Err(HaError::ReconcileMismatch);
    }
    Ok(())
}

/// A controller that cannot reach the lease authority for at least one full lease TTL must stop mutations.
pub fn must_self_fence(authority_silence: StdDuration, lease_seconds: u64) -> Result<(), HaError> {
    if authority_silence >= StdDuration::from_secs(lease_seconds) {
        Err(HaError::PartitionSelfFence)
    } else {
        Ok(())
    }
}

pub fn reconcile_sha256(items: &[(Uuid, String)]) -> String {
    let mut rows = items.to_vec();
    rows.sort_by_key(|r| r.0);
    let mut h = Sha256::new();
    for (id, state_hash) in rows {
        h.update(id.as_bytes());
        h.update([0]);
        h.update(state_hash.as_bytes());
        h.update([0xff]);
    }
    format!("{:x}", h.finalize())
}

/// Rendezvous hashing gives stable ownership and minimal reassignment when membership changes.
pub fn assign_shard(
    key: Uuid,
    controllers: &[ControllerMember],
    now: DateTime<Utc>,
    dead_after: ChronoDuration,
) -> Result<Uuid, HaError> {
    let mut best: Option<(Vec<u8>, Uuid)> = None;
    for c in controllers.iter().filter(|c| {
        !matches!(
            c.state,
            ControllerState::Offline | ControllerState::SelfFenced | ControllerState::Draining
        ) && now - c.last_heartbeat_at <= dead_after
    }) {
        let mut h = Sha256::new();
        h.update(key.as_bytes());
        h.update(c.controller_uuid.as_bytes());
        let score = h.finalize().to_vec();
        if best
            .as_ref()
            .map(|(b, _)| score.as_slice() > b.as_slice())
            .unwrap_or(true)
        {
            best = Some((score, c.controller_uuid));
        }
    }
    best.map(|(_, id)| id).ok_or(HaError::ControllerUnhealthy)
}

pub fn healthy_by_region(
    controllers: &[ControllerMember],
    now: DateTime<Utc>,
    dead_after: ChronoDuration,
) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for c in controllers.iter().filter(|c| {
        now - c.last_heartbeat_at <= dead_after
            && !matches!(
                c.state,
                ControllerState::Offline | ControllerState::SelfFenced
            )
    }) {
        *out.entry(c.region.clone()).or_insert(0) += 1;
    }
    out
}

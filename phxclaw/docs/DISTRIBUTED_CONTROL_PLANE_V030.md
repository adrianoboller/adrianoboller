# Distributed Control Plane v0.30

## Architecture

`N PhxClaw controllers -> one HA PostgreSQL authority -> Event/Evidence state -> F22/F28/F29 fleet runtimes`.

PostgreSQL database time is authoritative for leadership. The lease row carries a monotonically increasing epoch. State-changing operations must present `(controller_uuid, leader_epoch, source_state_sha256)` and are rejected on expired ownership, stale epoch, or source-state drift.

## Failover and partitions
- First lease starts at epoch 1.
- A healthy active leader renews the same epoch; it does not silently manufacture a new fence.
- After lease expiry, the next successful claimant increments the durable epoch.
- A stale leader cannot mutate after failover because its epoch is rejected.
- A controller unable to contact the authority store for a full lease TTL self-fences. Minority/isolated partitions sacrifice write availability instead of risking split brain.

## Reconciliation and sharding
Followers reconstruct current/desired state from PostgreSQL and Event/Evidence streams. Reconcile commits are leader-fenced and source-hash-bound. Non-leader distributed work uses rendezvous hashing across healthy controllers so membership changes move only the necessary shards.

## HA boundary
This removes the PhxClaw controller process as a single point of failure, but it does not create a database consensus system. Production requires PostgreSQL itself to be highly available. A single loopback PostgreSQL process is not an HA deployment.

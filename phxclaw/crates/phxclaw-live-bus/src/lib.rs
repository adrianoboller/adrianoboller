use chrono::Utc;
use phxclaw_event_bus::EventEnvelope;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
};
use thiserror::Error;
use tokio::sync::broadcast;
use uuid::Uuid;

/// In-process fan-out bus used by the Desktop Host, API Gateway and UI bridge.
/// PostgreSQL Outbox remains the durable publication source; this hub is the
/// low-latency delivery plane.
#[derive(Clone)]
pub struct LiveEventHub {
    tx: broadcast::Sender<EventEnvelope>,
    replay: Arc<RwLock<VecDeque<EventEnvelope>>>,
    replay_capacity: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveBusStats {
    pub replay_capacity: usize,
    pub replay_len: usize,
    pub receiver_count: usize,
}

#[derive(Debug, Error)]
pub enum LiveBusError {
    #[error("event topic cannot be empty")]
    EmptyTopic,
    #[error("event type cannot be empty")]
    EmptyEventType,
    #[error("replay lock poisoned")]
    Poisoned,
}

impl LiveEventHub {
    pub fn new(channel_capacity: usize, replay_capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(channel_capacity.max(16));
        Self {
            tx,
            replay: Arc::new(RwLock::new(VecDeque::with_capacity(replay_capacity.max(1)))),
            replay_capacity: replay_capacity.max(1),
        }
    }

    pub fn publish(&self, event: EventEnvelope) -> Result<usize, LiveBusError> {
        validate(&event)?;
        {
            let mut replay = self.replay.write().map_err(|_| LiveBusError::Poisoned)?;
            while replay.len() >= self.replay_capacity {
                replay.pop_front();
            }
            replay.push_back(event.clone());
        }
        // broadcast::send fails only when there are zero receivers. The event
        // still belongs in replay, so zero live receivers is not an error.
        Ok(self.tx.send(event).unwrap_or(0))
    }

    pub fn publish_json(
        &self,
        topic: impl Into<String>,
        event_type: impl Into<String>,
        payload: Value,
        correlation_uuid: Option<Uuid>,
        causation_uuid: Option<Uuid>,
    ) -> Result<EventEnvelope, LiveBusError> {
        let mut event = EventEnvelope::new(topic, event_type, payload);
        event.correlation_uuid = correlation_uuid;
        event.causation_uuid = causation_uuid;
        self.publish(event.clone())?;
        Ok(event)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.tx.subscribe()
    }

    pub fn snapshot(&self, limit: usize) -> Result<Vec<EventEnvelope>, LiveBusError> {
        let replay = self.replay.read().map_err(|_| LiveBusError::Poisoned)?;
        let take = limit.max(1).min(replay.len());
        Ok(replay
            .iter()
            .skip(replay.len().saturating_sub(take))
            .cloned()
            .collect())
    }

    pub fn since(
        &self,
        event_uuid: Uuid,
        limit: usize,
    ) -> Result<Vec<EventEnvelope>, LiveBusError> {
        let replay = self.replay.read().map_err(|_| LiveBusError::Poisoned)?;
        let start = replay
            .iter()
            .position(|e| e.uuid == event_uuid)
            .map(|i| i + 1)
            .unwrap_or(0);
        Ok(replay
            .iter()
            .skip(start)
            .take(limit.max(1))
            .cloned()
            .collect())
    }

    pub fn stats(&self) -> Result<LiveBusStats, LiveBusError> {
        let replay = self.replay.read().map_err(|_| LiveBusError::Poisoned)?;
        Ok(LiveBusStats {
            replay_capacity: self.replay_capacity,
            replay_len: replay.len(),
            receiver_count: self.tx.receiver_count(),
        })
    }

    /// Generates a structured diagnostic event when a receiver falls behind.
    /// The consumer decides whether to publish it, avoiding recursive publish loops.
    pub fn lag_event(dropped: u64, consumer: &str) -> EventEnvelope {
        EventEnvelope::new(
            "system.stream",
            "stream_lagged",
            json!({
                "consumer": consumer,
                "dropped": dropped,
                "observed_at": Utc::now(),
            }),
        )
    }
}

fn validate(event: &EventEnvelope) -> Result<(), LiveBusError> {
    if event.topic.trim().is_empty() {
        return Err(LiveBusError::EmptyTopic);
    }
    if event.event_type.trim().is_empty() {
        return Err(LiveBusError::EmptyEventType);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publish_replay_and_receive() {
        let hub = LiveEventHub::new(32, 3);
        let mut rx = hub.subscribe();
        let event = hub
            .publish_json("test", "created", json!({"ok": true}), None, None)
            .unwrap();
        assert_eq!(rx.recv().await.unwrap().uuid, event.uuid);
        assert_eq!(hub.snapshot(10).unwrap().len(), 1);
    }

    #[test]
    fn replay_is_bounded() {
        let hub = LiveEventHub::new(16, 2);
        for n in 0..4 {
            hub.publish_json("test", "n", json!({"n": n}), None, None)
                .unwrap();
        }
        let replay = hub.snapshot(10).unwrap();
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].payload["n"], 2);
        assert_eq!(replay[1].payload["n"], 3);
    }
}

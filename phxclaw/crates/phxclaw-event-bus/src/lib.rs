use chrono::{DateTime, Utc};
use phxclaw_types::new_uuid_v7;
use postgres::{Client, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub uuid: Uuid,
    pub topic: String,
    pub event_type: String,
    pub aggregate_uuid: Option<Uuid>,
    pub correlation_uuid: Option<Uuid>,
    pub causation_uuid: Option<Uuid>,
    pub payload: Value,
    pub occurred_at: DateTime<Utc>,
    pub schema_version: u16,
}

impl EventEnvelope {
    pub fn new(topic: impl Into<String>, event_type: impl Into<String>, payload: Value) -> Self {
        Self {
            uuid: new_uuid_v7(),
            topic: topic.into(),
            event_type: event_type.into(),
            aggregate_uuid: None,
            correlation_uuid: None,
            causation_uuid: None,
            payload,
            occurred_at: Utc::now(),
            schema_version: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxMessage {
    pub sequence: i64,
    pub event: EventEnvelope,
    pub attempts: i32,
    pub available_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum EventBusError {
    #[error("topic cannot be empty")]
    EmptyTopic,
    #[error("event type cannot be empty")]
    EmptyEventType,
    #[error("PostgreSQL error: {0}")]
    Postgres(#[from] postgres::Error),
}

pub struct PostgresOutbox<'a> {
    client: &'a mut Client,
}

impl<'a> PostgresOutbox<'a> {
    pub fn new(client: &'a mut Client) -> Self {
        Self { client }
    }

    pub fn enqueue(&mut self, event: &EventEnvelope) -> Result<i64, EventBusError> {
        validate_event(event)?;
        let row = self.client.query_one(
            r#"
            INSERT INTO phoenix_outbox
                (event_uuid, topic, event_type, aggregate_uuid, correlation_uuid,
                 causation_uuid, payload, occurred_at, schema_version)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            RETURNING sequence
            "#,
            &[
                &event.uuid,
                &event.topic,
                &event.event_type,
                &event.aggregate_uuid,
                &event.correlation_uuid,
                &event.causation_uuid,
                &event.payload,
                &event.occurred_at,
                &(event.schema_version as i32),
            ],
        )?;
        Ok(row.get(0))
    }

    pub fn enqueue_in_transaction(
        tx: &mut Transaction<'_>,
        event: &EventEnvelope,
    ) -> Result<i64, EventBusError> {
        validate_event(event)?;
        let row = tx.query_one(
            r#"
            INSERT INTO phoenix_outbox
                (event_uuid, topic, event_type, aggregate_uuid, correlation_uuid,
                 causation_uuid, payload, occurred_at, schema_version)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            RETURNING sequence
            "#,
            &[
                &event.uuid,
                &event.topic,
                &event.event_type,
                &event.aggregate_uuid,
                &event.correlation_uuid,
                &event.causation_uuid,
                &event.payload,
                &event.occurred_at,
                &(event.schema_version as i32),
            ],
        )?;
        Ok(row.get(0))
    }

    pub fn claim_batch(&mut self, limit: i64) -> Result<Vec<OutboxMessage>, EventBusError> {
        let mut tx = self.client.transaction()?;
        let rows = tx.query(
            r#"
            SELECT sequence, event_uuid, topic, event_type, aggregate_uuid,
                   correlation_uuid, causation_uuid, payload, occurred_at,
                   schema_version, attempts, available_at
              FROM phoenix_outbox
             WHERE published_at IS NULL
               AND available_at <= now()
             ORDER BY sequence
             FOR UPDATE SKIP LOCKED
             LIMIT $1
            "#,
            &[&limit],
        )?;

        let messages = rows
            .iter()
            .map(|row| OutboxMessage {
                sequence: row.get("sequence"),
                event: EventEnvelope {
                    uuid: row.get("event_uuid"),
                    topic: row.get("topic"),
                    event_type: row.get("event_type"),
                    aggregate_uuid: row.get("aggregate_uuid"),
                    correlation_uuid: row.get("correlation_uuid"),
                    causation_uuid: row.get("causation_uuid"),
                    payload: row.get("payload"),
                    occurred_at: row.get("occurred_at"),
                    schema_version: row.get::<_, i32>("schema_version") as u16,
                },
                attempts: row.get("attempts"),
                available_at: row.get("available_at"),
            })
            .collect::<Vec<_>>();

        let seqs = messages.iter().map(|m| m.sequence).collect::<Vec<_>>();
        if !seqs.is_empty() {
            tx.execute(
                "UPDATE phoenix_outbox SET attempts = attempts + 1, locked_at = now() WHERE sequence = ANY($1)",
                &[&seqs],
            )?;
        }
        tx.commit()?;
        Ok(messages)
    }

    pub fn mark_published(&mut self, sequence: i64) -> Result<(), EventBusError> {
        self.client.execute(
            "UPDATE phoenix_outbox SET published_at = now(), last_error = NULL WHERE sequence = $1",
            &[&sequence],
        )?;
        Ok(())
    }

    pub fn reschedule(
        &mut self,
        sequence: i64,
        error: &str,
        delay_seconds: i64,
    ) -> Result<(), EventBusError> {
        self.client.execute(
            "UPDATE phoenix_outbox SET last_error = $2, available_at = now() + ($3::bigint * interval '1 second') WHERE sequence = $1",
            &[&sequence, &error, &delay_seconds],
        )?;
        Ok(())
    }
}

fn validate_event(event: &EventEnvelope) -> Result<(), EventBusError> {
    if event.topic.trim().is_empty() {
        return Err(EventBusError::EmptyTopic);
    }
    if event.event_type.trim().is_empty() {
        return Err(EventBusError::EmptyEventType);
    }
    Ok(())
}

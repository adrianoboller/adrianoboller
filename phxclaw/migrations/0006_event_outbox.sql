BEGIN;
CREATE TABLE IF NOT EXISTS phoenix_outbox (
    sequence BIGSERIAL PRIMARY KEY,
    event_uuid UUID NOT NULL UNIQUE,
    topic TEXT NOT NULL CHECK (length(btrim(topic)) > 0),
    event_type TEXT NOT NULL CHECK (length(btrim(event_type)) > 0),
    aggregate_uuid UUID NULL,
    correlation_uuid UUID NULL,
    causation_uuid UUID NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    occurred_at TIMESTAMPTZ NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version > 0),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    locked_at TIMESTAMPTZ NULL,
    published_at TIMESTAMPTZ NULL,
    last_error TEXT NULL
);
CREATE INDEX IF NOT EXISTS idx_phoenix_outbox_ready
ON phoenix_outbox (available_at, sequence) WHERE published_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_phoenix_outbox_topic
ON phoenix_outbox (topic, sequence);
COMMIT;

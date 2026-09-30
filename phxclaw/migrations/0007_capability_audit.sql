BEGIN;
CREATE TABLE IF NOT EXISTS phoenix_capability_audit (
    sequence BIGSERIAL PRIMARY KEY,
    event_uuid UUID NOT NULL UNIQUE,
    actor_uuid UUID NULL,
    task_uuid UUID NULL,
    capability TEXT NOT NULL,
    permission TEXT NULL,
    target TEXT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('allow','deny','error')),
    request JSONB NOT NULL DEFAULT '{}'::jsonb,
    result JSONB NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_phoenix_capability_audit_actor ON phoenix_capability_audit(actor_uuid, sequence DESC);
CREATE INDEX IF NOT EXISTS idx_phoenix_capability_audit_capability ON phoenix_capability_audit(capability, sequence DESC);
COMMIT;

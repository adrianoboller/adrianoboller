BEGIN;

CREATE TABLE IF NOT EXISTS phoenix_desktop_sessions (
    session_uuid UUID PRIMARY KEY,
    host_name TEXT NULL,
    operating_system TEXT NULL,
    app_version TEXT NOT NULL,
    api_addr TEXT NULL,
    policy JSONB NOT NULL DEFAULT '{}'::jsonb,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    stopped_at TIMESTAMPTZ NULL
);

CREATE TABLE IF NOT EXISTS phoenix_desktop_actions (
    action_uuid UUID PRIMARY KEY,
    session_uuid UUID NULL REFERENCES phoenix_desktop_sessions(session_uuid) ON DELETE SET NULL,
    correlation_uuid UUID NULL,
    actor TEXT NOT NULL,
    capability TEXT NOT NULL,
    action TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('accepted','succeeded','failed','denied')),
    request_summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    result_summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    evidence_uuid UUID NULL UNIQUE,
    requested_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ NULL
);
CREATE INDEX IF NOT EXISTS idx_phoenix_desktop_actions_session_time
    ON phoenix_desktop_actions(session_uuid, requested_at DESC);
CREATE INDEX IF NOT EXISTS idx_phoenix_desktop_actions_correlation
    ON phoenix_desktop_actions(correlation_uuid) WHERE correlation_uuid IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_phoenix_desktop_actions_capability
    ON phoenix_desktop_actions(capability, requested_at DESC);

CREATE TABLE IF NOT EXISTS phoenix_evidence_records (
    evidence_uuid UUID PRIMARY KEY,
    action_uuid UUID NOT NULL,
    correlation_uuid UUID NULL,
    actor TEXT NOT NULL,
    capability TEXT NOT NULL,
    action TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('requested','succeeded','failed','denied','cancelled')),
    request_summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    result_summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    artifact_uris JSONB NOT NULL DEFAULT '[]'::jsonb,
    occurred_at TIMESTAMPTZ NOT NULL,
    previous_hash TEXT NULL,
    record_hash TEXT NOT NULL UNIQUE,
    CONSTRAINT phoenix_evidence_hash_shape CHECK (record_hash ~ '^[0-9a-f]{64}$'),
    CONSTRAINT phoenix_evidence_previous_hash_shape CHECK (previous_hash IS NULL OR previous_hash ~ '^[0-9a-f]{64}$')
);
CREATE INDEX IF NOT EXISTS idx_phoenix_evidence_action
    ON phoenix_evidence_records(action_uuid, occurred_at);
CREATE INDEX IF NOT EXISTS idx_phoenix_evidence_correlation
    ON phoenix_evidence_records(correlation_uuid, occurred_at) WHERE correlation_uuid IS NOT NULL;

COMMIT;

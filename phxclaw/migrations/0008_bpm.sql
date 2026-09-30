BEGIN;
CREATE TABLE IF NOT EXISTS phoenix_mindset_profiles (
    profile_uuid UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    version TEXT NOT NULL,
    principles JSONB NOT NULL,
    priorities JSONB NOT NULL,
    approval_rules JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS phoenix_bpm_processes (
    process_uuid UUID PRIMARY KEY,
    external_id TEXT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    bpmn_xml TEXT NOT NULL,
    compiled_graph JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(external_id, version)
);
CREATE TABLE IF NOT EXISTS phoenix_bpm_instances (
    instance_uuid UUID PRIMARY KEY,
    process_uuid UUID NOT NULL REFERENCES phoenix_bpm_processes(process_uuid),
    state TEXT NOT NULL,
    variables JSONB NOT NULL DEFAULT '{}'::jsonb,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ NULL
);
CREATE TABLE IF NOT EXISTS phoenix_bpm_tokens (
    token_uuid UUID PRIMARY KEY,
    instance_uuid UUID NOT NULL REFERENCES phoenix_bpm_instances(instance_uuid) ON DELETE CASCADE,
    node_id TEXT NOT NULL,
    state TEXT NOT NULL,
    lease_owner TEXT NULL,
    lease_until TIMESTAMPTZ NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS phoenix_bpm_history (
    sequence BIGSERIAL PRIMARY KEY,
    event_uuid UUID NOT NULL UNIQUE,
    instance_uuid UUID NOT NULL REFERENCES phoenix_bpm_instances(instance_uuid) ON DELETE CASCADE,
    token_uuid UUID NULL,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_phoenix_bpm_history_instance ON phoenix_bpm_history(instance_uuid, sequence);
COMMIT;

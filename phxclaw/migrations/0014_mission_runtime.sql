CREATE TABLE IF NOT EXISTS phoenix_missions (
    uuid UUID PRIMARY KEY,
    correlation_uuid UUID NOT NULL,
    objective_sha256 TEXT NOT NULL,
    project_root TEXT NOT NULL,
    base_ref TEXT NOT NULL,
    state TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS phoenix_missions_correlation_idx
    ON phoenix_missions (correlation_uuid, created_at DESC);

CREATE TABLE IF NOT EXISTS phoenix_mission_steps (
    uuid UUID PRIMARY KEY,
    mission_uuid UUID NOT NULL REFERENCES phoenix_missions(uuid) ON DELETE CASCADE,
    name TEXT NOT NULL,
    agent_capability TEXT NOT NULL,
    action JSONB NOT NULL,
    required BOOLEAN NOT NULL,
    status TEXT NOT NULL,
    agent_uuid UUID,
    agent_name TEXT,
    evidence_uuid UUID,
    output JSONB NOT NULL DEFAULT '{}'::jsonb,
    finished_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS phoenix_mission_steps_mission_idx
    ON phoenix_mission_steps (mission_uuid, finished_at);

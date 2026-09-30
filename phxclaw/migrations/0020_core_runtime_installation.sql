-- PhxClaw v0.20 unified runtime/bootstrap observability.
CREATE TABLE IF NOT EXISTS phoenix_core_runtime_instances (
    runtime_uuid uuid PRIMARY KEY,
    version text NOT NULL,
    host_name text,
    started_at timestamptz NOT NULL DEFAULT now(),
    stopped_at timestamptz,
    status jsonb NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS phoenix_install_events (
    event_uuid uuid PRIMARY KEY,
    component text NOT NULL,
    action text NOT NULL,
    outcome text NOT NULL,
    evidence jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS phoenix_install_events_created_idx
    ON phoenix_install_events(created_at DESC);

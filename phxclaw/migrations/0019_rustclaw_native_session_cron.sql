-- PhxClaw v0.19 — MIT-derived RustClaw native compatibility persistence.
-- PostgreSQL remains the official state store; the vendored RustClaw SQLite store is not adopted.

CREATE TABLE IF NOT EXISTS phoenix_native_sessions (
    uuid UUID PRIMARY KEY,
    principal_uuid UUID NULL,
    channel TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS phoenix_native_session_messages (
    uuid UUID PRIMARY KEY,
    session_uuid UUID NOT NULL REFERENCES phoenix_native_sessions(uuid) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    content_sha256 TEXT NOT NULL CHECK (length(content_sha256) = 64),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_native_session_messages_session
    ON phoenix_native_session_messages(session_uuid, created_at);

CREATE TABLE IF NOT EXISTS phoenix_scheduled_jobs (
    uuid UUID PRIMARY KEY,
    name TEXT NOT NULL,
    capability TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    schedule JSONB NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('disabled','ready','running','failed','cancelled')),
    next_run_at TIMESTAMPTZ NULL,
    last_run_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_ready
    ON phoenix_scheduled_jobs(state, next_run_at);

-- PhxClaw v0.64 — MCP/LSP managed runtime evidence store
CREATE TABLE IF NOT EXISTS phxclaw.protocol_sessions (
    session_uuid uuid PRIMARY KEY,
    tenant_uuid uuid NOT NULL,
    kind text NOT NULL CHECK (kind IN ('mcp_stdio','mcp_streamable_http','lsp_stdio')),
    endpoint_label text NOT NULL,
    transport text NOT NULL CHECK (transport IN ('stdio','streamable_http')),
    protocol_version text,
    state text NOT NULL CHECK (state IN ('starting','ready','degraded','closing','closed','failed')),
    restart_count integer NOT NULL DEFAULT 0 CHECK (restart_count >= 0),
    started_at timestamptz NOT NULL DEFAULT now(),
    closed_at timestamptz,
    source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$')
);

CREATE TABLE IF NOT EXISTS phxclaw.protocol_requests (
    request_uuid uuid PRIMARY KEY,
    session_uuid uuid NOT NULL REFERENCES phxclaw.protocol_sessions(session_uuid),
    tenant_uuid uuid NOT NULL,
    jsonrpc_id text,
    method text NOT NULL,
    capability text NOT NULL,
    status text NOT NULL CHECK (status IN ('requested','succeeded','failed','cancelled','timeout','denied')),
    requested_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz,
    duration_ms bigint CHECK (duration_ms IS NULL OR duration_ms >= 0),
    error_code text,
    evidence_uuid uuid
);

CREATE TABLE IF NOT EXISTS phxclaw.protocol_session_events (
    event_uuid uuid PRIMARY KEY,
    session_uuid uuid NOT NULL REFERENCES phxclaw.protocol_sessions(session_uuid),
    tenant_uuid uuid NOT NULL,
    event_type text NOT NULL,
    payload jsonb NOT NULL DEFAULT '{}'::jsonb,
    occurred_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_protocol_requests_session_time
    ON phxclaw.protocol_requests(session_uuid, requested_at DESC);
CREATE INDEX IF NOT EXISTS idx_protocol_events_session_time
    ON phxclaw.protocol_session_events(session_uuid, occurred_at DESC);

CREATE OR REPLACE FUNCTION phxclaw.protocol_append_only()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'protocol audit rows are append-only';
END $$;

DROP TRIGGER IF EXISTS trg_protocol_requests_append_only ON phxclaw.protocol_requests;
CREATE TRIGGER trg_protocol_requests_append_only
BEFORE UPDATE OR DELETE ON phxclaw.protocol_requests
FOR EACH ROW EXECUTE FUNCTION phxclaw.protocol_append_only();

DROP TRIGGER IF EXISTS trg_protocol_events_append_only ON phxclaw.protocol_session_events;
CREATE TRIGGER trg_protocol_events_append_only
BEFORE UPDATE OR DELETE ON phxclaw.protocol_session_events
FOR EACH ROW EXECUTE FUNCTION phxclaw.protocol_append_only();

ALTER TABLE phxclaw.protocol_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.protocol_sessions FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.protocol_requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.protocol_requests FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.protocol_session_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.protocol_session_events FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS protocol_sessions_tenant ON phxclaw.protocol_sessions;
CREATE POLICY protocol_sessions_tenant ON phxclaw.protocol_sessions
USING (tenant_uuid = phxclaw.current_tenant_uuid())
WITH CHECK (tenant_uuid = phxclaw.current_tenant_uuid());
DROP POLICY IF EXISTS protocol_requests_tenant ON phxclaw.protocol_requests;
CREATE POLICY protocol_requests_tenant ON phxclaw.protocol_requests
USING (tenant_uuid = phxclaw.current_tenant_uuid())
WITH CHECK (tenant_uuid = phxclaw.current_tenant_uuid());
DROP POLICY IF EXISTS protocol_events_tenant ON phxclaw.protocol_session_events;
CREATE POLICY protocol_events_tenant ON phxclaw.protocol_session_events
USING (tenant_uuid = phxclaw.current_tenant_uuid())
WITH CHECK (tenant_uuid = phxclaw.current_tenant_uuid());

-- PhxClaw v0.48 Executive Project Control Tower
CREATE TABLE IF NOT EXISTS phx_executive_portfolio_snapshots (
 tenant_uuid uuid NOT NULL,
 snapshot_uuid uuid NOT NULL,
 generated_at timestamptz NOT NULL DEFAULT now(),
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 snapshot_sha256 text NOT NULL CHECK (snapshot_sha256 ~ '^[0-9a-f]{64}$'),
 payload jsonb NOT NULL,
 PRIMARY KEY (tenant_uuid, snapshot_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_project_snapshots (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 snapshot_uuid uuid NOT NULL,
 portfolio_snapshot_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 health_score double precision NOT NULL CHECK (health_score BETWEEN 0 AND 1),
 attention_score double precision NOT NULL,
 payload jsonb NOT NULL,
 PRIMARY KEY (tenant_uuid, project_uuid, snapshot_uuid),
 FOREIGN KEY (tenant_uuid, portfolio_snapshot_uuid) REFERENCES phx_executive_portfolio_snapshots(tenant_uuid, snapshot_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_alerts (
 tenant_uuid uuid NOT NULL,
 alert_uuid uuid NOT NULL,
 project_uuid uuid NULL,
 severity text NOT NULL CHECK (severity IN ('low','medium','high','critical')),
 kind text NOT NULL,
 title text NOT NULL,
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT now(),
 acknowledged_at timestamptz NULL,
 PRIMARY KEY (tenant_uuid, alert_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_decision_requests (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 request_uuid uuid NOT NULL,
 supervisor_plan_uuid uuid NOT NULL,
 action_kind text NOT NULL,
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 requires_approval boolean NOT NULL DEFAULT true,
 status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected','expired')),
 created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, project_uuid, request_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_events (
 tenant_uuid uuid NOT NULL,
 event_uuid uuid NOT NULL,
 project_uuid uuid NULL,
 event_type text NOT NULL,
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 payload jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, event_uuid)
);

DO $$ DECLARE t text; BEGIN
 FOR t IN SELECT unnest(ARRAY['phx_executive_portfolio_snapshots','phx_executive_project_snapshots','phx_executive_alerts','phx_executive_decision_requests','phx_executive_events']) LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
  EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
  EXECUTE format('DROP POLICY IF EXISTS phx_tenant_isolation ON %I', t);
  EXECUTE format($policy$CREATE POLICY phx_tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)$policy$, t);
 END LOOP;
END $$;

CREATE OR REPLACE FUNCTION phx_executive_append_only() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only executive record'; END $$;
DROP TRIGGER IF EXISTS phx_executive_portfolio_append_only ON phx_executive_portfolio_snapshots;
CREATE TRIGGER phx_executive_portfolio_append_only BEFORE UPDATE OR DELETE ON phx_executive_portfolio_snapshots FOR EACH ROW EXECUTE FUNCTION phx_executive_append_only();
DROP TRIGGER IF EXISTS phx_executive_project_append_only ON phx_executive_project_snapshots;
CREATE TRIGGER phx_executive_project_append_only BEFORE UPDATE OR DELETE ON phx_executive_project_snapshots FOR EACH ROW EXECUTE FUNCTION phx_executive_append_only();
DROP TRIGGER IF EXISTS phx_executive_events_append_only ON phx_executive_events;
CREATE TRIGGER phx_executive_events_append_only BEFORE UPDATE OR DELETE ON phx_executive_events FOR EACH ROW EXECUTE FUNCTION phx_executive_append_only();

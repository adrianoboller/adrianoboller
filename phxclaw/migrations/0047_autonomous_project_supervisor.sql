-- PhxClaw v0.47 Autonomous Project Supervisor
CREATE TABLE IF NOT EXISTS phx_project_supervisor_health (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL, source_state_sha256 text NOT NULL, payload jsonb NOT NULL, evidence_sha256 text NOT NULL, snapshot_sha256 text NOT NULL, generated_at timestamptz NOT NULL, expires_at timestamptz NOT NULL, PRIMARY KEY (tenant_uuid, project_uuid, snapshot_uuid));
CREATE TABLE IF NOT EXISTS phx_project_supervisor_cycles (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, cycle_uuid uuid NOT NULL, source_state_sha256 text NOT NULL, phase text NOT NULL, payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY (tenant_uuid, project_uuid, cycle_uuid));
CREATE TABLE IF NOT EXISTS phx_project_supervisor_plans (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL, source_state_sha256 text NOT NULL, policy_sha256 text NOT NULL, plan_sha256 text NOT NULL, payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(), expires_at timestamptz NOT NULL, PRIMARY KEY (tenant_uuid, project_uuid, plan_uuid), FOREIGN KEY (tenant_uuid, project_uuid, snapshot_uuid) REFERENCES phx_project_supervisor_health(tenant_uuid, project_uuid, snapshot_uuid));
CREATE TABLE IF NOT EXISTS phx_project_supervisor_actions (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, action_uuid uuid NOT NULL, kind text NOT NULL, status text NOT NULL, requires_approval boolean NOT NULL, evidence_sha256 text, created_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY (tenant_uuid, project_uuid, action_uuid), FOREIGN KEY (tenant_uuid, project_uuid, plan_uuid) REFERENCES phx_project_supervisor_plans(tenant_uuid, project_uuid, plan_uuid));
CREATE TABLE IF NOT EXISTS phx_project_supervisor_events (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, event_uuid uuid PRIMARY KEY, correlation_uuid uuid NOT NULL, event_type text NOT NULL, payload jsonb NOT NULL, evidence_sha256 text NOT NULL, created_at timestamptz NOT NULL DEFAULT now());

DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['phx_project_supervisor_health','phx_project_supervisor_cycles','phx_project_supervisor_plans','phx_project_supervisor_actions','phx_project_supervisor_events'] LOOP
 EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
 EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
 EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I', t);
 EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), )::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(phxclaw.tenant_uuid, true), )::uuid)', t);
END LOOP; END $$;

CREATE OR REPLACE FUNCTION phx_supervisor_append_only() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only supervisor record'; END $$;
DROP TRIGGER IF EXISTS phx_supervisor_events_append_only ON phx_project_supervisor_events; CREATE TRIGGER phx_supervisor_events_append_only BEFORE UPDATE OR DELETE ON phx_project_supervisor_events FOR EACH ROW EXECUTE FUNCTION phx_supervisor_append_only();
DROP TRIGGER IF EXISTS phx_supervisor_health_append_only ON phx_project_supervisor_health; CREATE TRIGGER phx_supervisor_health_append_only BEFORE UPDATE OR DELETE ON phx_project_supervisor_health FOR EACH ROW EXECUTE FUNCTION phx_supervisor_append_only();

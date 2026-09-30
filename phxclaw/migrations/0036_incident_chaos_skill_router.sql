BEGIN;

CREATE TABLE IF NOT EXISTS phx_skill_sources_v036 (
  tenant_uuid uuid NOT NULL,
  source_uuid uuid NOT NULL,
  skill_id text NOT NULL,
  source_url text NOT NULL,
  license_expression text NOT NULL,
  license_status text NOT NULL,
  source_sha256 text NOT NULL CHECK (source_sha256 ~ '^[0-9a-fA-F]{64}$'),
  observed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  supersedes_source_uuid uuid NULL,
  PRIMARY KEY (tenant_uuid, source_uuid),
  UNIQUE (tenant_uuid, skill_id, source_sha256)
);
CREATE TABLE IF NOT EXISTS phx_skill_routes_v036 (
  tenant_uuid uuid NOT NULL, route_uuid uuid NOT NULL, request_uuid uuid NOT NULL,
  catalog_sha256 text NOT NULL CHECK (catalog_sha256 ~ '^[0-9a-fA-F]{64}$'),
  plan_sha256 text NOT NULL CHECK (plan_sha256 ~ '^[0-9a-fA-F]{64}$'),
  plan_json jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, route_uuid), UNIQUE (tenant_uuid, request_uuid, plan_sha256)
);
CREATE TABLE IF NOT EXISTS phx_skill_route_evidence_v036 (
  tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, route_uuid uuid NOT NULL,
  skill_id text NOT NULL, input_sha256 text NOT NULL CHECK (input_sha256 ~ '^[0-9a-fA-F]{64}$'),
  output_sha256 text NOT NULL CHECK (output_sha256 ~ '^[0-9a-fA-F]{64}$'), evidence_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,evidence_uuid),
  FOREIGN KEY (tenant_uuid,route_uuid) REFERENCES phx_skill_routes_v036(tenant_uuid,route_uuid)
);
CREATE TABLE IF NOT EXISTS phx_incidents_v036 (
  tenant_uuid uuid NOT NULL, incident_uuid uuid NOT NULL, severity text NOT NULL,
  title text NOT NULL, detection_evidence_sha256 text NOT NULL CHECK (detection_evidence_sha256 ~ '^[0-9a-fA-F]{64}$'),
  declared_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,incident_uuid)
);
CREATE TABLE IF NOT EXISTS phx_incident_events_v036 (
  tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, incident_uuid uuid NOT NULL,
  event_type text NOT NULL, evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-fA-F]{64}$'), payload jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,event_uuid),
  FOREIGN KEY (tenant_uuid,incident_uuid) REFERENCES phx_incidents_v036(tenant_uuid,incident_uuid)
);
CREATE TABLE IF NOT EXISTS phx_incident_actions_v036 (
  tenant_uuid uuid NOT NULL, action_uuid uuid NOT NULL, incident_uuid uuid NOT NULL,
  capability text NOT NULL, approval_uuid uuid NULL, leader_epoch bigint NULL, fencing_token bigint NULL,
  input_sha256 text NOT NULL CHECK (input_sha256 ~ '^[0-9a-fA-F]{64}$'), status text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,action_uuid),
  FOREIGN KEY (tenant_uuid,incident_uuid) REFERENCES phx_incidents_v036(tenant_uuid,incident_uuid)
);
CREATE TABLE IF NOT EXISTS phx_resilience_runbooks_v036 (
  tenant_uuid uuid NOT NULL, runbook_uuid uuid NOT NULL, name text NOT NULL, version text NOT NULL,
  content_sha256 text NOT NULL CHECK (content_sha256 ~ '^[0-9a-fA-F]{64}$'), content_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,runbook_uuid), UNIQUE(tenant_uuid,name,version)
);
CREATE TABLE IF NOT EXISTS phx_chaos_experiments_v036 (
  tenant_uuid uuid NOT NULL, experiment_uuid uuid NOT NULL, environment text NOT NULL, fault_kind text NOT NULL,
  blast_radius_basis_points integer NOT NULL CHECK (blast_radius_basis_points BETWEEN 0 AND 10000),
  steady_state_sha256 text NOT NULL CHECK (steady_state_sha256 ~ '^[0-9a-fA-F]{64}$'),
  abort_conditions_sha256 text NOT NULL CHECK (abort_conditions_sha256 ~ '^[0-9a-fA-F]{64}$'),
  recovery_runbook_uuid uuid NOT NULL, approval_uuid uuid NULL, leader_epoch bigint NULL, fencing_token bigint NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,experiment_uuid),
  FOREIGN KEY (tenant_uuid,recovery_runbook_uuid) REFERENCES phx_resilience_runbooks_v036(tenant_uuid,runbook_uuid)
);
CREATE TABLE IF NOT EXISTS phx_chaos_events_v036 (
  tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, experiment_uuid uuid NOT NULL, event_type text NOT NULL,
  evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-fA-F]{64}$'), payload jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,event_uuid),
  FOREIGN KEY (tenant_uuid,experiment_uuid) REFERENCES phx_chaos_experiments_v036(tenant_uuid,experiment_uuid)
);
CREATE TABLE IF NOT EXISTS phx_recovery_evidence_v036 (
  tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, experiment_uuid uuid NOT NULL,
  before_sha256 text NOT NULL CHECK (before_sha256 ~ '^[0-9a-fA-F]{64}$'), after_sha256 text NOT NULL CHECK (after_sha256 ~ '^[0-9a-fA-F]{64}$'),
  steady_state_restored boolean NOT NULL, evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-fA-F]{64}$'),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,evidence_uuid),
  FOREIGN KEY (tenant_uuid,experiment_uuid) REFERENCES phx_chaos_experiments_v036(tenant_uuid,experiment_uuid)
);
CREATE TABLE IF NOT EXISTS phx_postmortems_v036 (
  tenant_uuid uuid NOT NULL, postmortem_uuid uuid NOT NULL, incident_uuid uuid NOT NULL,
  summary_sha256 text NOT NULL CHECK (summary_sha256 ~ '^[0-9a-fA-F]{64}$'), report_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,postmortem_uuid),
  FOREIGN KEY (tenant_uuid,incident_uuid) REFERENCES phx_incidents_v036(tenant_uuid,incident_uuid)
);

CREATE OR REPLACE FUNCTION phxclaw_v036_append_only() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'PhxClaw v0.36 evidence/event tables are append-only'; END $$;

DO $$ DECLARE t text; BEGIN
  FOREACH t IN ARRAY ARRAY['phx_skill_sources_v036','phx_skill_route_evidence_v036','phx_incident_events_v036','phx_chaos_events_v036','phx_recovery_evidence_v036','phx_postmortems_v036'] LOOP
    EXECUTE format('DROP TRIGGER IF EXISTS trg_v036_append_only ON %I',t);
    EXECUTE format('CREATE TRIGGER trg_v036_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phxclaw_v036_append_only()',t);
  END LOOP;
END $$;

DO $$ DECLARE t text; BEGIN
  FOREACH t IN ARRAY ARRAY['phx_skill_sources_v036','phx_skill_routes_v036','phx_skill_route_evidence_v036','phx_incidents_v036','phx_incident_events_v036','phx_incident_actions_v036','phx_resilience_runbooks_v036','phx_chaos_experiments_v036','phx_chaos_events_v036','phx_recovery_evidence_v036','phx_postmortems_v036'] LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
    EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_v036 ON %I',t);
    EXECUTE format($cmd$CREATE POLICY tenant_isolation_v036 ON %I USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)$cmd$,t);
  END LOOP;
END $$;

COMMIT;

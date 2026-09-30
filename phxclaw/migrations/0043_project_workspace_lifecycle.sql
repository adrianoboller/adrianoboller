BEGIN;
CREATE TABLE IF NOT EXISTS phx_projects (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, name text NOT NULL, slug text NOT NULL,
 manifest_sha256 text NOT NULL CHECK (manifest_sha256 ~ '^[0-9a-f]{64}$'), root_path text NOT NULL,
 template_id text, archived boolean NOT NULL DEFAULT false, created_at timestamptz NOT NULL DEFAULT now(), last_opened_at timestamptz,
 PRIMARY KEY (tenant_uuid, project_uuid), UNIQUE (tenant_uuid, slug)
);
CREATE TABLE IF NOT EXISTS phx_project_versions (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, version_uuid uuid NOT NULL, semver text, git_commit text,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'), config_revision bigint NOT NULL,
 backup_uuid uuid, created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, project_uuid, version_uuid),
 FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid)
);
CREATE TABLE IF NOT EXISTS phx_project_log_events (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, log_uuid uuid NOT NULL, severity text NOT NULL,
 category text NOT NULL, message_redacted text NOT NULL, correlation_id text, source_path text NOT NULL,
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'), observed_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, project_uuid, log_uuid),
 FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid)
);
CREATE TABLE IF NOT EXISTS phx_project_backups (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, backup_uuid uuid NOT NULL, source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 backup_path text NOT NULL, manifest_sha256 text NOT NULL CHECK (manifest_sha256 ~ '^[0-9a-f]{64}$'), file_count bigint NOT NULL CHECK(file_count>=0), byte_count bigint NOT NULL CHECK(byte_count>=0),
 verified boolean NOT NULL DEFAULT false, created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, project_uuid, backup_uuid), FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid)
);
CREATE TABLE IF NOT EXISTS phx_project_activity_bindings (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, binding_uuid uuid NOT NULL, activity_pattern text NOT NULL,
 agent_uuid uuid, skill_id text, model_route text, enabled boolean NOT NULL DEFAULT true, created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, project_uuid, binding_uuid), FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid)
);
CREATE TABLE IF NOT EXISTS phx_project_events (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, event_uuid uuid NOT NULL, event_type text NOT NULL,
 payload_sha256 text NOT NULL CHECK(payload_sha256 ~ '^[0-9a-f]{64}$'), payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, project_uuid, event_uuid), FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid)
);
DO $$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['phx_projects','phx_project_versions','phx_project_log_events','phx_project_backups','phx_project_activity_bindings','phx_project_events'] LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
  EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
  EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I', t);
  EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), )::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(phxclaw.tenant_uuid, true), )::uuid)', t);
 END LOOP;
END $$;
CREATE OR REPLACE FUNCTION phx_forbid_project_event_update() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only table'; END $$;
DROP TRIGGER IF EXISTS trg_phx_project_events_append_only ON phx_project_events;
CREATE TRIGGER trg_phx_project_events_append_only BEFORE UPDATE OR DELETE ON phx_project_events FOR EACH ROW EXECUTE FUNCTION phx_forbid_project_event_update();
COMMIT;

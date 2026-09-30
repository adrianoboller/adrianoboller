BEGIN;
CREATE TABLE IF NOT EXISTS phoenix_config_revisions (
 tenant_uuid uuid NOT NULL,
 config_uuid uuid NOT NULL,
 revision bigint NOT NULL CHECK (revision > 0),
 config_sha256 text NOT NULL CHECK (config_sha256 ~ '^[0-9a-f]{64}$'),
 config_json jsonb NOT NULL,
 actor_uuid uuid,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, config_uuid, revision)
);
CREATE TABLE IF NOT EXISTS phoenix_factory_runs (
 tenant_uuid uuid NOT NULL,
 run_uuid uuid NOT NULL,
 requirement_id text NOT NULL CHECK (length(requirement_id)>0),
 stage text NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 checkpoint_uuid uuid,
 approved boolean NOT NULL DEFAULT false,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, run_uuid)
);
ALTER TABLE phoenix_config_revisions ENABLE ROW LEVEL SECURITY;
ALTER TABLE phoenix_config_revisions FORCE ROW LEVEL SECURITY;
ALTER TABLE phoenix_factory_runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE phoenix_factory_runs FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS phoenix_config_revisions_tenant ON phoenix_config_revisions;
CREATE POLICY phoenix_config_revisions_tenant ON phoenix_config_revisions USING (tenant_uuid = NULLIF(current_setting('phoenix.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phoenix.tenant_uuid', true), '')::uuid);
DROP POLICY IF EXISTS phoenix_factory_runs_tenant ON phoenix_factory_runs;
CREATE POLICY phoenix_factory_runs_tenant ON phoenix_factory_runs USING (tenant_uuid = NULLIF(current_setting('phoenix.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phoenix.tenant_uuid', true), '')::uuid);
CREATE OR REPLACE FUNCTION phoenix_block_config_revision_mutation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'configuration revision history is append-only'; END $$;
DROP TRIGGER IF EXISTS trg_config_revision_append_only ON phoenix_config_revisions;
CREATE TRIGGER trg_config_revision_append_only BEFORE UPDATE OR DELETE ON phoenix_config_revisions FOR EACH ROW EXECUTE FUNCTION phoenix_block_config_revision_mutation();
COMMIT;

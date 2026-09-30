-- PhxClaw v0.59 — Historical Backfill & Knowledge Migration
-- Control metadata is auditable; specialized historical tables remain authoritative.
CREATE OR REPLACE FUNCTION phx_v059_deny_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'PhxClaw v0.59 append-only relation'; END $$;

CREATE TABLE phx_historical_backfill_runs (
  tenant_uuid uuid NOT NULL, run_uuid uuid NOT NULL,
  source_state_sha256 char(64) NOT NULL, registry_sha256 char(64) NOT NULL,
  batch_size integer NOT NULL CHECK(batch_size BETWEEN 1 AND 10000),
  mode text NOT NULL CHECK(mode IN ('backfill','retry_orphans','plan_only')),
  requested_by text, started_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY(tenant_uuid,run_uuid)
);
CREATE TRIGGER phx_historical_backfill_runs_immutable BEFORE UPDATE OR DELETE ON phx_historical_backfill_runs FOR EACH ROW EXECUTE FUNCTION phx_v059_deny_mutation();

CREATE TABLE phx_historical_backfill_checkpoints (
  tenant_uuid uuid NOT NULL, run_uuid uuid NOT NULL, checkpoint_uuid uuid NOT NULL,
  source_table text NOT NULL, checkpoint_seq bigint NOT NULL CHECK(checkpoint_seq>0),
  offset_rows bigint NOT NULL CHECK(offset_rows>=0), processed_count bigint NOT NULL CHECK(processed_count>=0),
  inserted_count bigint NOT NULL CHECK(inserted_count>=0), skipped_count bigint NOT NULL CHECK(skipped_count>=0),
  orphan_count bigint NOT NULL CHECK(orphan_count>=0), cursor_json jsonb NOT NULL DEFAULT '{}'::jsonb,
  evidence_sha256 char(64) NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY(tenant_uuid,run_uuid,checkpoint_uuid), UNIQUE(tenant_uuid,run_uuid,source_table,checkpoint_seq),
  FOREIGN KEY(tenant_uuid,run_uuid) REFERENCES phx_historical_backfill_runs(tenant_uuid,run_uuid)
);
CREATE INDEX phx_historical_backfill_checkpoint_latest_idx ON phx_historical_backfill_checkpoints(tenant_uuid,run_uuid,source_table,checkpoint_seq DESC);
CREATE TRIGGER phx_historical_backfill_checkpoints_immutable BEFORE UPDATE OR DELETE ON phx_historical_backfill_checkpoints FOR EACH ROW EXECUTE FUNCTION phx_v059_deny_mutation();

CREATE TABLE phx_historical_backfill_identity_map (
  tenant_uuid uuid NOT NULL, source_table text NOT NULL, source_key_sha256 char(64) NOT NULL,
  object_uuid uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY(tenant_uuid,source_table,source_key_sha256), UNIQUE(tenant_uuid,object_uuid)
);
CREATE TRIGGER phx_historical_backfill_identity_immutable BEFORE UPDATE OR DELETE ON phx_historical_backfill_identity_map FOR EACH ROW EXECUTE FUNCTION phx_v059_deny_mutation();

CREATE TABLE phx_historical_backfill_scope_map (
  tenant_uuid uuid NOT NULL, scope_kind text NOT NULL CHECK(scope_kind IN ('portfolio','tenant')),
  scope_key text NOT NULL, project_uuid uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY(tenant_uuid,scope_kind,scope_key), UNIQUE(tenant_uuid,project_uuid)
);
CREATE TRIGGER phx_historical_backfill_scope_immutable BEFORE UPDATE OR DELETE ON phx_historical_backfill_scope_map FOR EACH ROW EXECUTE FUNCTION phx_v059_deny_mutation();

CREATE TABLE phx_historical_backfill_orphans (
  tenant_uuid uuid NOT NULL, run_uuid uuid NOT NULL, orphan_uuid uuid NOT NULL,
  source_table text NOT NULL, source_key_sha256 char(64) NOT NULL, row_sha256 char(64) NOT NULL,
  reason_code text NOT NULL, project_uuid uuid, candidate_refs jsonb NOT NULL DEFAULT '{}'::jsonb,
  evidence_sha256 char(64) NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY(tenant_uuid,run_uuid,orphan_uuid),
  FOREIGN KEY(tenant_uuid,run_uuid) REFERENCES phx_historical_backfill_runs(tenant_uuid,run_uuid)
);
CREATE INDEX phx_historical_backfill_orphans_reason_idx ON phx_historical_backfill_orphans(tenant_uuid,run_uuid,reason_code,source_table);
CREATE TRIGGER phx_historical_backfill_orphans_immutable BEFORE UPDATE OR DELETE ON phx_historical_backfill_orphans FOR EACH ROW EXECUTE FUNCTION phx_v059_deny_mutation();

CREATE TABLE phx_historical_backfill_coverage (
  tenant_uuid uuid NOT NULL, run_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL,
  source_table text NOT NULL, source_rows bigint NOT NULL CHECK(source_rows>=0),
  processed_rows bigint NOT NULL CHECK(processed_rows>=0), bound_rows bigint NOT NULL CHECK(bound_rows>=0),
  orphan_rows bigint NOT NULL CHECK(orphan_rows>=0), skipped_rows bigint NOT NULL CHECK(skipped_rows>=0),
  coverage_ratio numeric(8,6) NOT NULL CHECK(coverage_ratio BETWEEN 0 AND 1),
  status text NOT NULL CHECK(status IN ('complete','partial','missing_table','failed')),
  evidence_sha256 char(64) NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY(tenant_uuid,run_uuid,snapshot_uuid),
  FOREIGN KEY(tenant_uuid,run_uuid) REFERENCES phx_historical_backfill_runs(tenant_uuid,run_uuid)
);
CREATE INDEX phx_historical_backfill_coverage_idx ON phx_historical_backfill_coverage(tenant_uuid,run_uuid,source_table,created_at DESC);
CREATE TRIGGER phx_historical_backfill_coverage_immutable BEFORE UPDATE OR DELETE ON phx_historical_backfill_coverage FOR EACH ROW EXECUTE FUNCTION phx_v059_deny_mutation();

CREATE TABLE phx_historical_backfill_events (
  tenant_uuid uuid NOT NULL, run_uuid uuid NOT NULL, event_uuid uuid NOT NULL,
  source_table text, event_type text NOT NULL, evidence_sha256 char(64) NOT NULL,
  payload jsonb NOT NULL DEFAULT '{}'::jsonb, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY(tenant_uuid,run_uuid,event_uuid),
  FOREIGN KEY(tenant_uuid,run_uuid) REFERENCES phx_historical_backfill_runs(tenant_uuid,run_uuid)
);
CREATE INDEX phx_historical_backfill_events_idx ON phx_historical_backfill_events(tenant_uuid,run_uuid,source_table,created_at);
CREATE TRIGGER phx_historical_backfill_events_immutable BEFORE UPDATE OR DELETE ON phx_historical_backfill_events FOR EACH ROW EXECUTE FUNCTION phx_v059_deny_mutation();

-- RLS / FORCE RLS for every v0.59 control relation.
DO $phx$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['phx_historical_backfill_runs','phx_historical_backfill_checkpoints','phx_historical_backfill_identity_map','phx_historical_backfill_scope_map','phx_historical_backfill_orphans','phx_historical_backfill_coverage','phx_historical_backfill_events'] LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
    EXECUTE format('CREATE POLICY %I ON %I USING (tenant_uuid = nullif(current_setting(''phx.tenant_uuid'', true), )::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(phx.tenant_uuid, true), )::uuid)',t||'_tenant',t);
  END LOOP;
END $phx$;

CREATE OR REPLACE VIEW phx_historical_backfill_latest_coverage_v AS
SELECT DISTINCT ON (tenant_uuid,run_uuid,source_table)
 tenant_uuid,run_uuid,source_table,source_rows,processed_rows,bound_rows,orphan_rows,skipped_rows,coverage_ratio,status,created_at
FROM phx_historical_backfill_coverage ORDER BY tenant_uuid,run_uuid,source_table,created_at DESC;

CREATE OR REPLACE VIEW phx_historical_backfill_progress_v AS
SELECT r.tenant_uuid,r.run_uuid,r.source_state_sha256,r.registry_sha256,r.started_at,
       count(c.source_table) FILTER (WHERE c.status='complete') AS completed_tables,
       count(c.source_table) AS observed_tables,
       coalesce(sum(c.source_rows),0) AS source_rows,
       coalesce(sum(c.bound_rows),0) AS bound_rows,
       coalesce(sum(c.orphan_rows),0) AS orphan_rows
FROM phx_historical_backfill_runs r
LEFT JOIN phx_historical_backfill_latest_coverage_v c ON c.tenant_uuid=r.tenant_uuid AND c.run_uuid=r.run_uuid
GROUP BY r.tenant_uuid,r.run_uuid,r.source_state_sha256,r.registry_sha256,r.started_at;

BEGIN;
CREATE SCHEMA IF NOT EXISTS phxclaw;

CREATE TABLE IF NOT EXISTS phxclaw.backup_sets (
  backup_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  backup_kind text NOT NULL CHECK (backup_kind IN ('filesystem','postgresql','pre_upgrade','disaster_recovery')),
  source_version text NOT NULL,
  manifest_sha256 char(64) NOT NULL CHECK (manifest_sha256 ~ '^[a-f0-9]{64}$'),
  object_count bigint NOT NULL CHECK (object_count >= 0),
  total_bytes bigint NOT NULL CHECK (total_bytes >= 0),
  repository_uri text NOT NULL,
  state text NOT NULL CHECK (state IN ('creating','verified','failed','expired')),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  verified_at timestamptz NULL
);

CREATE TABLE IF NOT EXISTS phxclaw.backup_objects (
  backup_uuid uuid NOT NULL REFERENCES phxclaw.backup_sets(backup_uuid) ON DELETE RESTRICT,
  object_path text NOT NULL,
  blob_sha256 char(64) NOT NULL CHECK (blob_sha256 ~ '^[a-f0-9]{64}$'),
  size_bytes bigint NOT NULL CHECK (size_bytes >= 0),
  readonly boolean NOT NULL DEFAULT false,
  PRIMARY KEY (backup_uuid, object_path)
);

CREATE TABLE IF NOT EXISTS phxclaw.restore_runs (
  restore_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  backup_uuid uuid NOT NULL REFERENCES phxclaw.backup_sets(backup_uuid) ON DELETE RESTRICT,
  target_uri text NOT NULL,
  rollback_uri text NULL,
  state text NOT NULL CHECK (state IN ('planned','staging','committed','rolled_back','failed')),
  verified boolean NOT NULL DEFAULT false,
  error_text text NULL,
  started_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  completed_at timestamptz NULL
);

CREATE TABLE IF NOT EXISTS phxclaw.upgrade_runs (
  upgrade_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  from_version text NOT NULL,
  to_version text NOT NULL,
  pre_upgrade_backup_uuid uuid NOT NULL REFERENCES phxclaw.backup_sets(backup_uuid) ON DELETE RESTRICT,
  state text NOT NULL CHECK (state IN ('planned','applying','verifying','committed','rolling_back','rolled_back','failed')),
  apply_succeeded boolean NOT NULL DEFAULT false,
  verify_succeeded boolean NOT NULL DEFAULT false,
  rollback_attempted boolean NOT NULL DEFAULT false,
  rollback_succeeded boolean NOT NULL DEFAULT false,
  started_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  completed_at timestamptz NULL,
  CHECK (from_version <> to_version)
);

CREATE TABLE IF NOT EXISTS phxclaw.disaster_recovery_drills (
  drill_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  backup_uuid uuid NOT NULL REFERENCES phxclaw.backup_sets(backup_uuid) ON DELETE RESTRICT,
  restore_uuid uuid NULL REFERENCES phxclaw.restore_runs(restore_uuid) ON DELETE RESTRICT,
  state text NOT NULL CHECK (state IN ('planned','running','passed','failed')),
  target_rpo_seconds bigint NULL CHECK (target_rpo_seconds IS NULL OR target_rpo_seconds >= 0),
  measured_rpo_seconds bigint NULL CHECK (measured_rpo_seconds IS NULL OR measured_rpo_seconds >= 0),
  target_rto_seconds bigint NULL CHECK (target_rto_seconds IS NULL OR target_rto_seconds >= 0),
  measured_rto_seconds bigint NULL CHECK (measured_rto_seconds IS NULL OR measured_rto_seconds >= 0),
  evidence jsonb NOT NULL DEFAULT '{}'::jsonb,
  started_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  completed_at timestamptz NULL
);

CREATE INDEX IF NOT EXISTS backup_sets_tenant_created_idx ON phxclaw.backup_sets(tenant_uuid, created_at DESC);
CREATE INDEX IF NOT EXISTS restore_runs_tenant_started_idx ON phxclaw.restore_runs(tenant_uuid, started_at DESC);
CREATE INDEX IF NOT EXISTS upgrade_runs_tenant_started_idx ON phxclaw.upgrade_runs(tenant_uuid, started_at DESC);

CREATE OR REPLACE FUNCTION phxclaw.reject_backup_evidence_delete() RETURNS trigger LANGUAGE plpgsql AS $fn$
BEGIN
  RAISE EXCEPTION 'installer/backup evidence cannot be deleted; create a compensating record';
END $fn$;

DO $trg$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['backup_sets','backup_objects','restore_runs','upgrade_runs','disaster_recovery_drills'] LOOP
    EXECUTE format('DROP TRIGGER IF EXISTS %I ON phxclaw.%I','trg_'||t||'_no_delete',t);
    EXECUTE format('CREATE TRIGGER %I BEFORE DELETE ON phxclaw.%I FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_backup_evidence_delete()','trg_'||t||'_no_delete',t);
  END LOOP;
END $trg$;

DO $rls$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['backup_sets','restore_runs','upgrade_runs','disaster_recovery_drills'] LOOP
    EXECUTE format('ALTER TABLE phxclaw.%I ENABLE ROW LEVEL SECURITY',t);
    EXECUTE format('ALTER TABLE phxclaw.%I FORCE ROW LEVEL SECURITY',t);
    EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_v065 ON phxclaw.%I',t);
    EXECUTE format('CREATE POLICY tenant_isolation_v065 ON phxclaw.%I USING (tenant_uuid=phxclaw.current_tenant_uuid()) WITH CHECK (tenant_uuid=phxclaw.current_tenant_uuid())',t);
  END LOOP;
  ALTER TABLE phxclaw.backup_objects ENABLE ROW LEVEL SECURITY;
  ALTER TABLE phxclaw.backup_objects FORCE ROW LEVEL SECURITY;
  DROP POLICY IF EXISTS tenant_isolation_v065 ON phxclaw.backup_objects;
  CREATE POLICY tenant_isolation_v065 ON phxclaw.backup_objects
    USING (EXISTS (SELECT 1 FROM phxclaw.backup_sets b WHERE b.backup_uuid=backup_objects.backup_uuid AND b.tenant_uuid=phxclaw.current_tenant_uuid()))
    WITH CHECK (EXISTS (SELECT 1 FROM phxclaw.backup_sets b WHERE b.backup_uuid=backup_objects.backup_uuid AND b.tenant_uuid=phxclaw.current_tenant_uuid()));
END $rls$;
COMMIT;

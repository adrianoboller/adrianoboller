BEGIN;
CREATE TABLE IF NOT EXISTS phoenix_fleet_rollouts (
  tenant_uuid uuid NOT NULL,
  rollout_uuid uuid NOT NULL,
  channel text NOT NULL CHECK (channel IN ('canary','beta','stable')),
  target_version text NOT NULL,
  update_sequence bigint NOT NULL CHECK (update_sequence > 0),
  source_state_sha256 char(64) NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-fA-F]{64}$'),
  update_manifest_sha256 char(64) NOT NULL CHECK (update_manifest_sha256 ~ '^[0-9a-fA-F]{64}$'),
  state text NOT NULL CHECK (state IN ('draft','active','paused','rollback_required','rolling_back','completed','aborted')),
  stage_index integer NOT NULL DEFAULT 0 CHECK(stage_index >= 0),
  generation bigint NOT NULL DEFAULT 1 CHECK(generation > 0),
  fencing_token bigint NOT NULL DEFAULT 1 CHECK(fencing_token > 0),
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, rollout_uuid),
  UNIQUE (tenant_uuid, update_sequence, channel)
);
CREATE TABLE IF NOT EXISTS phoenix_fleet_nodes (
  tenant_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL,
  fleet_uuid uuid NOT NULL,
  current_version text NOT NULL,
  current_sequence bigint NOT NULL CHECK(current_sequence >= 0),
  known_good_version text NOT NULL,
  known_good_sequence bigint NOT NULL CHECK(known_good_sequence >= 0),
  last_heartbeat_at timestamptz,
  PRIMARY KEY (tenant_uuid, node_uuid)
);
CREATE TABLE IF NOT EXISTS phoenix_fleet_assignments (
  tenant_uuid uuid NOT NULL,
  rollout_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL,
  cohort_bucket integer NOT NULL CHECK(cohort_bucket BETWEEN 0 AND 9999),
  stage_index integer NOT NULL CHECK(stage_index >= 0),
  status text NOT NULL CHECK(status IN ('pending','selected','installing','healthy','failed','rolled_back','excluded')),
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, rollout_uuid, node_uuid),
  FOREIGN KEY (tenant_uuid, rollout_uuid) REFERENCES phoenix_fleet_rollouts(tenant_uuid, rollout_uuid) ON DELETE CASCADE,
  FOREIGN KEY (tenant_uuid, node_uuid) REFERENCES phoenix_fleet_nodes(tenant_uuid, node_uuid) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS phoenix_fleet_health_evidence (
  tenant_uuid uuid NOT NULL,
  evidence_uuid uuid NOT NULL,
  rollout_uuid uuid NOT NULL,
  stage_index integer NOT NULL,
  evidence_sha256 char(64) NOT NULL CHECK(evidence_sha256 ~ '^[0-9a-fA-F]{64}$'),
  signer_key_id text NOT NULL,
  created_at timestamptz NOT NULL,
  payload jsonb NOT NULL,
  PRIMARY KEY (tenant_uuid, evidence_uuid),
  FOREIGN KEY (tenant_uuid, rollout_uuid) REFERENCES phoenix_fleet_rollouts(tenant_uuid, rollout_uuid) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS phoenix_fleet_events (
  tenant_uuid uuid NOT NULL,
  event_uuid uuid NOT NULL,
  rollout_uuid uuid NOT NULL,
  generation bigint NOT NULL,
  fencing_token bigint NOT NULL,
  event_type text NOT NULL,
  event_sha256 char(64) NOT NULL CHECK(event_sha256 ~ '^[0-9a-fA-F]{64}$'),
  created_at timestamptz NOT NULL DEFAULT now(),
  payload jsonb NOT NULL,
  PRIMARY KEY (tenant_uuid, event_uuid),
  FOREIGN KEY (tenant_uuid, rollout_uuid) REFERENCES phoenix_fleet_rollouts(tenant_uuid, rollout_uuid) ON DELETE RESTRICT
);

ALTER TABLE phoenix_fleet_rollouts ENABLE ROW LEVEL SECURITY; ALTER TABLE phoenix_fleet_rollouts FORCE ROW LEVEL SECURITY;
ALTER TABLE phoenix_fleet_nodes ENABLE ROW LEVEL SECURITY; ALTER TABLE phoenix_fleet_nodes FORCE ROW LEVEL SECURITY;
ALTER TABLE phoenix_fleet_assignments ENABLE ROW LEVEL SECURITY; ALTER TABLE phoenix_fleet_assignments FORCE ROW LEVEL SECURITY;
ALTER TABLE phoenix_fleet_health_evidence ENABLE ROW LEVEL SECURITY; ALTER TABLE phoenix_fleet_health_evidence FORCE ROW LEVEL SECURITY;
ALTER TABLE phoenix_fleet_events ENABLE ROW LEVEL SECURITY; ALTER TABLE phoenix_fleet_events FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_fleet_rollouts ON phoenix_fleet_rollouts;
CREATE POLICY tenant_isolation_fleet_rollouts ON phoenix_fleet_rollouts USING (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true)) WITH CHECK (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true));
DROP POLICY IF EXISTS tenant_isolation_fleet_nodes ON phoenix_fleet_nodes;
CREATE POLICY tenant_isolation_fleet_nodes ON phoenix_fleet_nodes USING (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true)) WITH CHECK (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true));
DROP POLICY IF EXISTS tenant_isolation_fleet_assignments ON phoenix_fleet_assignments;
CREATE POLICY tenant_isolation_fleet_assignments ON phoenix_fleet_assignments USING (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true)) WITH CHECK (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true));
DROP POLICY IF EXISTS tenant_isolation_fleet_health ON phoenix_fleet_health_evidence;
CREATE POLICY tenant_isolation_fleet_health ON phoenix_fleet_health_evidence USING (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true)) WITH CHECK (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true));
DROP POLICY IF EXISTS tenant_isolation_fleet_events ON phoenix_fleet_events;
CREATE POLICY tenant_isolation_fleet_events ON phoenix_fleet_events USING (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true)) WITH CHECK (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true));

CREATE OR REPLACE FUNCTION phoenix_append_only_guard() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only table'; END $$;
DROP TRIGGER IF EXISTS fleet_health_append_only ON phoenix_fleet_health_evidence;
CREATE TRIGGER fleet_health_append_only BEFORE UPDATE OR DELETE ON phoenix_fleet_health_evidence FOR EACH ROW EXECUTE FUNCTION phoenix_append_only_guard();
DROP TRIGGER IF EXISTS fleet_events_append_only ON phoenix_fleet_events;
CREATE TRIGGER fleet_events_append_only BEFORE UPDATE OR DELETE ON phoenix_fleet_events FOR EACH ROW EXECUTE FUNCTION phoenix_append_only_guard();

CREATE OR REPLACE FUNCTION phoenix_fleet_transition(
  p_tenant_uuid uuid,
  p_rollout_uuid uuid,
  p_expected_fencing_token bigint,
  p_new_state text,
  p_new_stage_index integer
) RETURNS bigint
LANGUAGE plpgsql
SECURITY INVOKER
AS $$
DECLARE v_new_fencing bigint;
BEGIN
  IF p_tenant_uuid::text IS DISTINCT FROM current_setting('phxclaw.tenant_uuid', true) THEN
    RAISE EXCEPTION 'tenant context mismatch';
  END IF;
  UPDATE phoenix_fleet_rollouts
     SET state=p_new_state,
         stage_index=p_new_stage_index,
         generation=generation+1,
         fencing_token=fencing_token+1,
         updated_at=now()
   WHERE tenant_uuid=p_tenant_uuid
     AND rollout_uuid=p_rollout_uuid
     AND fencing_token=p_expected_fencing_token
  RETURNING fencing_token INTO v_new_fencing;
  IF v_new_fencing IS NULL THEN RAISE EXCEPTION 'stale fleet fencing token'; END IF;
  RETURN v_new_fencing;
END $$;

COMMIT;

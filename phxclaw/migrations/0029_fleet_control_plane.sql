BEGIN;
CREATE SCHEMA IF NOT EXISTS phxclaw;

CREATE TABLE IF NOT EXISTS phxclaw.fleet_control_nodes (
  tenant_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL,
  fleet_uuid uuid NOT NULL,
  region text NOT NULL CHECK(length(region) BETWEEN 1 AND 128),
  control_state text NOT NULL CHECK(control_state IN ('online','stale','offline','recovering','maintenance','quarantined','revoked')),
  current_version text NOT NULL,
  current_sequence bigint NOT NULL CHECK(current_sequence >= 0),
  session_fencing_token bigint NOT NULL CHECK(session_fencing_token >= 0),
  last_heartbeat_at timestamptz,
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid,node_uuid),
  FOREIGN KEY (tenant_uuid,node_uuid) REFERENCES phxclaw.device_nodes(tenant_uuid,node_uuid) ON DELETE CASCADE,
  FOREIGN KEY (tenant_uuid,node_uuid) REFERENCES phoenix_fleet_nodes(tenant_uuid,node_uuid) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_control_groups (
  tenant_uuid uuid NOT NULL, group_uuid uuid NOT NULL, fleet_uuid uuid NOT NULL,
  name text NOT NULL CHECK(length(name) BETWEEN 1 AND 128), created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(tenant_uuid,group_uuid), UNIQUE(tenant_uuid,fleet_uuid,name)
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_control_group_members (
  tenant_uuid uuid NOT NULL, group_uuid uuid NOT NULL, node_uuid uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(tenant_uuid,group_uuid,node_uuid),
  FOREIGN KEY(tenant_uuid,group_uuid) REFERENCES phxclaw.fleet_control_groups(tenant_uuid,group_uuid) ON DELETE CASCADE,
  FOREIGN KEY(tenant_uuid,node_uuid) REFERENCES phxclaw.fleet_control_nodes(tenant_uuid,node_uuid) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_control_tags (
  tenant_uuid uuid NOT NULL, node_uuid uuid NOT NULL, tag text NOT NULL CHECK(length(tag) BETWEEN 1 AND 128), created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(tenant_uuid,node_uuid,tag),
  FOREIGN KEY(tenant_uuid,node_uuid) REFERENCES phxclaw.fleet_control_nodes(tenant_uuid,node_uuid) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_selector_snapshots (
  tenant_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL, fleet_uuid uuid NOT NULL,
  selector_sha256 char(64) NOT NULL CHECK(selector_sha256 ~ '^[0-9a-f]{64}$'),
  membership_sha256 char(64) NOT NULL CHECK(membership_sha256 ~ '^[0-9a-f]{64}$'),
  member_count integer NOT NULL CHECK(member_count >= 0), selector jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(tenant_uuid,snapshot_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_maintenance_windows (
  tenant_uuid uuid NOT NULL, maintenance_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL,
  starts_at timestamptz NOT NULL, ends_at timestamptz NOT NULL, reason text NOT NULL, approval_uuid uuid,
  created_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(tenant_uuid,maintenance_uuid), CHECK(ends_at>starts_at),
  FOREIGN KEY(tenant_uuid,snapshot_uuid) REFERENCES phxclaw.fleet_selector_snapshots(tenant_uuid,snapshot_uuid) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_remote_command_plans (
  tenant_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL, capability text NOT NULL,
  arguments_sha256 char(64) NOT NULL CHECK(arguments_sha256 ~ '^[0-9a-f]{64}$'), risk text NOT NULL CHECK(risk IN ('low','medium','high','critical')),
  approval_uuid uuid, max_parallel integer NOT NULL CHECK(max_parallel BETWEEN 1 AND 100), expires_at timestamptz NOT NULL,
  state text NOT NULL CHECK(state IN ('draft','approved','dispatching','completed','cancelled','expired','failed')),
  generation bigint NOT NULL DEFAULT 1 CHECK(generation>0), fencing_token bigint NOT NULL DEFAULT 1 CHECK(fencing_token>0),
  created_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(tenant_uuid,plan_uuid),
  FOREIGN KEY(tenant_uuid,snapshot_uuid) REFERENCES phxclaw.fleet_selector_snapshots(tenant_uuid,snapshot_uuid) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_remote_assignments (
  tenant_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, node_uuid uuid NOT NULL, command_uuid uuid,
  state text NOT NULL CHECK(state IN ('pending','submitted','claimed','succeeded','failed','cancelled','expired')),
  updated_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(tenant_uuid,plan_uuid,node_uuid),
  FOREIGN KEY(tenant_uuid,plan_uuid) REFERENCES phxclaw.fleet_remote_command_plans(tenant_uuid,plan_uuid) ON DELETE CASCADE,
  FOREIGN KEY(tenant_uuid,node_uuid) REFERENCES phxclaw.fleet_control_nodes(tenant_uuid,node_uuid) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_recovery_evidence (
  tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, node_uuid uuid NOT NULL,
  previous_sequence bigint NOT NULL CHECK(previous_sequence>=0), claimed_sequence bigint NOT NULL CHECK(claimed_sequence>=0),
  expected_fencing_token bigint NOT NULL CHECK(expected_fencing_token>=0), received_fencing_token bigint NOT NULL CHECK(received_fencing_token>=0),
  evidence_sha256 char(64) NOT NULL CHECK(evidence_sha256 ~ '^[0-9a-f]{64}$'), payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(tenant_uuid,evidence_uuid), FOREIGN KEY(tenant_uuid,node_uuid) REFERENCES phxclaw.fleet_control_nodes(tenant_uuid,node_uuid) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_control_leases (
  tenant_uuid uuid NOT NULL, fleet_uuid uuid NOT NULL, controller_uuid uuid NOT NULL,
  generation bigint NOT NULL DEFAULT 1 CHECK(generation>0), fencing_token bigint NOT NULL DEFAULT 1 CHECK(fencing_token>0),
  lease_expires_at timestamptz NOT NULL, updated_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(tenant_uuid,fleet_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_control_events (
  tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, fleet_uuid uuid NOT NULL, node_uuid uuid,
  event_type text NOT NULL, event_sha256 char(64) NOT NULL CHECK(event_sha256 ~ '^[0-9a-f]{64}$'), payload jsonb NOT NULL,
  correlation_uuid uuid, causation_uuid uuid, recorded_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(tenant_uuid,event_uuid)
);

DO $$ DECLARE t text; BEGIN
  FOREACH t IN ARRAY ARRAY['fleet_control_nodes','fleet_control_groups','fleet_control_group_members','fleet_control_tags','fleet_selector_snapshots','fleet_maintenance_windows','fleet_remote_command_plans','fleet_remote_assignments','fleet_recovery_evidence','fleet_control_leases','fleet_control_events'] LOOP
    EXECUTE format('ALTER TABLE phxclaw.%I ENABLE ROW LEVEL SECURITY',t);
    EXECUTE format('ALTER TABLE phxclaw.%I FORCE ROW LEVEL SECURITY',t);
    EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON phxclaw.%I',t);
    EXECUTE format('CREATE POLICY tenant_isolation ON phxclaw.%I USING (tenant_uuid::text = current_setting(''phxclaw.tenant_uuid'', true)) WITH CHECK (tenant_uuid::text = current_setting(''phxclaw.tenant_uuid'', true))',t);
  END LOOP;
END $$;

DROP TRIGGER IF EXISTS fleet_selector_snapshot_append_only ON phxclaw.fleet_selector_snapshots;
CREATE TRIGGER fleet_selector_snapshot_append_only BEFORE UPDATE OR DELETE ON phxclaw.fleet_selector_snapshots FOR EACH ROW EXECUTE FUNCTION phoenix_append_only_guard();
DROP TRIGGER IF EXISTS fleet_recovery_evidence_append_only ON phxclaw.fleet_recovery_evidence;
CREATE TRIGGER fleet_recovery_evidence_append_only BEFORE UPDATE OR DELETE ON phxclaw.fleet_recovery_evidence FOR EACH ROW EXECUTE FUNCTION phoenix_append_only_guard();
DROP TRIGGER IF EXISTS fleet_control_events_append_only ON phxclaw.fleet_control_events;
CREATE TRIGGER fleet_control_events_append_only BEFORE UPDATE OR DELETE ON phxclaw.fleet_control_events FOR EACH ROW EXECUTE FUNCTION phoenix_append_only_guard();

CREATE OR REPLACE FUNCTION phoenix_fleet_control_claim_lease(
  p_tenant_uuid uuid,p_fleet_uuid uuid,p_controller_uuid uuid,p_expected_fencing_token bigint,p_lease_seconds integer
) RETURNS bigint LANGUAGE plpgsql SECURITY INVOKER AS $$
DECLARE v_token bigint;
BEGIN
  IF p_tenant_uuid::text IS DISTINCT FROM current_setting('phxclaw.tenant_uuid',true) THEN RAISE EXCEPTION 'tenant context mismatch'; END IF;
  IF p_lease_seconds < 5 OR p_lease_seconds > 300 THEN RAISE EXCEPTION 'invalid lease duration'; END IF;
  UPDATE phxclaw.fleet_control_leases SET controller_uuid=p_controller_uuid,generation=generation+1,fencing_token=fencing_token+1,
    lease_expires_at=now()+make_interval(secs=>p_lease_seconds),updated_at=now()
  WHERE tenant_uuid=p_tenant_uuid AND fleet_uuid=p_fleet_uuid AND fencing_token=p_expected_fencing_token
    AND (controller_uuid=p_controller_uuid OR lease_expires_at<=now()) RETURNING fencing_token INTO v_token;
  IF v_token IS NULL THEN RAISE EXCEPTION 'stale or busy fleet-control fencing token'; END IF;
  RETURN v_token;
END $$;

CREATE OR REPLACE FUNCTION phoenix_fleet_remote_plan_transition(
  p_tenant_uuid uuid,p_plan_uuid uuid,p_expected_fencing_token bigint,p_new_state text
) RETURNS bigint LANGUAGE plpgsql SECURITY INVOKER AS $$
DECLARE v_token bigint;
BEGIN
  IF p_tenant_uuid::text IS DISTINCT FROM current_setting('phxclaw.tenant_uuid',true) THEN RAISE EXCEPTION 'tenant context mismatch'; END IF;
  UPDATE phxclaw.fleet_remote_command_plans SET state=p_new_state,generation=generation+1,fencing_token=fencing_token+1
   WHERE tenant_uuid=p_tenant_uuid AND plan_uuid=p_plan_uuid AND fencing_token=p_expected_fencing_token RETURNING fencing_token INTO v_token;
  IF v_token IS NULL THEN RAISE EXCEPTION 'stale remote-plan fencing token'; END IF;
  RETURN v_token;
END $$;
COMMIT;

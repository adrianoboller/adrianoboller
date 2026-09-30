BEGIN;
CREATE TABLE IF NOT EXISTS phxclaw_controller_members (
 tenant_uuid uuid NOT NULL,
 controller_uuid uuid NOT NULL,
 region text NOT NULL CHECK (length(btrim(region)) > 0),
 state text NOT NULL CHECK (state IN ('joining','follower','leader','self_fenced','draining','offline')),
 build_sha256 text NOT NULL CHECK (build_sha256 ~ '^[0-9a-f]{64}$'),
 started_at timestamptz NOT NULL,
 last_heartbeat_at timestamptz NOT NULL,
 PRIMARY KEY (tenant_uuid, controller_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_controller_leases (
 tenant_uuid uuid PRIMARY KEY,
 lease_uuid uuid NOT NULL UNIQUE,
 holder_uuid uuid NOT NULL,
 epoch bigint NOT NULL CHECK (epoch > 0),
 acquired_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 expires_at timestamptz NOT NULL,
 FOREIGN KEY (tenant_uuid, holder_uuid) REFERENCES phxclaw_controller_members(tenant_uuid, controller_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_reconcile_intents (
 tenant_uuid uuid NOT NULL,
 intent_uuid uuid NOT NULL,
 controller_uuid uuid NOT NULL,
 leader_epoch bigint NOT NULL CHECK (leader_epoch > 0),
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 target_state_sha256 text NOT NULL CHECK (target_state_sha256 ~ '^[0-9a-f]{64}$'),
 status text NOT NULL CHECK (status IN ('planned','committed','rejected','superseded')),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 committed_at timestamptz,
 PRIMARY KEY (tenant_uuid, intent_uuid),
 FOREIGN KEY (tenant_uuid, controller_uuid) REFERENCES phxclaw_controller_members(tenant_uuid, controller_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_controller_events (
 tenant_uuid uuid NOT NULL,
 event_uuid uuid NOT NULL,
 controller_uuid uuid,
 event_type text NOT NULL,
 leader_epoch bigint,
 evidence_sha256 text CHECK (evidence_sha256 IS NULL OR evidence_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 payload jsonb NOT NULL DEFAULT '{}'::jsonb,
 PRIMARY KEY (tenant_uuid, event_uuid)
);

CREATE OR REPLACE FUNCTION phxclaw_claim_controller_leader(p_tenant uuid,p_controller uuid,p_lease uuid,p_seconds integer)
RETURNS TABLE(lease_uuid uuid,holder_uuid uuid,epoch bigint,expires_at timestamptz) LANGUAGE plpgsql AS $$
DECLARE n timestamptz := clock_timestamp(); current_row phxclaw_controller_leases%ROWTYPE;
BEGIN
 IF p_seconds < 5 OR p_seconds > 120 THEN RAISE EXCEPTION 'invalid lease ttl'; END IF;
 PERFORM 1 FROM phxclaw_controller_members WHERE tenant_uuid=p_tenant AND controller_uuid=p_controller FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'unknown controller'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended(p_tenant::text, 3030));
 SELECT * INTO current_row FROM phxclaw_controller_leases WHERE tenant_uuid=p_tenant FOR UPDATE;
 IF NOT FOUND THEN
   INSERT INTO phxclaw_controller_leases(tenant_uuid,lease_uuid,holder_uuid,epoch,acquired_at,expires_at)
   VALUES(p_tenant,p_lease,p_controller,1,n,n+make_interval(secs=>p_seconds));
 ELSIF current_row.holder_uuid=p_controller AND current_row.expires_at>n THEN
   NULL; -- active owner must use renew; do not create a new epoch silently
 ELSIF current_row.expires_at<=n THEN
   UPDATE phxclaw_controller_leases SET lease_uuid=p_lease,holder_uuid=p_controller,epoch=current_row.epoch+1,acquired_at=n,expires_at=n+make_interval(secs=>p_seconds)
   WHERE tenant_uuid=p_tenant AND epoch=current_row.epoch;
 ELSE
   RETURN;
 END IF;
 RETURN QUERY SELECT l.lease_uuid,l.holder_uuid,l.epoch,l.expires_at FROM phxclaw_controller_leases l WHERE l.tenant_uuid=p_tenant AND l.holder_uuid=p_controller AND l.expires_at>n;
END $$;

CREATE OR REPLACE FUNCTION phxclaw_renew_controller_leader(p_tenant uuid,p_controller uuid,p_epoch bigint,p_seconds integer)
RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE n timestamptz := clock_timestamp(); changed integer;
BEGIN
 IF p_seconds < 5 OR p_seconds > 120 THEN RAISE EXCEPTION 'invalid lease ttl'; END IF;
 UPDATE phxclaw_controller_leases SET expires_at=n+make_interval(secs=>p_seconds)
 WHERE tenant_uuid=p_tenant AND holder_uuid=p_controller AND epoch=p_epoch AND expires_at>n;
 GET DIAGNOSTICS changed = ROW_COUNT; RETURN changed=1;
END $$;

CREATE OR REPLACE FUNCTION phxclaw_release_controller_leader(p_tenant uuid,p_controller uuid,p_epoch bigint)
RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE changed integer;
BEGIN
 UPDATE phxclaw_controller_leases SET expires_at=clock_timestamp()
 WHERE tenant_uuid=p_tenant AND holder_uuid=p_controller AND epoch=p_epoch AND expires_at>clock_timestamp();
 GET DIAGNOSTICS changed = ROW_COUNT; RETURN changed=1;
END $$;

CREATE OR REPLACE FUNCTION phxclaw_assert_controller_fence(p_tenant uuid,p_controller uuid,p_epoch bigint)
RETURNS boolean LANGUAGE sql STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM phxclaw_controller_leases WHERE tenant_uuid=p_tenant AND holder_uuid=p_controller AND epoch=p_epoch AND expires_at>clock_timestamp())
$$;

CREATE OR REPLACE FUNCTION phxclaw_commit_reconcile_intent(p_tenant uuid,p_intent uuid,p_controller uuid,p_epoch bigint,p_source_sha256 text)
RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE changed integer;
BEGIN
 IF NOT phxclaw_assert_controller_fence(p_tenant,p_controller,p_epoch) THEN RETURN false; END IF;
 UPDATE phxclaw_reconcile_intents SET status='committed',committed_at=clock_timestamp()
 WHERE tenant_uuid=p_tenant AND intent_uuid=p_intent AND controller_uuid=p_controller AND leader_epoch=p_epoch AND source_state_sha256=p_source_sha256 AND status='planned';
 GET DIAGNOSTICS changed = ROW_COUNT; RETURN changed=1;
END $$;

CREATE OR REPLACE FUNCTION phxclaw_forbid_controller_event_mutation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'controller events are append-only'; END $$;
DROP TRIGGER IF EXISTS phxclaw_controller_events_append_only ON phxclaw_controller_events;
CREATE TRIGGER phxclaw_controller_events_append_only BEFORE UPDATE OR DELETE ON phxclaw_controller_events FOR EACH ROW EXECUTE FUNCTION phxclaw_forbid_controller_event_mutation();

DO $$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['phxclaw_controller_members','phxclaw_controller_leases','phxclaw_reconcile_intents','phxclaw_controller_events'] LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
  EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
  EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I', t);
  EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), )::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(phxclaw.tenant_uuid, true), )::uuid)', t);
 END LOOP;
END $$;
COMMIT;

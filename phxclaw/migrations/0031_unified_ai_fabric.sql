-- PhxClaw v0.31 — Unified AI Fabric
CREATE TABLE IF NOT EXISTS phxclaw_ai_providers (
 tenant_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, provider_name text NOT NULL,
 location text NOT NULL CHECK (location IN ('local','cloud')), enabled boolean NOT NULL DEFAULT true,
 priority integer NOT NULL DEFAULT 100, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_uuid,provider_uuid), UNIQUE(tenant_uuid,provider_name)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_models (
 tenant_uuid uuid NOT NULL, model_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, model_id text NOT NULL,
 capabilities jsonb NOT NULL DEFAULT '[]'::jsonb, context_tokens bigint NOT NULL,
 input_cost_micro_usd_per_million bigint, output_cost_micro_usd_per_million bigint,
 data_controls jsonb NOT NULL DEFAULT '[]'::jsonb, catalog_observed_at timestamptz NOT NULL,
 catalog_ttl_seconds integer NOT NULL CHECK (catalog_ttl_seconds > 0),
 PRIMARY KEY(tenant_uuid,model_uuid), UNIQUE(tenant_uuid,provider_uuid,model_id),
 FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_health_observations (
 tenant_uuid uuid NOT NULL, observation_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, model_uuid uuid,
 healthy boolean NOT NULL, health_basis_points integer NOT NULL CHECK(health_basis_points BETWEEN 0 AND 10000),
 p95_latency_ms bigint, observed_at timestamptz NOT NULL DEFAULT clock_timestamp(), ttl_seconds integer NOT NULL CHECK(ttl_seconds>0),
 evidence_sha256 text NOT NULL CHECK(evidence_sha256 ~ '^[0-9a-f]{64}$'),
 PRIMARY KEY(tenant_uuid,observation_uuid),
 FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid),
 FOREIGN KEY(tenant_uuid,model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_route_decisions (
 tenant_uuid uuid NOT NULL, decision_uuid uuid NOT NULL, request_uuid uuid NOT NULL,
 prompt_sha256 text NOT NULL CHECK(prompt_sha256 ~ '^[0-9a-f]{64}$'), selected_provider_uuid uuid NOT NULL, selected_model_uuid uuid NOT NULL,
 decision_sha256 text NOT NULL CHECK(decision_sha256 ~ '^[0-9a-f]{64}$'), candidate_summary jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,decision_uuid), UNIQUE(tenant_uuid,request_uuid),
 FOREIGN KEY(tenant_uuid,selected_provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid),
 FOREIGN KEY(tenant_uuid,selected_model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_budget_accounts (
 tenant_uuid uuid NOT NULL, account_uuid uuid NOT NULL, limit_micro_usd bigint NOT NULL CHECK(limit_micro_usd>=0),
 reserved_micro_usd bigint NOT NULL DEFAULT 0 CHECK(reserved_micro_usd>=0), spent_micro_usd bigint NOT NULL DEFAULT 0 CHECK(spent_micro_usd>=0),
 version bigint NOT NULL DEFAULT 0, PRIMARY KEY(tenant_uuid,account_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_budget_reservations (
 tenant_uuid uuid NOT NULL, reservation_uuid uuid NOT NULL, account_uuid uuid NOT NULL, request_uuid uuid NOT NULL,
 reserved_micro_usd bigint NOT NULL CHECK(reserved_micro_usd>=0), settled_micro_usd bigint,
 state text NOT NULL CHECK(state IN ('reserved','settled','released')), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_uuid,reservation_uuid), UNIQUE(tenant_uuid,request_uuid),
 FOREIGN KEY(tenant_uuid,account_uuid) REFERENCES phxclaw_ai_budget_accounts(tenant_uuid,account_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_provider_circuits (
 tenant_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, state text NOT NULL CHECK(state IN ('closed','open','half_open')),
 consecutive_failures integer NOT NULL DEFAULT 0, opened_at timestamptz, version bigint NOT NULL DEFAULT 0,
 PRIMARY KEY(tenant_uuid,provider_uuid), FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_events (
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, kind text NOT NULL, object_uuid uuid, payload_sha256 text NOT NULL CHECK(payload_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,event_uuid)
);

DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['phxclaw_ai_providers','phxclaw_ai_models','phxclaw_ai_health_observations','phxclaw_ai_route_decisions','phxclaw_ai_budget_accounts','phxclaw_ai_budget_reservations','phxclaw_ai_provider_circuits','phxclaw_ai_events'] LOOP
 EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
 EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t);
 EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t);
 END LOOP; END $$;

CREATE OR REPLACE FUNCTION phxclaw_ai_no_update_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only table'; END $$;
DROP TRIGGER IF EXISTS ai_health_append_only ON phxclaw_ai_health_observations;
CREATE TRIGGER ai_health_append_only BEFORE UPDATE OR DELETE ON phxclaw_ai_health_observations FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_no_update_delete();
DROP TRIGGER IF EXISTS ai_route_append_only ON phxclaw_ai_route_decisions;
CREATE TRIGGER ai_route_append_only BEFORE UPDATE OR DELETE ON phxclaw_ai_route_decisions FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_no_update_delete();
DROP TRIGGER IF EXISTS ai_events_append_only ON phxclaw_ai_events;
CREATE TRIGGER ai_events_append_only BEFORE UPDATE OR DELETE ON phxclaw_ai_events FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_no_update_delete();

CREATE OR REPLACE FUNCTION phxclaw_ai_reserve_budget(p_tenant uuid,p_account uuid,p_reservation uuid,p_request uuid,p_amount bigint) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE a phxclaw_ai_budget_accounts%ROWTYPE; existing phxclaw_ai_budget_reservations%ROWTYPE; BEGIN
 IF p_amount < 0 THEN RETURN false; END IF;
 PERFORM set_config('phxclaw.tenant_uuid',p_tenant::text,true);
 SELECT * INTO existing FROM phxclaw_ai_budget_reservations WHERE tenant_uuid=p_tenant AND request_uuid=p_request FOR UPDATE;
 IF FOUND THEN RETURN existing.account_uuid=p_account AND existing.reserved_micro_usd=p_amount AND existing.state IN ('reserved','settled'); END IF;
 SELECT * INTO a FROM phxclaw_ai_budget_accounts WHERE tenant_uuid=p_tenant AND account_uuid=p_account FOR UPDATE;
 IF NOT FOUND OR a.spent_micro_usd + a.reserved_micro_usd + p_amount > a.limit_micro_usd THEN RETURN false; END IF;
 INSERT INTO phxclaw_ai_budget_reservations(tenant_uuid,reservation_uuid,account_uuid,request_uuid,reserved_micro_usd,state) VALUES(p_tenant,p_reservation,p_account,p_request,p_amount,'reserved');
 UPDATE phxclaw_ai_budget_accounts SET reserved_micro_usd=reserved_micro_usd+p_amount, version=version+1 WHERE tenant_uuid=p_tenant AND account_uuid=p_account;
 RETURN true; END $$;

CREATE OR REPLACE FUNCTION phxclaw_ai_settle_budget(p_tenant uuid,p_request uuid,p_actual bigint) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE r phxclaw_ai_budget_reservations%ROWTYPE; BEGIN
 IF p_actual < 0 THEN RETURN false; END IF; PERFORM set_config('phxclaw.tenant_uuid',p_tenant::text,true);
 SELECT * INTO r FROM phxclaw_ai_budget_reservations WHERE tenant_uuid=p_tenant AND request_uuid=p_request FOR UPDATE;
 IF NOT FOUND OR p_actual > r.reserved_micro_usd THEN RETURN false; END IF;
 IF r.state='settled' THEN RETURN r.settled_micro_usd=p_actual; END IF;
 IF r.state <> 'reserved' THEN RETURN false; END IF;
 UPDATE phxclaw_ai_budget_reservations SET settled_micro_usd=p_actual,state='settled' WHERE tenant_uuid=p_tenant AND reservation_uuid=r.reservation_uuid;
 UPDATE phxclaw_ai_budget_accounts SET reserved_micro_usd=reserved_micro_usd-r.reserved_micro_usd, spent_micro_usd=spent_micro_usd+p_actual, version=version+1 WHERE tenant_uuid=p_tenant AND account_uuid=r.account_uuid;
 RETURN true; END $$;

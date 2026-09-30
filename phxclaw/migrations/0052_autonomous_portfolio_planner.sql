BEGIN;
CREATE TABLE IF NOT EXISTS phx_portfolio_plans(
 tenant_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'), selected_candidate_hash text NOT NULL CHECK(selected_candidate_hash~'^[a-f0-9]{64}$'), plan_sha256 text NOT NULL CHECK(plan_sha256~'^[a-f0-9]{64}$'), requires_approval boolean NOT NULL, total_budget_reserved numeric NOT NULL CHECK(total_budget_reserved>=0), direct_mutation boolean NOT NULL DEFAULT false CHECK(direct_mutation=false), payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,plan_uuid), UNIQUE(tenant_uuid,plan_uuid,plan_sha256));
CREATE TABLE IF NOT EXISTS phx_portfolio_plan_items(
 tenant_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, project_uuid uuid NOT NULL, sequence_no integer NOT NULL CHECK(sequence_no>0), planned_start_day integer NOT NULL, planned_finish_day integer NOT NULL CHECK(planned_finish_day>=planned_start_day), recommendation text NOT NULL, budget_reservation numeric NOT NULL CHECK(budget_reservation>=0), payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,plan_uuid,project_uuid), FOREIGN KEY(tenant_uuid,plan_uuid) REFERENCES phx_portfolio_plans(tenant_uuid,plan_uuid));
CREATE TABLE IF NOT EXISTS phx_portfolio_resource_reservations(
 tenant_uuid uuid NOT NULL, reservation_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, project_uuid uuid NOT NULL, resource_kind text NOT NULL, resource_key text NOT NULL, start_day integer NOT NULL, finish_day integer NOT NULL CHECK(finish_day>=start_day), amount numeric NOT NULL CHECK(amount>=0), evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,reservation_uuid), FOREIGN KEY(tenant_uuid,plan_uuid) REFERENCES phx_portfolio_plans(tenant_uuid,plan_uuid));
CREATE TABLE IF NOT EXISTS phx_portfolio_plan_delegations(
 tenant_uuid uuid NOT NULL, delegation_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, plan_sha256 text NOT NULL CHECK(plan_sha256~'^[a-f0-9]{64}$'), delegate_to text NOT NULL CHECK(delegate_to='executive_decision_center'), direct_mutation boolean NOT NULL DEFAULT false CHECK(direct_mutation=false), evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,delegation_uuid), FOREIGN KEY(tenant_uuid,plan_uuid,plan_sha256) REFERENCES phx_portfolio_plans(tenant_uuid,plan_uuid,plan_sha256));
CREATE TABLE IF NOT EXISTS phx_portfolio_planner_events(
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, plan_uuid uuid, event_type text NOT NULL, evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'), payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,event_uuid));

DO $ddl$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['phx_portfolio_plans','phx_portfolio_plan_items','phx_portfolio_resource_reservations','phx_portfolio_plan_delegations','phx_portfolio_planner_events'] LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format($p$CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)$p$,t);
 END LOOP;
END $ddl$;

CREATE OR REPLACE FUNCTION phx_portfolio_planner_append_only() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'append-only table';END$$;
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['phx_portfolio_plans','phx_portfolio_plan_items','phx_portfolio_resource_reservations','phx_portfolio_plan_delegations','phx_portfolio_planner_events'] LOOP EXECUTE format('DROP TRIGGER IF EXISTS zz_append_only ON %I',t); EXECUTE format('CREATE TRIGGER zz_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phx_portfolio_planner_append_only()',t); END LOOP; END $$;
COMMIT;

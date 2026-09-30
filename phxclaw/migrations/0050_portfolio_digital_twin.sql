-- PhxClaw v0.50 Portfolio Digital Twin & Monte Carlo Planning
CREATE TABLE IF NOT EXISTS phx_portfolio_twins (
  tenant_uuid uuid NOT NULL,
  twin_uuid uuid NOT NULL,
  source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
  created_at timestamptz NOT NULL DEFAULT now(),
  twin_json jsonb NOT NULL,
  twin_sha256 text NOT NULL CHECK (twin_sha256 ~ '^[0-9a-f]{64}$'),
  PRIMARY KEY (tenant_uuid, twin_uuid),
  UNIQUE (tenant_uuid, twin_uuid, source_state_sha256)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_scenarios (
  tenant_uuid uuid NOT NULL,
  twin_uuid uuid NOT NULL,
  scenario_uuid uuid NOT NULL,
  source_state_sha256 text NOT NULL,
  scenario_sha256 text NOT NULL CHECK (scenario_sha256 ~ '^[0-9a-f]{64}$'),
  spec jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, scenario_uuid),
  FOREIGN KEY (tenant_uuid, twin_uuid, source_state_sha256) REFERENCES phx_portfolio_twins(tenant_uuid,twin_uuid,source_state_sha256)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_simulation_runs (
  tenant_uuid uuid NOT NULL,
  scenario_uuid uuid NOT NULL,
  run_uuid uuid NOT NULL,
  source_state_sha256 text NOT NULL,
  iterations integer NOT NULL CHECK (iterations BETWEEN 100 AND 100000),
  seed bigint NOT NULL,
  result_json jsonb NOT NULL,
  evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, run_uuid),
  FOREIGN KEY (tenant_uuid, scenario_uuid) REFERENCES phx_portfolio_scenarios(tenant_uuid,scenario_uuid)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_calibration_evidence (
  tenant_uuid uuid NOT NULL,
  evidence_uuid uuid NOT NULL,
  source_state_sha256 text NOT NULL,
  promoted boolean NOT NULL,
  fresh_until timestamptz NOT NULL,
  sample_count integer NOT NULL CHECK (sample_count > 0),
  evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
  evidence_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid,evidence_uuid)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_decision_delegations (
  tenant_uuid uuid NOT NULL,
  decision_case_uuid uuid NOT NULL,
  scenario_uuid uuid NOT NULL,
  simulation_evidence_sha256 text NOT NULL,
  delegate_to text NOT NULL CHECK (delegate_to='executive_decision_center'),
  direct_mutation boolean NOT NULL CHECK (direct_mutation=false),
  delegation_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid,decision_case_uuid),
  FOREIGN KEY (tenant_uuid,scenario_uuid) REFERENCES phx_portfolio_scenarios(tenant_uuid,scenario_uuid)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_events (
  tenant_uuid uuid NOT NULL,
  event_uuid uuid NOT NULL,
  source_state_sha256 text NOT NULL,
  event_type text NOT NULL,
  event_sha256 text NOT NULL CHECK (event_sha256 ~ '^[0-9a-f]{64}$'),
  payload jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid,event_uuid)
);

DO $phx$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['phx_portfolio_twins','phx_portfolio_scenarios','phx_portfolio_simulation_runs','phx_portfolio_calibration_evidence','phx_portfolio_decision_delegations','phx_portfolio_events'] LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
    EXECUTE format($policy$CREATE POLICY %I_tenant ON %I USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)$policy$,t,t);
  END LOOP;
END $phx$;

CREATE OR REPLACE FUNCTION phx_portfolio_append_only() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'append-only table'; END $$;
DO $$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['phx_portfolio_twins','phx_portfolio_scenarios','phx_portfolio_simulation_runs','phx_portfolio_calibration_evidence','phx_portfolio_decision_delegations','phx_portfolio_events'] LOOP
   EXECUTE format('DROP TRIGGER IF EXISTS %I_append_only ON %I',t,t);
   EXECUTE format('CREATE TRIGGER %I_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phx_portfolio_append_only()',t,t);
 END LOOP;
END $$;

-- PhxClaw v0.51 Portfolio Optimizer & Scenario Search
CREATE TABLE IF NOT EXISTS phx_portfolio_optimization_searches (
  tenant_uuid uuid NOT NULL,
  search_uuid uuid NOT NULL,
  twin_uuid uuid NOT NULL,
  source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
  plan_sha256 text NOT NULL CHECK (plan_sha256 ~ '^[0-9a-f]{64}$'),
  spec jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, search_uuid),
  FOREIGN KEY (tenant_uuid, twin_uuid, source_state_sha256) REFERENCES phx_portfolio_twins(tenant_uuid, twin_uuid, source_state_sha256)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_optimization_candidates (
  tenant_uuid uuid NOT NULL,
  search_uuid uuid NOT NULL,
  candidate_uuid uuid NOT NULL,
  candidate_sha256 text NOT NULL CHECK (candidate_sha256 ~ '^[0-9a-f]{64}$'),
  feasible boolean NOT NULL,
  metrics jsonb NOT NULL,
  simulation_evidence_sha256 text NOT NULL CHECK (simulation_evidence_sha256 ~ '^[0-9a-f]{64}$'),
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, search_uuid, candidate_uuid),
  FOREIGN KEY (tenant_uuid, search_uuid) REFERENCES phx_portfolio_optimization_searches(tenant_uuid, search_uuid)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_optimization_frontier (
  tenant_uuid uuid NOT NULL,
  search_uuid uuid NOT NULL,
  candidate_uuid uuid NOT NULL,
  metrics jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, search_uuid, candidate_uuid),
  FOREIGN KEY (tenant_uuid, search_uuid, candidate_uuid) REFERENCES phx_portfolio_optimization_candidates(tenant_uuid, search_uuid, candidate_uuid)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_optimization_recommendations (
  tenant_uuid uuid NOT NULL,
  recommendation_uuid uuid NOT NULL,
  search_uuid uuid NOT NULL,
  candidate_uuid uuid NOT NULL,
  confidence double precision NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
  rank_stability double precision NOT NULL CHECK (rank_stability >= 0 AND rank_stability <= 1),
  recommendation_sha256 text NOT NULL CHECK (recommendation_sha256 ~ '^[0-9a-f]{64}$'),
  direct_mutation boolean NOT NULL DEFAULT false CHECK (direct_mutation=false),
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, recommendation_uuid),
  FOREIGN KEY (tenant_uuid, search_uuid, candidate_uuid) REFERENCES phx_portfolio_optimization_candidates(tenant_uuid, search_uuid, candidate_uuid)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_optimization_events (
  tenant_uuid uuid NOT NULL,
  event_uuid uuid NOT NULL,
  search_uuid uuid NOT NULL,
  event_type text NOT NULL,
  payload jsonb NOT NULL,
  evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, event_uuid),
  FOREIGN KEY (tenant_uuid, search_uuid) REFERENCES phx_portfolio_optimization_searches(tenant_uuid, search_uuid)
);

DO $rls$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['phx_portfolio_optimization_searches','phx_portfolio_optimization_candidates','phx_portfolio_optimization_frontier','phx_portfolio_optimization_recommendations','phx_portfolio_optimization_events'] LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
    EXECUTE format($policy$CREATE POLICY %I_tenant ON %I USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)$policy$,t,t);
  END LOOP;
END $rls$;

CREATE OR REPLACE FUNCTION phx_portfolio_optimizer_append_only() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only table'; END $$;
DO $trg$ DECLARE t text; BEGIN
  FOREACH t IN ARRAY ARRAY['phx_portfolio_optimization_candidates','phx_portfolio_optimization_frontier','phx_portfolio_optimization_recommendations','phx_portfolio_optimization_events'] LOOP
    EXECUTE format('DROP TRIGGER IF EXISTS %I_append_only ON %I',t,t);
    EXECUTE format('CREATE TRIGGER %I_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phx_portfolio_optimizer_append_only()',t,t);
  END LOOP;
END $trg$;

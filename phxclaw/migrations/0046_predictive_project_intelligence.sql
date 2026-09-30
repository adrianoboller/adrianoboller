BEGIN;

CREATE TABLE IF NOT EXISTS phx_predictive_route_forecasts (
  tenant_uuid uuid NOT NULL,
  project_uuid uuid NOT NULL,
  forecast_uuid uuid NOT NULL,
  task_class text NOT NULL,
  context_fingerprint text NOT NULL,
  agent_uuid uuid NOT NULL,
  model_profile_uuid uuid NOT NULL,
  provider text NOT NULL,
  local boolean NOT NULL,
  sample_count integer NOT NULL CHECK (sample_count > 0),
  success_probability numeric(9,6) NOT NULL CHECK (success_probability BETWEEN 0 AND 1),
  expected_quality numeric(9,6) NOT NULL CHECK (expected_quality BETWEEN 0 AND 1),
  expected_cost_usd numeric(18,8) NOT NULL CHECK (expected_cost_usd >= 0),
  expected_duration_ms bigint NOT NULL CHECK (expected_duration_ms >= 0),
  expected_retries numeric(12,6) NOT NULL CHECK (expected_retries >= 0),
  effective_cost_usd numeric(18,8) NOT NULL CHECK (effective_cost_usd >= 0),
  confidence numeric(9,6) NOT NULL CHECK (confidence BETWEEN 0 AND 1),
  evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-fA-F]{64}$'),
  forecast_sha256 text NOT NULL CHECK (forecast_sha256 ~ '^[0-9a-fA-F]{64}$'),
  generated_at timestamptz NOT NULL,
  expires_at timestamptz NOT NULL,
  PRIMARY KEY (tenant_uuid, project_uuid, forecast_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS phx_predictive_recommendations (
  tenant_uuid uuid NOT NULL,
  project_uuid uuid NOT NULL,
  recommendation_uuid uuid NOT NULL,
  task_uuid uuid NOT NULL,
  forecast_uuid uuid NOT NULL,
  agent_uuid uuid NOT NULL,
  model_profile_uuid uuid NOT NULL,
  provider text NOT NULL,
  local boolean NOT NULL,
  predicted_cost_usd numeric(18,8) NOT NULL CHECK (predicted_cost_usd >= 0),
  predicted_quality numeric(9,6) NOT NULL CHECK (predicted_quality BETWEEN 0 AND 1),
  predicted_success_probability numeric(9,6) NOT NULL CHECK (predicted_success_probability BETWEEN 0 AND 1),
  recommendation_sha256 text NOT NULL CHECK (recommendation_sha256 ~ '^[0-9a-fA-F]{64}$'),
  rationale jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, recommendation_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid, task_uuid) REFERENCES phx_project_task_queue(tenant_uuid, project_uuid, task_uuid) ON DELETE RESTRICT,
  FOREIGN KEY (tenant_uuid, project_uuid, forecast_uuid) REFERENCES phx_predictive_route_forecasts(tenant_uuid, project_uuid, forecast_uuid) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS phx_project_predictive_forecasts (
  tenant_uuid uuid NOT NULL,
  project_uuid uuid NOT NULL,
  forecast_uuid uuid NOT NULL,
  task_count integer NOT NULL CHECK (task_count >= 0),
  expected_remaining_cost_usd numeric(18,8) NOT NULL CHECK (expected_remaining_cost_usd >= 0),
  expected_remaining_duration_ms bigint NOT NULL CHECK (expected_remaining_duration_ms >= 0),
  expected_rework_cost_usd numeric(18,8) NOT NULL CHECK (expected_rework_cost_usd >= 0),
  deadline_risk numeric(9,6) NOT NULL CHECK (deadline_risk BETWEEN 0 AND 1),
  budget_overrun_risk numeric(9,6) NOT NULL CHECK (budget_overrun_risk BETWEEN 0 AND 1),
  source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-fA-F]{64}$'),
  forecast_sha256 text NOT NULL CHECK (forecast_sha256 ~ '^[0-9a-fA-F]{64}$'),
  generated_at timestamptz NOT NULL,
  expires_at timestamptz NOT NULL,
  PRIMARY KEY (tenant_uuid, project_uuid, forecast_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS phx_forecast_feedback (
  tenant_uuid uuid NOT NULL,
  project_uuid uuid NOT NULL,
  feedback_uuid uuid NOT NULL,
  forecast_uuid uuid NOT NULL,
  run_uuid uuid NOT NULL,
  actual_success boolean NOT NULL,
  actual_quality numeric(9,6) NOT NULL CHECK (actual_quality BETWEEN 0 AND 1),
  actual_cost_usd numeric(18,8) NOT NULL CHECK (actual_cost_usd >= 0),
  actual_duration_ms bigint NOT NULL CHECK (actual_duration_ms >= 0),
  cost_absolute_error numeric(18,8) NOT NULL CHECK (cost_absolute_error >= 0),
  duration_absolute_error_ms bigint NOT NULL CHECK (duration_absolute_error_ms >= 0),
  quality_absolute_error numeric(9,6) NOT NULL CHECK (quality_absolute_error >= 0),
  evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-fA-F]{64}$'),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, feedback_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid, forecast_uuid) REFERENCES phx_predictive_route_forecasts(tenant_uuid, project_uuid, forecast_uuid) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS phx_predictive_events (
  tenant_uuid uuid NOT NULL,
  project_uuid uuid NOT NULL,
  event_uuid uuid NOT NULL,
  event_type text NOT NULL,
  object_uuid uuid,
  payload jsonb NOT NULL,
  evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-fA-F]{64}$'),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, event_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid) ON DELETE CASCADE
);

DO $$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['phx_predictive_route_forecasts','phx_predictive_recommendations','phx_project_predictive_forecasts','phx_forecast_feedback','phx_predictive_events'] LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
    EXECUTE format('DROP POLICY IF EXISTS %I ON %I', t || '_tenant', t);
    EXECUTE format('CREATE POLICY %I ON %I USING (tenant_uuid = nullif(current_setting(''phx.tenant_uuid'', true), )::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(phx.tenant_uuid, true), )::uuid)', t || '_tenant', t);
  END LOOP;
END $$;

CREATE OR REPLACE FUNCTION phx_deny_predictive_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'predictive evidence is append-only'; END $$;
DO $$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['phx_predictive_route_forecasts','phx_predictive_recommendations','phx_project_predictive_forecasts','phx_forecast_feedback','phx_predictive_events'] LOOP
    EXECUTE format('DROP TRIGGER IF EXISTS %I ON %I', t || '_append_only', t);
    EXECUTE format('CREATE TRIGGER %I BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phx_deny_predictive_mutation()', t || '_append_only', t);
  END LOOP;
END $$;

COMMIT;

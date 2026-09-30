BEGIN;
CREATE TABLE IF NOT EXISTS phxclaw_ai_sre_policies (
 tenant_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, service_name text NOT NULL CHECK(length(btrim(service_name))>0), portfolio_uuid uuid NOT NULL,
 document_sha256 text NOT NULL CHECK(document_sha256 ~ '^[0-9a-f]{64}$'), signer_id text NOT NULL, signature_hex text NOT NULL CHECK(signature_hex ~ '^[0-9a-f]{128}$'),
 policy_json jsonb NOT NULL, starts_at timestamptz NOT NULL, expires_at timestamptz NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_uuid,policy_uuid), UNIQUE(tenant_uuid,document_sha256), FOREIGN KEY(tenant_uuid,portfolio_uuid) REFERENCES phxclaw_ai_portfolios(tenant_uuid,portfolio_uuid), CHECK(starts_at<expires_at)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_slo_samples (
 tenant_uuid uuid NOT NULL, sample_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, success_count bigint NOT NULL CHECK(success_count>=0), total_count bigint NOT NULL CHECK(total_count>0 AND success_count<=total_count), p95_latency_ms bigint NOT NULL CHECK(p95_latency_ms>=0), p95_queue_wait_ms bigint NOT NULL CHECK(p95_queue_wait_ms>=0), observed_at timestamptz NOT NULL, window_seconds integer NOT NULL CHECK(window_seconds>0), evidence_sha256 text NOT NULL CHECK(evidence_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,sample_uuid), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_slo_evaluations (
 tenant_uuid uuid NOT NULL, evaluation_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, availability_basis_points integer NOT NULL CHECK(availability_basis_points BETWEEN 0 AND 10000), error_budget_burn_milli bigint NOT NULL CHECK(error_budget_burn_milli>=0), latency_violated boolean NOT NULL, queue_wait_violated boolean NOT NULL, source_evidence_hashes jsonb NOT NULL, evaluation_sha256 text NOT NULL CHECK(evaluation_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,evaluation_uuid), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_demand_samples (
 tenant_uuid uuid NOT NULL, sample_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, window_start timestamptz NOT NULL, window_seconds integer NOT NULL CHECK(window_seconds>0), requests bigint NOT NULL CHECK(requests>=0), tokens bigint NOT NULL CHECK(tokens>=0), peak_concurrency integer NOT NULL CHECK(peak_concurrency>=0), evidence_sha256 text NOT NULL CHECK(evidence_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,sample_uuid), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_demand_forecasts (
 tenant_uuid uuid NOT NULL, forecast_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, requests_per_minute bigint NOT NULL CHECK(requests_per_minute>=0), tokens_per_minute bigint NOT NULL CHECK(tokens_per_minute>=0), peak_concurrency integer NOT NULL CHECK(peak_concurrency>=0), recommended_concurrency integer NOT NULL CHECK(recommended_concurrency>0), horizon_seconds integer NOT NULL CHECK(horizon_seconds>0), source_evidence_hashes jsonb NOT NULL, forecast_sha256 text NOT NULL CHECK(forecast_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,forecast_uuid), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_rate_limit_snapshots (
 tenant_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, request_limit bigint NOT NULL CHECK(request_limit>0), requests_remaining bigint NOT NULL CHECK(requests_remaining>=0 AND requests_remaining<=request_limit), token_limit bigint NOT NULL CHECK(token_limit>0), tokens_remaining bigint NOT NULL CHECK(tokens_remaining>=0 AND tokens_remaining<=token_limit), reset_at timestamptz NOT NULL, observed_at timestamptz NOT NULL, ttl_seconds integer NOT NULL CHECK(ttl_seconds>0), snapshot_sha256 text NOT NULL CHECK(snapshot_sha256 ~ '^[0-9a-f]{64}$'), signer_id text NOT NULL, signature_hex text NOT NULL CHECK(signature_hex ~ '^[0-9a-f]{128}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,snapshot_uuid), FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_admission_decisions (
 tenant_uuid uuid NOT NULL, decision_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, request_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, request_class text NOT NULL CHECK(request_class IN ('critical','interactive','batch','background')), action text NOT NULL CHECK(action IN ('admit','queue','reject')), priority_score bigint NOT NULL CHECK(priority_score>=0), rate_limit_snapshot_uuid uuid NOT NULL, decision_sha256 text NOT NULL CHECK(decision_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,decision_uuid), UNIQUE(tenant_uuid,request_uuid), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid), FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid), FOREIGN KEY(tenant_uuid,rate_limit_snapshot_uuid) REFERENCES phxclaw_ai_rate_limit_snapshots(tenant_uuid,snapshot_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_budget_snapshots (
 tenant_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL, account_uuid uuid NOT NULL, limit_micro_usd bigint NOT NULL CHECK(limit_micro_usd>=0), spent_micro_usd bigint NOT NULL CHECK(spent_micro_usd>=0), reserved_micro_usd bigint NOT NULL CHECK(reserved_micro_usd>=0), observed_at timestamptz NOT NULL, snapshot_sha256 text NOT NULL CHECK(snapshot_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,snapshot_uuid), FOREIGN KEY(tenant_uuid,account_uuid) REFERENCES phxclaw_ai_budget_accounts(tenant_uuid,account_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_cost_forecasts (
 tenant_uuid uuid NOT NULL, forecast_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, account_uuid uuid NOT NULL, budget_limit_micro_usd bigint NOT NULL CHECK(budget_limit_micro_usd>=0), current_committed_micro_usd bigint NOT NULL CHECK(current_committed_micro_usd>=0), projected_total_micro_usd bigint NOT NULL CHECK(projected_total_micro_usd>=0), projected_budget_basis_points integer NOT NULL CHECK(projected_budget_basis_points BETWEEN 0 AND 10000), budget_state text NOT NULL CHECK(budget_state IN ('healthy','soft_limit','hard_limit')), source_evidence_hashes jsonb NOT NULL, forecast_sha256 text NOT NULL CHECK(forecast_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,forecast_uuid), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid), FOREIGN KEY(tenant_uuid,account_uuid) REFERENCES phxclaw_ai_budget_accounts(tenant_uuid,account_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_incident_events (
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, incident_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, severity text NOT NULL CHECK(severity IN ('info','warning','critical')), event_type text NOT NULL CHECK(event_type IN ('detected','opened','updated','mitigated','resolved')), evidence_hashes jsonb NOT NULL, event_sha256 text NOT NULL CHECK(event_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,event_uuid), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_autopilot_plans (
 tenant_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, portfolio_uuid uuid NOT NULL, source_state_sha256 text NOT NULL CHECK(source_state_sha256 ~ '^[0-9a-f]{64}$'), trigger_evidence_hashes jsonb NOT NULL, automatic_allowed boolean NOT NULL, requires_human_approval boolean NOT NULL, actions jsonb NOT NULL, document_sha256 text NOT NULL CHECK(document_sha256 ~ '^[0-9a-f]{64}$'), signer_id text NOT NULL, signature_hex text NOT NULL CHECK(signature_hex ~ '^[0-9a-f]{128}$'), created_at timestamptz NOT NULL, expires_at timestamptz NOT NULL, PRIMARY KEY(tenant_uuid,plan_uuid), UNIQUE(tenant_uuid,document_sha256), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid), FOREIGN KEY(tenant_uuid,portfolio_uuid) REFERENCES phxclaw_ai_portfolios(tenant_uuid,portfolio_uuid), CHECK(created_at<expires_at), CHECK(automatic_allowed <> requires_human_approval)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_autopilot_executions (
 tenant_uuid uuid NOT NULL, execution_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, controller_uuid uuid NOT NULL, leader_epoch bigint NOT NULL CHECK(leader_epoch>0), source_state_sha256 text NOT NULL CHECK(source_state_sha256 ~ '^[0-9a-f]{64}$'), result_state_sha256 text NOT NULL CHECK(result_state_sha256 ~ '^[0-9a-f]{64}$'), execution_sha256 text NOT NULL CHECK(execution_sha256 ~ '^[0-9a-f]{64}$'), executed_at timestamptz NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,execution_uuid), UNIQUE(tenant_uuid,plan_uuid), FOREIGN KEY(tenant_uuid,plan_uuid) REFERENCES phxclaw_ai_autopilot_plans(tenant_uuid,plan_uuid), FOREIGN KEY(tenant_uuid,controller_uuid) REFERENCES phxclaw_controller_members(tenant_uuid,controller_uuid)
);

CREATE OR REPLACE FUNCTION phxclaw_ai_autopilot_execute(
 p_tenant uuid,p_execution uuid,p_plan uuid,p_controller uuid,p_epoch bigint,p_source_sha text,p_result_sha text,p_execution_sha text
) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE p phxclaw_ai_autopilot_plans%ROWTYPE; e phxclaw_ai_autopilot_executions%ROWTYPE;
BEGIN
 IF p_source_sha !~ '^[0-9a-f]{64}$' OR p_result_sha !~ '^[0-9a-f]{64}$' OR p_execution_sha !~ '^[0-9a-f]{64}$' THEN RETURN false; END IF;
 IF NOT phxclaw_assert_controller_fence(p_tenant,p_controller,p_epoch) THEN RETURN false; END IF;
 SELECT * INTO p FROM phxclaw_ai_autopilot_plans WHERE tenant_uuid=p_tenant AND plan_uuid=p_plan;
 IF NOT FOUND OR NOT p.automatic_allowed OR p.requires_human_approval OR p.source_state_sha256<>p_source_sha OR p.expires_at<=clock_timestamp() THEN RETURN false; END IF;
 INSERT INTO phxclaw_ai_autopilot_executions(tenant_uuid,execution_uuid,plan_uuid,controller_uuid,leader_epoch,source_state_sha256,result_state_sha256,execution_sha256,executed_at)
 VALUES(p_tenant,p_execution,p_plan,p_controller,p_epoch,p_source_sha,p_result_sha,p_execution_sha,clock_timestamp()) ON CONFLICT (tenant_uuid,plan_uuid) DO NOTHING;
 SELECT * INTO e FROM phxclaw_ai_autopilot_executions WHERE tenant_uuid=p_tenant AND plan_uuid=p_plan;
 RETURN e.controller_uuid=p_controller AND e.leader_epoch=p_epoch AND e.source_state_sha256=p_source_sha AND e.result_state_sha256=p_result_sha AND e.execution_sha256=p_execution_sha;
END $$;

DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY[
 'phxclaw_ai_sre_policies','phxclaw_ai_slo_samples','phxclaw_ai_slo_evaluations','phxclaw_ai_demand_samples','phxclaw_ai_demand_forecasts','phxclaw_ai_rate_limit_snapshots','phxclaw_ai_admission_decisions','phxclaw_ai_budget_snapshots','phxclaw_ai_cost_forecasts','phxclaw_ai_incident_events','phxclaw_ai_autopilot_plans','phxclaw_ai_autopilot_executions'
] LOOP
 EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t); EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t);
 EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t);
END LOOP; END $$;

CREATE OR REPLACE FUNCTION phxclaw_ai_sre_no_mutation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'AI SRE evidence/configuration is append-only'; END $$;
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY[
 'phxclaw_ai_sre_policies','phxclaw_ai_slo_samples','phxclaw_ai_slo_evaluations','phxclaw_ai_demand_samples','phxclaw_ai_demand_forecasts','phxclaw_ai_rate_limit_snapshots','phxclaw_ai_admission_decisions','phxclaw_ai_budget_snapshots','phxclaw_ai_cost_forecasts','phxclaw_ai_incident_events','phxclaw_ai_autopilot_plans','phxclaw_ai_autopilot_executions'
] LOOP
 EXECUTE format('DROP TRIGGER IF EXISTS ai_sre_append_only ON %I',t); EXECUTE format('CREATE TRIGGER ai_sre_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_sre_no_mutation()',t);
END LOOP; END $$;
COMMIT;

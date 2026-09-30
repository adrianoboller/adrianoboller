-- PhxClaw v0.32 — AI Benchmark & Adaptive Model Intelligence
CREATE TABLE IF NOT EXISTS phxclaw_ai_benchmark_suites (
 tenant_uuid uuid NOT NULL, suite_uuid uuid NOT NULL, name text NOT NULL, suite_version text NOT NULL,
 task_family text NOT NULL, complexity text NOT NULL CHECK(complexity IN ('low','medium','high','extreme')), evidence_class text NOT NULL CHECK(evidence_class IN ('production','fixture')),
 dataset_sha256 text NOT NULL CHECK(dataset_sha256 ~ '^[0-9a-f]{64}$'), scorer_sha256 text NOT NULL CHECK(scorer_sha256 ~ '^[0-9a-f]{64}$'),
 environment_sha256 text NOT NULL CHECK(environment_sha256 ~ '^[0-9a-f]{64}$'), case_count integer NOT NULL CHECK(case_count>0),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,suite_uuid), UNIQUE(tenant_uuid,name,suite_version)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_benchmark_cases (
 tenant_uuid uuid NOT NULL, suite_uuid uuid NOT NULL, case_uuid uuid NOT NULL,
 prompt_sha256 text NOT NULL CHECK(prompt_sha256 ~ '^[0-9a-f]{64}$'), expected_contract_sha256 text NOT NULL CHECK(expected_contract_sha256 ~ '^[0-9a-f]{64}$'),
 tags jsonb NOT NULL DEFAULT '[]'::jsonb, weight integer NOT NULL CHECK(weight>0),
 PRIMARY KEY(tenant_uuid,suite_uuid,case_uuid), FOREIGN KEY(tenant_uuid,suite_uuid) REFERENCES phxclaw_ai_benchmark_suites(tenant_uuid,suite_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_benchmark_runs (
 tenant_uuid uuid NOT NULL, run_uuid uuid NOT NULL, suite_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, model_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK(source_state_sha256 ~ '^[0-9a-f]{64}$'), started_at timestamptz NOT NULL DEFAULT clock_timestamp(), finished_at timestamptz,
 state text NOT NULL CHECK(state IN ('planned','running','completed','failed','cancelled')), PRIMARY KEY(tenant_uuid,run_uuid),
 FOREIGN KEY(tenant_uuid,suite_uuid) REFERENCES phxclaw_ai_benchmark_suites(tenant_uuid,suite_uuid),
 FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid), FOREIGN KEY(tenant_uuid,model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_benchmark_observations (
 tenant_uuid uuid NOT NULL, observation_uuid uuid NOT NULL, run_uuid uuid NOT NULL, suite_uuid uuid NOT NULL, case_uuid uuid NOT NULL,
 provider_uuid uuid NOT NULL, model_uuid uuid NOT NULL, dataset_sha256 text NOT NULL CHECK(dataset_sha256 ~ '^[0-9a-f]{64}$'),
 scorer_sha256 text NOT NULL CHECK(scorer_sha256 ~ '^[0-9a-f]{64}$'), environment_sha256 text NOT NULL CHECK(environment_sha256 ~ '^[0-9a-f]{64}$'),
 success boolean NOT NULL, quality_basis_points integer NOT NULL CHECK(quality_basis_points BETWEEN 0 AND 10000),
 tool_accuracy_basis_points integer CHECK(tool_accuracy_basis_points BETWEEN 0 AND 10000), structured_validity_basis_points integer CHECK(structured_validity_basis_points BETWEEN 0 AND 10000),
 latency_ms bigint NOT NULL CHECK(latency_ms>=0), input_tokens bigint NOT NULL CHECK(input_tokens>=0), output_tokens bigint NOT NULL CHECK(output_tokens>=0),
 actual_cost_micro_usd bigint CHECK(actual_cost_micro_usd>=0), output_sha256 text NOT NULL CHECK(output_sha256 ~ '^[0-9a-f]{64}$'), error_class text,
 observed_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,observation_uuid), UNIQUE(tenant_uuid,run_uuid,case_uuid),
 FOREIGN KEY(tenant_uuid,run_uuid) REFERENCES phxclaw_ai_benchmark_runs(tenant_uuid,run_uuid),
 FOREIGN KEY(tenant_uuid,suite_uuid,case_uuid) REFERENCES phxclaw_ai_benchmark_cases(tenant_uuid,suite_uuid,case_uuid),
 FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid), FOREIGN KEY(tenant_uuid,model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_performance_profiles (
 tenant_uuid uuid NOT NULL, profile_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, model_uuid uuid NOT NULL, suite_uuid uuid NOT NULL,
 task_family text NOT NULL, complexity text NOT NULL CHECK(complexity IN ('low','medium','high','extreme')), evidence_class text NOT NULL CHECK(evidence_class IN ('production','fixture')),
 dataset_sha256 text NOT NULL CHECK(dataset_sha256 ~ '^[0-9a-f]{64}$'), scorer_sha256 text NOT NULL CHECK(scorer_sha256 ~ '^[0-9a-f]{64}$'), environment_sha256 text NOT NULL CHECK(environment_sha256 ~ '^[0-9a-f]{64}$'),
 sample_count integer NOT NULL CHECK(sample_count>0), success_basis_points integer NOT NULL CHECK(success_basis_points BETWEEN 0 AND 10000), quality_basis_points integer NOT NULL CHECK(quality_basis_points BETWEEN 0 AND 10000),
 tool_accuracy_basis_points integer CHECK(tool_accuracy_basis_points BETWEEN 0 AND 10000), structured_validity_basis_points integer CHECK(structured_validity_basis_points BETWEEN 0 AND 10000),
 p95_latency_ms bigint NOT NULL CHECK(p95_latency_ms>=0), median_cost_micro_usd bigint CHECK(median_cost_micro_usd>=0), evidence_coverage_basis_points integer NOT NULL CHECK(evidence_coverage_basis_points BETWEEN 0 AND 10000),
 profile_sha256 text NOT NULL CHECK(profile_sha256 ~ '^[0-9a-f]{64}$'), observed_at timestamptz NOT NULL, ttl_seconds integer NOT NULL CHECK(ttl_seconds>0),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,profile_uuid), UNIQUE(tenant_uuid,provider_uuid,model_uuid,suite_uuid,profile_sha256),
 FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid), FOREIGN KEY(tenant_uuid,model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid),
 FOREIGN KEY(tenant_uuid,suite_uuid) REFERENCES phxclaw_ai_benchmark_suites(tenant_uuid,suite_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_profile_promotions (
 tenant_uuid uuid NOT NULL, promotion_uuid uuid NOT NULL, profile_uuid uuid NOT NULL, previous_profile_uuid uuid,
 policy_sha256 text NOT NULL CHECK(policy_sha256 ~ '^[0-9a-f]{64}$'), approved_by text NOT NULL, promoted_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_uuid,promotion_uuid), FOREIGN KEY(tenant_uuid,profile_uuid) REFERENCES phxclaw_ai_performance_profiles(tenant_uuid,profile_uuid),
 FOREIGN KEY(tenant_uuid,previous_profile_uuid) REFERENCES phxclaw_ai_performance_profiles(tenant_uuid,profile_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_adaptive_route_evidence (
 tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, request_uuid uuid NOT NULL, base_decision_sha256 text NOT NULL CHECK(base_decision_sha256 ~ '^[0-9a-f]{64}$'),
 adaptive_decision_sha256 text NOT NULL CHECK(adaptive_decision_sha256 ~ '^[0-9a-f]{64}$'), selected_provider_uuid uuid NOT NULL, selected_model_uuid uuid NOT NULL,
 task_family text NOT NULL, complexity text NOT NULL, profile_hashes jsonb NOT NULL DEFAULT '[]'::jsonb, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_uuid,evidence_uuid), UNIQUE(tenant_uuid,request_uuid), FOREIGN KEY(tenant_uuid,selected_provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid),
 FOREIGN KEY(tenant_uuid,selected_model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid)
);

DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY[
 'phxclaw_ai_benchmark_suites','phxclaw_ai_benchmark_cases','phxclaw_ai_benchmark_runs','phxclaw_ai_benchmark_observations',
 'phxclaw_ai_performance_profiles','phxclaw_ai_profile_promotions','phxclaw_ai_adaptive_route_evidence'
] LOOP
 EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
 EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t);
 EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t);
END LOOP; END $$;

CREATE OR REPLACE FUNCTION phxclaw_ai_benchmark_no_mutation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'benchmark evidence is append-only'; END $$;
DROP TRIGGER IF EXISTS ai_benchmark_observation_append_only ON phxclaw_ai_benchmark_observations;
CREATE TRIGGER ai_benchmark_observation_append_only BEFORE UPDATE OR DELETE ON phxclaw_ai_benchmark_observations FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_benchmark_no_mutation();
DROP TRIGGER IF EXISTS ai_profile_append_only ON phxclaw_ai_performance_profiles;
CREATE TRIGGER ai_profile_append_only BEFORE UPDATE OR DELETE ON phxclaw_ai_performance_profiles FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_benchmark_no_mutation();
DROP TRIGGER IF EXISTS ai_profile_promotion_append_only ON phxclaw_ai_profile_promotions;
CREATE TRIGGER ai_profile_promotion_append_only BEFORE UPDATE OR DELETE ON phxclaw_ai_profile_promotions FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_benchmark_no_mutation();
DROP TRIGGER IF EXISTS ai_adaptive_evidence_append_only ON phxclaw_ai_adaptive_route_evidence;
CREATE TRIGGER ai_adaptive_evidence_append_only BEFORE UPDATE OR DELETE ON phxclaw_ai_adaptive_route_evidence FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_benchmark_no_mutation();

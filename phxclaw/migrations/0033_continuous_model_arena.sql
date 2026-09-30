-- PhxClaw v0.33 — Continuous Model Arena + Drift
CREATE TABLE IF NOT EXISTS phxclaw_ai_arenas (
 tenant_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, name text NOT NULL, mode text NOT NULL CHECK(mode IN ('shadow','canary','paired')),
 task_family text NOT NULL, complexity text NOT NULL CHECK(complexity IN ('low','medium','high','extreme')), evidence_class text NOT NULL CHECK(evidence_class IN ('production','fixture')),
 champion_provider_uuid uuid NOT NULL, champion_model_uuid uuid NOT NULL, challenger_traffic_basis_points integer NOT NULL CHECK(challenger_traffic_basis_points BETWEEN 0 AND 10000),
 assignment_salt_sha256 text NOT NULL CHECK(assignment_salt_sha256 ~ '^[0-9a-f]{64}$'), policy_sha256 text NOT NULL CHECK(policy_sha256 ~ '^[0-9a-f]{64}$'), document_sha256 text NOT NULL CHECK(document_sha256 ~ '^[0-9a-f]{64}$'),
 signer_id text NOT NULL, initial_state text NOT NULL DEFAULT 'draft' CHECK(initial_state='draft'), starts_at timestamptz NOT NULL, expires_at timestamptz NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,arena_uuid),
 FOREIGN KEY(tenant_uuid,champion_provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid), FOREIGN KEY(tenant_uuid,champion_model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_arena_state_events (
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, from_state text NOT NULL CHECK(from_state IN ('draft','active','paused','completed','cancelled')), to_state text NOT NULL CHECK(to_state IN ('draft','active','paused','completed','cancelled')),
 actor_id text NOT NULL, policy_sha256 text NOT NULL CHECK(policy_sha256 ~ '^[0-9a-f]{64}$'), event_sha256 text NOT NULL CHECK(event_sha256 ~ '^[0-9a-f]{64}$'), occurred_at timestamptz NOT NULL,
 PRIMARY KEY(tenant_uuid,event_uuid), FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_arena_challengers (
 tenant_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, model_uuid uuid NOT NULL, ordinal integer NOT NULL CHECK(ordinal>=0),
 PRIMARY KEY(tenant_uuid,arena_uuid,provider_uuid,model_uuid), UNIQUE(tenant_uuid,arena_uuid,ordinal),
 FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid), FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid), FOREIGN KEY(tenant_uuid,model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_arena_assignments (
 tenant_uuid uuid NOT NULL, assignment_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, request_uuid uuid NOT NULL, served_provider_uuid uuid NOT NULL, served_model_uuid uuid NOT NULL,
 challenger_provider_uuid uuid, challenger_model_uuid uuid, execute_shadow boolean NOT NULL, assignment_bucket integer NOT NULL CHECK(assignment_bucket BETWEEN 0 AND 9999), assignment_sha256 text NOT NULL CHECK(assignment_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,assignment_uuid), UNIQUE(tenant_uuid,arena_uuid,request_uuid), FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_arena_pair_observations (
 tenant_uuid uuid NOT NULL, observation_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, request_uuid uuid NOT NULL, case_uuid uuid NOT NULL, evidence_class text NOT NULL CHECK(evidence_class IN ('production','fixture')),
 champion_provider_uuid uuid NOT NULL, champion_model_uuid uuid NOT NULL, challenger_provider_uuid uuid NOT NULL, challenger_model_uuid uuid NOT NULL,
 champion_success boolean NOT NULL, challenger_success boolean NOT NULL, champion_quality_basis_points integer NOT NULL CHECK(champion_quality_basis_points BETWEEN 0 AND 10000), challenger_quality_basis_points integer NOT NULL CHECK(challenger_quality_basis_points BETWEEN 0 AND 10000),
 champion_latency_ms bigint NOT NULL CHECK(champion_latency_ms>=0), challenger_latency_ms bigint NOT NULL CHECK(challenger_latency_ms>=0), champion_cost_micro_usd bigint CHECK(champion_cost_micro_usd>=0), challenger_cost_micro_usd bigint CHECK(challenger_cost_micro_usd>=0),
 champion_safety_violation boolean NOT NULL, challenger_safety_violation boolean NOT NULL, champion_output_sha256 text NOT NULL CHECK(champion_output_sha256 ~ '^[0-9a-f]{64}$'), challenger_output_sha256 text NOT NULL CHECK(challenger_output_sha256 ~ '^[0-9a-f]{64}$'),
 dataset_sha256 text NOT NULL CHECK(dataset_sha256 ~ '^[0-9a-f]{64}$'), scorer_sha256 text NOT NULL CHECK(scorer_sha256 ~ '^[0-9a-f]{64}$'), environment_sha256 text NOT NULL CHECK(environment_sha256 ~ '^[0-9a-f]{64}$'), observed_at timestamptz NOT NULL,
 PRIMARY KEY(tenant_uuid,observation_uuid), UNIQUE(tenant_uuid,arena_uuid,request_uuid,case_uuid,challenger_provider_uuid,challenger_model_uuid), FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_arena_windows (
 tenant_uuid uuid NOT NULL, window_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, challenger_provider_uuid uuid NOT NULL, challenger_model_uuid uuid NOT NULL,
 sample_count integer NOT NULL CHECK(sample_count>=0), champion_success_basis_points integer NOT NULL CHECK(champion_success_basis_points BETWEEN 0 AND 10000), challenger_success_basis_points integer NOT NULL CHECK(challenger_success_basis_points BETWEEN 0 AND 10000),
 champion_quality_basis_points integer NOT NULL CHECK(champion_quality_basis_points BETWEEN 0 AND 10000), challenger_quality_basis_points integer NOT NULL CHECK(challenger_quality_basis_points BETWEEN 0 AND 10000), quality_delta_basis_points integer NOT NULL, success_delta_basis_points integer NOT NULL,
 latency_regression_basis_points integer NOT NULL, cost_regression_basis_points integer, safety_violations integer NOT NULL CHECK(safety_violations>=0), evidence_class text NOT NULL CHECK(evidence_class IN ('production','fixture')), window_sha256 text NOT NULL CHECK(window_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,window_uuid), FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_arena_verdicts (
 tenant_uuid uuid NOT NULL, verdict_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, window_uuid uuid NOT NULL, verdict text NOT NULL CHECK(verdict IN ('insufficient','continue','challenger_wins','challenger_regressed','pause_safety')),
 policy_sha256 text NOT NULL CHECK(policy_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,verdict_uuid), FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid), FOREIGN KEY(tenant_uuid,window_uuid) REFERENCES phxclaw_ai_arena_windows(tenant_uuid,window_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_arena_promotion_recommendations (
 tenant_uuid uuid NOT NULL, recommendation_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, challenger_provider_uuid uuid NOT NULL, challenger_model_uuid uuid NOT NULL,
 champion_provider_uuid uuid NOT NULL, champion_model_uuid uuid NOT NULL, policy_sha256 text NOT NULL CHECK(policy_sha256 ~ '^[0-9a-f]{64}$'), supporting_window_hashes jsonb NOT NULL,
 recommendation_sha256 text NOT NULL CHECK(recommendation_sha256 ~ '^[0-9a-f]{64}$'), requires_v032_promotion_gate boolean NOT NULL DEFAULT true CHECK(requires_v032_promotion_gate), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_uuid,recommendation_uuid), FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_drift_events (
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, baseline_profile_uuid uuid NOT NULL, observed_window_uuid uuid NOT NULL,
 severity text NOT NULL CHECK(severity IN ('none','warning','critical')), action text NOT NULL CHECK(action IN ('continue','pause_arena','fallback_base_router')), reason text NOT NULL,
 event_sha256 text NOT NULL CHECK(event_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,event_uuid),
 FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid), FOREIGN KEY(tenant_uuid,baseline_profile_uuid) REFERENCES phxclaw_ai_performance_profiles(tenant_uuid,profile_uuid), FOREIGN KEY(tenant_uuid,observed_window_uuid) REFERENCES phxclaw_ai_arena_windows(tenant_uuid,window_uuid)
);
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY[
 'phxclaw_ai_arenas','phxclaw_ai_arena_state_events','phxclaw_ai_arena_challengers','phxclaw_ai_arena_assignments','phxclaw_ai_arena_pair_observations','phxclaw_ai_arena_windows','phxclaw_ai_arena_verdicts','phxclaw_ai_arena_promotion_recommendations','phxclaw_ai_drift_events'
] LOOP
 EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t); EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t);
 EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t);
END LOOP; END $$;
CREATE OR REPLACE FUNCTION phxclaw_ai_arena_no_mutation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'arena evidence is append-only'; END $$;
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['phxclaw_ai_arenas','phxclaw_ai_arena_state_events','phxclaw_ai_arena_challengers','phxclaw_ai_arena_assignments','phxclaw_ai_arena_pair_observations','phxclaw_ai_arena_windows','phxclaw_ai_arena_verdicts','phxclaw_ai_arena_promotion_recommendations','phxclaw_ai_drift_events'] LOOP
 EXECUTE format('DROP TRIGGER IF EXISTS arena_append_only ON %I',t); EXECUTE format('CREATE TRIGGER arena_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_arena_no_mutation()',t);
END LOOP; END $$;

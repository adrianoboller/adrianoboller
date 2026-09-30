BEGIN;
CREATE TABLE IF NOT EXISTS phx_model_outcomes_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, outcome_uuid uuid NOT NULL,
 task_uuid uuid NOT NULL, agent_uuid uuid NOT NULL, task_class text NOT NULL,
 model_profile_uuid uuid NOT NULL, success boolean NOT NULL, quality_score double precision NOT NULL,
 actual_cost numeric(18,8) NOT NULL, latency_ms bigint NOT NULL, input_tokens bigint NOT NULL,
 output_tokens bigint NOT NULL, failure_signature text, failure_origin text,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, outcome_uuid)
);
CREATE TABLE IF NOT EXISTS phx_agent_model_affinity_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, affinity_uuid uuid NOT NULL,
 agent_uuid uuid NOT NULL, task_class text NOT NULL, model_profile_uuid uuid NOT NULL,
 sample_count integer NOT NULL CHECK(sample_count>=1), success_rate double precision NOT NULL,
 avg_quality double precision NOT NULL, avg_cost numeric(18,8) NOT NULL, avg_latency_ms double precision NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 promoted boolean NOT NULL DEFAULT false, fresh_until timestamptz NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, affinity_uuid)
);
CREATE TABLE IF NOT EXISTS phx_context_budget_decisions_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, decision_uuid uuid NOT NULL,
 task_uuid uuid NOT NULL, requested_tokens integer NOT NULL, budget_tokens integer NOT NULL,
 model_limit_tokens integer NOT NULL, compressed_locally boolean NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 decision_sha256 text NOT NULL CHECK (decision_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, decision_uuid)
);
CREATE TABLE IF NOT EXISTS phx_prompt_plans_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, prompt_uuid uuid NOT NULL,
 task_uuid uuid NOT NULL, model_profile_uuid uuid NOT NULL, provider text NOT NULL,
 prompt_sha256 text NOT NULL CHECK (prompt_sha256 ~ '^[0-9a-f]{64}$'),
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 policy_sha256 text NOT NULL CHECK (policy_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, prompt_uuid)
);
CREATE TABLE IF NOT EXISTS phx_semantic_cache_entries_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, cache_uuid uuid NOT NULL,
 cache_key_sha256 text NOT NULL CHECK (cache_key_sha256 ~ '^[0-9a-f]{64}$'),
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 policy_sha256 text NOT NULL CHECK (policy_sha256 ~ '^[0-9a-f]{64}$'),
 model_profile_uuid uuid NOT NULL, payload_sha256 text NOT NULL CHECK (payload_sha256 ~ '^[0-9a-f]{64}$'),
 fresh_until timestamptz NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, cache_uuid), UNIQUE(tenant_uuid, project_uuid, cache_key_sha256)
);
CREATE TABLE IF NOT EXISTS phx_semantic_cache_events_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, event_uuid uuid NOT NULL,
 cache_uuid uuid NOT NULL, event_kind text NOT NULL CHECK(event_kind IN ('hit','miss','invalidate')),
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, event_uuid)
);
CREATE TABLE IF NOT EXISTS phx_ollama_runtime_samples_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, sample_uuid uuid NOT NULL,
 model text NOT NULL, loaded boolean NOT NULL, size_vram bigint, queue_depth integer,
 latency_ms bigint, tokens_per_second double precision, context_length integer,
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 sampled_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, sample_uuid)
);
CREATE TABLE IF NOT EXISTS phx_performance_events_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, event_uuid uuid NOT NULL,
 event_kind text NOT NULL, source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, event_uuid)
);

DO $do$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['phx_model_outcomes_v054','phx_agent_model_affinity_v054','phx_context_budget_decisions_v054','phx_prompt_plans_v054','phx_semantic_cache_entries_v054','phx_semantic_cache_events_v054','phx_ollama_runtime_samples_v054','phx_performance_events_v054'] LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t);
  EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t);
  EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = NULLIF(current_setting($q$phxclaw.tenant_uuid$q$, true), $q$$q$)::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting($q$phxclaw.tenant_uuid$q$, true), $q$$q$)::uuid)',t);
 END LOOP;
END $do$;

CREATE OR REPLACE FUNCTION phx_v054_append_only() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'append-only table'; END$$;
DO $do$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['phx_model_outcomes_v054','phx_agent_model_affinity_v054','phx_context_budget_decisions_v054','phx_prompt_plans_v054','phx_semantic_cache_entries_v054','phx_semantic_cache_events_v054','phx_ollama_runtime_samples_v054','phx_performance_events_v054'] LOOP
  EXECUTE format('DROP TRIGGER IF EXISTS trg_append_only_v054 ON %I',t);
  EXECUTE format('CREATE TRIGGER trg_append_only_v054 BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phx_v054_append_only()',t);
 END LOOP;
END $do$;
COMMIT;

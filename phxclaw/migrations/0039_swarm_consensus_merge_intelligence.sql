BEGIN;
CREATE TABLE IF NOT EXISTS swarm_change_intents (
 tenant_uuid uuid NOT NULL, intent_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, team text NOT NULL, actor_uuid uuid NOT NULL,
 base_source_sha256 text NOT NULL CHECK (base_source_sha256 ~ '^[0-9A-Fa-f]{64}$'), artifact_sha256 text NOT NULL CHECK (artifact_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 changed_paths jsonb NOT NULL, changed_symbols jsonb NOT NULL, contract_changes jsonb NOT NULL, depends_on_intents jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,intent_uuid), UNIQUE (tenant_uuid,swarm_uuid,intent_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_contract_surfaces (
 tenant_uuid uuid NOT NULL, contract_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, kind text NOT NULL CHECK (kind IN ('api','database','schema','behavior','security_policy','event','config')),
 name text NOT NULL, version text NOT NULL, schema_sha256 text NOT NULL CHECK (schema_sha256 ~ '^[0-9A-Fa-f]{64}$'), breaking boolean NOT NULL DEFAULT false, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,contract_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_semantic_conflicts_v039 (
 tenant_uuid uuid NOT NULL, conflict_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, class text NOT NULL CHECK (class IN ('symbol_collision','contract_violation','dependency_order','schema_migration_conflict','behavioral_divergence','security_policy_conflict','test_expectation_conflict','evidence_conflict')),
 severity text NOT NULL CHECK (severity IN ('info','low','medium','high','critical')), intent_uuids jsonb NOT NULL, subject text NOT NULL,
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9A-Fa-f]{64}$'), blocking boolean NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,conflict_uuid), UNIQUE (tenant_uuid,swarm_uuid,conflict_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_merge_dependencies_v039 (
 tenant_uuid uuid NOT NULL, dependency_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, before_intent_uuid uuid NOT NULL, after_intent_uuid uuid NOT NULL,
 reason text NOT NULL, evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9A-Fa-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,dependency_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid,before_intent_uuid) REFERENCES swarm_change_intents(tenant_uuid,swarm_uuid,intent_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid,after_intent_uuid) REFERENCES swarm_change_intents(tenant_uuid,swarm_uuid,intent_uuid));
CREATE TABLE IF NOT EXISTS swarm_merge_plans_v039 (
 tenant_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, base_source_sha256 text NOT NULL CHECK (base_source_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 ordered_intents jsonb NOT NULL, plan_sha256 text NOT NULL CHECK (plan_sha256 ~ '^[0-9A-Fa-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,plan_uuid), UNIQUE (tenant_uuid,swarm_uuid,plan_uuid), UNIQUE (tenant_uuid,swarm_uuid,plan_sha256), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_integration_replays_v039 (
 tenant_uuid uuid NOT NULL, replay_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, plan_uuid uuid NOT NULL,
 base_source_sha256 text NOT NULL CHECK (base_source_sha256 ~ '^[0-9A-Fa-f]{64}$'), merge_plan_sha256 text NOT NULL CHECK (merge_plan_sha256 ~ '^[0-9A-Fa-f]{64}$'), candidate_artifacts jsonb NOT NULL,
 replay_tree_sha256 text NOT NULL CHECK (replay_tree_sha256 ~ '^[0-9A-Fa-f]{64}$'), test_suite_sha256 text NOT NULL CHECK (test_suite_sha256 ~ '^[0-9A-Fa-f]{64}$'), status text NOT NULL CHECK (status IN ('passed','failed','diverged','side_effect_mismatch')),
 side_effects_sha256 text NOT NULL CHECK (side_effects_sha256 ~ '^[0-9A-Fa-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,replay_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid,plan_uuid) REFERENCES swarm_merge_plans_v039(tenant_uuid,swarm_uuid,plan_uuid));
CREATE TABLE IF NOT EXISTS swarm_mutation_evidence_v039 (
 tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 total_mutants integer NOT NULL CHECK (total_mutants>=0), killed_mutants integer NOT NULL CHECK (killed_mutants>=0 AND killed_mutants<=total_mutants), critical_survivors integer NOT NULL CHECK (critical_survivors>=0), report_sha256 text NOT NULL CHECK (report_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,evidence_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_resolution_documents_v039 (
 tenant_uuid uuid NOT NULL, resolution_uuid uuid NOT NULL, conflict_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, signer_key_id text NOT NULL,
 conflict_evidence_sha256 text NOT NULL CHECK (conflict_evidence_sha256 ~ '^[0-9A-Fa-f]{64}$'), chosen_resolution_sha256 text NOT NULL CHECK (chosen_resolution_sha256 ~ '^[0-9A-Fa-f]{64}$'), rationale_sha256 text NOT NULL CHECK (rationale_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 document_sha256 text NOT NULL CHECK (document_sha256 ~ '^[0-9A-Fa-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,resolution_uuid), UNIQUE (tenant_uuid,conflict_uuid,resolution_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid,conflict_uuid) REFERENCES swarm_semantic_conflicts_v039(tenant_uuid,swarm_uuid,conflict_uuid));
CREATE TABLE IF NOT EXISTS swarm_consensus_decisions_v039 (
 tenant_uuid uuid NOT NULL, decision_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, decision text NOT NULL CHECK (decision IN ('allow','block','needs_approval')),
 evidence_bundle_sha256 text NOT NULL CHECK (evidence_bundle_sha256 ~ '^[0-9A-Fa-f]{64}$'), fencing_token bigint NOT NULL CHECK (fencing_token>=0), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,decision_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid,plan_uuid) REFERENCES swarm_merge_plans_v039(tenant_uuid,swarm_uuid,plan_uuid));
CREATE TABLE IF NOT EXISTS swarm_merge_execution_events_v039 (
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, event_type text NOT NULL CHECK (event_type IN ('planned','replayed','approved','committed','rolled_back','failed')),
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9A-Fa-f]{64}$'), integration_tree_sha256 text NOT NULL CHECK (integration_tree_sha256 ~ '^[0-9A-Fa-f]{64}$'), fencing_token bigint NOT NULL CHECK (fencing_token>=0), payload_sha256 text NOT NULL CHECK (payload_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,event_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid,plan_uuid) REFERENCES swarm_merge_plans_v039(tenant_uuid,swarm_uuid,plan_uuid));
CREATE OR REPLACE FUNCTION phxclaw_prevent_mutation_0039() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only table: %', TG_TABLE_NAME; END $$;
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['swarm_change_intents','swarm_contract_surfaces','swarm_semantic_conflicts_v039','swarm_merge_dependencies_v039','swarm_merge_plans_v039','swarm_integration_replays_v039','swarm_mutation_evidence_v039','swarm_resolution_documents_v039','swarm_consensus_decisions_v039','swarm_merge_execution_events_v039'] LOOP
 EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
 EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t);
 EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t);
 EXECUTE format('DROP TRIGGER IF EXISTS %I_append_only ON %I',t,t); EXECUTE format('CREATE TRIGGER %I_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0039()',t,t);
 END LOOP; END $$;
COMMIT;

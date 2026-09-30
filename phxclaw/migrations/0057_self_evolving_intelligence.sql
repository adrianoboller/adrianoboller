-- PhxClaw v0.57 — governed self-evolving intelligence
CREATE OR REPLACE FUNCTION phx_v057_deny_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'PhxClaw v0.57 append-only relation'; END $$;

CREATE TABLE phx_experience_episodes (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, episode_uuid uuid NOT NULL,
  task_uuid uuid NOT NULL, agent_uuid uuid NOT NULL, model_profile_uuid uuid,
  task_class text NOT NULL, context_fingerprint_sha256 char(64) NOT NULL,
  source_state_sha256 char(64) NOT NULL, prompt_plan_sha256 char(64),
  outcome text NOT NULL CHECK (outcome IN ('success','failure','partial')),
  quality_score double precision NOT NULL CHECK (quality_score BETWEEN 0 AND 1),
  actual_cost_microunits bigint NOT NULL DEFAULT 0 CHECK (actual_cost_microunits >= 0),
  duration_ms bigint NOT NULL DEFAULT 0 CHECK (duration_ms >= 0),
  evidence_class text NOT NULL CHECK (evidence_class IN ('production','fixture')),
  evidence_sha256 char(64) NOT NULL, payload jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, episode_uuid)
);

CREATE TABLE phx_context_fingerprints (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, fingerprint_sha256 char(64) NOT NULL,
  source_state_sha256 char(64) NOT NULL, dimensions jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, fingerprint_sha256)
);

CREATE TABLE phx_knowledge_patterns (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, pattern_uuid uuid NOT NULL,
  pattern_kind text NOT NULL CHECK (pattern_kind IN ('fruitful','unfruitful')),
  pattern_key text NOT NULL, context_fingerprint_sha256 char(64) NOT NULL,
  promotion_state text NOT NULL CHECK (promotion_state IN ('candidate','accepted','governed','stale','revoked')),
  evidence_class text NOT NULL CHECK (evidence_class IN ('production','fixture')),
  evidence_count integer NOT NULL DEFAULT 0 CHECK (evidence_count >= 0),
  success_count integer NOT NULL DEFAULT 0 CHECK (success_count >= 0),
  failure_count integer NOT NULL DEFAULT 0 CHECK (failure_count >= 0),
  reuse_count integer NOT NULL DEFAULT 0 CHECK (reuse_count >= 0),
  confidence double precision NOT NULL DEFAULT 0 CHECK (confidence BETWEEN 0 AND 1),
  root_cause text, remediation text, safe_retry_conditions text,
  source_state_sha256 char(64) NOT NULL, evidence_sha256 char(64) NOT NULL,
  fresh_until timestamptz NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, pattern_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid, context_fingerprint_sha256)
    REFERENCES phx_context_fingerprints(tenant_uuid, project_uuid, fingerprint_sha256)
);

CREATE TABLE phx_evolution_candidates (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, candidate_uuid uuid NOT NULL,
  candidate_kind text NOT NULL CHECK (candidate_kind IN ('knowledge','prompt','skill','workflow','code','core_policy')),
  risk text NOT NULL CHECK (risk IN ('low','medium','high','critical')),
  reversible boolean NOT NULL DEFAULT false, touches_core_or_policy boolean NOT NULL DEFAULT false,
  evidence_class text NOT NULL CHECK (evidence_class IN ('production','fixture')),
  source_state_sha256 char(64) NOT NULL, artifact_sha256 char(64) NOT NULL,
  state text NOT NULL DEFAULT 'candidate' CHECK (state IN ('candidate','experiment','passed','rejected','promoted','rolled_back','stale')),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, candidate_uuid)
);

CREATE TABLE phx_evolution_experiments (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, experiment_uuid uuid NOT NULL,
  candidate_uuid uuid NOT NULL, sandbox_ref text NOT NULL, checkpoint_ref text NOT NULL,
  source_state_sha256 char(64) NOT NULL, result_sha256 char(64), status text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, experiment_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid, candidate_uuid)
    REFERENCES phx_evolution_candidates(tenant_uuid, project_uuid, candidate_uuid)
);

CREATE TABLE phx_evolution_promotion_decisions (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, decision_uuid uuid NOT NULL,
  candidate_uuid uuid NOT NULL, decision text NOT NULL CHECK (decision IN ('approve','reject','rollback','stale')),
  automatic boolean NOT NULL DEFAULT false, approver_uuid uuid, reason text NOT NULL,
  evidence_sha256 char(64) NOT NULL, source_state_sha256 char(64) NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, decision_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid, candidate_uuid)
    REFERENCES phx_evolution_candidates(tenant_uuid, project_uuid, candidate_uuid)
);

CREATE TABLE phx_learning_contradictions (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, contradiction_uuid uuid NOT NULL,
  left_pattern_uuid uuid NOT NULL, right_pattern_uuid uuid NOT NULL,
  state text NOT NULL DEFAULT 'open' CHECK (state IN ('open','resolved','superseded')),
  evidence_sha256 char(64) NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, contradiction_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid, left_pattern_uuid) REFERENCES phx_knowledge_patterns(tenant_uuid, project_uuid, pattern_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid, right_pattern_uuid) REFERENCES phx_knowledge_patterns(tenant_uuid, project_uuid, pattern_uuid)
);

CREATE TABLE phx_learning_contradiction_resolutions (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, resolution_uuid uuid NOT NULL,
  contradiction_uuid uuid NOT NULL, resolution text NOT NULL, evidence_sha256 char(64) NOT NULL,
  approver_uuid uuid, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, resolution_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid, contradiction_uuid) REFERENCES phx_learning_contradictions(tenant_uuid, project_uuid, contradiction_uuid)
);

CREATE TABLE phx_self_improvement_changes (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, change_uuid uuid NOT NULL,
  candidate_uuid uuid NOT NULL, branch_ref text NOT NULL CHECK (branch_ref LIKE 'agent/self-improve/%'),
  worktree_ref text NOT NULL, source_state_sha256 char(64) NOT NULL,
  core_auto_merge boolean NOT NULL DEFAULT false CHECK (core_auto_merge = false),
  human_approval_required boolean NOT NULL DEFAULT true CHECK (human_approval_required = true),
  status text NOT NULL DEFAULT 'sandbox', created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, change_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid, candidate_uuid) REFERENCES phx_evolution_candidates(tenant_uuid, project_uuid, candidate_uuid)
);

CREATE TABLE phx_learning_events (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, event_uuid uuid NOT NULL,
  event_type text NOT NULL, correlation_uuid uuid, causation_uuid uuid,
  source_state_sha256 char(64) NOT NULL, evidence_sha256 char(64) NOT NULL,
  payload jsonb NOT NULL DEFAULT '{}'::jsonb, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, event_uuid)
);

ALTER TABLE phx_experience_episodes ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_experience_episodes FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_context_fingerprints ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_context_fingerprints FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_knowledge_patterns ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_knowledge_patterns FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_evolution_candidates ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_evolution_candidates FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_evolution_experiments ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_evolution_experiments FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_evolution_promotion_decisions ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_evolution_promotion_decisions FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_learning_contradictions ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_learning_contradictions FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_learning_contradiction_resolutions ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_learning_contradiction_resolutions FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_self_improvement_changes ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_self_improvement_changes FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_learning_events ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_learning_events FORCE ROW LEVEL SECURITY;

CREATE POLICY phx_experience_episodes_tenant ON phx_experience_episodes USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_context_fingerprints_tenant ON phx_context_fingerprints USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_knowledge_patterns_tenant ON phx_knowledge_patterns USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_evolution_candidates_tenant ON phx_evolution_candidates USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_evolution_experiments_tenant ON phx_evolution_experiments USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_evolution_promotion_decisions_tenant ON phx_evolution_promotion_decisions USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_learning_contradictions_tenant ON phx_learning_contradictions USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_learning_contradiction_resolutions_tenant ON phx_learning_contradiction_resolutions USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_self_improvement_changes_tenant ON phx_self_improvement_changes USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_learning_events_tenant ON phx_learning_events USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);

CREATE TRIGGER phx_experience_episodes_immutable BEFORE UPDATE OR DELETE ON phx_experience_episodes FOR EACH ROW EXECUTE FUNCTION phx_v057_deny_mutation();
CREATE TRIGGER phx_evolution_promotion_decisions_immutable BEFORE UPDATE OR DELETE ON phx_evolution_promotion_decisions FOR EACH ROW EXECUTE FUNCTION phx_v057_deny_mutation();
CREATE TRIGGER phx_learning_contradiction_resolutions_immutable BEFORE UPDATE OR DELETE ON phx_learning_contradiction_resolutions FOR EACH ROW EXECUTE FUNCTION phx_v057_deny_mutation();
CREATE TRIGGER phx_learning_events_immutable BEFORE UPDATE OR DELETE ON phx_learning_events FOR EACH ROW EXECUTE FUNCTION phx_v057_deny_mutation();

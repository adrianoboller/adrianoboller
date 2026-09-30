-- PhxClaw v0.22 / F24 Auto-learning + Skill Evolution
-- migration_uuid: 01a0e800-cf88-73b0-8373-83be48bfff29
-- PostgreSQL is authoritative. Learning output cannot mutate core/policy state directly.

BEGIN;
CREATE SCHEMA IF NOT EXISTS phxclaw;

CREATE TABLE IF NOT EXISTS phxclaw.learning_observations (
  observation_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  source_kind text NOT NULL CHECK (length(source_kind) BETWEEN 1 AND 80),
  source_ref text NOT NULL CHECK (length(source_ref) BETWEEN 1 AND 1000),
  payload_sha256 bytea NOT NULL CHECK (octet_length(payload_sha256) = 32),
  state text NOT NULL CHECK (state IN ('raw_observation','unverified_context','accepted_evidence','rejected','quarantined')),
  confidence_ppm integer NOT NULL DEFAULT 0 CHECK (confidence_ppm BETWEEN 0 AND 1000000),
  provenance jsonb NOT NULL DEFAULT '{}'::jsonb,
  observed_at timestamptz NOT NULL,
  accepted_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_uuid, source_kind, source_ref, payload_sha256)
);

CREATE TABLE IF NOT EXISTS phxclaw.skill_candidates (
  candidate_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  skill_uuid uuid NOT NULL,
  base_release_uuid uuid,
  target_namespace text NOT NULL CHECK (target_namespace LIKE 'skills.%'),
  boundary text NOT NULL CHECK (boundary = 'skill_plugin'),
  artifact_sha256 bytea NOT NULL CHECK (octet_length(artifact_sha256) = 32),
  manifest_sha256 bytea NOT NULL CHECK (octet_length(manifest_sha256) = 32),
  source_state_sha256 bytea NOT NULL CHECK (octet_length(source_state_sha256) = 32),
  risk text NOT NULL CHECK (risk IN ('low','medium','high','critical')),
  behavior_change boolean NOT NULL DEFAULT true,
  reversible boolean NOT NULL DEFAULT false,
  state text NOT NULL CHECK (state IN ('draft','evaluating','eligible','awaiting_approval','rejected','quarantined','promoted')),
  synthesized_by_agent_uuid uuid,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_uuid, skill_uuid, artifact_sha256)
);

CREATE TABLE IF NOT EXISTS phxclaw.skill_candidate_sources (
  tenant_uuid uuid NOT NULL,
  candidate_uuid uuid NOT NULL REFERENCES phxclaw.skill_candidates(candidate_uuid) ON DELETE CASCADE,
  observation_uuid uuid NOT NULL REFERENCES phxclaw.learning_observations(observation_uuid) ON DELETE RESTRICT,
  PRIMARY KEY (tenant_uuid, candidate_uuid, observation_uuid)
);

CREATE TABLE IF NOT EXISTS phxclaw.skill_candidate_evidence (
  evidence_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  candidate_uuid uuid NOT NULL REFERENCES phxclaw.skill_candidates(candidate_uuid) ON DELETE CASCADE,
  evaluator_uuid uuid NOT NULL,
  evidence_kind text NOT NULL CHECK (evidence_kind IN ('test','security','benchmark','differential','policy','provenance')),
  outcome text NOT NULL CHECK (outcome IN ('pass','fail','warn')),
  artifact_sha256 bytea NOT NULL CHECK (octet_length(artifact_sha256) = 32),
  source_state_sha256 bytea NOT NULL CHECK (octet_length(source_state_sha256) = 32),
  metrics jsonb NOT NULL DEFAULT '{}'::jsonb,
  evidence_ref text,
  recorded_at timestamptz NOT NULL,
  UNIQUE (tenant_uuid, candidate_uuid, evaluator_uuid, evidence_kind, evidence_ref)
);

CREATE TABLE IF NOT EXISTS phxclaw.skill_promotion_requests (
  promotion_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  candidate_uuid uuid NOT NULL REFERENCES phxclaw.skill_candidates(candidate_uuid) ON DELETE RESTRICT,
  state text NOT NULL CHECK (state IN ('pending','approved','rejected','published','activated','quarantined','rolled_back')),
  auto_eligible boolean NOT NULL DEFAULT false,
  human_approval_required boolean NOT NULL DEFAULT true,
  requested_by_uuid uuid NOT NULL,
  approval_uuid uuid,
  approved_by_uuid uuid,
  approved_at timestamptz,
  approval_expires_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_uuid, candidate_uuid, state) DEFERRABLE INITIALLY IMMEDIATE
);

CREATE TABLE IF NOT EXISTS phxclaw.skill_releases (
  release_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  skill_uuid uuid NOT NULL,
  candidate_uuid uuid NOT NULL REFERENCES phxclaw.skill_candidates(candidate_uuid) ON DELETE RESTRICT,
  version text NOT NULL CHECK (length(version) BETWEEN 1 AND 64),
  artifact_sha256 bytea NOT NULL CHECK (octet_length(artifact_sha256) = 32),
  manifest_sha256 bytea NOT NULL CHECK (octet_length(manifest_sha256) = 32),
  previous_release_uuid uuid REFERENCES phxclaw.skill_releases(release_uuid) ON DELETE RESTRICT,
  state text NOT NULL CHECK (state IN ('staged','canary','active','quarantined','rolled_back')),
  activated_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_uuid, skill_uuid, version),
  UNIQUE (tenant_uuid, candidate_uuid)
);

CREATE TABLE IF NOT EXISTS phxclaw.skill_evolution_events (
  event_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  candidate_uuid uuid,
  release_uuid uuid,
  event_type text NOT NULL,
  event_data jsonb NOT NULL DEFAULT '{}'::jsonb,
  correlation_uuid uuid,
  causation_uuid uuid,
  recorded_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS learning_observations_state_idx ON phxclaw.learning_observations (tenant_uuid, state, observed_at);
CREATE INDEX IF NOT EXISTS skill_candidates_state_idx ON phxclaw.skill_candidates (tenant_uuid, state, created_at);
CREATE INDEX IF NOT EXISTS skill_evidence_candidate_idx ON phxclaw.skill_candidate_evidence (tenant_uuid, candidate_uuid, recorded_at);
CREATE INDEX IF NOT EXISTS skill_releases_active_idx ON phxclaw.skill_releases (tenant_uuid, skill_uuid, state);

ALTER TABLE phxclaw.learning_observations ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.skill_candidates ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.skill_candidate_sources ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.skill_candidate_evidence ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.skill_promotion_requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.skill_releases ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.skill_evolution_events ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation ON phxclaw.learning_observations;
CREATE POLICY tenant_isolation ON phxclaw.learning_observations USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.skill_candidates;
CREATE POLICY tenant_isolation ON phxclaw.skill_candidates USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.skill_candidate_sources;
CREATE POLICY tenant_isolation ON phxclaw.skill_candidate_sources USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.skill_candidate_evidence;
CREATE POLICY tenant_isolation ON phxclaw.skill_candidate_evidence USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.skill_promotion_requests;
CREATE POLICY tenant_isolation ON phxclaw.skill_promotion_requests USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.skill_releases;
CREATE POLICY tenant_isolation ON phxclaw.skill_releases USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.skill_evolution_events;
CREATE POLICY tenant_isolation ON phxclaw.skill_evolution_events USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);

COMMIT;

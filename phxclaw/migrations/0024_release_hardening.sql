-- PhxClaw v0.24 Release Hardening
-- migration_uuid: 01a0e800-dca8-7e40-9bf1-6e4e2f6f8024
-- Adds DB-enforced tenant relationship integrity, FORCE RLS and evidence-driven release gates.

BEGIN;
CREATE SCHEMA IF NOT EXISTS phxclaw;
REVOKE ALL ON SCHEMA phxclaw FROM PUBLIC;
REVOKE ALL ON ALL TABLES IN SCHEMA phxclaw FROM PUBLIC;
REVOKE ALL ON ALL SEQUENCES IN SCHEMA phxclaw FROM PUBLIC;
REVOKE ALL ON ALL FUNCTIONS IN SCHEMA phxclaw FROM PUBLIC;

-- Parent composite keys required for tenant-aware foreign keys.
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='uq_device_commands_tenant_command' AND conrelid='phxclaw.device_commands'::regclass) THEN
    ALTER TABLE phxclaw.device_commands ADD CONSTRAINT uq_device_commands_tenant_command UNIQUE (tenant_uuid, command_uuid);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='uq_learning_observations_tenant_observation' AND conrelid='phxclaw.learning_observations'::regclass) THEN
    ALTER TABLE phxclaw.learning_observations ADD CONSTRAINT uq_learning_observations_tenant_observation UNIQUE (tenant_uuid, observation_uuid);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='uq_skill_candidates_tenant_candidate' AND conrelid='phxclaw.skill_candidates'::regclass) THEN
    ALTER TABLE phxclaw.skill_candidates ADD CONSTRAINT uq_skill_candidates_tenant_candidate UNIQUE (tenant_uuid, candidate_uuid);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='uq_skill_releases_tenant_release' AND conrelid='phxclaw.skill_releases'::regclass) THEN
    ALTER TABLE phxclaw.skill_releases ADD CONSTRAINT uq_skill_releases_tenant_release UNIQUE (tenant_uuid, release_uuid);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='uq_knowledge_nodes_tenant_node' AND conrelid='phxclaw.knowledge_nodes'::regclass) THEN
    ALTER TABLE phxclaw.knowledge_nodes ADD CONSTRAINT uq_knowledge_nodes_tenant_node UNIQUE (tenant_uuid, node_uuid);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='uq_knowledge_contradictions_tenant_contradiction' AND conrelid='phxclaw.knowledge_contradictions'::regclass) THEN
    ALTER TABLE phxclaw.knowledge_contradictions ADD CONSTRAINT uq_knowledge_contradictions_tenant_contradiction UNIQUE (tenant_uuid, contradiction_uuid);
  END IF;
END $$;

-- F22: tenant-aware relationship integrity.
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_device_capabilities_tenant_node') THEN
    ALTER TABLE phxclaw.device_capabilities ADD CONSTRAINT fk_device_capabilities_tenant_node
      FOREIGN KEY (tenant_uuid, node_uuid) REFERENCES phxclaw.device_nodes(tenant_uuid, node_uuid) ON DELETE CASCADE NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_device_enrollment_consumed_tenant_node') THEN
    ALTER TABLE phxclaw.device_enrollment_tokens ADD CONSTRAINT fk_device_enrollment_consumed_tenant_node
      FOREIGN KEY (tenant_uuid, consumed_by_node_uuid) REFERENCES phxclaw.device_nodes(tenant_uuid, node_uuid) ON DELETE RESTRICT NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_device_sessions_tenant_node') THEN
    ALTER TABLE phxclaw.device_sessions ADD CONSTRAINT fk_device_sessions_tenant_node
      FOREIGN KEY (tenant_uuid, node_uuid) REFERENCES phxclaw.device_nodes(tenant_uuid, node_uuid) ON DELETE CASCADE NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_device_replay_tenant_session') THEN
    ALTER TABLE phxclaw.device_replay_reservations ADD CONSTRAINT fk_device_replay_tenant_session
      FOREIGN KEY (tenant_uuid, node_uuid, session_uuid) REFERENCES phxclaw.device_sessions(tenant_uuid, node_uuid, session_uuid) ON DELETE CASCADE NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_device_commands_tenant_node') THEN
    ALTER TABLE phxclaw.device_commands ADD CONSTRAINT fk_device_commands_tenant_node
      FOREIGN KEY (tenant_uuid, node_uuid) REFERENCES phxclaw.device_nodes(tenant_uuid, node_uuid) ON DELETE RESTRICT NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_device_events_tenant_command') THEN
    ALTER TABLE phxclaw.device_command_events ADD CONSTRAINT fk_device_events_tenant_command
      FOREIGN KEY (tenant_uuid, command_uuid) REFERENCES phxclaw.device_commands(tenant_uuid, command_uuid) ON DELETE CASCADE NOT VALID;
  END IF;
END $$;

-- F24: candidate/observation/release relations cannot cross tenants.
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_skill_sources_tenant_candidate') THEN
    ALTER TABLE phxclaw.skill_candidate_sources ADD CONSTRAINT fk_skill_sources_tenant_candidate
      FOREIGN KEY (tenant_uuid, candidate_uuid) REFERENCES phxclaw.skill_candidates(tenant_uuid, candidate_uuid) ON DELETE CASCADE NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_skill_sources_tenant_observation') THEN
    ALTER TABLE phxclaw.skill_candidate_sources ADD CONSTRAINT fk_skill_sources_tenant_observation
      FOREIGN KEY (tenant_uuid, observation_uuid) REFERENCES phxclaw.learning_observations(tenant_uuid, observation_uuid) ON DELETE RESTRICT NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_skill_evidence_tenant_candidate') THEN
    ALTER TABLE phxclaw.skill_candidate_evidence ADD CONSTRAINT fk_skill_evidence_tenant_candidate
      FOREIGN KEY (tenant_uuid, candidate_uuid) REFERENCES phxclaw.skill_candidates(tenant_uuid, candidate_uuid) ON DELETE CASCADE NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_skill_promotion_tenant_candidate') THEN
    ALTER TABLE phxclaw.skill_promotion_requests ADD CONSTRAINT fk_skill_promotion_tenant_candidate
      FOREIGN KEY (tenant_uuid, candidate_uuid) REFERENCES phxclaw.skill_candidates(tenant_uuid, candidate_uuid) ON DELETE RESTRICT NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_skill_releases_tenant_candidate') THEN
    ALTER TABLE phxclaw.skill_releases ADD CONSTRAINT fk_skill_releases_tenant_candidate
      FOREIGN KEY (tenant_uuid, candidate_uuid) REFERENCES phxclaw.skill_candidates(tenant_uuid, candidate_uuid) ON DELETE RESTRICT NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_skill_releases_tenant_previous') THEN
    ALTER TABLE phxclaw.skill_releases ADD CONSTRAINT fk_skill_releases_tenant_previous
      FOREIGN KEY (tenant_uuid, previous_release_uuid) REFERENCES phxclaw.skill_releases(tenant_uuid, release_uuid) ON DELETE RESTRICT NOT VALID;
  END IF;
END $$;

-- F25: every graph relation is tenant-aware at the database layer.
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_knowledge_edges_tenant_from') THEN
    ALTER TABLE phxclaw.knowledge_edges ADD CONSTRAINT fk_knowledge_edges_tenant_from
      FOREIGN KEY (tenant_uuid, from_node_uuid) REFERENCES phxclaw.knowledge_nodes(tenant_uuid, node_uuid) ON DELETE RESTRICT NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_knowledge_edges_tenant_to') THEN
    ALTER TABLE phxclaw.knowledge_edges ADD CONSTRAINT fk_knowledge_edges_tenant_to
      FOREIGN KEY (tenant_uuid, to_node_uuid) REFERENCES phxclaw.knowledge_nodes(tenant_uuid, node_uuid) ON DELETE RESTRICT NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_knowledge_bindings_tenant_claim') THEN
    ALTER TABLE phxclaw.knowledge_evidence_bindings ADD CONSTRAINT fk_knowledge_bindings_tenant_claim
      FOREIGN KEY (tenant_uuid, claim_node_uuid) REFERENCES phxclaw.knowledge_nodes(tenant_uuid, node_uuid) ON DELETE RESTRICT NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_knowledge_bindings_tenant_evidence') THEN
    ALTER TABLE phxclaw.knowledge_evidence_bindings ADD CONSTRAINT fk_knowledge_bindings_tenant_evidence
      FOREIGN KEY (tenant_uuid, evidence_node_uuid) REFERENCES phxclaw.knowledge_nodes(tenant_uuid, node_uuid) ON DELETE RESTRICT NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_knowledge_contradictions_tenant_left') THEN
    ALTER TABLE phxclaw.knowledge_contradictions ADD CONSTRAINT fk_knowledge_contradictions_tenant_left
      FOREIGN KEY (tenant_uuid, left_claim_uuid) REFERENCES phxclaw.knowledge_nodes(tenant_uuid, node_uuid) ON DELETE RESTRICT NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_knowledge_contradictions_tenant_right') THEN
    ALTER TABLE phxclaw.knowledge_contradictions ADD CONSTRAINT fk_knowledge_contradictions_tenant_right
      FOREIGN KEY (tenant_uuid, right_claim_uuid) REFERENCES phxclaw.knowledge_nodes(tenant_uuid, node_uuid) ON DELETE RESTRICT NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_knowledge_resolutions_tenant_contradiction') THEN
    ALTER TABLE phxclaw.knowledge_contradiction_resolutions ADD CONSTRAINT fk_knowledge_resolutions_tenant_contradiction
      FOREIGN KEY (tenant_uuid, contradiction_uuid) REFERENCES phxclaw.knowledge_contradictions(tenant_uuid, contradiction_uuid) ON DELETE RESTRICT NOT VALID;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname='fk_knowledge_resolutions_tenant_node') THEN
    ALTER TABLE phxclaw.knowledge_contradiction_resolutions ADD CONSTRAINT fk_knowledge_resolutions_tenant_node
      FOREIGN KEY (tenant_uuid, resolution_node_uuid) REFERENCES phxclaw.knowledge_nodes(tenant_uuid, node_uuid) ON DELETE RESTRICT NOT VALID;
  END IF;
END $$;

-- Validate every new composite FK now. Existing cross-tenant corruption aborts migration.
ALTER TABLE phxclaw.device_capabilities VALIDATE CONSTRAINT fk_device_capabilities_tenant_node;
ALTER TABLE phxclaw.device_enrollment_tokens VALIDATE CONSTRAINT fk_device_enrollment_consumed_tenant_node;
ALTER TABLE phxclaw.device_sessions VALIDATE CONSTRAINT fk_device_sessions_tenant_node;
ALTER TABLE phxclaw.device_replay_reservations VALIDATE CONSTRAINT fk_device_replay_tenant_session;
ALTER TABLE phxclaw.device_commands VALIDATE CONSTRAINT fk_device_commands_tenant_node;
ALTER TABLE phxclaw.device_command_events VALIDATE CONSTRAINT fk_device_events_tenant_command;
ALTER TABLE phxclaw.skill_candidate_sources VALIDATE CONSTRAINT fk_skill_sources_tenant_candidate;
ALTER TABLE phxclaw.skill_candidate_sources VALIDATE CONSTRAINT fk_skill_sources_tenant_observation;
ALTER TABLE phxclaw.skill_candidate_evidence VALIDATE CONSTRAINT fk_skill_evidence_tenant_candidate;
ALTER TABLE phxclaw.skill_promotion_requests VALIDATE CONSTRAINT fk_skill_promotion_tenant_candidate;
ALTER TABLE phxclaw.skill_releases VALIDATE CONSTRAINT fk_skill_releases_tenant_candidate;
ALTER TABLE phxclaw.skill_releases VALIDATE CONSTRAINT fk_skill_releases_tenant_previous;
ALTER TABLE phxclaw.knowledge_edges VALIDATE CONSTRAINT fk_knowledge_edges_tenant_from;
ALTER TABLE phxclaw.knowledge_edges VALIDATE CONSTRAINT fk_knowledge_edges_tenant_to;
ALTER TABLE phxclaw.knowledge_evidence_bindings VALIDATE CONSTRAINT fk_knowledge_bindings_tenant_claim;
ALTER TABLE phxclaw.knowledge_evidence_bindings VALIDATE CONSTRAINT fk_knowledge_bindings_tenant_evidence;
ALTER TABLE phxclaw.knowledge_contradictions VALIDATE CONSTRAINT fk_knowledge_contradictions_tenant_left;
ALTER TABLE phxclaw.knowledge_contradictions VALIDATE CONSTRAINT fk_knowledge_contradictions_tenant_right;
ALTER TABLE phxclaw.knowledge_contradiction_resolutions VALIDATE CONSTRAINT fk_knowledge_resolutions_tenant_contradiction;
ALTER TABLE phxclaw.knowledge_contradiction_resolutions VALIDATE CONSTRAINT fk_knowledge_resolutions_tenant_node;

-- FORCE RLS: table owners do not silently bypass tenant policies.
ALTER TABLE phxclaw.device_nodes FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_capabilities FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_enrollment_tokens FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_sessions FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_replay_reservations FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_commands FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_command_events FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.learning_observations FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.skill_candidates FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.skill_candidate_sources FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.skill_candidate_evidence FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.skill_promotion_requests FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.skill_releases FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.skill_evolution_events FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.knowledge_nodes FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.knowledge_edges FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.knowledge_evidence_bindings FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.knowledge_contradictions FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.knowledge_contradiction_resolutions FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.knowledge_snapshots FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.knowledge_graph_events FORCE ROW LEVEL SECURITY;

CREATE TABLE IF NOT EXISTS phxclaw.release_gate_runs (
  run_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  release_uuid uuid NOT NULL,
  version text NOT NULL CHECK (length(version) BETWEEN 1 AND 64),
  workspace_sha256 bytea NOT NULL CHECK (octet_length(workspace_sha256) = 32),
  state text NOT NULL CHECK (state IN ('running','complete','failed','cancelled')),
  started_at timestamptz NOT NULL,
  finished_at timestamptz,
  created_by_uuid uuid,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_uuid, run_uuid),
  UNIQUE (tenant_uuid, release_uuid, workspace_sha256)
);

CREATE TABLE IF NOT EXISTS phxclaw.release_gate_evidence (
  evidence_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  run_uuid uuid NOT NULL,
  gate_name text NOT NULL CHECK (length(gate_name) BETWEEN 1 AND 120),
  status text NOT NULL CHECK (status IN ('verified','failed','unavailable')),
  source_state_sha256 bytea NOT NULL CHECK (octet_length(source_state_sha256) = 32),
  evidence_sha256 bytea CHECK (evidence_sha256 IS NULL OR octet_length(evidence_sha256) = 32),
  tool_name text NOT NULL CHECK (length(tool_name) BETWEEN 1 AND 160),
  tool_version text,
  evidence_ref text,
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
  verified_at timestamptz NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT fk_release_evidence_tenant_run FOREIGN KEY (tenant_uuid, run_uuid)
    REFERENCES phxclaw.release_gate_runs(tenant_uuid, run_uuid) ON DELETE CASCADE,
  CHECK ((status = 'unavailable') OR evidence_sha256 IS NOT NULL),
  UNIQUE (tenant_uuid, run_uuid, gate_name)
);

CREATE TABLE IF NOT EXISTS phxclaw.release_attestations (
  attestation_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  run_uuid uuid NOT NULL,
  source_ready boolean NOT NULL,
  static_verified boolean NOT NULL,
  runtime_verified boolean NOT NULL,
  e2e_verified boolean NOT NULL,
  release_ready boolean NOT NULL,
  attestation_sha256 bytea NOT NULL CHECK (octet_length(attestation_sha256) = 32),
  signer_key_id text,
  signature bytea,
  created_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT fk_release_attestation_tenant_run FOREIGN KEY (tenant_uuid, run_uuid)
    REFERENCES phxclaw.release_gate_runs(tenant_uuid, run_uuid) ON DELETE RESTRICT,
  UNIQUE (tenant_uuid, run_uuid, attestation_sha256),
  CHECK (NOT release_ready OR (source_ready AND static_verified AND runtime_verified AND e2e_verified)),
  CHECK ((signature IS NULL AND signer_key_id IS NULL) OR (signature IS NOT NULL AND signer_key_id IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS release_gate_runs_idx ON phxclaw.release_gate_runs (tenant_uuid, release_uuid, started_at DESC);
CREATE INDEX IF NOT EXISTS release_gate_evidence_idx ON phxclaw.release_gate_evidence (tenant_uuid, run_uuid, gate_name);

ALTER TABLE phxclaw.release_gate_runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.release_gate_evidence ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.release_attestations ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.release_gate_runs FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.release_gate_evidence FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.release_attestations FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.release_gate_runs;
CREATE POLICY tenant_isolation ON phxclaw.release_gate_runs
  USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.release_gate_evidence;
CREATE POLICY tenant_isolation ON phxclaw.release_gate_evidence
  USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.release_attestations;
CREATE POLICY tenant_isolation ON phxclaw.release_attestations
  USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);

CREATE OR REPLACE FUNCTION phxclaw.reject_release_evidence_mutation()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'release evidence and attestations are append-only';
  RETURN NULL;
END;
$$;
DROP TRIGGER IF EXISTS trg_release_evidence_immutable ON phxclaw.release_gate_evidence;
CREATE TRIGGER trg_release_evidence_immutable BEFORE UPDATE OR DELETE ON phxclaw.release_gate_evidence
FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_release_evidence_mutation();
DROP TRIGGER IF EXISTS trg_release_attestations_immutable ON phxclaw.release_attestations;
CREATE TRIGGER trg_release_attestations_immutable BEFORE UPDATE OR DELETE ON phxclaw.release_attestations
FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_release_evidence_mutation();

-- Complete F25 append-only coverage for snapshot/event evidence.
DROP TRIGGER IF EXISTS trg_knowledge_snapshots_immutable ON phxclaw.knowledge_snapshots;
CREATE TRIGGER trg_knowledge_snapshots_immutable BEFORE UPDATE OR DELETE ON phxclaw.knowledge_snapshots
FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_knowledge_mutation();
DROP TRIGGER IF EXISTS trg_knowledge_graph_events_immutable ON phxclaw.knowledge_graph_events;
CREATE TRIGGER trg_knowledge_graph_events_immutable BEFORE UPDATE OR DELETE ON phxclaw.knowledge_graph_events
FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_knowledge_mutation();

-- F24 evaluation evidence and event journal are immutable facts.
DROP TRIGGER IF EXISTS trg_skill_candidate_evidence_immutable ON phxclaw.skill_candidate_evidence;
CREATE TRIGGER trg_skill_candidate_evidence_immutable BEFORE UPDATE OR DELETE ON phxclaw.skill_candidate_evidence
FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_release_evidence_mutation();
DROP TRIGGER IF EXISTS trg_skill_evolution_events_immutable ON phxclaw.skill_evolution_events;
CREATE TRIGGER trg_skill_evolution_events_immutable BEFORE UPDATE OR DELETE ON phxclaw.skill_evolution_events
FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_release_evidence_mutation();

-- F22 replay reservations and command event audit are immutable facts.
DROP TRIGGER IF EXISTS trg_device_replay_immutable ON phxclaw.device_replay_reservations;
CREATE TRIGGER trg_device_replay_immutable BEFORE UPDATE OR DELETE ON phxclaw.device_replay_reservations
FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_release_evidence_mutation();
DROP TRIGGER IF EXISTS trg_device_command_events_immutable ON phxclaw.device_command_events;
CREATE TRIGGER trg_device_command_events_immutable BEFORE UPDATE OR DELETE ON phxclaw.device_command_events
FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_release_evidence_mutation();

COMMIT;

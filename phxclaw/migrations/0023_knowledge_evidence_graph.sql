-- PhxClaw v0.23 / F25 Knowledge / Evidence Graph
-- migration_uuid: 01a0e800-cf88-7d25-9a40-bf9178b2f0c8
-- PostgreSQL is authoritative. Raw sources are immutable; interpretations are versioned/superseded.

BEGIN;
CREATE SCHEMA IF NOT EXISTS phxclaw;

CREATE TABLE IF NOT EXISTS phxclaw.knowledge_nodes (
  node_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  kind text NOT NULL CHECK (kind IN (
    'raw_source','artifact','claim','evidence','hypothesis','decision','requirement','task',
    'agent','skill','release','test','policy','constraint','external_fact'
  )),
  epistemic_state text NOT NULL CHECK (epistemic_state IN (
    'raw_observation','unverified','accepted','governed','rejected','quarantined'
  )),
  content_sha256 bytea NOT NULL CHECK (octet_length(content_sha256) = 32),
  source_state_sha256 bytea NOT NULL CHECK (octet_length(source_state_sha256) = 32),
  subject_key text,
  predicate_key text,
  value_sha256 bytea CHECK (value_sha256 IS NULL OR octet_length(value_sha256) = 32),
  confidence_ppm integer NOT NULL DEFAULT 0 CHECK (confidence_ppm BETWEEN 0 AND 1000000),
  provenance jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_by_uuid uuid,
  created_at timestamptz NOT NULL DEFAULT now(),
  -- Reparo native-v070: o motor promove/rejeita clonando o no e trocando so o estado
  -- (promoted_claim_version); sem epistemic_state na chave, a versao que ele gera era recusada.
  UNIQUE (tenant_uuid, kind, content_sha256, source_state_sha256, epistemic_state)
);

CREATE TABLE IF NOT EXISTS phxclaw.knowledge_edges (
  edge_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  from_node_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_nodes(node_uuid),
  to_node_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_nodes(node_uuid),
  kind text NOT NULL CHECK (kind IN (
    'supports','refutes','derived_from','produced_by','validates','tests','depends_on','implements',
    'supersedes','contradicts','approved_by','promoted_from','caused_by','relates_to'
  )),
  evidence_sha256 bytea CHECK (evidence_sha256 IS NULL OR octet_length(evidence_sha256) = 32),
  attributes jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_at timestamptz NOT NULL DEFAULT now(),
  CHECK (from_node_uuid <> to_node_uuid),
  UNIQUE (tenant_uuid, from_node_uuid, to_node_uuid, kind, evidence_sha256)
);

CREATE TABLE IF NOT EXISTS phxclaw.knowledge_evidence_bindings (
  binding_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  claim_node_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_nodes(node_uuid),
  evidence_node_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_nodes(node_uuid),
  relation text NOT NULL CHECK (relation IN ('supports','refutes')),
  evidence_sha256 bytea NOT NULL CHECK (octet_length(evidence_sha256) = 32),
  source_state_sha256 bytea NOT NULL CHECK (octet_length(source_state_sha256) = 32),
  mechanism text NOT NULL CHECK (length(mechanism) BETWEEN 1 AND 160),
  collected_at timestamptz NOT NULL,
  valid_until timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  CHECK (valid_until IS NULL OR valid_until > collected_at),
  UNIQUE (tenant_uuid, claim_node_uuid, evidence_node_uuid, relation, source_state_sha256)
);

CREATE TABLE IF NOT EXISTS phxclaw.knowledge_contradictions (
  contradiction_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  left_claim_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_nodes(node_uuid),
  right_claim_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_nodes(node_uuid),
  subject_key text NOT NULL,
  predicate_key text NOT NULL,
  detected_at timestamptz NOT NULL DEFAULT now(),
  CHECK (left_claim_uuid <> right_claim_uuid),
  UNIQUE (tenant_uuid, left_claim_uuid, right_claim_uuid)
);

CREATE TABLE IF NOT EXISTS phxclaw.knowledge_contradiction_resolutions (
  resolution_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  contradiction_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_contradictions(contradiction_uuid),
  resolution_node_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_nodes(node_uuid),
  resolved_by_uuid uuid NOT NULL,
  resolution_sha256 bytea NOT NULL CHECK (octet_length(resolution_sha256) = 32),
  resolution_notes text,
  resolved_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_uuid, contradiction_uuid)
);

CREATE TABLE IF NOT EXISTS phxclaw.knowledge_snapshots (
  snapshot_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  node_count bigint NOT NULL CHECK (node_count >= 0),
  edge_count bigint NOT NULL CHECK (edge_count >= 0),
  root_sha256 bytea NOT NULL CHECK (octet_length(root_sha256) = 32),
  source_cursor jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_uuid, root_sha256)
);

CREATE TABLE IF NOT EXISTS phxclaw.knowledge_graph_events (
  event_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  event_type text NOT NULL CHECK (length(event_type) BETWEEN 1 AND 120),
  actor_uuid uuid,
  correlation_uuid uuid,
  causation_uuid uuid,
  subject_uuid uuid,
  payload jsonb NOT NULL DEFAULT '{}'::jsonb,
  payload_sha256 bytea NOT NULL CHECK (octet_length(payload_sha256) = 32),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS knowledge_nodes_subject_idx
  ON phxclaw.knowledge_nodes (tenant_uuid, subject_key, predicate_key)
  WHERE subject_key IS NOT NULL AND predicate_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS knowledge_edges_from_idx
  ON phxclaw.knowledge_edges (tenant_uuid, from_node_uuid, kind);
CREATE INDEX IF NOT EXISTS knowledge_edges_to_idx
  ON phxclaw.knowledge_edges (tenant_uuid, to_node_uuid, kind);
CREATE INDEX IF NOT EXISTS knowledge_bindings_claim_idx
  ON phxclaw.knowledge_evidence_bindings (tenant_uuid, claim_node_uuid, relation);
CREATE INDEX IF NOT EXISTS knowledge_contradictions_idx
  ON phxclaw.knowledge_contradictions (tenant_uuid, detected_at);
CREATE INDEX IF NOT EXISTS knowledge_resolution_idx
  ON phxclaw.knowledge_contradiction_resolutions (tenant_uuid, contradiction_uuid, resolved_at);

ALTER TABLE phxclaw.knowledge_nodes ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.knowledge_edges ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.knowledge_evidence_bindings ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.knowledge_contradictions ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.knowledge_contradiction_resolutions ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.knowledge_snapshots ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.knowledge_graph_events ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation ON phxclaw.knowledge_nodes;
CREATE POLICY tenant_isolation ON phxclaw.knowledge_nodes
  USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.knowledge_edges;
CREATE POLICY tenant_isolation ON phxclaw.knowledge_edges
  USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.knowledge_evidence_bindings;
CREATE POLICY tenant_isolation ON phxclaw.knowledge_evidence_bindings
  USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.knowledge_contradictions;
CREATE POLICY tenant_isolation ON phxclaw.knowledge_contradictions
  USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.knowledge_contradiction_resolutions;
CREATE POLICY tenant_isolation ON phxclaw.knowledge_contradiction_resolutions
  USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.knowledge_snapshots;
CREATE POLICY tenant_isolation ON phxclaw.knowledge_snapshots
  USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.knowledge_graph_events;
CREATE POLICY tenant_isolation ON phxclaw.knowledge_graph_events
  USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);

-- Guardrails: graph facts are append-only. New interpretations use new nodes + supersedes edges.
CREATE OR REPLACE FUNCTION phxclaw.reject_knowledge_mutation()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'knowledge graph records are append-only; create a new version/edge instead';
  RETURN NULL;
END;
$$;

DROP TRIGGER IF EXISTS trg_knowledge_nodes_immutable ON phxclaw.knowledge_nodes;
CREATE TRIGGER trg_knowledge_nodes_immutable
BEFORE UPDATE OR DELETE ON phxclaw.knowledge_nodes
FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_knowledge_mutation();

DROP TRIGGER IF EXISTS trg_knowledge_edges_immutable ON phxclaw.knowledge_edges;
CREATE TRIGGER trg_knowledge_edges_immutable
BEFORE UPDATE OR DELETE ON phxclaw.knowledge_edges
FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_knowledge_mutation();

DROP TRIGGER IF EXISTS trg_knowledge_bindings_immutable ON phxclaw.knowledge_evidence_bindings;
CREATE TRIGGER trg_knowledge_bindings_immutable
BEFORE UPDATE OR DELETE ON phxclaw.knowledge_evidence_bindings
FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_knowledge_mutation();

DROP TRIGGER IF EXISTS trg_knowledge_contradictions_immutable ON phxclaw.knowledge_contradictions;
CREATE TRIGGER trg_knowledge_contradictions_immutable
BEFORE UPDATE OR DELETE ON phxclaw.knowledge_contradictions
FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_knowledge_mutation();

DROP TRIGGER IF EXISTS trg_knowledge_resolutions_immutable ON phxclaw.knowledge_contradiction_resolutions;
CREATE TRIGGER trg_knowledge_resolutions_immutable
BEFORE UPDATE OR DELETE ON phxclaw.knowledge_contradiction_resolutions
FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_knowledge_mutation();

COMMIT;

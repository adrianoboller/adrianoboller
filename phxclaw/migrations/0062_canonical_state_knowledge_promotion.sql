-- PhxClaw v0.62 — Canonical Project State + Knowledge Promotion Gate
-- Append-only project-state snapshots; evidence-bound knowledge promotion; human-gated governance/revocation.

CREATE SCHEMA IF NOT EXISTS phxclaw;

CREATE OR REPLACE FUNCTION phxclaw.current_tenant_uuid()
RETURNS uuid LANGUAGE plpgsql STABLE AS $fn$
DECLARE v text;
BEGIN
  v := nullif(current_setting('phxclaw.tenant_uuid', true), '');
  IF v IS NULL THEN v := nullif(current_setting('phxclaw.tenant_id', true), ''); END IF;
  IF v IS NULL THEN RETURN NULL; END IF;
  RETURN v::uuid;
END
$fn$;

CREATE TABLE IF NOT EXISTS phxclaw.project_state_snapshots (
  snapshot_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  project_key text NOT NULL,
  project_version text NOT NULL,
  state_sha256 bytea NOT NULL CHECK (octet_length(state_sha256)=32),
  state_document jsonb NOT NULL,
  supersedes_snapshot_uuid uuid NULL REFERENCES phxclaw.project_state_snapshots(snapshot_uuid) ON DELETE RESTRICT,
  actor text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(tenant_uuid,project_key,state_sha256)
);
CREATE INDEX IF NOT EXISTS project_state_current_idx ON phxclaw.project_state_snapshots(tenant_uuid,project_key,created_at DESC,snapshot_uuid DESC);
CREATE OR REPLACE FUNCTION phxclaw.project_current_state_for(p_project_key text)
RETURNS TABLE(snapshot_uuid uuid,tenant_uuid uuid,project_key text,project_version text,state_sha256 bytea,state_document jsonb,actor text,created_at timestamptz)
LANGUAGE sql STABLE SECURITY INVOKER AS $fn$
  SELECT s.snapshot_uuid,s.tenant_uuid,s.project_key,s.project_version,s.state_sha256,s.state_document,s.actor,s.created_at
  FROM phxclaw.project_state_snapshots s
  WHERE s.tenant_uuid=phxclaw.current_tenant_uuid() AND s.project_key=p_project_key
  ORDER BY s.created_at DESC,s.snapshot_uuid DESC LIMIT 1
$fn$;

CREATE TABLE IF NOT EXISTS phxclaw.knowledge_promotion_requests (
  request_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  candidate_uuid uuid NULL REFERENCES phx_source_knowledge_candidate(candidate_uuid) ON DELETE RESTRICT,
  claim_node_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_nodes(node_uuid) ON DELETE RESTRICT,
  target_state text NOT NULL CHECK (target_state IN ('accepted','governed')),
  expected_source_state_sha256 bytea NOT NULL CHECK (octet_length(expected_source_state_sha256)=32),
  min_supporting_evidence integer NOT NULL DEFAULT 2 CHECK(min_supporting_evidence>=1),
  min_independent_mechanisms integer NOT NULL DEFAULT 2 CHECK(min_independent_mechanisms>=1),
  max_evidence_age_seconds bigint NOT NULL DEFAULT 2592000 CHECK(max_evidence_age_seconds>=0),
  requested_by text NOT NULL,
  requested_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS phxclaw.knowledge_promotion_evidence (
  request_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_promotion_requests(request_uuid) ON DELETE RESTRICT,
  evidence_uuid uuid NOT NULL,
  relation text NOT NULL CHECK(relation IN ('supports','refutes')),
  evidence_sha256 bytea NOT NULL CHECK(octet_length(evidence_sha256)=32),
  source_state_sha256 bytea NOT NULL CHECK(octet_length(source_state_sha256)=32),
  mechanism text NOT NULL CHECK(length(mechanism) BETWEEN 1 AND 160),
  collected_at timestamptz NOT NULL,
  valid_until timestamptz NULL,
  PRIMARY KEY(request_uuid,evidence_uuid),
  CHECK(valid_until IS NULL OR valid_until>collected_at)
);
CREATE TABLE IF NOT EXISTS phxclaw.knowledge_promotion_reviews (
  review_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  request_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_promotion_requests(request_uuid) ON DELETE RESTRICT,
  authority text NOT NULL CHECK(authority IN ('system','human')),
  reviewer text NOT NULL,
  decision text NOT NULL CHECK(decision IN ('approved','rejected')),
  decision_sha256 bytea NOT NULL CHECK(octet_length(decision_sha256)=32),
  notes text NULL,
  reviewed_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(request_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw.knowledge_promotion_receipts (
  promotion_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  request_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_promotion_requests(request_uuid) ON DELETE RESTRICT,
  previous_node_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_nodes(node_uuid) ON DELETE RESTRICT,
  promoted_node_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_nodes(node_uuid) ON DELETE RESTRICT,
  target_state text NOT NULL CHECK(target_state IN ('accepted','governed')),
  review_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_promotion_reviews(review_uuid) ON DELETE RESTRICT,
  promoted_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(request_uuid), UNIQUE(promoted_node_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw.knowledge_revocations (
  revocation_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  promotion_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_promotion_receipts(promotion_uuid) ON DELETE RESTRICT,
  rejected_node_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_nodes(node_uuid) ON DELETE RESTRICT,
  reviewer text NOT NULL,
  reason_sha256 bytea NOT NULL CHECK(octet_length(reason_sha256)=32),
  revoked_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(promotion_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw.knowledge_promotion_events (
  event_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  request_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_promotion_requests(request_uuid) ON DELETE RESTRICT,
  event_type text NOT NULL CHECK(event_type IN ('requested','assessed','approved','rejected','promoted','revoked')),
  actor text NOT NULL,
  payload jsonb NOT NULL DEFAULT '{}'::jsonb,
  payload_sha256 bytea NOT NULL CHECK(octet_length(payload_sha256)=32),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE OR REPLACE FUNCTION phxclaw.reject_v062_mutation()
RETURNS trigger LANGUAGE plpgsql AS $fn$ BEGIN RAISE EXCEPTION 'v0.62 audit/state rows are append-only'; END $fn$;
DO $do$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['project_state_snapshots','knowledge_promotion_requests','knowledge_promotion_evidence','knowledge_promotion_reviews','knowledge_promotion_receipts','knowledge_revocations','knowledge_promotion_events'] LOOP
    EXECUTE format('DROP TRIGGER IF EXISTS %I ON phxclaw.%I','trg_'||t||'_immutable',t);
    EXECUTE format('CREATE TRIGGER %I BEFORE UPDATE OR DELETE ON phxclaw.%I FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_v062_mutation()','trg_'||t||'_immutable',t);
  END LOOP;
END $do$;

CREATE OR REPLACE FUNCTION phxclaw.validate_knowledge_promotion_review()
RETURNS trigger LANGUAGE plpgsql AS $fn$
DECLARE r phxclaw.knowledge_promotion_requests%ROWTYPE; candidate_ok boolean; support_count integer; mechanism_count integer; refute_count integer; stale_count integer; mismatch_count integer; unresolved_count integer;
BEGIN
  SELECT * INTO r FROM phxclaw.knowledge_promotion_requests WHERE request_uuid=NEW.request_uuid;
  IF NOT FOUND OR r.tenant_uuid<>NEW.tenant_uuid THEN RAISE EXCEPTION 'promotion request/tenant mismatch'; END IF;
  IF NEW.decision='approved' THEN
    IF r.target_state='governed' AND NEW.authority<>'human' THEN RAISE EXCEPTION 'governed knowledge requires human review'; END IF;
    IF r.candidate_uuid IS NOT NULL THEN
      SELECT (h.final_decision='ALLOW') INTO candidate_ok FROM phx_source_knowledge_candidate c JOIN phx_source_harvest_run h ON h.run_uuid=c.run_uuid WHERE c.candidate_uuid=r.candidate_uuid AND c.tenant_uuid=r.tenant_uuid;
      IF candidate_ok IS DISTINCT FROM true THEN RAISE EXCEPTION 'candidate is not backed by ALLOW harvest'; END IF;
    END IF;
    SELECT count(*) FILTER(WHERE relation='supports'),count(DISTINCT mechanism) FILTER(WHERE relation='supports'),count(*) FILTER(WHERE relation='refutes'),
      count(*) FILTER(WHERE collected_at>clock_timestamp() OR clock_timestamp()-collected_at>make_interval(secs=>r.max_evidence_age_seconds) OR (valid_until IS NOT NULL AND clock_timestamp()>=valid_until)),
      count(*) FILTER(WHERE source_state_sha256<>r.expected_source_state_sha256)
    INTO support_count,mechanism_count,refute_count,stale_count,mismatch_count FROM phxclaw.knowledge_promotion_evidence WHERE request_uuid=r.request_uuid;
    IF support_count<r.min_supporting_evidence THEN RAISE EXCEPTION 'insufficient supporting evidence'; END IF;
    IF mechanism_count<r.min_independent_mechanisms THEN RAISE EXCEPTION 'insufficient independent mechanisms'; END IF;
    IF refute_count>0 THEN RAISE EXCEPTION 'active refuting evidence blocks promotion'; END IF;
    IF stale_count>0 THEN RAISE EXCEPTION 'stale/expired/future evidence blocks promotion'; END IF;
    IF mismatch_count>0 THEN RAISE EXCEPTION 'source-state mismatch blocks promotion'; END IF;
    SELECT count(*) INTO unresolved_count FROM phxclaw.knowledge_contradictions c LEFT JOIN phxclaw.knowledge_contradiction_resolutions x ON x.contradiction_uuid=c.contradiction_uuid AND x.tenant_uuid=c.tenant_uuid WHERE c.tenant_uuid=r.tenant_uuid AND (c.left_claim_uuid=r.claim_node_uuid OR c.right_claim_uuid=r.claim_node_uuid) AND x.resolution_uuid IS NULL;
    IF unresolved_count>0 THEN RAISE EXCEPTION 'unresolved contradiction blocks promotion'; END IF;
  END IF;
  RETURN NEW;
END $fn$;
DROP TRIGGER IF EXISTS trg_validate_knowledge_promotion_review ON phxclaw.knowledge_promotion_reviews;
CREATE TRIGGER trg_validate_knowledge_promotion_review BEFORE INSERT ON phxclaw.knowledge_promotion_reviews FOR EACH ROW EXECUTE FUNCTION phxclaw.validate_knowledge_promotion_review();

CREATE OR REPLACE FUNCTION phxclaw.validate_knowledge_promotion_receipt()
RETURNS trigger LANGUAGE plpgsql AS $fn$
DECLARE r phxclaw.knowledge_promotion_requests%ROWTYPE; v phxclaw.knowledge_promotion_reviews%ROWTYPE; p phxclaw.knowledge_nodes%ROWTYPE;
BEGIN
  SELECT * INTO r FROM phxclaw.knowledge_promotion_requests WHERE request_uuid=NEW.request_uuid;
  SELECT * INTO v FROM phxclaw.knowledge_promotion_reviews WHERE review_uuid=NEW.review_uuid AND request_uuid=NEW.request_uuid;
  SELECT * INTO p FROM phxclaw.knowledge_nodes WHERE node_uuid=NEW.promoted_node_uuid;
  IF v.decision IS DISTINCT FROM 'approved' THEN RAISE EXCEPTION 'promotion receipt requires approved review'; END IF;
  IF r.claim_node_uuid<>NEW.previous_node_uuid OR r.target_state<>NEW.target_state OR r.tenant_uuid<>NEW.tenant_uuid THEN RAISE EXCEPTION 'promotion receipt/request mismatch'; END IF;
  IF p.tenant_uuid<>NEW.tenant_uuid OR p.epistemic_state<>NEW.target_state THEN RAISE EXCEPTION 'promoted knowledge node mismatch'; END IF;
  RETURN NEW;
END $fn$;
DROP TRIGGER IF EXISTS trg_validate_knowledge_promotion_receipt ON phxclaw.knowledge_promotion_receipts;
CREATE TRIGGER trg_validate_knowledge_promotion_receipt BEFORE INSERT ON phxclaw.knowledge_promotion_receipts FOR EACH ROW EXECUTE FUNCTION phxclaw.validate_knowledge_promotion_receipt();

-- Harden the v0.61 bridge table: a promotion row now requires a v0.62 gated receipt.
CREATE OR REPLACE FUNCTION phx_source_promotion_requires_v062_gate()
RETURNS trigger LANGUAGE plpgsql AS $fn$
DECLARE ok boolean;
BEGIN
  SELECT true INTO ok FROM phxclaw.knowledge_promotion_receipts p JOIN phxclaw.knowledge_promotion_requests r ON r.request_uuid=p.request_uuid JOIN phxclaw.knowledge_promotion_reviews v ON v.review_uuid=p.review_uuid WHERE r.candidate_uuid=NEW.candidate_uuid AND p.promoted_node_uuid=NEW.knowledge_node_uuid AND p.tenant_uuid=NEW.tenant_uuid AND v.decision='approved';
  IF ok IS DISTINCT FROM true THEN RAISE EXCEPTION 'candidate promotion requires v0.62 Knowledge Promotion Gate receipt'; END IF;
  RETURN NEW;
END $fn$;
DROP TRIGGER IF EXISTS trg_phx_source_promotion_requires_v062_gate ON phx_source_candidate_promotion;
CREATE TRIGGER trg_phx_source_promotion_requires_v062_gate BEFORE INSERT ON phx_source_candidate_promotion FOR EACH ROW EXECUTE FUNCTION phx_source_promotion_requires_v062_gate();

CREATE INDEX IF NOT EXISTS knowledge_promotion_request_tenant_idx ON phxclaw.knowledge_promotion_requests(tenant_uuid,requested_at DESC);
CREATE INDEX IF NOT EXISTS knowledge_promotion_event_tenant_idx ON phxclaw.knowledge_promotion_events(tenant_uuid,created_at DESC);

-- Normalize and harden the original F25 graph RLS using the compatibility tenant accessor.
DO $kg_rls$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['knowledge_nodes','knowledge_edges','knowledge_evidence_bindings','knowledge_contradictions','knowledge_contradiction_resolutions','knowledge_snapshots','knowledge_graph_events'] LOOP
    EXECUTE format('ALTER TABLE phxclaw.%I ENABLE ROW LEVEL SECURITY',t);
    EXECUTE format('ALTER TABLE phxclaw.%I FORCE ROW LEVEL SECURITY',t);
    EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON phxclaw.%I',t);
    EXECUTE format('CREATE POLICY tenant_isolation ON phxclaw.%I USING (tenant_uuid=phxclaw.current_tenant_uuid()) WITH CHECK (tenant_uuid=phxclaw.current_tenant_uuid())',t);
  END LOOP;
END $kg_rls$;

DO $rls$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['project_state_snapshots','knowledge_promotion_requests','knowledge_promotion_evidence','knowledge_promotion_reviews','knowledge_promotion_receipts','knowledge_revocations','knowledge_promotion_events'] LOOP
    EXECUTE format('ALTER TABLE phxclaw.%I ENABLE ROW LEVEL SECURITY',t);
    EXECUTE format('ALTER TABLE phxclaw.%I FORCE ROW LEVEL SECURITY',t);
    EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_v062 ON phxclaw.%I',t);
    IF t='knowledge_promotion_evidence' THEN
      EXECUTE 'CREATE POLICY tenant_isolation_v062 ON phxclaw.knowledge_promotion_evidence USING (EXISTS (SELECT 1 FROM phxclaw.knowledge_promotion_requests r WHERE r.request_uuid=knowledge_promotion_evidence.request_uuid AND r.tenant_uuid=phxclaw.current_tenant_uuid())) WITH CHECK (EXISTS (SELECT 1 FROM phxclaw.knowledge_promotion_requests r WHERE r.request_uuid=knowledge_promotion_evidence.request_uuid AND r.tenant_uuid=phxclaw.current_tenant_uuid()))';
    ELSE
      EXECUTE format('CREATE POLICY tenant_isolation_v062 ON phxclaw.%I USING (tenant_uuid=phxclaw.current_tenant_uuid()) WITH CHECK (tenant_uuid=phxclaw.current_tenant_uuid())',t);
    END IF;
  END LOOP;
END $rls$;

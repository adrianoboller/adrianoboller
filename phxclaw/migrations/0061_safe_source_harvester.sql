-- PhxClaw v0.61 — Safe Source Harvester
-- Fail-closed ingestion receipts, immutable file evidence and ALLOW-only knowledge candidates.

CREATE TABLE IF NOT EXISTS phx_source_harvest_run (
    run_uuid UUID PRIMARY KEY,
    tenant_uuid UUID NOT NULL,
    artifact_uuid UUID NOT NULL REFERENCES phx_source_artifact(artifact_uuid) ON DELETE RESTRICT,
    actor TEXT NOT NULL,
    provenance_decision TEXT NOT NULL CHECK (provenance_decision IN ('ALLOW','QUARANTINE','DENY')),
    final_decision TEXT NOT NULL CHECK (final_decision IN ('ALLOW','QUARANTINE','DENY')),
    provenance_reasons JSONB NOT NULL DEFAULT '[]'::jsonb,
    security_reasons JSONB NOT NULL DEFAULT '[]'::jsonb,
    files_seen BIGINT NOT NULL DEFAULT 0 CHECK (files_seen >= 0),
    bytes_seen BIGINT NOT NULL DEFAULT 0 CHECK (bytes_seen >= 0),
    candidate_count BIGINT NOT NULL DEFAULT 0 CHECK (candidate_count >= 0),
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ NOT NULL,
    CHECK (completed_at >= started_at),
    CHECK (final_decision = 'ALLOW' OR candidate_count = 0)
);

CREATE TABLE IF NOT EXISTS phx_source_harvest_file (
    run_uuid UUID NOT NULL REFERENCES phx_source_harvest_run(run_uuid) ON DELETE RESTRICT,
    relative_path TEXT NOT NULL,
    byte_len BIGINT NOT NULL CHECK (byte_len >= 0),
    sha256 CHAR(64) NULL,
    state TEXT NOT NULL CHECK (state IN ('accepted','quarantined_secret','quarantined_oversize','skipped_binary','skipped_extension','skipped_symlink')),
    content_address_uri TEXT NULL,
    detected_markers JSONB NOT NULL DEFAULT '[]'::jsonb,
    PRIMARY KEY (run_uuid, relative_path),
    CHECK (sha256 IS NULL OR sha256 ~ '^[a-f0-9]{64}$')
);

CREATE TABLE IF NOT EXISTS phx_source_knowledge_candidate (
    candidate_uuid UUID PRIMARY KEY,
    tenant_uuid UUID NOT NULL,
    run_uuid UUID NOT NULL REFERENCES phx_source_harvest_run(run_uuid) ON DELETE RESTRICT,
    artifact_uuid UUID NOT NULL REFERENCES phx_source_artifact(artifact_uuid) ON DELETE RESTRICT,
    source_path TEXT NOT NULL,
    content_sha256 CHAR(64) NOT NULL CHECK (content_sha256 ~ '^[a-f0-9]{64}$'),
    source_state_sha256 CHAR(64) NOT NULL CHECK (source_state_sha256 ~ '^[a-f0-9]{64}$'),
    mechanism TEXT NOT NULL,
    excerpt TEXT NOT NULL,
    epistemic_state TEXT NOT NULL DEFAULT 'raw_observation' CHECK (epistemic_state IN ('raw_observation','unverified')),
    collected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_uuid, artifact_uuid, source_path, content_sha256)
);

CREATE TABLE IF NOT EXISTS phx_source_candidate_promotion (
    promotion_uuid UUID PRIMARY KEY,
    tenant_uuid UUID NOT NULL,
    candidate_uuid UUID NOT NULL REFERENCES phx_source_knowledge_candidate(candidate_uuid) ON DELETE RESTRICT,
    knowledge_node_uuid UUID NOT NULL,
    authority TEXT NOT NULL CHECK (authority IN ('system','human')),
    evidence_sha256 CHAR(64) NOT NULL CHECK (evidence_sha256 ~ '^[a-f0-9]{64}$'),
    promoted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (candidate_uuid, knowledge_node_uuid)
);

CREATE OR REPLACE FUNCTION phx_source_candidate_must_be_allowed()
RETURNS trigger LANGUAGE plpgsql AS $fn$
DECLARE d TEXT;
BEGIN
    SELECT final_decision INTO d FROM phx_source_harvest_run WHERE run_uuid = NEW.run_uuid;
    IF d IS DISTINCT FROM 'ALLOW' THEN
        RAISE EXCEPTION 'knowledge candidate requires ALLOW harvest run: %', NEW.run_uuid;
    END IF;
    RETURN NEW;
END
$fn$;

DROP TRIGGER IF EXISTS trg_phx_source_candidate_must_be_allowed ON phx_source_knowledge_candidate;
CREATE TRIGGER trg_phx_source_candidate_must_be_allowed
BEFORE INSERT ON phx_source_knowledge_candidate
FOR EACH ROW EXECUTE FUNCTION phx_source_candidate_must_be_allowed();

CREATE OR REPLACE FUNCTION phx_source_harvest_immutable()
RETURNS trigger LANGUAGE plpgsql AS $fn$
BEGIN
    RAISE EXCEPTION 'harvest evidence is immutable; append a new run instead';
END
$fn$;

DROP TRIGGER IF EXISTS trg_phx_source_harvest_run_immutable ON phx_source_harvest_run;
CREATE TRIGGER trg_phx_source_harvest_run_immutable
BEFORE UPDATE OR DELETE ON phx_source_harvest_run
FOR EACH ROW EXECUTE FUNCTION phx_source_harvest_immutable();

DROP TRIGGER IF EXISTS trg_phx_source_harvest_file_immutable ON phx_source_harvest_file;
CREATE TRIGGER trg_phx_source_harvest_file_immutable
BEFORE UPDATE OR DELETE ON phx_source_harvest_file
FOR EACH ROW EXECUTE FUNCTION phx_source_harvest_immutable();

CREATE INDEX IF NOT EXISTS idx_phx_source_harvest_run_tenant_time ON phx_source_harvest_run(tenant_uuid, completed_at DESC);
CREATE INDEX IF NOT EXISTS idx_phx_source_harvest_run_decision ON phx_source_harvest_run(final_decision);
CREATE INDEX IF NOT EXISTS idx_phx_source_harvest_file_sha ON phx_source_harvest_file(sha256) WHERE sha256 IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_phx_source_knowledge_candidate_tenant ON phx_source_knowledge_candidate(tenant_uuid, collected_at DESC);

ALTER TABLE phx_source_harvest_run ENABLE ROW LEVEL SECURITY;
ALTER TABLE phx_source_harvest_run FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_source_knowledge_candidate ENABLE ROW LEVEL SECURITY;
ALTER TABLE phx_source_knowledge_candidate FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_source_candidate_promotion ENABLE ROW LEVEL SECURITY;
ALTER TABLE phx_source_candidate_promotion FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS phx_source_harvest_run_tenant ON phx_source_harvest_run;
CREATE POLICY phx_source_harvest_run_tenant ON phx_source_harvest_run
USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);

DROP POLICY IF EXISTS phx_source_knowledge_candidate_tenant ON phx_source_knowledge_candidate;
CREATE POLICY phx_source_knowledge_candidate_tenant ON phx_source_knowledge_candidate
USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);

DROP POLICY IF EXISTS phx_source_candidate_promotion_tenant ON phx_source_candidate_promotion;
CREATE POLICY phx_source_candidate_promotion_tenant ON phx_source_candidate_promotion
USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);

-- PhxClaw v0.60 — Source Provenance & License Firewall
-- Overlay migration. Assign the next canonical numeric migration id when merging into the cumulative tree.

CREATE TABLE IF NOT EXISTS phx_source_artifact (
    artifact_uuid UUID PRIMARY KEY,
    source_name TEXT NOT NULL,
    sha256 CHAR(64) NOT NULL UNIQUE,
    origin_kind TEXT NOT NULL CHECK (origin_kind IN ('user_supplied','upstream_public','first_party','unknown')),
    origin_url TEXT NULL,
    origin_known BOOLEAN NOT NULL DEFAULT FALSE,
    license_class TEXT NOT NULL CHECK (license_class IN ('permissive','copyleft','proprietary','unknown')),
    license_spdx TEXT NULL,
    local_license_evidence BOOLEAN NOT NULL DEFAULT FALSE,
    license_evidence_sha256 CHAR(64) NULL,
    declared_leak BOOLEAN NOT NULL DEFAULT FALSE,
    redistribution_prohibited BOOLEAN NOT NULL DEFAULT FALSE,
    decision TEXT NOT NULL CHECK (decision IN ('ALLOW','QUARANTINE','DENY')),
    reasons JSONB NOT NULL DEFAULT '[]'::jsonb,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS phx_source_evidence (
    evidence_uuid UUID PRIMARY KEY,
    artifact_uuid UUID NOT NULL REFERENCES phx_source_artifact(artifact_uuid) ON DELETE CASCADE,
    evidence_kind TEXT NOT NULL,
    locator TEXT NULL,
    evidence_sha256 CHAR(64) NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_phx_source_artifact_decision ON phx_source_artifact(decision);
CREATE INDEX IF NOT EXISTS idx_phx_source_evidence_artifact ON phx_source_evidence(artifact_uuid);

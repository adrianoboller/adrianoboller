BEGIN;

CREATE TABLE IF NOT EXISTS phoenix_agent_catalog (
    uuid uuid PRIMARY KEY,
    agent_id integer NOT NULL UNIQUE,
    name text NOT NULL UNIQUE,
    manifest jsonb NOT NULL,
    source_workbook text NOT NULL,
    source_sheet text NOT NULL,
    source_row integer NOT NULL,
    source_sha256 text,
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS phoenix_knowledge_sources (
    uuid uuid PRIMARY KEY,
    name text NOT NULL UNIQUE,
    ecosystem text NOT NULL,
    mode text NOT NULL CHECK (mode IN ('online','offline','hybrid')),
    authoritative boolean NOT NULL DEFAULT false,
    config jsonb NOT NULL,
    last_synced_at timestamptz,
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS phoenix_knowledge_access_log (
    uuid uuid PRIMARY KEY,
    source_uuid uuid NOT NULL REFERENCES phoenix_knowledge_sources(uuid),
    agent_uuid uuid NOT NULL,
    correlation_uuid uuid,
    mode text NOT NULL CHECK (mode IN ('online','offline')),
    query text,
    evidence_uuid uuid,
    accessed_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS phoenix_knowledge_access_agent_idx ON phoenix_knowledge_access_log(agent_uuid, accessed_at DESC);

COMMIT;

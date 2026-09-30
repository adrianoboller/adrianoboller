BEGIN;

CREATE TABLE IF NOT EXISTS phoenix_skills (
    uuid uuid PRIMARY KEY,
    name text NOT NULL,
    version text NOT NULL,
    state text NOT NULL CHECK (state IN ('candidate','validated','promoted','disabled','rejected')),
    manifest jsonb NOT NULL,
    sha256 text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (name, version)
);

CREATE TABLE IF NOT EXISTS phoenix_skill_evidence (
    skill_uuid uuid NOT NULL REFERENCES phoenix_skills(uuid) ON DELETE CASCADE,
    evidence_uuid uuid NOT NULL,
    kind text NOT NULL,
    recorded_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (skill_uuid, evidence_uuid)
);

CREATE TABLE IF NOT EXISTS phoenix_memory_records (
    uuid uuid PRIMARY KEY,
    namespace text NOT NULL,
    memory_key text NOT NULL,
    scope jsonb NOT NULL,
    classification text NOT NULL,
    value jsonb NOT NULL,
    confidence_millis integer NOT NULL CHECK (confidence_millis BETWEEN 0 AND 1000),
    evidence jsonb NOT NULL DEFAULT '[]'::jsonb,
    sha256 text NOT NULL,
    expires_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (namespace, memory_key)
);

CREATE INDEX IF NOT EXISTS phoenix_memory_namespace_idx ON phoenix_memory_records(namespace);
CREATE INDEX IF NOT EXISTS phoenix_memory_expires_idx ON phoenix_memory_records(expires_at) WHERE expires_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS phoenix_context_packs (
    uuid uuid PRIMARY KEY,
    correlation_uuid uuid NOT NULL,
    policy jsonb NOT NULL,
    items jsonb NOT NULL,
    bytes_estimate bigint NOT NULL,
    compiled_at timestamptz NOT NULL DEFAULT now()
);

COMMIT;

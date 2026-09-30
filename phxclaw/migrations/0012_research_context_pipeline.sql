BEGIN;

CREATE TABLE IF NOT EXISTS phoenix_research_runs (
    uuid uuid PRIMARY KEY,
    correlation_uuid uuid NOT NULL,
    agent_uuid uuid NOT NULL,
    skill_uuid uuid,
    source_uuid uuid,
    source_name text NOT NULL,
    source_mode text NOT NULL CHECK (source_mode IN ('offline','online')),
    freshness text NOT NULL CHECK (freshness IN ('stable','current','latest')),
    query_sha256 text NOT NULL CHECK (length(query_sha256) = 64),
    context_pack_uuid uuid,
    evidence_uuid uuid,
    status text NOT NULL CHECK (status IN ('started','context_ready','executing','completed','failed')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz
);

CREATE INDEX IF NOT EXISTS phoenix_research_runs_correlation_idx
    ON phoenix_research_runs(correlation_uuid, created_at DESC);
CREATE INDEX IF NOT EXISTS phoenix_research_runs_agent_idx
    ON phoenix_research_runs(agent_uuid, created_at DESC);

CREATE TABLE IF NOT EXISTS phoenix_skill_resolutions (
    uuid uuid PRIMARY KEY,
    research_run_uuid uuid NOT NULL REFERENCES phoenix_research_runs(uuid) ON DELETE CASCADE,
    skill_uuid uuid NOT NULL,
    skill_name text NOT NULL,
    skill_version text NOT NULL,
    skill_state text NOT NULL,
    score bigint NOT NULL,
    matched_triggers jsonb NOT NULL DEFAULT '[]'::jsonb,
    resolved_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS phoenix_research_source_hits (
    uuid uuid PRIMARY KEY,
    research_run_uuid uuid NOT NULL REFERENCES phoenix_research_runs(uuid) ON DELETE CASCADE,
    rank integer NOT NULL CHECK (rank > 0),
    source_uri text NOT NULL,
    title text NOT NULL,
    document_sha256 text NOT NULL CHECK (length(document_sha256) = 64),
    relevance_score bigint NOT NULL,
    excerpt_sha256 text NOT NULL CHECK (length(excerpt_sha256) = 64),
    evidence_uuid uuid,
    recorded_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (research_run_uuid, rank)
);

CREATE TABLE IF NOT EXISTS phoenix_research_event_links (
    research_run_uuid uuid NOT NULL REFERENCES phoenix_research_runs(uuid) ON DELETE CASCADE,
    event_uuid uuid NOT NULL,
    event_topic text NOT NULL,
    event_type text NOT NULL,
    sequence_no integer NOT NULL CHECK (sequence_no > 0),
    PRIMARY KEY (research_run_uuid, event_uuid),
    UNIQUE (research_run_uuid, sequence_no)
);

COMMIT;

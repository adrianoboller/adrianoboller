-- ============================================================================
-- PhxClaw — PostgreSQL FULL INSTALL v0.70
-- Gerado em: 2026-09-29T23:14:00-03:00
-- Origem: checkout consolidado v0.70 (histórico v0.20 + overlays até v0.70)
-- Finalidade: instalação NOVA em PostgreSQL.
--
-- REQUISITOS:
--   * PostgreSQL 14+ (15+ recomendado)
--   * usuário dono do banco; permissão para CREATE EXTENSION pgcrypto
--   * banco dedicado/vazio recomendado
--
-- GARANTIAS DO INSTALADOR:
--   * uma única transação;
--   * advisory lock contra instalação concorrente;
--   * ledger 1..61 com SHA-256 source/effective;
--   * v0.34 explicitamente marcada como reconstructed;
--   * versões 25/26/27/41/56 registradas como NO-OP histórico;
--   * v0.61 instala o Safe Source Harvester com ALLOW-only knowledge candidates;
--   * v0.62 instala estado canônico + Knowledge Promotion Gate + RLS tenant compatível.
-- ============================================================================

BEGIN;
SELECT pg_advisory_xact_lock(hashtextextended('phxclaw:full-install:v0.70',0));

CREATE TABLE IF NOT EXISTS phxclaw_schema_migrations (
    version integer PRIMARY KEY,
    migration_name text NOT NULL,
    source_sha256 char(64) NOT NULL,
    effective_sha256 char(64) NOT NULL,
    source_status text NOT NULL CHECK (source_status IN ('original','reconstructed','no_op')),
    repair_notes text NULL,
    applied_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

DO $phx_preflight$
BEGIN
    IF EXISTS (SELECT 1 FROM phxclaw_schema_migrations WHERE version = 70) THEN
        RAISE EXCEPTION 'PhxClaw v0.70 já consta como instalado. Use migration incremental, não FULL INSTALL.';
    END IF;
END
$phx_preflight$;

-- ============================================================================
-- MIGRATION 0001: 0001_core.sql
-- source_sha256:    f2485cf33b54af12c7854c3be64abc096bf59acf550e3728da8cf9343dee7462
-- effective_sha256: f2485cf33b54af12c7854c3be64abc096bf59acf550e3728da8cf9343dee7462
-- source_status:    original
-- ============================================================================
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS phoenix_objects (
  uuid uuid PRIMARY KEY,
  object_type text NOT NULL,
  payload jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_phoenix_objects_type
  ON phoenix_objects(object_type);

CREATE INDEX IF NOT EXISTS idx_phoenix_objects_payload_gin
  ON phoenix_objects USING gin(payload);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (1,'0001_core.sql','f2485cf33b54af12c7854c3be64abc096bf59acf550e3728da8cf9343dee7462','f2485cf33b54af12c7854c3be64abc096bf59acf550e3728da8cf9343dee7462','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0002: 0002_plugin_registry.sql
-- source_sha256:    02bf59886e1465ed113bd2b5b120b7496f3f0066b1a7c7b4ff6ee52eacc9e2b4
-- effective_sha256: 02bf59886e1465ed113bd2b5b120b7496f3f0066b1a7c7b4ff6ee52eacc9e2b4
-- source_status:    original
-- ============================================================================
CREATE TABLE IF NOT EXISTS phoenix_plugin_manifests (
  uuid uuid PRIMARY KEY,
  name text NOT NULL UNIQUE,
  version text NOT NULL,
  core_api text NOT NULL,
  status text NOT NULL DEFAULT 'discovered'
    CHECK (status IN ('discovered', 'validated', 'enabled', 'disabled', 'quarantined', 'failed')),
  manifest jsonb NOT NULL,
  digest_sha256 text NOT NULL,
  signature text NOT NULL,
  signer text NOT NULL,
  provenance text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_phoenix_plugin_name_version
  ON phoenix_plugin_manifests(name, version);

CREATE INDEX IF NOT EXISTS idx_phoenix_plugin_manifest_gin
  ON phoenix_plugin_manifests USING gin(manifest);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (2,'0002_plugin_registry.sql','02bf59886e1465ed113bd2b5b120b7496f3f0066b1a7c7b4ff6ee52eacc9e2b4','02bf59886e1465ed113bd2b5b120b7496f3f0066b1a7c7b4ff6ee52eacc9e2b4','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0003: 0003_plugin_quarantine_and_agents.sql
-- source_sha256:    e61f73f3508f8c84de519cdae6a5d0e45bd1da0eef8da865a237941e5279eb03
-- effective_sha256: e61f73f3508f8c84de519cdae6a5d0e45bd1da0eef8da865a237941e5279eb03
-- source_status:    original
-- ============================================================================
CREATE TABLE IF NOT EXISTS phoenix_plugin_quarantine (
  id bigserial PRIMARY KEY,
  plugin_uuid uuid NULL,
  plugin_name text NULL,
  source text NOT NULL,
  reason text NOT NULL,
  observed_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_phoenix_plugin_quarantine_uuid
  ON phoenix_plugin_quarantine(plugin_uuid);

CREATE INDEX IF NOT EXISTS idx_phoenix_plugin_quarantine_observed_at
  ON phoenix_plugin_quarantine(observed_at DESC);

CREATE TABLE IF NOT EXISTS phoenix_agent_instances (
  uuid uuid PRIMARY KEY,
  plugin_uuid uuid NOT NULL REFERENCES phoenix_plugin_manifests(uuid) ON DELETE RESTRICT,
  state text NOT NULL CHECK (state IN (
    'registered','starting','ready','busy','draining','stopped','failed','quarantined'
  )),
  capabilities jsonb NOT NULL DEFAULT '[]'::jsonb,
  started_at timestamptz NULL,
  stopped_at timestamptz NULL,
  last_health_at timestamptz NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_phoenix_agent_instances_plugin
  ON phoenix_agent_instances(plugin_uuid);

CREATE INDEX IF NOT EXISTS idx_phoenix_agent_instances_state
  ON phoenix_agent_instances(state);

CREATE TABLE IF NOT EXISTS phoenix_agent_events (
  uuid uuid PRIMARY KEY,
  agent_uuid uuid NOT NULL REFERENCES phoenix_agent_instances(uuid) ON DELETE CASCADE,
  event_type text NOT NULL,
  payload jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_phoenix_agent_events_agent_created
  ON phoenix_agent_events(agent_uuid, created_at DESC);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (3,'0003_plugin_quarantine_and_agents.sql','e61f73f3508f8c84de519cdae6a5d0e45bd1da0eef8da865a237941e5279eb03','e61f73f3508f8c84de519cdae6a5d0e45bd1da0eef8da865a237941e5279eb03','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0004: 0004_task_graph.sql
-- source_sha256:    bda0bdf72d7c4aed50686a37e660cf28bba0e05d40d5ecde03d25e101eca9254
-- effective_sha256: d93705ac0d950f57999a77ade3150551c7245aa267c5c73b02d8b9a278634304
-- source_status:    original
-- ============================================================================

CREATE TABLE IF NOT EXISTS phoenix_tasks (
    uuid uuid PRIMARY KEY,
    name text NOT NULL,
    capability text NOT NULL,
    payload jsonb NOT NULL DEFAULT '{}'::jsonb,
    dependencies jsonb NOT NULL DEFAULT '[]'::jsonb,
    requested_permissions jsonb NOT NULL DEFAULT '[]'::jsonb,
    retry_policy jsonb NOT NULL,
    approval_gate jsonb,
    idempotency_key text NOT NULL UNIQUE,
    priority integer NOT NULL DEFAULT 100 CHECK (priority BETWEEN 0 AND 65535),
    status text NOT NULL CHECK (status IN (
        'blocked','pending','waiting_approval','ready','running','succeeded','failed','cancelled','dead_letter'
    )),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    next_eligible_at timestamptz NOT NULL DEFAULT now(),
    active_run_uuid uuid,
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS phoenix_task_runs (
    uuid uuid PRIMARY KEY,
    task_uuid uuid NOT NULL REFERENCES phoenix_tasks(uuid) ON DELETE CASCADE,
    attempt integer NOT NULL CHECK (attempt > 0),
    status text NOT NULL CHECK (status IN ('running','succeeded','failed','cancelled')),
    result jsonb,
    error text,
    started_at timestamptz NOT NULL,
    finished_at timestamptz,
    UNIQUE (task_uuid, attempt)
);

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'phoenix_tasks_active_run_fk'
    ) THEN
        ALTER TABLE phoenix_tasks
            ADD CONSTRAINT phoenix_tasks_active_run_fk
            FOREIGN KEY (active_run_uuid) REFERENCES phoenix_task_runs(uuid) DEFERRABLE INITIALLY DEFERRED;
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS phoenix_task_approvals (
    uuid uuid PRIMARY KEY,
    task_uuid uuid NOT NULL REFERENCES phoenix_tasks(uuid) ON DELETE CASCADE,
    decision text NOT NULL CHECK (decision IN ('approved','rejected')),
    actor text NOT NULL,
    note text,
    decided_at timestamptz NOT NULL,
    UNIQUE (task_uuid)
);

CREATE TABLE IF NOT EXISTS phoenix_task_events (
    uuid uuid PRIMARY KEY,
    task_uuid uuid NOT NULL REFERENCES phoenix_tasks(uuid) ON DELETE CASCADE,
    run_uuid uuid REFERENCES phoenix_task_runs(uuid) ON DELETE SET NULL,
    event_type text NOT NULL,
    payload jsonb NOT NULL DEFAULT '{}'::jsonb,
    occurred_at timestamptz NOT NULL
);

CREATE INDEX IF NOT EXISTS phoenix_tasks_claim_idx
    ON phoenix_tasks (priority DESC, next_eligible_at, uuid)
    WHERE status = 'ready';

CREATE INDEX IF NOT EXISTS phoenix_task_events_task_time_idx
    ON phoenix_task_events (task_uuid, occurred_at, uuid);

CREATE INDEX IF NOT EXISTS phoenix_task_runs_task_idx
    ON phoenix_task_runs (task_uuid, attempt DESC);

COMMENT ON TABLE phoenix_tasks IS
    'F06 deterministic DAG tasks. Workers should claim ready rows using FOR UPDATE SKIP LOCKED.';

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (4,'0004_task_graph.sql','bda0bdf72d7c4aed50686a37e660cf28bba0e05d40d5ecde03d25e101eca9254','d93705ac0d950f57999a77ade3150551c7245aa267c5c73b02d8b9a278634304','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0005: 0005_model_gateway.sql
-- source_sha256:    55e5a058ad3f719101d238f19277b0ea3de35e783e73d9ec68f65fc94e43eb8f
-- effective_sha256: 4236125d792aaff3a3e7e3bf93c87b050ab2f894a9abdf9ca437f6dfccebca8e
-- source_status:    original
-- ============================================================================

CREATE TABLE IF NOT EXISTS phoenix_model_budget_accounts (
    name text PRIMARY KEY,
    ceiling_microunits bigint NOT NULL CHECK (ceiling_microunits >= 0),
    reserved_microunits bigint NOT NULL DEFAULT 0 CHECK (reserved_microunits >= 0),
    spent_microunits bigint NOT NULL DEFAULT 0 CHECK (spent_microunits >= 0),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (reserved_microunits + spent_microunits <= ceiling_microunits)
);

CREATE TABLE IF NOT EXISTS phoenix_model_route_events (
    uuid uuid PRIMARY KEY,
    request_uuid uuid NOT NULL,
    provider_uuid uuid NOT NULL,
    provider_name text NOT NULL,
    model_id text NOT NULL,
    capability text NOT NULL,
    classification text NOT NULL CHECK (classification IN ('public','internal','confidential','restricted')),
    requires_local boolean NOT NULL,
    estimated_cost_microunits bigint NOT NULL CHECK (estimated_cost_microunits >= 0),
    budget_account text NOT NULL REFERENCES phoenix_model_budget_accounts(name),
    routed_at timestamptz NOT NULL
);

CREATE TABLE IF NOT EXISTS phoenix_model_telemetry (
    uuid uuid PRIMARY KEY,
    request_uuid uuid NOT NULL,
    provider_uuid uuid NOT NULL,
    model_id text NOT NULL,
    latency_ms bigint NOT NULL CHECK (latency_ms >= 0),
    input_tokens bigint NOT NULL CHECK (input_tokens >= 0),
    output_tokens bigint NOT NULL CHECK (output_tokens >= 0),
    cost_microunits bigint NOT NULL CHECK (cost_microunits >= 0),
    success boolean NOT NULL,
    observed_at timestamptz NOT NULL
);

CREATE INDEX IF NOT EXISTS phoenix_model_route_request_idx
    ON phoenix_model_route_events (request_uuid, routed_at);

CREATE INDEX IF NOT EXISTS phoenix_model_telemetry_provider_time_idx
    ON phoenix_model_telemetry (provider_uuid, observed_at DESC);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (5,'0005_model_gateway.sql','55e5a058ad3f719101d238f19277b0ea3de35e783e73d9ec68f65fc94e43eb8f','4236125d792aaff3a3e7e3bf93c87b050ab2f894a9abdf9ca437f6dfccebca8e','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0006: 0006_event_outbox.sql
-- source_sha256:    afc78cfc6133a485f84fdc33ff8517013edf744bc036102b765cf69d4a83edc7
-- effective_sha256: e60fac3750a838eb75bad93c4d84210e33b868d1a6f7500baddd2bbf403448c7
-- source_status:    original
-- ============================================================================
CREATE TABLE IF NOT EXISTS phoenix_outbox (
    sequence BIGSERIAL PRIMARY KEY,
    event_uuid UUID NOT NULL UNIQUE,
    topic TEXT NOT NULL CHECK (length(btrim(topic)) > 0),
    event_type TEXT NOT NULL CHECK (length(btrim(event_type)) > 0),
    aggregate_uuid UUID NULL,
    correlation_uuid UUID NULL,
    causation_uuid UUID NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    occurred_at TIMESTAMPTZ NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version > 0),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    locked_at TIMESTAMPTZ NULL,
    published_at TIMESTAMPTZ NULL,
    last_error TEXT NULL
);
CREATE INDEX IF NOT EXISTS idx_phoenix_outbox_ready
ON phoenix_outbox (available_at, sequence) WHERE published_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_phoenix_outbox_topic
ON phoenix_outbox (topic, sequence);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (6,'0006_event_outbox.sql','afc78cfc6133a485f84fdc33ff8517013edf744bc036102b765cf69d4a83edc7','e60fac3750a838eb75bad93c4d84210e33b868d1a6f7500baddd2bbf403448c7','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0007: 0007_capability_audit.sql
-- source_sha256:    350b512b7404b1609ead9724f96d26ab617a62e6a601228274436a6f76bf27c2
-- effective_sha256: af739fa3441e9c6a3200364f35082ec0d4eba176ea2252c21d1ff5cdf260c098
-- source_status:    original
-- ============================================================================
CREATE TABLE IF NOT EXISTS phoenix_capability_audit (
    sequence BIGSERIAL PRIMARY KEY,
    event_uuid UUID NOT NULL UNIQUE,
    actor_uuid UUID NULL,
    task_uuid UUID NULL,
    capability TEXT NOT NULL,
    permission TEXT NULL,
    target TEXT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('allow','deny','error')),
    request JSONB NOT NULL DEFAULT '{}'::jsonb,
    result JSONB NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_phoenix_capability_audit_actor ON phoenix_capability_audit(actor_uuid, sequence DESC);
CREATE INDEX IF NOT EXISTS idx_phoenix_capability_audit_capability ON phoenix_capability_audit(capability, sequence DESC);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (7,'0007_capability_audit.sql','350b512b7404b1609ead9724f96d26ab617a62e6a601228274436a6f76bf27c2','af739fa3441e9c6a3200364f35082ec0d4eba176ea2252c21d1ff5cdf260c098','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0008: 0008_bpm.sql
-- source_sha256:    981317d58c563ea8ef1098a2beecf192fe684d8c591d5076822cf9340993f0a3
-- effective_sha256: 672c302b8c247f8253304b609aea110c96214e28013c48296a39acbcf896fa2e
-- source_status:    original
-- ============================================================================
CREATE TABLE IF NOT EXISTS phoenix_mindset_profiles (
    profile_uuid UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    version TEXT NOT NULL,
    principles JSONB NOT NULL,
    priorities JSONB NOT NULL,
    approval_rules JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS phoenix_bpm_processes (
    process_uuid UUID PRIMARY KEY,
    external_id TEXT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    bpmn_xml TEXT NOT NULL,
    compiled_graph JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(external_id, version)
);
CREATE TABLE IF NOT EXISTS phoenix_bpm_instances (
    instance_uuid UUID PRIMARY KEY,
    process_uuid UUID NOT NULL REFERENCES phoenix_bpm_processes(process_uuid),
    state TEXT NOT NULL,
    variables JSONB NOT NULL DEFAULT '{}'::jsonb,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ NULL
);
CREATE TABLE IF NOT EXISTS phoenix_bpm_tokens (
    token_uuid UUID PRIMARY KEY,
    instance_uuid UUID NOT NULL REFERENCES phoenix_bpm_instances(instance_uuid) ON DELETE CASCADE,
    node_id TEXT NOT NULL,
    state TEXT NOT NULL,
    lease_owner TEXT NULL,
    lease_until TIMESTAMPTZ NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS phoenix_bpm_history (
    sequence BIGSERIAL PRIMARY KEY,
    event_uuid UUID NOT NULL UNIQUE,
    instance_uuid UUID NOT NULL REFERENCES phoenix_bpm_instances(instance_uuid) ON DELETE CASCADE,
    token_uuid UUID NULL,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_phoenix_bpm_history_instance ON phoenix_bpm_history(instance_uuid, sequence);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (8,'0008_bpm.sql','981317d58c563ea8ef1098a2beecf192fe684d8c591d5076822cf9340993f0a3','672c302b8c247f8253304b609aea110c96214e28013c48296a39acbcf896fa2e','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0009: 0009_desktop_host_and_evidence.sql
-- source_sha256:    7d7a0f973662ab751ce446b2893715e1fb17f746b1052e0ca0f5599864cfddb1
-- effective_sha256: cf2676b3819c5d0b0ee8bb0ada37ad624717ffd3b49a02c8eca2cd995ed52107
-- source_status:    original
-- ============================================================================

CREATE TABLE IF NOT EXISTS phoenix_desktop_sessions (
    session_uuid UUID PRIMARY KEY,
    host_name TEXT NULL,
    operating_system TEXT NULL,
    app_version TEXT NOT NULL,
    api_addr TEXT NULL,
    policy JSONB NOT NULL DEFAULT '{}'::jsonb,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    stopped_at TIMESTAMPTZ NULL
);

CREATE TABLE IF NOT EXISTS phoenix_desktop_actions (
    action_uuid UUID PRIMARY KEY,
    session_uuid UUID NULL REFERENCES phoenix_desktop_sessions(session_uuid) ON DELETE SET NULL,
    correlation_uuid UUID NULL,
    actor TEXT NOT NULL,
    capability TEXT NOT NULL,
    action TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('accepted','succeeded','failed','denied')),
    request_summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    result_summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    evidence_uuid UUID NULL UNIQUE,
    requested_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ NULL
);
CREATE INDEX IF NOT EXISTS idx_phoenix_desktop_actions_session_time
    ON phoenix_desktop_actions(session_uuid, requested_at DESC);
CREATE INDEX IF NOT EXISTS idx_phoenix_desktop_actions_correlation
    ON phoenix_desktop_actions(correlation_uuid) WHERE correlation_uuid IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_phoenix_desktop_actions_capability
    ON phoenix_desktop_actions(capability, requested_at DESC);

CREATE TABLE IF NOT EXISTS phoenix_evidence_records (
    evidence_uuid UUID PRIMARY KEY,
    action_uuid UUID NOT NULL,
    correlation_uuid UUID NULL,
    actor TEXT NOT NULL,
    capability TEXT NOT NULL,
    action TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('requested','succeeded','failed','denied','cancelled')),
    request_summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    result_summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    artifact_uris JSONB NOT NULL DEFAULT '[]'::jsonb,
    occurred_at TIMESTAMPTZ NOT NULL,
    previous_hash TEXT NULL,
    record_hash TEXT NOT NULL UNIQUE,
    CONSTRAINT phoenix_evidence_hash_shape CHECK (record_hash ~ '^[0-9a-f]{64}$'),
    CONSTRAINT phoenix_evidence_previous_hash_shape CHECK (previous_hash IS NULL OR previous_hash ~ '^[0-9a-f]{64}$')
);
CREATE INDEX IF NOT EXISTS idx_phoenix_evidence_action
    ON phoenix_evidence_records(action_uuid, occurred_at);
CREATE INDEX IF NOT EXISTS idx_phoenix_evidence_correlation
    ON phoenix_evidence_records(correlation_uuid, occurred_at) WHERE correlation_uuid IS NOT NULL;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (9,'0009_desktop_host_and_evidence.sql','7d7a0f973662ab751ce446b2893715e1fb17f746b1052e0ca0f5599864cfddb1','cf2676b3819c5d0b0ee8bb0ada37ad624717ffd3b49a02c8eca2cd995ed52107','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0010: 0010_skill_memory_context.sql
-- source_sha256:    83be717555c82c366c38786b265f65c1cf240234878e202eb3cab50f0e6cbe9b
-- effective_sha256: 662d6c4fa95247e7c778b4b174e659cf981f9a2f7ef827833efaf627d4c57fd1
-- source_status:    original
-- ============================================================================

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

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (10,'0010_skill_memory_context.sql','83be717555c82c366c38786b265f65c1cf240234878e202eb3cab50f0e6cbe9b','662d6c4fa95247e7c778b4b174e659cf981f9a2f7ef827833efaf627d4c57fd1','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0011: 0011_agent_registry_and_knowledge_sources.sql
-- source_sha256:    70a6cee502d4bdc77052a2106dfa39a2680eb4bec2e25046578ff35d3f965f8a
-- effective_sha256: 29236dfc83e21531cab3a855f04dfd2636482755989ec2288fe6dfb35f1fcf7e
-- source_status:    original
-- ============================================================================

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

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (11,'0011_agent_registry_and_knowledge_sources.sql','70a6cee502d4bdc77052a2106dfa39a2680eb4bec2e25046578ff35d3f965f8a','29236dfc83e21531cab3a855f04dfd2636482755989ec2288fe6dfb35f1fcf7e','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0012: 0012_research_context_pipeline.sql
-- source_sha256:    3fe13c7928bc4d49c39bc65045cd629071c9335f3f7e84c4a9454428c5a929bd
-- effective_sha256: d5a8d015ffc28eb0f793fd6e20a3ef02cdf5783f51cad75bf4a441127e3f55d6
-- source_status:    original
-- ============================================================================

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

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (12,'0012_research_context_pipeline.sql','3fe13c7928bc4d49c39bc65045cd629071c9335f3f7e84c4a9454428c5a929bd','d5a8d015ffc28eb0f793fd6e20a3ef02cdf5783f51cad75bf4a441127e3f55d6','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0013: 0013_community_plugins.sql
-- source_sha256:    a05e98711081eb23ee2ac4f3620813759752483f2df9b8f83d467109ce96fd12
-- effective_sha256: 907e371e0ca1d73cd43dcf4f7759d1a777b555f436a54c0c05d96504249a1941
-- source_status:    original
-- ============================================================================

CREATE TABLE IF NOT EXISTS phoenix_plugin_publishers (
    publisher_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    signer TEXT NOT NULL,
    trust_tier TEXT NOT NULL CHECK (trust_tier IN ('builtin','verified','community','local_development')),
    website TEXT,
    repository TEXT,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','suspended','revoked')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS phoenix_plugin_releases (
    plugin_uuid UUID NOT NULL,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    core_api TEXT NOT NULL,
    publisher_id TEXT NOT NULL REFERENCES phoenix_plugin_publishers(publisher_id),
    license TEXT NOT NULL,
    source_repository TEXT NOT NULL,
    package_url TEXT NOT NULL,
    package_sha256 TEXT NOT NULL,
    manifest_sha256 TEXT NOT NULL,
    signature_algorithm TEXT NOT NULL CHECK (signature_algorithm = 'ed25519'),
    signature TEXT NOT NULL,
    signer TEXT NOT NULL,
    provenance TEXT NOT NULL,
    manifest JSONB NOT NULL,
    yanked BOOLEAN NOT NULL DEFAULT FALSE,
    published_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (plugin_uuid, version)
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_phoenix_plugin_releases_name_version
    ON phoenix_plugin_releases(name, version);

CREATE TABLE IF NOT EXISTS phoenix_plugin_installs (
    install_uuid UUID PRIMARY KEY,
    plugin_uuid UUID NOT NULL,
    version TEXT NOT NULL,
    source_registry TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('downloaded','verified','installed','enabled','disabled','quarantined','failed','rolled_back')),
    installed_manifest JSONB NOT NULL,
    package_sha256 TEXT NOT NULL,
    installed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (plugin_uuid, version, install_uuid)
);

CREATE TABLE IF NOT EXISTS phoenix_plugin_capability_routes (
    capability TEXT PRIMARY KEY,
    plugin_uuid UUID NOT NULL,
    plugin_version TEXT NOT NULL,
    pinned_by TEXT NOT NULL,
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS phoenix_plugin_extension_events (
    event_uuid UUID PRIMARY KEY,
    plugin_uuid UUID NOT NULL,
    capability TEXT,
    correlation_uuid UUID,
    causation_uuid UUID,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (13,'0013_community_plugins.sql','a05e98711081eb23ee2ac4f3620813759752483f2df9b8f83d467109ce96fd12','907e371e0ca1d73cd43dcf4f7759d1a777b555f436a54c0c05d96504249a1941','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0014: 0014_mission_runtime.sql
-- source_sha256:    407bfb1458fe55e964572ce0e2e7d795eaf06be52d62155d7d51cc7e70a2fb94
-- effective_sha256: 407bfb1458fe55e964572ce0e2e7d795eaf06be52d62155d7d51cc7e70a2fb94
-- source_status:    original
-- ============================================================================
CREATE TABLE IF NOT EXISTS phoenix_missions (
    uuid UUID PRIMARY KEY,
    correlation_uuid UUID NOT NULL,
    objective_sha256 TEXT NOT NULL,
    project_root TEXT NOT NULL,
    base_ref TEXT NOT NULL,
    state TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS phoenix_missions_correlation_idx
    ON phoenix_missions (correlation_uuid, created_at DESC);

CREATE TABLE IF NOT EXISTS phoenix_mission_steps (
    uuid UUID PRIMARY KEY,
    mission_uuid UUID NOT NULL REFERENCES phoenix_missions(uuid) ON DELETE CASCADE,
    name TEXT NOT NULL,
    agent_capability TEXT NOT NULL,
    action JSONB NOT NULL,
    required BOOLEAN NOT NULL,
    status TEXT NOT NULL,
    agent_uuid UUID,
    agent_name TEXT,
    evidence_uuid UUID,
    output JSONB NOT NULL DEFAULT '{}'::jsonb,
    finished_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS phoenix_mission_steps_mission_idx
    ON phoenix_mission_steps (mission_uuid, finished_at);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (14,'0014_mission_runtime.sql','407bfb1458fe55e964572ce0e2e7d795eaf06be52d62155d7d51cc7e70a2fb94','407bfb1458fe55e964572ce0e2e7d795eaf06be52d62155d7d51cc7e70a2fb94','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0015: 0015_private_plugins_checkpoint_repo.sql
-- source_sha256:    0bea9a1fbe407a3ea4db89683b5fb6759b432fcf586f25c561ab6e18abb9ed88
-- effective_sha256: 0bea9a1fbe407a3ea4db89683b5fb6759b432fcf586f25c561ab6e18abb9ed88
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.13: private plugin lifecycle, checkpoint metadata and repository intelligence.
CREATE TABLE IF NOT EXISTS phoenix_private_plugin_installations (
    plugin_uuid uuid NOT NULL,
    name text NOT NULL,
    version text NOT NULL,
    state text NOT NULL CHECK (state IN ('disabled','enabled','quarantined')),
    manifest_sha256 text NOT NULL,
    package_files_digest text NOT NULL,
    signer text NOT NULL,
    install_path text NOT NULL,
    installed_at timestamptz NOT NULL DEFAULT now(),
    state_changed_at timestamptz,
    last_doctor_at timestamptz,
    last_health jsonb,
    PRIMARY KEY (plugin_uuid, version)
);
CREATE INDEX IF NOT EXISTS phoenix_private_plugin_installations_name_idx
    ON phoenix_private_plugin_installations(name, installed_at DESC);

CREATE TABLE IF NOT EXISTS phoenix_checkpoints (
    uuid uuid PRIMARY KEY,
    workspace text NOT NULL,
    created_at timestamptz NOT NULL,
    file_count integer NOT NULL,
    total_bytes bigint NOT NULL DEFAULT 0,
    manifest jsonb NOT NULL
);
CREATE TABLE IF NOT EXISTS phoenix_checkpoint_files (
    checkpoint_uuid uuid NOT NULL REFERENCES phoenix_checkpoints(uuid) ON DELETE CASCADE,
    path text NOT NULL,
    sha256 text NOT NULL,
    bytes bigint NOT NULL,
    PRIMARY KEY (checkpoint_uuid, path)
);

CREATE TABLE IF NOT EXISTS phoenix_repo_inventory_runs (
    uuid uuid PRIMARY KEY,
    workspace text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    file_count integer NOT NULL,
    total_bytes bigint NOT NULL,
    total_lines bigint NOT NULL,
    summary jsonb NOT NULL
);
CREATE TABLE IF NOT EXISTS phoenix_repo_inventory_files (
    run_uuid uuid NOT NULL REFERENCES phoenix_repo_inventory_runs(uuid) ON DELETE CASCADE,
    path text NOT NULL,
    language text NOT NULL,
    sha256 text NOT NULL,
    bytes bigint NOT NULL,
    lines bigint NOT NULL,
    score bigint NOT NULL,
    PRIMARY KEY (run_uuid, path)
);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (15,'0015_private_plugins_checkpoint_repo.sql','0bea9a1fbe407a3ea4db89683b5fb6759b432fcf586f25c561ab6e18abb9ed88','0bea9a1fbe407a3ea4db89683b5fb6759b432fcf586f25c561ab6e18abb9ed88','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0016: 0016_team_runtime.sql
-- source_sha256:    e7f8a9357e8d1084f8b33e7399f5740226960e429b3b37ce19144876a505a60c
-- effective_sha256: e7f8a9357e8d1084f8b33e7399f5740226960e429b3b37ce19144876a505a60c
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.14 / F20 Team Runtime
CREATE TABLE IF NOT EXISTS phoenix_team_sessions (
  uuid uuid PRIMARY KEY, correlation_uuid uuid NOT NULL, name text NOT NULL, state text NOT NULL,
  max_parallelism integer NOT NULL CHECK (max_parallelism > 0), created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS phoenix_team_tasks (
  uuid uuid PRIMARY KEY, team_uuid uuid NOT NULL REFERENCES phoenix_team_sessions(uuid) ON DELETE CASCADE,
  name text NOT NULL, capability text NOT NULL, depends_on jsonb NOT NULL DEFAULT '[]'::jsonb, priority integer NOT NULL DEFAULT 0,
  state text NOT NULL, worker_id text, fencing_token bigint NOT NULL DEFAULT 0, lease_expires_at timestamptz, heartbeat_at timestamptz,
  attempts integer NOT NULL DEFAULT 0, cancel_requested boolean NOT NULL DEFAULT false, output jsonb
);
CREATE INDEX IF NOT EXISTS idx_phoenix_team_tasks_ready ON phoenix_team_tasks(team_uuid,state,priority);
CREATE INDEX IF NOT EXISTS idx_phoenix_team_tasks_lease ON phoenix_team_tasks(state,lease_expires_at);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (16,'0016_team_runtime.sql','e7f8a9357e8d1084f8b33e7399f5740226960e429b3b37ce19144876a505a60c','e7f8a9357e8d1084f8b33e7399f5740226960e429b3b37ce19144876a505a60c','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0017: 0017_channel_gateway.sql
-- source_sha256:    4836f476822525dc0bf68f204ee0cd4d906ece9f4a7e7670a4af551fb2d6c8c5
-- effective_sha256: 4836f476822525dc0bf68f204ee0cd4d906ece9f4a7e7670a4af551fb2d6c8c5
-- source_status:    original
-- ============================================================================
-- PhxClaw F21 Channel Gateway
CREATE TABLE IF NOT EXISTS phoenix_channel_identities (
    uuid UUID PRIMARY KEY,
    channel TEXT NOT NULL,
    account_id TEXT NOT NULL,
    external_user_id TEXT NOT NULL,
    principal_uuid UUID NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending','active','blocked')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(channel, account_id, external_user_id)
);

CREATE TABLE IF NOT EXISTS phoenix_channel_sessions (
    uuid UUID PRIMARY KEY,
    principal_uuid UUID NOT NULL,
    channel TEXT NOT NULL,
    account_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(principal_uuid, channel, account_id, conversation_id)
);

CREATE TABLE IF NOT EXISTS phoenix_channel_messages (
    uuid UUID PRIMARY KEY,
    session_uuid UUID NOT NULL REFERENCES phoenix_channel_sessions(uuid),
    external_message_id TEXT,
    direction TEXT NOT NULL CHECK (direction IN ('inbound','outbound')),
    channel TEXT NOT NULL,
    account_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(channel, account_id, external_message_id)
);

CREATE INDEX IF NOT EXISTS idx_phoenix_channel_sessions_principal ON phoenix_channel_sessions(principal_uuid);
CREATE INDEX IF NOT EXISTS idx_phoenix_channel_messages_session ON phoenix_channel_messages(session_uuid, created_at);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (17,'0017_channel_gateway.sql','4836f476822525dc0bf68f204ee0cd4d906ece9f4a7e7670a4af551fb2d6c8c5','4836f476822525dc0bf68f204ee0cd4d906ece9f4a7e7670a4af551fb2d6c8c5','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0018: 0018_secret_broker_and_channel_providers.sql
-- source_sha256:    e25765f8d6018de1ea46619045e03aeabd199a5585d994a78b95309ad61f76bc
-- effective_sha256: e25765f8d6018de1ea46619045e03aeabd199a5585d994a78b95309ad61f76bc
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.18 — F23 Secret & Credential Broker + F21 secret-backed channel providers

CREATE TABLE IF NOT EXISTS phoenix_secret_descriptors (
    secret_uuid uuid PRIMARY KEY,
    name text NOT NULL,
    namespace text NOT NULL,
    version bigint NOT NULL CHECK (version >= 1),
    scopes jsonb NOT NULL DEFAULT '[]'::jsonb,
    sha256 text NOT NULL,
    key_provider text NOT NULL,
    storage_uri text NOT NULL,
    created_at timestamptz NOT NULL,
    rotated_at timestamptz,
    revoked_at timestamptz,
    UNIQUE(namespace, name)
);

CREATE TABLE IF NOT EXISTS phoenix_secret_leases (
    lease_uuid uuid PRIMARY KEY,
    secret_uuid uuid NOT NULL REFERENCES phoenix_secret_descriptors(secret_uuid),
    secret_version bigint NOT NULL,
    consumer text NOT NULL,
    scope text NOT NULL,
    issued_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    CHECK (expires_at > issued_at)
);

CREATE INDEX IF NOT EXISTS phoenix_secret_leases_secret_idx
    ON phoenix_secret_leases(secret_uuid, expires_at);

CREATE TABLE IF NOT EXISTS phoenix_channel_provider_accounts (
    provider_account_uuid uuid PRIMARY KEY,
    provider_id text NOT NULL,
    account_id text NOT NULL,
    token_secret_uuid uuid NOT NULL REFERENCES phoenix_secret_descriptors(secret_uuid),
    enabled boolean NOT NULL DEFAULT false,
    endpoint_origin text NOT NULL,
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    UNIQUE(provider_id, account_id)
);

-- Secret values are deliberately NOT stored in PostgreSQL by this migration.
-- Only encrypted store URIs / metadata / lease records are persisted here.

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (18,'0018_secret_broker_and_channel_providers.sql','e25765f8d6018de1ea46619045e03aeabd199a5585d994a78b95309ad61f76bc','e25765f8d6018de1ea46619045e03aeabd199a5585d994a78b95309ad61f76bc','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0019: 0019_rustclaw_native_session_cron.sql
-- source_sha256:    18ed46863e256c7bac137bf66b8c1fd66963dc4d197588f47447799bf04f4f81
-- effective_sha256: 18ed46863e256c7bac137bf66b8c1fd66963dc4d197588f47447799bf04f4f81
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.19 — MIT-derived RustClaw native compatibility persistence.
-- PostgreSQL remains the official state store; the vendored RustClaw SQLite store is not adopted.

CREATE TABLE IF NOT EXISTS phoenix_native_sessions (
    uuid UUID PRIMARY KEY,
    principal_uuid UUID NULL,
    channel TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS phoenix_native_session_messages (
    uuid UUID PRIMARY KEY,
    session_uuid UUID NOT NULL REFERENCES phoenix_native_sessions(uuid) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    content_sha256 TEXT NOT NULL CHECK (length(content_sha256) = 64),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_native_session_messages_session
    ON phoenix_native_session_messages(session_uuid, created_at);

CREATE TABLE IF NOT EXISTS phoenix_scheduled_jobs (
    uuid UUID PRIMARY KEY,
    name TEXT NOT NULL,
    capability TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    schedule JSONB NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('disabled','ready','running','failed','cancelled')),
    next_run_at TIMESTAMPTZ NULL,
    last_run_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_ready
    ON phoenix_scheduled_jobs(state, next_run_at);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (19,'0019_rustclaw_native_session_cron.sql','18ed46863e256c7bac137bf66b8c1fd66963dc4d197588f47447799bf04f4f81','18ed46863e256c7bac137bf66b8c1fd66963dc4d197588f47447799bf04f4f81','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0020: 0020_core_runtime_installation.sql
-- source_sha256:    377b7244406732487454c168d35f70333dde06243c8c791267b4fd3f07c92dcd
-- effective_sha256: 377b7244406732487454c168d35f70333dde06243c8c791267b4fd3f07c92dcd
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.20 unified runtime/bootstrap observability.
CREATE TABLE IF NOT EXISTS phoenix_core_runtime_instances (
    runtime_uuid uuid PRIMARY KEY,
    version text NOT NULL,
    host_name text,
    started_at timestamptz NOT NULL DEFAULT now(),
    stopped_at timestamptz,
    status jsonb NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS phoenix_install_events (
    event_uuid uuid PRIMARY KEY,
    component text NOT NULL,
    action text NOT NULL,
    outcome text NOT NULL,
    evidence jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS phoenix_install_events_created_idx
    ON phoenix_install_events(created_at DESC);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (20,'0020_core_runtime_installation.sql','377b7244406732487454c168d35f70333dde06243c8c791267b4fd3f07c92dcd','377b7244406732487454c168d35f70333dde06243c8c791267b4fd3f07c92dcd','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0021: 0021_device_nodes.sql
-- source_sha256:    4ebfc729ade0602b21dc9e938404139d3d7a52841552ad39a53dfc15816b37e6
-- effective_sha256: 9ff86519b488944f9e8cfef89b9631a45ec4d5c2fc154784fb5a16df046f3918
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.21 / F22 Device Nodes — corrected by v0.22
-- original migration_uuid retained in v0.21 source history
-- Repair: valid tenant RLS current_setting expressions; transaction remains all-or-nothing.

CREATE SCHEMA IF NOT EXISTS phxclaw;

CREATE TABLE IF NOT EXISTS phxclaw.device_nodes (
  node_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  display_name text NOT NULL CHECK (length(display_name) BETWEEN 1 AND 200),
  state text NOT NULL CHECK (state IN ('pending','enrolled','active','degraded','quarantined','revoked')),
  public_key_ed25519 bytea NOT NULL CHECK (octet_length(public_key_ed25519) = 32),
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
  last_seen_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_uuid, node_uuid)
);

CREATE TABLE IF NOT EXISTS phxclaw.device_capabilities (
  tenant_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL REFERENCES phxclaw.device_nodes(node_uuid) ON DELETE CASCADE,
  capability text NOT NULL,
  version text NOT NULL,
  declared_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, node_uuid, capability)
);

CREATE TABLE IF NOT EXISTS phxclaw.device_enrollment_tokens (
  enrollment_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  token_sha256 bytea NOT NULL CHECK (octet_length(token_sha256) = 32),
  expires_at timestamptz NOT NULL,
  consumed_at timestamptz,
  consumed_by_node_uuid uuid,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_uuid, token_sha256)
);

CREATE TABLE IF NOT EXISTS phxclaw.device_sessions (
  session_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL REFERENCES phxclaw.device_nodes(node_uuid) ON DELETE CASCADE,
  started_at timestamptz NOT NULL DEFAULT now(),
  expires_at timestamptz NOT NULL,
  last_sequence bigint NOT NULL DEFAULT 0 CHECK (last_sequence >= 0),
  fencing_token bigint NOT NULL DEFAULT 0 CHECK (fencing_token >= 0),
  revoked_at timestamptz,
  UNIQUE (tenant_uuid, node_uuid, session_uuid)
);

CREATE TABLE IF NOT EXISTS phxclaw.device_replay_reservations (
  tenant_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL,
  session_uuid uuid NOT NULL,
  sequence bigint NOT NULL CHECK (sequence >= 0),
  nonce bytea NOT NULL CHECK (octet_length(nonce) = 16),
  message_uuid uuid NOT NULL,
  reserved_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, node_uuid, session_uuid, sequence),
  UNIQUE (tenant_uuid, node_uuid, session_uuid, nonce),
  UNIQUE (tenant_uuid, message_uuid)
);

CREATE TABLE IF NOT EXISTS phxclaw.device_commands (
  command_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL REFERENCES phxclaw.device_nodes(node_uuid) ON DELETE RESTRICT,
  capability text NOT NULL,
  arguments jsonb NOT NULL DEFAULT '{}'::jsonb,
  secret_handles jsonb NOT NULL DEFAULT '[]'::jsonb,
  idempotency_key text NOT NULL CHECK (length(idempotency_key) BETWEEN 1 AND 200),
  risk text NOT NULL CHECK (risk IN ('low','medium','high','critical')),
  approval_uuid uuid,
  state text NOT NULL CHECK (state IN ('queued','claimed','succeeded','failed','cancelled','expired')),
  fencing_token bigint NOT NULL CHECK (fencing_token >= 0),
  submitted_at timestamptz NOT NULL,
  not_before timestamptz NOT NULL,
  expires_at timestamptz NOT NULL,
  claimed_at timestamptz,
  completed_at timestamptz,
  result_summary jsonb,
  CHECK (expires_at > not_before),
  UNIQUE (tenant_uuid, node_uuid, idempotency_key)
);

CREATE TABLE IF NOT EXISTS phxclaw.device_command_events (
  event_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  command_uuid uuid NOT NULL REFERENCES phxclaw.device_commands(command_uuid) ON DELETE CASCADE,
  event_type text NOT NULL,
  event_data jsonb NOT NULL DEFAULT '{}'::jsonb,
  correlation_uuid uuid,
  causation_uuid uuid,
  recorded_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS phxclaw.key_provider_metadata (
  provider_uuid uuid PRIMARY KEY,
  provider_kind text NOT NULL CHECK (provider_kind IN ('os_keyring','external_kms','dev_env')),
  key_id text NOT NULL,
  production_safe boolean NOT NULL,
  active boolean NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT now(),
  rotated_at timestamptz,
  UNIQUE (provider_kind, key_id),
  CHECK (production_safe OR provider_kind = 'dev_env')
);

CREATE INDEX IF NOT EXISTS device_nodes_tenant_state_idx ON phxclaw.device_nodes (tenant_uuid, state);
CREATE INDEX IF NOT EXISTS device_commands_claim_idx ON phxclaw.device_commands (tenant_uuid, node_uuid, state, not_before, expires_at);
CREATE INDEX IF NOT EXISTS device_command_events_command_idx ON phxclaw.device_command_events (tenant_uuid, command_uuid, recorded_at);

ALTER TABLE phxclaw.device_nodes ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_capabilities ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_enrollment_tokens ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_replay_reservations ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_commands ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_command_events ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation ON phxclaw.device_nodes;
CREATE POLICY tenant_isolation ON phxclaw.device_nodes USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.device_capabilities;
CREATE POLICY tenant_isolation ON phxclaw.device_capabilities USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.device_enrollment_tokens;
CREATE POLICY tenant_isolation ON phxclaw.device_enrollment_tokens USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.device_sessions;
CREATE POLICY tenant_isolation ON phxclaw.device_sessions USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.device_replay_reservations;
CREATE POLICY tenant_isolation ON phxclaw.device_replay_reservations USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.device_commands;
CREATE POLICY tenant_isolation ON phxclaw.device_commands USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.device_command_events;
CREATE POLICY tenant_isolation ON phxclaw.device_command_events USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (21,'0021_device_nodes.sql','4ebfc729ade0602b21dc9e938404139d3d7a52841552ad39a53dfc15816b37e6','9ff86519b488944f9e8cfef89b9631a45ec4d5c2fc154784fb5a16df046f3918','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0022: 0022_skill_evolution.sql
-- source_sha256:    b4957401365f55d8b090dcd3c88a7a53acc6b3a5b792d85370c3c720fdee5841
-- effective_sha256: c624c5cac41e3fb560bbf86eff10ad4f44e699c82618cc029d6c4835b56059fd
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.22 / F24 Auto-learning + Skill Evolution
-- migration_uuid: 01a0e800-cf88-73b0-8373-83be48bfff29
-- PostgreSQL is authoritative. Learning output cannot mutate core/policy state directly.

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

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (22,'0022_skill_evolution.sql','b4957401365f55d8b090dcd3c88a7a53acc6b3a5b792d85370c3c720fdee5841','c624c5cac41e3fb560bbf86eff10ad4f44e699c82618cc029d6c4835b56059fd','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0023: 0023_knowledge_evidence_graph.sql
-- source_sha256:    6f27ee3e147ad449476e9db5117c76f7481bca1df28f272756bd88c13514801b
-- effective_sha256: fca76dabc2b7c65e0ea1450be8f4fa5bf5062225ca5023d67b7145472e062b70
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.23 / F25 Knowledge / Evidence Graph
-- migration_uuid: 01a0e800-cf88-7d25-9a40-bf9178b2f0c8
-- PostgreSQL is authoritative. Raw sources are immutable; interpretations are versioned/superseded.

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

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (23,'0023_knowledge_evidence_graph.sql','6f27ee3e147ad449476e9db5117c76f7481bca1df28f272756bd88c13514801b','fca76dabc2b7c65e0ea1450be8f4fa5bf5062225ca5023d67b7145472e062b70','original','native-v070: UNIQUE de knowledge_nodes ganhou epistemic_state; o motor versiona o no trocando so o estado') ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0024: 0024_release_hardening.sql
-- source_sha256:    088f1d7382589ec438d116ef2c8b69b006fbbe468a006e15ae06689fbab9f2dc
-- effective_sha256: 1ea4a84a792ba7facfaef0010acaab3150e79520a870e802dc38c546f084d99b
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.24 Release Hardening
-- migration_uuid: 01a0e800-dca8-7e40-9bf1-6e4e2f6f8024
-- Adds DB-enforced tenant relationship integrity, FORCE RLS and evidence-driven release gates.

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

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (24,'0024_release_hardening.sql','088f1d7382589ec438d116ef2c8b69b006fbbe468a006e15ae06689fbab9f2dc','1ea4a84a792ba7facfaef0010acaab3150e79520a870e802dc38c546f084d99b','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0025 — NO-OP
-- v0.25 release qualification — sem DDL persistente
-- ============================================================================
INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (25,'0025_NO_OP','62e729ec32bc54787cbd66c99d23744a135c04de69834ab03e9f32c6dbd669f1','62e729ec32bc54787cbd66c99d23744a135c04de69834ab03e9f32c6dbd669f1','no_op','v0.25 release qualification — sem DDL persistente') ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0026 — NO-OP
-- v0.26 release candidate factory — sem DDL persistente
-- ============================================================================
INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (26,'0026_NO_OP','b5f4474fc3b067efe282e77c2058c3bb598eb838e42f66ee22b35c02fa7e75f6','b5f4474fc3b067efe282e77c2058c3bb598eb838e42f66ee22b35c02fa7e75f6','no_op','v0.26 release candidate factory — sem DDL persistente') ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0027 — NO-OP
-- v0.27 multi-platform GA — sem DDL persistente
-- ============================================================================
INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (27,'0027_NO_OP','ab5652c0657251ba68c27d0649c299b62ff780a6c16eb5931868a4f67ffa011e','ab5652c0657251ba68c27d0649c299b62ff780a6c16eb5931868a4f67ffa011e','no_op','v0.27 multi-platform GA — sem DDL persistente') ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0028: 0028_secure_fleet_rollout.sql
-- source_sha256:    3ef15fc146f3629a44a83f2c0f56305bec95b03bb6a898189f7a6e7c01c0eecf
-- effective_sha256: 0d34a3a9049013a4491afdfee555b49fdac5209e6757463a0d5d6ada007ed578
-- source_status:    original
-- ============================================================================
CREATE TABLE IF NOT EXISTS phoenix_fleet_rollouts (
  tenant_uuid uuid NOT NULL,
  rollout_uuid uuid NOT NULL,
  channel text NOT NULL CHECK (channel IN ('canary','beta','stable')),
  target_version text NOT NULL,
  update_sequence bigint NOT NULL CHECK (update_sequence > 0),
  source_state_sha256 char(64) NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-fA-F]{64}$'),
  update_manifest_sha256 char(64) NOT NULL CHECK (update_manifest_sha256 ~ '^[0-9a-fA-F]{64}$'),
  state text NOT NULL CHECK (state IN ('draft','active','paused','rollback_required','rolling_back','completed','aborted')),
  stage_index integer NOT NULL DEFAULT 0 CHECK(stage_index >= 0),
  generation bigint NOT NULL DEFAULT 1 CHECK(generation > 0),
  fencing_token bigint NOT NULL DEFAULT 1 CHECK(fencing_token > 0),
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, rollout_uuid),
  UNIQUE (tenant_uuid, update_sequence, channel)
);
CREATE TABLE IF NOT EXISTS phoenix_fleet_nodes (
  tenant_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL,
  fleet_uuid uuid NOT NULL,
  current_version text NOT NULL,
  current_sequence bigint NOT NULL CHECK(current_sequence >= 0),
  known_good_version text NOT NULL,
  known_good_sequence bigint NOT NULL CHECK(known_good_sequence >= 0),
  last_heartbeat_at timestamptz,
  PRIMARY KEY (tenant_uuid, node_uuid)
);
CREATE TABLE IF NOT EXISTS phoenix_fleet_assignments (
  tenant_uuid uuid NOT NULL,
  rollout_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL,
  cohort_bucket integer NOT NULL CHECK(cohort_bucket BETWEEN 0 AND 9999),
  stage_index integer NOT NULL CHECK(stage_index >= 0),
  status text NOT NULL CHECK(status IN ('pending','selected','installing','healthy','failed','rolled_back','excluded')),
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, rollout_uuid, node_uuid),
  FOREIGN KEY (tenant_uuid, rollout_uuid) REFERENCES phoenix_fleet_rollouts(tenant_uuid, rollout_uuid) ON DELETE CASCADE,
  FOREIGN KEY (tenant_uuid, node_uuid) REFERENCES phoenix_fleet_nodes(tenant_uuid, node_uuid) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS phoenix_fleet_health_evidence (
  tenant_uuid uuid NOT NULL,
  evidence_uuid uuid NOT NULL,
  rollout_uuid uuid NOT NULL,
  stage_index integer NOT NULL,
  evidence_sha256 char(64) NOT NULL CHECK(evidence_sha256 ~ '^[0-9a-fA-F]{64}$'),
  signer_key_id text NOT NULL,
  created_at timestamptz NOT NULL,
  payload jsonb NOT NULL,
  PRIMARY KEY (tenant_uuid, evidence_uuid),
  FOREIGN KEY (tenant_uuid, rollout_uuid) REFERENCES phoenix_fleet_rollouts(tenant_uuid, rollout_uuid) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS phoenix_fleet_events (
  tenant_uuid uuid NOT NULL,
  event_uuid uuid NOT NULL,
  rollout_uuid uuid NOT NULL,
  generation bigint NOT NULL,
  fencing_token bigint NOT NULL,
  event_type text NOT NULL,
  event_sha256 char(64) NOT NULL CHECK(event_sha256 ~ '^[0-9a-fA-F]{64}$'),
  created_at timestamptz NOT NULL DEFAULT now(),
  payload jsonb NOT NULL,
  PRIMARY KEY (tenant_uuid, event_uuid),
  FOREIGN KEY (tenant_uuid, rollout_uuid) REFERENCES phoenix_fleet_rollouts(tenant_uuid, rollout_uuid) ON DELETE RESTRICT
);

ALTER TABLE phoenix_fleet_rollouts ENABLE ROW LEVEL SECURITY; ALTER TABLE phoenix_fleet_rollouts FORCE ROW LEVEL SECURITY;
ALTER TABLE phoenix_fleet_nodes ENABLE ROW LEVEL SECURITY; ALTER TABLE phoenix_fleet_nodes FORCE ROW LEVEL SECURITY;
ALTER TABLE phoenix_fleet_assignments ENABLE ROW LEVEL SECURITY; ALTER TABLE phoenix_fleet_assignments FORCE ROW LEVEL SECURITY;
ALTER TABLE phoenix_fleet_health_evidence ENABLE ROW LEVEL SECURITY; ALTER TABLE phoenix_fleet_health_evidence FORCE ROW LEVEL SECURITY;
ALTER TABLE phoenix_fleet_events ENABLE ROW LEVEL SECURITY; ALTER TABLE phoenix_fleet_events FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_fleet_rollouts ON phoenix_fleet_rollouts;
CREATE POLICY tenant_isolation_fleet_rollouts ON phoenix_fleet_rollouts USING (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true)) WITH CHECK (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true));
DROP POLICY IF EXISTS tenant_isolation_fleet_nodes ON phoenix_fleet_nodes;
CREATE POLICY tenant_isolation_fleet_nodes ON phoenix_fleet_nodes USING (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true)) WITH CHECK (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true));
DROP POLICY IF EXISTS tenant_isolation_fleet_assignments ON phoenix_fleet_assignments;
CREATE POLICY tenant_isolation_fleet_assignments ON phoenix_fleet_assignments USING (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true)) WITH CHECK (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true));
DROP POLICY IF EXISTS tenant_isolation_fleet_health ON phoenix_fleet_health_evidence;
CREATE POLICY tenant_isolation_fleet_health ON phoenix_fleet_health_evidence USING (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true)) WITH CHECK (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true));
DROP POLICY IF EXISTS tenant_isolation_fleet_events ON phoenix_fleet_events;
CREATE POLICY tenant_isolation_fleet_events ON phoenix_fleet_events USING (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true)) WITH CHECK (tenant_uuid::text = current_setting('phxclaw.tenant_uuid', true));

CREATE OR REPLACE FUNCTION phoenix_append_only_guard() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only table'; END $$;
DROP TRIGGER IF EXISTS fleet_health_append_only ON phoenix_fleet_health_evidence;
CREATE TRIGGER fleet_health_append_only BEFORE UPDATE OR DELETE ON phoenix_fleet_health_evidence FOR EACH ROW EXECUTE FUNCTION phoenix_append_only_guard();
DROP TRIGGER IF EXISTS fleet_events_append_only ON phoenix_fleet_events;
CREATE TRIGGER fleet_events_append_only BEFORE UPDATE OR DELETE ON phoenix_fleet_events FOR EACH ROW EXECUTE FUNCTION phoenix_append_only_guard();

CREATE OR REPLACE FUNCTION phoenix_fleet_transition(
  p_tenant_uuid uuid,
  p_rollout_uuid uuid,
  p_expected_fencing_token bigint,
  p_new_state text,
  p_new_stage_index integer
) RETURNS bigint
LANGUAGE plpgsql
SECURITY INVOKER
AS $$
DECLARE v_new_fencing bigint;
BEGIN
  IF p_tenant_uuid::text IS DISTINCT FROM current_setting('phxclaw.tenant_uuid', true) THEN
    RAISE EXCEPTION 'tenant context mismatch';
  END IF;
  UPDATE phoenix_fleet_rollouts
     SET state=p_new_state,
         stage_index=p_new_stage_index,
         generation=generation+1,
         fencing_token=fencing_token+1,
         updated_at=now()
   WHERE tenant_uuid=p_tenant_uuid
     AND rollout_uuid=p_rollout_uuid
     AND fencing_token=p_expected_fencing_token
  RETURNING fencing_token INTO v_new_fencing;
  IF v_new_fencing IS NULL THEN RAISE EXCEPTION 'stale fleet fencing token'; END IF;
  RETURN v_new_fencing;
END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (28,'0028_secure_fleet_rollout.sql','3ef15fc146f3629a44a83f2c0f56305bec95b03bb6a898189f7a6e7c01c0eecf','0d34a3a9049013a4491afdfee555b49fdac5209e6757463a0d5d6ada007ed578','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0029: 0029_fleet_control_plane.sql
-- source_sha256:    574f924fbb0f82026233b4e1314fe32eed80cf0f02207366df8781d494754101
-- effective_sha256: 89946a43a7322e72bbc807a5c10dde716d005b1cafaf52c4c8b70c6d08b88708
-- source_status:    original
-- ============================================================================
CREATE SCHEMA IF NOT EXISTS phxclaw;

CREATE TABLE IF NOT EXISTS phxclaw.fleet_control_nodes (
  tenant_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL,
  fleet_uuid uuid NOT NULL,
  region text NOT NULL CHECK(length(region) BETWEEN 1 AND 128),
  control_state text NOT NULL CHECK(control_state IN ('online','stale','offline','recovering','maintenance','quarantined','revoked')),
  current_version text NOT NULL,
  current_sequence bigint NOT NULL CHECK(current_sequence >= 0),
  session_fencing_token bigint NOT NULL CHECK(session_fencing_token >= 0),
  last_heartbeat_at timestamptz,
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid,node_uuid),
  FOREIGN KEY (tenant_uuid,node_uuid) REFERENCES phxclaw.device_nodes(tenant_uuid,node_uuid) ON DELETE CASCADE,
  FOREIGN KEY (tenant_uuid,node_uuid) REFERENCES phoenix_fleet_nodes(tenant_uuid,node_uuid) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_control_groups (
  tenant_uuid uuid NOT NULL, group_uuid uuid NOT NULL, fleet_uuid uuid NOT NULL,
  name text NOT NULL CHECK(length(name) BETWEEN 1 AND 128), created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(tenant_uuid,group_uuid), UNIQUE(tenant_uuid,fleet_uuid,name)
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_control_group_members (
  tenant_uuid uuid NOT NULL, group_uuid uuid NOT NULL, node_uuid uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(tenant_uuid,group_uuid,node_uuid),
  FOREIGN KEY(tenant_uuid,group_uuid) REFERENCES phxclaw.fleet_control_groups(tenant_uuid,group_uuid) ON DELETE CASCADE,
  FOREIGN KEY(tenant_uuid,node_uuid) REFERENCES phxclaw.fleet_control_nodes(tenant_uuid,node_uuid) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_control_tags (
  tenant_uuid uuid NOT NULL, node_uuid uuid NOT NULL, tag text NOT NULL CHECK(length(tag) BETWEEN 1 AND 128), created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(tenant_uuid,node_uuid,tag),
  FOREIGN KEY(tenant_uuid,node_uuid) REFERENCES phxclaw.fleet_control_nodes(tenant_uuid,node_uuid) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_selector_snapshots (
  tenant_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL, fleet_uuid uuid NOT NULL,
  selector_sha256 char(64) NOT NULL CHECK(selector_sha256 ~ '^[0-9a-f]{64}$'),
  membership_sha256 char(64) NOT NULL CHECK(membership_sha256 ~ '^[0-9a-f]{64}$'),
  member_count integer NOT NULL CHECK(member_count >= 0), selector jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(tenant_uuid,snapshot_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_maintenance_windows (
  tenant_uuid uuid NOT NULL, maintenance_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL,
  starts_at timestamptz NOT NULL, ends_at timestamptz NOT NULL, reason text NOT NULL, approval_uuid uuid,
  created_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(tenant_uuid,maintenance_uuid), CHECK(ends_at>starts_at),
  FOREIGN KEY(tenant_uuid,snapshot_uuid) REFERENCES phxclaw.fleet_selector_snapshots(tenant_uuid,snapshot_uuid) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_remote_command_plans (
  tenant_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL, capability text NOT NULL,
  arguments_sha256 char(64) NOT NULL CHECK(arguments_sha256 ~ '^[0-9a-f]{64}$'), risk text NOT NULL CHECK(risk IN ('low','medium','high','critical')),
  approval_uuid uuid, max_parallel integer NOT NULL CHECK(max_parallel BETWEEN 1 AND 100), expires_at timestamptz NOT NULL,
  state text NOT NULL CHECK(state IN ('draft','approved','dispatching','completed','cancelled','expired','failed')),
  generation bigint NOT NULL DEFAULT 1 CHECK(generation>0), fencing_token bigint NOT NULL DEFAULT 1 CHECK(fencing_token>0),
  created_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(tenant_uuid,plan_uuid),
  FOREIGN KEY(tenant_uuid,snapshot_uuid) REFERENCES phxclaw.fleet_selector_snapshots(tenant_uuid,snapshot_uuid) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_remote_assignments (
  tenant_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, node_uuid uuid NOT NULL, command_uuid uuid,
  state text NOT NULL CHECK(state IN ('pending','submitted','claimed','succeeded','failed','cancelled','expired')),
  updated_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(tenant_uuid,plan_uuid,node_uuid),
  FOREIGN KEY(tenant_uuid,plan_uuid) REFERENCES phxclaw.fleet_remote_command_plans(tenant_uuid,plan_uuid) ON DELETE CASCADE,
  FOREIGN KEY(tenant_uuid,node_uuid) REFERENCES phxclaw.fleet_control_nodes(tenant_uuid,node_uuid) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_recovery_evidence (
  tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, node_uuid uuid NOT NULL,
  previous_sequence bigint NOT NULL CHECK(previous_sequence>=0), claimed_sequence bigint NOT NULL CHECK(claimed_sequence>=0),
  expected_fencing_token bigint NOT NULL CHECK(expected_fencing_token>=0), received_fencing_token bigint NOT NULL CHECK(received_fencing_token>=0),
  evidence_sha256 char(64) NOT NULL CHECK(evidence_sha256 ~ '^[0-9a-f]{64}$'), payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(tenant_uuid,evidence_uuid), FOREIGN KEY(tenant_uuid,node_uuid) REFERENCES phxclaw.fleet_control_nodes(tenant_uuid,node_uuid) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_control_leases (
  tenant_uuid uuid NOT NULL, fleet_uuid uuid NOT NULL, controller_uuid uuid NOT NULL,
  generation bigint NOT NULL DEFAULT 1 CHECK(generation>0), fencing_token bigint NOT NULL DEFAULT 1 CHECK(fencing_token>0),
  lease_expires_at timestamptz NOT NULL, updated_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(tenant_uuid,fleet_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw.fleet_control_events (
  tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, fleet_uuid uuid NOT NULL, node_uuid uuid,
  event_type text NOT NULL, event_sha256 char(64) NOT NULL CHECK(event_sha256 ~ '^[0-9a-f]{64}$'), payload jsonb NOT NULL,
  correlation_uuid uuid, causation_uuid uuid, recorded_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(tenant_uuid,event_uuid)
);

DO $$ DECLARE t text; BEGIN
  FOREACH t IN ARRAY ARRAY['fleet_control_nodes','fleet_control_groups','fleet_control_group_members','fleet_control_tags','fleet_selector_snapshots','fleet_maintenance_windows','fleet_remote_command_plans','fleet_remote_assignments','fleet_recovery_evidence','fleet_control_leases','fleet_control_events'] LOOP
    EXECUTE format('ALTER TABLE phxclaw.%I ENABLE ROW LEVEL SECURITY',t);
    EXECUTE format('ALTER TABLE phxclaw.%I FORCE ROW LEVEL SECURITY',t);
    EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON phxclaw.%I',t);
    EXECUTE format('CREATE POLICY tenant_isolation ON phxclaw.%I USING (tenant_uuid::text = current_setting(''phxclaw.tenant_uuid'', true)) WITH CHECK (tenant_uuid::text = current_setting(''phxclaw.tenant_uuid'', true))',t);
  END LOOP;
END $$;

DROP TRIGGER IF EXISTS fleet_selector_snapshot_append_only ON phxclaw.fleet_selector_snapshots;
CREATE TRIGGER fleet_selector_snapshot_append_only BEFORE UPDATE OR DELETE ON phxclaw.fleet_selector_snapshots FOR EACH ROW EXECUTE FUNCTION phoenix_append_only_guard();
DROP TRIGGER IF EXISTS fleet_recovery_evidence_append_only ON phxclaw.fleet_recovery_evidence;
CREATE TRIGGER fleet_recovery_evidence_append_only BEFORE UPDATE OR DELETE ON phxclaw.fleet_recovery_evidence FOR EACH ROW EXECUTE FUNCTION phoenix_append_only_guard();
DROP TRIGGER IF EXISTS fleet_control_events_append_only ON phxclaw.fleet_control_events;
CREATE TRIGGER fleet_control_events_append_only BEFORE UPDATE OR DELETE ON phxclaw.fleet_control_events FOR EACH ROW EXECUTE FUNCTION phoenix_append_only_guard();

CREATE OR REPLACE FUNCTION phoenix_fleet_control_claim_lease(
  p_tenant_uuid uuid,p_fleet_uuid uuid,p_controller_uuid uuid,p_expected_fencing_token bigint,p_lease_seconds integer
) RETURNS bigint LANGUAGE plpgsql SECURITY INVOKER AS $$
DECLARE v_token bigint;
BEGIN
  IF p_tenant_uuid::text IS DISTINCT FROM current_setting('phxclaw.tenant_uuid',true) THEN RAISE EXCEPTION 'tenant context mismatch'; END IF;
  IF p_lease_seconds < 5 OR p_lease_seconds > 300 THEN RAISE EXCEPTION 'invalid lease duration'; END IF;
  UPDATE phxclaw.fleet_control_leases SET controller_uuid=p_controller_uuid,generation=generation+1,fencing_token=fencing_token+1,
    lease_expires_at=now()+make_interval(secs=>p_lease_seconds),updated_at=now()
  WHERE tenant_uuid=p_tenant_uuid AND fleet_uuid=p_fleet_uuid AND fencing_token=p_expected_fencing_token
    AND (controller_uuid=p_controller_uuid OR lease_expires_at<=now()) RETURNING fencing_token INTO v_token;
  IF v_token IS NULL THEN RAISE EXCEPTION 'stale or busy fleet-control fencing token'; END IF;
  RETURN v_token;
END $$;

CREATE OR REPLACE FUNCTION phoenix_fleet_remote_plan_transition(
  p_tenant_uuid uuid,p_plan_uuid uuid,p_expected_fencing_token bigint,p_new_state text
) RETURNS bigint LANGUAGE plpgsql SECURITY INVOKER AS $$
DECLARE v_token bigint;
BEGIN
  IF p_tenant_uuid::text IS DISTINCT FROM current_setting('phxclaw.tenant_uuid',true) THEN RAISE EXCEPTION 'tenant context mismatch'; END IF;
  UPDATE phxclaw.fleet_remote_command_plans SET state=p_new_state,generation=generation+1,fencing_token=fencing_token+1
   WHERE tenant_uuid=p_tenant_uuid AND plan_uuid=p_plan_uuid AND fencing_token=p_expected_fencing_token RETURNING fencing_token INTO v_token;
  IF v_token IS NULL THEN RAISE EXCEPTION 'stale remote-plan fencing token'; END IF;
  RETURN v_token;
END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (29,'0029_fleet_control_plane.sql','574f924fbb0f82026233b4e1314fe32eed80cf0f02207366df8781d494754101','89946a43a7322e72bbc807a5c10dde716d005b1cafaf52c4c8b70c6d08b88708','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0030: 0030_distributed_control_plane.sql
-- source_sha256:    a1e461d5140a37763f2376b41e11ebc97e9ca797e54559e2852484f9149d6a09
-- effective_sha256: 7b6e7efc52304c0b7347adfcb95a0fc43a153bd6f03f3b2a52984ec996d2dac1
-- source_status:    original
-- repair_notes:     reparo sintático current_setting/NULLIF: 2 ocorrência(s)
-- ============================================================================
CREATE TABLE IF NOT EXISTS phxclaw_controller_members (
 tenant_uuid uuid NOT NULL,
 controller_uuid uuid NOT NULL,
 region text NOT NULL CHECK (length(btrim(region)) > 0),
 state text NOT NULL CHECK (state IN ('joining','follower','leader','self_fenced','draining','offline')),
 build_sha256 text NOT NULL CHECK (build_sha256 ~ '^[0-9a-f]{64}$'),
 started_at timestamptz NOT NULL,
 last_heartbeat_at timestamptz NOT NULL,
 PRIMARY KEY (tenant_uuid, controller_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_controller_leases (
 tenant_uuid uuid PRIMARY KEY,
 lease_uuid uuid NOT NULL UNIQUE,
 holder_uuid uuid NOT NULL,
 epoch bigint NOT NULL CHECK (epoch > 0),
 acquired_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 expires_at timestamptz NOT NULL,
 FOREIGN KEY (tenant_uuid, holder_uuid) REFERENCES phxclaw_controller_members(tenant_uuid, controller_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_reconcile_intents (
 tenant_uuid uuid NOT NULL,
 intent_uuid uuid NOT NULL,
 controller_uuid uuid NOT NULL,
 leader_epoch bigint NOT NULL CHECK (leader_epoch > 0),
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 target_state_sha256 text NOT NULL CHECK (target_state_sha256 ~ '^[0-9a-f]{64}$'),
 status text NOT NULL CHECK (status IN ('planned','committed','rejected','superseded')),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 committed_at timestamptz,
 PRIMARY KEY (tenant_uuid, intent_uuid),
 FOREIGN KEY (tenant_uuid, controller_uuid) REFERENCES phxclaw_controller_members(tenant_uuid, controller_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_controller_events (
 tenant_uuid uuid NOT NULL,
 event_uuid uuid NOT NULL,
 controller_uuid uuid,
 event_type text NOT NULL,
 leader_epoch bigint,
 evidence_sha256 text CHECK (evidence_sha256 IS NULL OR evidence_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 payload jsonb NOT NULL DEFAULT '{}'::jsonb,
 PRIMARY KEY (tenant_uuid, event_uuid)
);

CREATE OR REPLACE FUNCTION phxclaw_claim_controller_leader(p_tenant uuid,p_controller uuid,p_lease uuid,p_seconds integer)
RETURNS TABLE(lease_uuid uuid,holder_uuid uuid,epoch bigint,expires_at timestamptz) LANGUAGE plpgsql AS $$
DECLARE n timestamptz := clock_timestamp(); current_row phxclaw_controller_leases%ROWTYPE;
BEGIN
 IF p_seconds < 5 OR p_seconds > 120 THEN RAISE EXCEPTION 'invalid lease ttl'; END IF;
 PERFORM 1 FROM phxclaw_controller_members WHERE tenant_uuid=p_tenant AND controller_uuid=p_controller FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'unknown controller'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended(p_tenant::text, 3030));
 SELECT * INTO current_row FROM phxclaw_controller_leases WHERE tenant_uuid=p_tenant FOR UPDATE;
 IF NOT FOUND THEN
   INSERT INTO phxclaw_controller_leases(tenant_uuid,lease_uuid,holder_uuid,epoch,acquired_at,expires_at)
   VALUES(p_tenant,p_lease,p_controller,1,n,n+make_interval(secs=>p_seconds));
 ELSIF current_row.holder_uuid=p_controller AND current_row.expires_at>n THEN
   NULL; -- active owner must use renew; do not create a new epoch silently
 ELSIF current_row.expires_at<=n THEN
   UPDATE phxclaw_controller_leases SET lease_uuid=p_lease,holder_uuid=p_controller,epoch=current_row.epoch+1,acquired_at=n,expires_at=n+make_interval(secs=>p_seconds)
   WHERE tenant_uuid=p_tenant AND epoch=current_row.epoch;
 ELSE
   RETURN;
 END IF;
 RETURN QUERY SELECT l.lease_uuid,l.holder_uuid,l.epoch,l.expires_at FROM phxclaw_controller_leases l WHERE l.tenant_uuid=p_tenant AND l.holder_uuid=p_controller AND l.expires_at>n;
END $$;

CREATE OR REPLACE FUNCTION phxclaw_renew_controller_leader(p_tenant uuid,p_controller uuid,p_epoch bigint,p_seconds integer)
RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE n timestamptz := clock_timestamp(); changed integer;
BEGIN
 IF p_seconds < 5 OR p_seconds > 120 THEN RAISE EXCEPTION 'invalid lease ttl'; END IF;
 UPDATE phxclaw_controller_leases SET expires_at=n+make_interval(secs=>p_seconds)
 WHERE tenant_uuid=p_tenant AND holder_uuid=p_controller AND epoch=p_epoch AND expires_at>n;
 GET DIAGNOSTICS changed = ROW_COUNT; RETURN changed=1;
END $$;

CREATE OR REPLACE FUNCTION phxclaw_release_controller_leader(p_tenant uuid,p_controller uuid,p_epoch bigint)
RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE changed integer;
BEGIN
 UPDATE phxclaw_controller_leases SET expires_at=clock_timestamp()
 WHERE tenant_uuid=p_tenant AND holder_uuid=p_controller AND epoch=p_epoch AND expires_at>clock_timestamp();
 GET DIAGNOSTICS changed = ROW_COUNT; RETURN changed=1;
END $$;

CREATE OR REPLACE FUNCTION phxclaw_assert_controller_fence(p_tenant uuid,p_controller uuid,p_epoch bigint)
RETURNS boolean LANGUAGE sql STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM phxclaw_controller_leases WHERE tenant_uuid=p_tenant AND holder_uuid=p_controller AND epoch=p_epoch AND expires_at>clock_timestamp())
$$;

CREATE OR REPLACE FUNCTION phxclaw_commit_reconcile_intent(p_tenant uuid,p_intent uuid,p_controller uuid,p_epoch bigint,p_source_sha256 text)
RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE changed integer;
BEGIN
 IF NOT phxclaw_assert_controller_fence(p_tenant,p_controller,p_epoch) THEN RETURN false; END IF;
 UPDATE phxclaw_reconcile_intents SET status='committed',committed_at=clock_timestamp()
 WHERE tenant_uuid=p_tenant AND intent_uuid=p_intent AND controller_uuid=p_controller AND leader_epoch=p_epoch AND source_state_sha256=p_source_sha256 AND status='planned';
 GET DIAGNOSTICS changed = ROW_COUNT; RETURN changed=1;
END $$;

CREATE OR REPLACE FUNCTION phxclaw_forbid_controller_event_mutation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'controller events are append-only'; END $$;
DROP TRIGGER IF EXISTS phxclaw_controller_events_append_only ON phxclaw_controller_events;
CREATE TRIGGER phxclaw_controller_events_append_only BEFORE UPDATE OR DELETE ON phxclaw_controller_events FOR EACH ROW EXECUTE FUNCTION phxclaw_forbid_controller_event_mutation();

DO $$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['phxclaw_controller_members','phxclaw_controller_leases','phxclaw_reconcile_intents','phxclaw_controller_events'] LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
  EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
  EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I', t);
  EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)', t);
 END LOOP;
END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (30,'0030_distributed_control_plane.sql','a1e461d5140a37763f2376b41e11ebc97e9ca797e54559e2852484f9149d6a09','7b6e7efc52304c0b7347adfcb95a0fc43a153bd6f03f3b2a52984ec996d2dac1','original','reparo sintático current_setting/NULLIF: 2 ocorrência(s)') ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0031: 0031_unified_ai_fabric.sql
-- source_sha256:    3ca6213c61d29f177b86185446c667a7f168f9d2827b6147577f199529b6ffa6
-- effective_sha256: 3ca6213c61d29f177b86185446c667a7f168f9d2827b6147577f199529b6ffa6
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.31 — Unified AI Fabric
CREATE TABLE IF NOT EXISTS phxclaw_ai_providers (
 tenant_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, provider_name text NOT NULL,
 location text NOT NULL CHECK (location IN ('local','cloud')), enabled boolean NOT NULL DEFAULT true,
 priority integer NOT NULL DEFAULT 100, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_uuid,provider_uuid), UNIQUE(tenant_uuid,provider_name)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_models (
 tenant_uuid uuid NOT NULL, model_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, model_id text NOT NULL,
 capabilities jsonb NOT NULL DEFAULT '[]'::jsonb, context_tokens bigint NOT NULL,
 input_cost_micro_usd_per_million bigint, output_cost_micro_usd_per_million bigint,
 data_controls jsonb NOT NULL DEFAULT '[]'::jsonb, catalog_observed_at timestamptz NOT NULL,
 catalog_ttl_seconds integer NOT NULL CHECK (catalog_ttl_seconds > 0),
 PRIMARY KEY(tenant_uuid,model_uuid), UNIQUE(tenant_uuid,provider_uuid,model_id),
 FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_health_observations (
 tenant_uuid uuid NOT NULL, observation_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, model_uuid uuid,
 healthy boolean NOT NULL, health_basis_points integer NOT NULL CHECK(health_basis_points BETWEEN 0 AND 10000),
 p95_latency_ms bigint, observed_at timestamptz NOT NULL DEFAULT clock_timestamp(), ttl_seconds integer NOT NULL CHECK(ttl_seconds>0),
 evidence_sha256 text NOT NULL CHECK(evidence_sha256 ~ '^[0-9a-f]{64}$'),
 PRIMARY KEY(tenant_uuid,observation_uuid),
 FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid),
 FOREIGN KEY(tenant_uuid,model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_route_decisions (
 tenant_uuid uuid NOT NULL, decision_uuid uuid NOT NULL, request_uuid uuid NOT NULL,
 prompt_sha256 text NOT NULL CHECK(prompt_sha256 ~ '^[0-9a-f]{64}$'), selected_provider_uuid uuid NOT NULL, selected_model_uuid uuid NOT NULL,
 decision_sha256 text NOT NULL CHECK(decision_sha256 ~ '^[0-9a-f]{64}$'), candidate_summary jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,decision_uuid), UNIQUE(tenant_uuid,request_uuid),
 FOREIGN KEY(tenant_uuid,selected_provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid),
 FOREIGN KEY(tenant_uuid,selected_model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_budget_accounts (
 tenant_uuid uuid NOT NULL, account_uuid uuid NOT NULL, limit_micro_usd bigint NOT NULL CHECK(limit_micro_usd>=0),
 reserved_micro_usd bigint NOT NULL DEFAULT 0 CHECK(reserved_micro_usd>=0), spent_micro_usd bigint NOT NULL DEFAULT 0 CHECK(spent_micro_usd>=0),
 version bigint NOT NULL DEFAULT 0, PRIMARY KEY(tenant_uuid,account_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_budget_reservations (
 tenant_uuid uuid NOT NULL, reservation_uuid uuid NOT NULL, account_uuid uuid NOT NULL, request_uuid uuid NOT NULL,
 reserved_micro_usd bigint NOT NULL CHECK(reserved_micro_usd>=0), settled_micro_usd bigint,
 state text NOT NULL CHECK(state IN ('reserved','settled','released')), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_uuid,reservation_uuid), UNIQUE(tenant_uuid,request_uuid),
 FOREIGN KEY(tenant_uuid,account_uuid) REFERENCES phxclaw_ai_budget_accounts(tenant_uuid,account_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_provider_circuits (
 tenant_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, state text NOT NULL CHECK(state IN ('closed','open','half_open')),
 consecutive_failures integer NOT NULL DEFAULT 0, opened_at timestamptz, version bigint NOT NULL DEFAULT 0,
 PRIMARY KEY(tenant_uuid,provider_uuid), FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_events (
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, kind text NOT NULL, object_uuid uuid, payload_sha256 text NOT NULL CHECK(payload_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,event_uuid)
);

DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['phxclaw_ai_providers','phxclaw_ai_models','phxclaw_ai_health_observations','phxclaw_ai_route_decisions','phxclaw_ai_budget_accounts','phxclaw_ai_budget_reservations','phxclaw_ai_provider_circuits','phxclaw_ai_events'] LOOP
 EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
 EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t);
 EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t);
 END LOOP; END $$;

CREATE OR REPLACE FUNCTION phxclaw_ai_no_update_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only table'; END $$;
DROP TRIGGER IF EXISTS ai_health_append_only ON phxclaw_ai_health_observations;
CREATE TRIGGER ai_health_append_only BEFORE UPDATE OR DELETE ON phxclaw_ai_health_observations FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_no_update_delete();
DROP TRIGGER IF EXISTS ai_route_append_only ON phxclaw_ai_route_decisions;
CREATE TRIGGER ai_route_append_only BEFORE UPDATE OR DELETE ON phxclaw_ai_route_decisions FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_no_update_delete();
DROP TRIGGER IF EXISTS ai_events_append_only ON phxclaw_ai_events;
CREATE TRIGGER ai_events_append_only BEFORE UPDATE OR DELETE ON phxclaw_ai_events FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_no_update_delete();

CREATE OR REPLACE FUNCTION phxclaw_ai_reserve_budget(p_tenant uuid,p_account uuid,p_reservation uuid,p_request uuid,p_amount bigint) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE a phxclaw_ai_budget_accounts%ROWTYPE; existing phxclaw_ai_budget_reservations%ROWTYPE; BEGIN
 IF p_amount < 0 THEN RETURN false; END IF;
 PERFORM set_config('phxclaw.tenant_uuid',p_tenant::text,true);
 SELECT * INTO existing FROM phxclaw_ai_budget_reservations WHERE tenant_uuid=p_tenant AND request_uuid=p_request FOR UPDATE;
 IF FOUND THEN RETURN existing.account_uuid=p_account AND existing.reserved_micro_usd=p_amount AND existing.state IN ('reserved','settled'); END IF;
 SELECT * INTO a FROM phxclaw_ai_budget_accounts WHERE tenant_uuid=p_tenant AND account_uuid=p_account FOR UPDATE;
 IF NOT FOUND OR a.spent_micro_usd + a.reserved_micro_usd + p_amount > a.limit_micro_usd THEN RETURN false; END IF;
 INSERT INTO phxclaw_ai_budget_reservations(tenant_uuid,reservation_uuid,account_uuid,request_uuid,reserved_micro_usd,state) VALUES(p_tenant,p_reservation,p_account,p_request,p_amount,'reserved');
 UPDATE phxclaw_ai_budget_accounts SET reserved_micro_usd=reserved_micro_usd+p_amount, version=version+1 WHERE tenant_uuid=p_tenant AND account_uuid=p_account;
 RETURN true; END $$;

CREATE OR REPLACE FUNCTION phxclaw_ai_settle_budget(p_tenant uuid,p_request uuid,p_actual bigint) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE r phxclaw_ai_budget_reservations%ROWTYPE; BEGIN
 IF p_actual < 0 THEN RETURN false; END IF; PERFORM set_config('phxclaw.tenant_uuid',p_tenant::text,true);
 SELECT * INTO r FROM phxclaw_ai_budget_reservations WHERE tenant_uuid=p_tenant AND request_uuid=p_request FOR UPDATE;
 IF NOT FOUND OR p_actual > r.reserved_micro_usd THEN RETURN false; END IF;
 IF r.state='settled' THEN RETURN r.settled_micro_usd=p_actual; END IF;
 IF r.state <> 'reserved' THEN RETURN false; END IF;
 UPDATE phxclaw_ai_budget_reservations SET settled_micro_usd=p_actual,state='settled' WHERE tenant_uuid=p_tenant AND reservation_uuid=r.reservation_uuid;
 UPDATE phxclaw_ai_budget_accounts SET reserved_micro_usd=reserved_micro_usd-r.reserved_micro_usd, spent_micro_usd=spent_micro_usd+p_actual, version=version+1 WHERE tenant_uuid=p_tenant AND account_uuid=r.account_uuid;
 RETURN true; END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (31,'0031_unified_ai_fabric.sql','3ca6213c61d29f177b86185446c667a7f168f9d2827b6147577f199529b6ffa6','3ca6213c61d29f177b86185446c667a7f168f9d2827b6147577f199529b6ffa6','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0032: 0032_ai_benchmark_adaptive.sql
-- source_sha256:    d45c3441699300a01508293e60ff2795155bb441339e1b23be3b522101dbad85
-- effective_sha256: d45c3441699300a01508293e60ff2795155bb441339e1b23be3b522101dbad85
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.32 — AI Benchmark & Adaptive Model Intelligence
CREATE TABLE IF NOT EXISTS phxclaw_ai_benchmark_suites (
 tenant_uuid uuid NOT NULL, suite_uuid uuid NOT NULL, name text NOT NULL, suite_version text NOT NULL,
 task_family text NOT NULL, complexity text NOT NULL CHECK(complexity IN ('low','medium','high','extreme')), evidence_class text NOT NULL CHECK(evidence_class IN ('production','fixture')),
 dataset_sha256 text NOT NULL CHECK(dataset_sha256 ~ '^[0-9a-f]{64}$'), scorer_sha256 text NOT NULL CHECK(scorer_sha256 ~ '^[0-9a-f]{64}$'),
 environment_sha256 text NOT NULL CHECK(environment_sha256 ~ '^[0-9a-f]{64}$'), case_count integer NOT NULL CHECK(case_count>0),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,suite_uuid), UNIQUE(tenant_uuid,name,suite_version)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_benchmark_cases (
 tenant_uuid uuid NOT NULL, suite_uuid uuid NOT NULL, case_uuid uuid NOT NULL,
 prompt_sha256 text NOT NULL CHECK(prompt_sha256 ~ '^[0-9a-f]{64}$'), expected_contract_sha256 text NOT NULL CHECK(expected_contract_sha256 ~ '^[0-9a-f]{64}$'),
 tags jsonb NOT NULL DEFAULT '[]'::jsonb, weight integer NOT NULL CHECK(weight>0),
 PRIMARY KEY(tenant_uuid,suite_uuid,case_uuid), FOREIGN KEY(tenant_uuid,suite_uuid) REFERENCES phxclaw_ai_benchmark_suites(tenant_uuid,suite_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_benchmark_runs (
 tenant_uuid uuid NOT NULL, run_uuid uuid NOT NULL, suite_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, model_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK(source_state_sha256 ~ '^[0-9a-f]{64}$'), started_at timestamptz NOT NULL DEFAULT clock_timestamp(), finished_at timestamptz,
 state text NOT NULL CHECK(state IN ('planned','running','completed','failed','cancelled')), PRIMARY KEY(tenant_uuid,run_uuid),
 FOREIGN KEY(tenant_uuid,suite_uuid) REFERENCES phxclaw_ai_benchmark_suites(tenant_uuid,suite_uuid),
 FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid), FOREIGN KEY(tenant_uuid,model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_benchmark_observations (
 tenant_uuid uuid NOT NULL, observation_uuid uuid NOT NULL, run_uuid uuid NOT NULL, suite_uuid uuid NOT NULL, case_uuid uuid NOT NULL,
 provider_uuid uuid NOT NULL, model_uuid uuid NOT NULL, dataset_sha256 text NOT NULL CHECK(dataset_sha256 ~ '^[0-9a-f]{64}$'),
 scorer_sha256 text NOT NULL CHECK(scorer_sha256 ~ '^[0-9a-f]{64}$'), environment_sha256 text NOT NULL CHECK(environment_sha256 ~ '^[0-9a-f]{64}$'),
 success boolean NOT NULL, quality_basis_points integer NOT NULL CHECK(quality_basis_points BETWEEN 0 AND 10000),
 tool_accuracy_basis_points integer CHECK(tool_accuracy_basis_points BETWEEN 0 AND 10000), structured_validity_basis_points integer CHECK(structured_validity_basis_points BETWEEN 0 AND 10000),
 latency_ms bigint NOT NULL CHECK(latency_ms>=0), input_tokens bigint NOT NULL CHECK(input_tokens>=0), output_tokens bigint NOT NULL CHECK(output_tokens>=0),
 actual_cost_micro_usd bigint CHECK(actual_cost_micro_usd>=0), output_sha256 text NOT NULL CHECK(output_sha256 ~ '^[0-9a-f]{64}$'), error_class text,
 observed_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,observation_uuid), UNIQUE(tenant_uuid,run_uuid,case_uuid),
 FOREIGN KEY(tenant_uuid,run_uuid) REFERENCES phxclaw_ai_benchmark_runs(tenant_uuid,run_uuid),
 FOREIGN KEY(tenant_uuid,suite_uuid,case_uuid) REFERENCES phxclaw_ai_benchmark_cases(tenant_uuid,suite_uuid,case_uuid),
 FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid), FOREIGN KEY(tenant_uuid,model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_performance_profiles (
 tenant_uuid uuid NOT NULL, profile_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, model_uuid uuid NOT NULL, suite_uuid uuid NOT NULL,
 task_family text NOT NULL, complexity text NOT NULL CHECK(complexity IN ('low','medium','high','extreme')), evidence_class text NOT NULL CHECK(evidence_class IN ('production','fixture')),
 dataset_sha256 text NOT NULL CHECK(dataset_sha256 ~ '^[0-9a-f]{64}$'), scorer_sha256 text NOT NULL CHECK(scorer_sha256 ~ '^[0-9a-f]{64}$'), environment_sha256 text NOT NULL CHECK(environment_sha256 ~ '^[0-9a-f]{64}$'),
 sample_count integer NOT NULL CHECK(sample_count>0), success_basis_points integer NOT NULL CHECK(success_basis_points BETWEEN 0 AND 10000), quality_basis_points integer NOT NULL CHECK(quality_basis_points BETWEEN 0 AND 10000),
 tool_accuracy_basis_points integer CHECK(tool_accuracy_basis_points BETWEEN 0 AND 10000), structured_validity_basis_points integer CHECK(structured_validity_basis_points BETWEEN 0 AND 10000),
 p95_latency_ms bigint NOT NULL CHECK(p95_latency_ms>=0), median_cost_micro_usd bigint CHECK(median_cost_micro_usd>=0), evidence_coverage_basis_points integer NOT NULL CHECK(evidence_coverage_basis_points BETWEEN 0 AND 10000),
 profile_sha256 text NOT NULL CHECK(profile_sha256 ~ '^[0-9a-f]{64}$'), observed_at timestamptz NOT NULL, ttl_seconds integer NOT NULL CHECK(ttl_seconds>0),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,profile_uuid), UNIQUE(tenant_uuid,provider_uuid,model_uuid,suite_uuid,profile_sha256),
 FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid), FOREIGN KEY(tenant_uuid,model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid),
 FOREIGN KEY(tenant_uuid,suite_uuid) REFERENCES phxclaw_ai_benchmark_suites(tenant_uuid,suite_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_profile_promotions (
 tenant_uuid uuid NOT NULL, promotion_uuid uuid NOT NULL, profile_uuid uuid NOT NULL, previous_profile_uuid uuid,
 policy_sha256 text NOT NULL CHECK(policy_sha256 ~ '^[0-9a-f]{64}$'), approved_by text NOT NULL, promoted_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_uuid,promotion_uuid), FOREIGN KEY(tenant_uuid,profile_uuid) REFERENCES phxclaw_ai_performance_profiles(tenant_uuid,profile_uuid),
 FOREIGN KEY(tenant_uuid,previous_profile_uuid) REFERENCES phxclaw_ai_performance_profiles(tenant_uuid,profile_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_adaptive_route_evidence (
 tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, request_uuid uuid NOT NULL, base_decision_sha256 text NOT NULL CHECK(base_decision_sha256 ~ '^[0-9a-f]{64}$'),
 adaptive_decision_sha256 text NOT NULL CHECK(adaptive_decision_sha256 ~ '^[0-9a-f]{64}$'), selected_provider_uuid uuid NOT NULL, selected_model_uuid uuid NOT NULL,
 task_family text NOT NULL, complexity text NOT NULL, profile_hashes jsonb NOT NULL DEFAULT '[]'::jsonb, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_uuid,evidence_uuid), UNIQUE(tenant_uuid,request_uuid), FOREIGN KEY(tenant_uuid,selected_provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid),
 FOREIGN KEY(tenant_uuid,selected_model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid)
);

DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY[
 'phxclaw_ai_benchmark_suites','phxclaw_ai_benchmark_cases','phxclaw_ai_benchmark_runs','phxclaw_ai_benchmark_observations',
 'phxclaw_ai_performance_profiles','phxclaw_ai_profile_promotions','phxclaw_ai_adaptive_route_evidence'
] LOOP
 EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
 EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t);
 EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t);
END LOOP; END $$;

CREATE OR REPLACE FUNCTION phxclaw_ai_benchmark_no_mutation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'benchmark evidence is append-only'; END $$;
DROP TRIGGER IF EXISTS ai_benchmark_observation_append_only ON phxclaw_ai_benchmark_observations;
CREATE TRIGGER ai_benchmark_observation_append_only BEFORE UPDATE OR DELETE ON phxclaw_ai_benchmark_observations FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_benchmark_no_mutation();
DROP TRIGGER IF EXISTS ai_profile_append_only ON phxclaw_ai_performance_profiles;
CREATE TRIGGER ai_profile_append_only BEFORE UPDATE OR DELETE ON phxclaw_ai_performance_profiles FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_benchmark_no_mutation();
DROP TRIGGER IF EXISTS ai_profile_promotion_append_only ON phxclaw_ai_profile_promotions;
CREATE TRIGGER ai_profile_promotion_append_only BEFORE UPDATE OR DELETE ON phxclaw_ai_profile_promotions FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_benchmark_no_mutation();
DROP TRIGGER IF EXISTS ai_adaptive_evidence_append_only ON phxclaw_ai_adaptive_route_evidence;
CREATE TRIGGER ai_adaptive_evidence_append_only BEFORE UPDATE OR DELETE ON phxclaw_ai_adaptive_route_evidence FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_benchmark_no_mutation();

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (32,'0032_ai_benchmark_adaptive.sql','d45c3441699300a01508293e60ff2795155bb441339e1b23be3b522101dbad85','d45c3441699300a01508293e60ff2795155bb441339e1b23be3b522101dbad85','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0033: 0033_continuous_model_arena.sql
-- source_sha256:    394b4f1286f57721bec497d44fc3ad5d8424a22826ebadd1279167967d860def
-- effective_sha256: 394b4f1286f57721bec497d44fc3ad5d8424a22826ebadd1279167967d860def
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.33 — Continuous Model Arena + Drift
CREATE TABLE IF NOT EXISTS phxclaw_ai_arenas (
 tenant_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, name text NOT NULL, mode text NOT NULL CHECK(mode IN ('shadow','canary','paired')),
 task_family text NOT NULL, complexity text NOT NULL CHECK(complexity IN ('low','medium','high','extreme')), evidence_class text NOT NULL CHECK(evidence_class IN ('production','fixture')),
 champion_provider_uuid uuid NOT NULL, champion_model_uuid uuid NOT NULL, challenger_traffic_basis_points integer NOT NULL CHECK(challenger_traffic_basis_points BETWEEN 0 AND 10000),
 assignment_salt_sha256 text NOT NULL CHECK(assignment_salt_sha256 ~ '^[0-9a-f]{64}$'), policy_sha256 text NOT NULL CHECK(policy_sha256 ~ '^[0-9a-f]{64}$'), document_sha256 text NOT NULL CHECK(document_sha256 ~ '^[0-9a-f]{64}$'),
 signer_id text NOT NULL, initial_state text NOT NULL DEFAULT 'draft' CHECK(initial_state='draft'), starts_at timestamptz NOT NULL, expires_at timestamptz NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,arena_uuid),
 FOREIGN KEY(tenant_uuid,champion_provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid), FOREIGN KEY(tenant_uuid,champion_model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_arena_state_events (
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, from_state text NOT NULL CHECK(from_state IN ('draft','active','paused','completed','cancelled')), to_state text NOT NULL CHECK(to_state IN ('draft','active','paused','completed','cancelled')),
 actor_id text NOT NULL, policy_sha256 text NOT NULL CHECK(policy_sha256 ~ '^[0-9a-f]{64}$'), event_sha256 text NOT NULL CHECK(event_sha256 ~ '^[0-9a-f]{64}$'), occurred_at timestamptz NOT NULL,
 PRIMARY KEY(tenant_uuid,event_uuid), FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_arena_challengers (
 tenant_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, model_uuid uuid NOT NULL, ordinal integer NOT NULL CHECK(ordinal>=0),
 PRIMARY KEY(tenant_uuid,arena_uuid,provider_uuid,model_uuid), UNIQUE(tenant_uuid,arena_uuid,ordinal),
 FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid), FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid), FOREIGN KEY(tenant_uuid,model_uuid) REFERENCES phxclaw_ai_models(tenant_uuid,model_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_arena_assignments (
 tenant_uuid uuid NOT NULL, assignment_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, request_uuid uuid NOT NULL, served_provider_uuid uuid NOT NULL, served_model_uuid uuid NOT NULL,
 challenger_provider_uuid uuid, challenger_model_uuid uuid, execute_shadow boolean NOT NULL, assignment_bucket integer NOT NULL CHECK(assignment_bucket BETWEEN 0 AND 9999), assignment_sha256 text NOT NULL CHECK(assignment_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,assignment_uuid), UNIQUE(tenant_uuid,arena_uuid,request_uuid), FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_arena_pair_observations (
 tenant_uuid uuid NOT NULL, observation_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, request_uuid uuid NOT NULL, case_uuid uuid NOT NULL, evidence_class text NOT NULL CHECK(evidence_class IN ('production','fixture')),
 champion_provider_uuid uuid NOT NULL, champion_model_uuid uuid NOT NULL, challenger_provider_uuid uuid NOT NULL, challenger_model_uuid uuid NOT NULL,
 champion_success boolean NOT NULL, challenger_success boolean NOT NULL, champion_quality_basis_points integer NOT NULL CHECK(champion_quality_basis_points BETWEEN 0 AND 10000), challenger_quality_basis_points integer NOT NULL CHECK(challenger_quality_basis_points BETWEEN 0 AND 10000),
 champion_latency_ms bigint NOT NULL CHECK(champion_latency_ms>=0), challenger_latency_ms bigint NOT NULL CHECK(challenger_latency_ms>=0), champion_cost_micro_usd bigint CHECK(champion_cost_micro_usd>=0), challenger_cost_micro_usd bigint CHECK(challenger_cost_micro_usd>=0),
 champion_safety_violation boolean NOT NULL, challenger_safety_violation boolean NOT NULL, champion_output_sha256 text NOT NULL CHECK(champion_output_sha256 ~ '^[0-9a-f]{64}$'), challenger_output_sha256 text NOT NULL CHECK(challenger_output_sha256 ~ '^[0-9a-f]{64}$'),
 dataset_sha256 text NOT NULL CHECK(dataset_sha256 ~ '^[0-9a-f]{64}$'), scorer_sha256 text NOT NULL CHECK(scorer_sha256 ~ '^[0-9a-f]{64}$'), environment_sha256 text NOT NULL CHECK(environment_sha256 ~ '^[0-9a-f]{64}$'), observed_at timestamptz NOT NULL,
 PRIMARY KEY(tenant_uuid,observation_uuid), UNIQUE(tenant_uuid,arena_uuid,request_uuid,case_uuid,challenger_provider_uuid,challenger_model_uuid), FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_arena_windows (
 tenant_uuid uuid NOT NULL, window_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, challenger_provider_uuid uuid NOT NULL, challenger_model_uuid uuid NOT NULL,
 sample_count integer NOT NULL CHECK(sample_count>=0), champion_success_basis_points integer NOT NULL CHECK(champion_success_basis_points BETWEEN 0 AND 10000), challenger_success_basis_points integer NOT NULL CHECK(challenger_success_basis_points BETWEEN 0 AND 10000),
 champion_quality_basis_points integer NOT NULL CHECK(champion_quality_basis_points BETWEEN 0 AND 10000), challenger_quality_basis_points integer NOT NULL CHECK(challenger_quality_basis_points BETWEEN 0 AND 10000), quality_delta_basis_points integer NOT NULL, success_delta_basis_points integer NOT NULL,
 latency_regression_basis_points integer NOT NULL, cost_regression_basis_points integer, safety_violations integer NOT NULL CHECK(safety_violations>=0), evidence_class text NOT NULL CHECK(evidence_class IN ('production','fixture')), window_sha256 text NOT NULL CHECK(window_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,window_uuid), FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_arena_verdicts (
 tenant_uuid uuid NOT NULL, verdict_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, window_uuid uuid NOT NULL, verdict text NOT NULL CHECK(verdict IN ('insufficient','continue','challenger_wins','challenger_regressed','pause_safety')),
 policy_sha256 text NOT NULL CHECK(policy_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,verdict_uuid), FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid), FOREIGN KEY(tenant_uuid,window_uuid) REFERENCES phxclaw_ai_arena_windows(tenant_uuid,window_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_arena_promotion_recommendations (
 tenant_uuid uuid NOT NULL, recommendation_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, challenger_provider_uuid uuid NOT NULL, challenger_model_uuid uuid NOT NULL,
 champion_provider_uuid uuid NOT NULL, champion_model_uuid uuid NOT NULL, policy_sha256 text NOT NULL CHECK(policy_sha256 ~ '^[0-9a-f]{64}$'), supporting_window_hashes jsonb NOT NULL,
 recommendation_sha256 text NOT NULL CHECK(recommendation_sha256 ~ '^[0-9a-f]{64}$'), requires_v032_promotion_gate boolean NOT NULL DEFAULT true CHECK(requires_v032_promotion_gate), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_uuid,recommendation_uuid), FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_drift_events (
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, arena_uuid uuid NOT NULL, baseline_profile_uuid uuid NOT NULL, observed_window_uuid uuid NOT NULL,
 severity text NOT NULL CHECK(severity IN ('none','warning','critical')), action text NOT NULL CHECK(action IN ('continue','pause_arena','fallback_base_router')), reason text NOT NULL,
 event_sha256 text NOT NULL CHECK(event_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,event_uuid),
 FOREIGN KEY(tenant_uuid,arena_uuid) REFERENCES phxclaw_ai_arenas(tenant_uuid,arena_uuid), FOREIGN KEY(tenant_uuid,baseline_profile_uuid) REFERENCES phxclaw_ai_performance_profiles(tenant_uuid,profile_uuid), FOREIGN KEY(tenant_uuid,observed_window_uuid) REFERENCES phxclaw_ai_arena_windows(tenant_uuid,window_uuid)
);
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY[
 'phxclaw_ai_arenas','phxclaw_ai_arena_state_events','phxclaw_ai_arena_challengers','phxclaw_ai_arena_assignments','phxclaw_ai_arena_pair_observations','phxclaw_ai_arena_windows','phxclaw_ai_arena_verdicts','phxclaw_ai_arena_promotion_recommendations','phxclaw_ai_drift_events'
] LOOP
 EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t); EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t);
 EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t);
END LOOP; END $$;
CREATE OR REPLACE FUNCTION phxclaw_ai_arena_no_mutation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'arena evidence is append-only'; END $$;
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['phxclaw_ai_arenas','phxclaw_ai_arena_state_events','phxclaw_ai_arena_challengers','phxclaw_ai_arena_assignments','phxclaw_ai_arena_pair_observations','phxclaw_ai_arena_windows','phxclaw_ai_arena_verdicts','phxclaw_ai_arena_promotion_recommendations','phxclaw_ai_drift_events'] LOOP
 EXECUTE format('DROP TRIGGER IF EXISTS arena_append_only ON %I',t); EXECUTE format('CREATE TRIGGER arena_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_arena_no_mutation()',t);
END LOOP; END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (33,'0033_continuous_model_arena.sql','394b4f1286f57721bec497d44fc3ad5d8424a22826ebadd1279167967d860def','394b4f1286f57721bec497d44fc3ad5d8424a22826ebadd1279167967d860def','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0034: 0034_ai_portfolio_manager.sql
-- source_sha256:    135603d3696bbbf9064ddd27762529ec64e95badf09bce29bc3c2953bf4e775d
-- effective_sha256: 135603d3696bbbf9064ddd27762529ec64e95badf09bce29bc3c2953bf4e775d
-- source_status:    reconstructed
-- repair_notes:     v0.34 AIPortfolioManager reconstruída; artefato original indisponível para materialização
-- ============================================================================
-- Reconstructed PhxClaw v0.34 AI portfolio/capacity persistence.
CREATE TABLE IF NOT EXISTS phxclaw_ai_portfolios(
 tenant_uuid uuid NOT NULL, portfolio_uuid uuid NOT NULL, policy_sha256 text NOT NULL CHECK(policy_sha256 ~ '^[0-9a-f]{64}$'), source_arena_sha256 text NOT NULL CHECK(source_arena_sha256 ~ '^[0-9a-f]{64}$'), document_sha256 text NOT NULL CHECK(document_sha256 ~ '^[0-9a-f]{64}$'), signer_id text NOT NULL, signature_hex text NOT NULL CHECK(signature_hex ~ '^[0-9a-f]{128}$'), starts_at timestamptz NOT NULL, expires_at timestamptz NOT NULL, portfolio_json jsonb NOT NULL, PRIMARY KEY(tenant_uuid,portfolio_uuid), UNIQUE(tenant_uuid,document_sha256), CHECK(starts_at<expires_at));
CREATE TABLE IF NOT EXISTS phxclaw_ai_portfolio_slots(tenant_uuid uuid NOT NULL,portfolio_uuid uuid NOT NULL,slot_uuid uuid NOT NULL,task_family text NOT NULL,complexity text NOT NULL,diversity text NOT NULL,PRIMARY KEY(tenant_uuid,portfolio_uuid,slot_uuid),FOREIGN KEY(tenant_uuid,portfolio_uuid) REFERENCES phxclaw_ai_portfolios(tenant_uuid,portfolio_uuid));
CREATE TABLE IF NOT EXISTS phxclaw_ai_portfolio_members(tenant_uuid uuid NOT NULL,portfolio_uuid uuid NOT NULL,slot_uuid uuid NOT NULL,provider_uuid uuid NOT NULL,model_id text NOT NULL,weight_basis_points integer NOT NULL CHECK(weight_basis_points BETWEEN 1 AND 10000),promoted_profile_sha256 text NOT NULL CHECK(promoted_profile_sha256 ~ '^[0-9a-f]{64}$'),PRIMARY KEY(tenant_uuid,portfolio_uuid,slot_uuid,provider_uuid,model_id),FOREIGN KEY(tenant_uuid,portfolio_uuid,slot_uuid) REFERENCES phxclaw_ai_portfolio_slots(tenant_uuid,portfolio_uuid,slot_uuid));
CREATE TABLE IF NOT EXISTS phxclaw_provider_capacity_snapshots(tenant_uuid uuid NOT NULL,snapshot_uuid uuid NOT NULL,provider_uuid uuid NOT NULL,document_sha256 text NOT NULL CHECK(document_sha256 ~ '^[0-9a-f]{64}$'),signer_id text NOT NULL,signature_hex text NOT NULL CHECK(signature_hex ~ '^[0-9a-f]{128}$'),capacity_json jsonb NOT NULL,observed_at timestamptz NOT NULL,expires_at timestamptz NOT NULL,reset_at timestamptz NOT NULL,PRIMARY KEY(tenant_uuid,snapshot_uuid));
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['phxclaw_ai_portfolios','phxclaw_ai_portfolio_slots','phxclaw_ai_portfolio_members','phxclaw_provider_capacity_snapshots'] LOOP EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t); EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t); EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t); END LOOP; END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (34,'0034_ai_portfolio_manager.sql','135603d3696bbbf9064ddd27762529ec64e95badf09bce29bc3c2953bf4e775d','135603d3696bbbf9064ddd27762529ec64e95badf09bce29bc3c2953bf4e775d','reconstructed','v0.34 AIPortfolioManager reconstruída; artefato original indisponível para materialização') ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0035: 0035_ai_sre_autopilot.sql
-- source_sha256:    34a41049aa66fdaa486c50777678336832973ed3000e2e794b6b9c10accd2986
-- effective_sha256: 1dc84852db03b98fd76b706c4e9b0e1ebb79114389c089ed6a75fff7086d924d
-- source_status:    original
-- ============================================================================
CREATE TABLE IF NOT EXISTS phxclaw_ai_sre_policies (
 tenant_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, service_name text NOT NULL CHECK(length(btrim(service_name))>0), portfolio_uuid uuid NOT NULL,
 document_sha256 text NOT NULL CHECK(document_sha256 ~ '^[0-9a-f]{64}$'), signer_id text NOT NULL, signature_hex text NOT NULL CHECK(signature_hex ~ '^[0-9a-f]{128}$'),
 policy_json jsonb NOT NULL, starts_at timestamptz NOT NULL, expires_at timestamptz NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(tenant_uuid,policy_uuid), UNIQUE(tenant_uuid,document_sha256), FOREIGN KEY(tenant_uuid,portfolio_uuid) REFERENCES phxclaw_ai_portfolios(tenant_uuid,portfolio_uuid), CHECK(starts_at<expires_at)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_slo_samples (
 tenant_uuid uuid NOT NULL, sample_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, success_count bigint NOT NULL CHECK(success_count>=0), total_count bigint NOT NULL CHECK(total_count>0 AND success_count<=total_count), p95_latency_ms bigint NOT NULL CHECK(p95_latency_ms>=0), p95_queue_wait_ms bigint NOT NULL CHECK(p95_queue_wait_ms>=0), observed_at timestamptz NOT NULL, window_seconds integer NOT NULL CHECK(window_seconds>0), evidence_sha256 text NOT NULL CHECK(evidence_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,sample_uuid), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_slo_evaluations (
 tenant_uuid uuid NOT NULL, evaluation_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, availability_basis_points integer NOT NULL CHECK(availability_basis_points BETWEEN 0 AND 10000), error_budget_burn_milli bigint NOT NULL CHECK(error_budget_burn_milli>=0), latency_violated boolean NOT NULL, queue_wait_violated boolean NOT NULL, source_evidence_hashes jsonb NOT NULL, evaluation_sha256 text NOT NULL CHECK(evaluation_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,evaluation_uuid), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_demand_samples (
 tenant_uuid uuid NOT NULL, sample_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, window_start timestamptz NOT NULL, window_seconds integer NOT NULL CHECK(window_seconds>0), requests bigint NOT NULL CHECK(requests>=0), tokens bigint NOT NULL CHECK(tokens>=0), peak_concurrency integer NOT NULL CHECK(peak_concurrency>=0), evidence_sha256 text NOT NULL CHECK(evidence_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,sample_uuid), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_demand_forecasts (
 tenant_uuid uuid NOT NULL, forecast_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, requests_per_minute bigint NOT NULL CHECK(requests_per_minute>=0), tokens_per_minute bigint NOT NULL CHECK(tokens_per_minute>=0), peak_concurrency integer NOT NULL CHECK(peak_concurrency>=0), recommended_concurrency integer NOT NULL CHECK(recommended_concurrency>0), horizon_seconds integer NOT NULL CHECK(horizon_seconds>0), source_evidence_hashes jsonb NOT NULL, forecast_sha256 text NOT NULL CHECK(forecast_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,forecast_uuid), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_rate_limit_snapshots (
 tenant_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, request_limit bigint NOT NULL CHECK(request_limit>0), requests_remaining bigint NOT NULL CHECK(requests_remaining>=0 AND requests_remaining<=request_limit), token_limit bigint NOT NULL CHECK(token_limit>0), tokens_remaining bigint NOT NULL CHECK(tokens_remaining>=0 AND tokens_remaining<=token_limit), reset_at timestamptz NOT NULL, observed_at timestamptz NOT NULL, ttl_seconds integer NOT NULL CHECK(ttl_seconds>0), snapshot_sha256 text NOT NULL CHECK(snapshot_sha256 ~ '^[0-9a-f]{64}$'), signer_id text NOT NULL, signature_hex text NOT NULL CHECK(signature_hex ~ '^[0-9a-f]{128}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,snapshot_uuid), FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_admission_decisions (
 tenant_uuid uuid NOT NULL, decision_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, request_uuid uuid NOT NULL, provider_uuid uuid NOT NULL, request_class text NOT NULL CHECK(request_class IN ('critical','interactive','batch','background')), action text NOT NULL CHECK(action IN ('admit','queue','reject')), priority_score bigint NOT NULL CHECK(priority_score>=0), rate_limit_snapshot_uuid uuid NOT NULL, decision_sha256 text NOT NULL CHECK(decision_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,decision_uuid), UNIQUE(tenant_uuid,request_uuid), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid), FOREIGN KEY(tenant_uuid,provider_uuid) REFERENCES phxclaw_ai_providers(tenant_uuid,provider_uuid), FOREIGN KEY(tenant_uuid,rate_limit_snapshot_uuid) REFERENCES phxclaw_ai_rate_limit_snapshots(tenant_uuid,snapshot_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_budget_snapshots (
 tenant_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL, account_uuid uuid NOT NULL, limit_micro_usd bigint NOT NULL CHECK(limit_micro_usd>=0), spent_micro_usd bigint NOT NULL CHECK(spent_micro_usd>=0), reserved_micro_usd bigint NOT NULL CHECK(reserved_micro_usd>=0), observed_at timestamptz NOT NULL, snapshot_sha256 text NOT NULL CHECK(snapshot_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,snapshot_uuid), FOREIGN KEY(tenant_uuid,account_uuid) REFERENCES phxclaw_ai_budget_accounts(tenant_uuid,account_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_cost_forecasts (
 tenant_uuid uuid NOT NULL, forecast_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, account_uuid uuid NOT NULL, budget_limit_micro_usd bigint NOT NULL CHECK(budget_limit_micro_usd>=0), current_committed_micro_usd bigint NOT NULL CHECK(current_committed_micro_usd>=0), projected_total_micro_usd bigint NOT NULL CHECK(projected_total_micro_usd>=0), projected_budget_basis_points integer NOT NULL CHECK(projected_budget_basis_points BETWEEN 0 AND 10000), budget_state text NOT NULL CHECK(budget_state IN ('healthy','soft_limit','hard_limit')), source_evidence_hashes jsonb NOT NULL, forecast_sha256 text NOT NULL CHECK(forecast_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,forecast_uuid), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid), FOREIGN KEY(tenant_uuid,account_uuid) REFERENCES phxclaw_ai_budget_accounts(tenant_uuid,account_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_incident_events (
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, incident_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, severity text NOT NULL CHECK(severity IN ('info','warning','critical')), event_type text NOT NULL CHECK(event_type IN ('detected','opened','updated','mitigated','resolved')), evidence_hashes jsonb NOT NULL, event_sha256 text NOT NULL CHECK(event_sha256 ~ '^[0-9a-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,event_uuid), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_autopilot_plans (
 tenant_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, policy_uuid uuid NOT NULL, portfolio_uuid uuid NOT NULL, source_state_sha256 text NOT NULL CHECK(source_state_sha256 ~ '^[0-9a-f]{64}$'), trigger_evidence_hashes jsonb NOT NULL, automatic_allowed boolean NOT NULL, requires_human_approval boolean NOT NULL, actions jsonb NOT NULL, document_sha256 text NOT NULL CHECK(document_sha256 ~ '^[0-9a-f]{64}$'), signer_id text NOT NULL, signature_hex text NOT NULL CHECK(signature_hex ~ '^[0-9a-f]{128}$'), created_at timestamptz NOT NULL, expires_at timestamptz NOT NULL, PRIMARY KEY(tenant_uuid,plan_uuid), UNIQUE(tenant_uuid,document_sha256), FOREIGN KEY(tenant_uuid,policy_uuid) REFERENCES phxclaw_ai_sre_policies(tenant_uuid,policy_uuid), FOREIGN KEY(tenant_uuid,portfolio_uuid) REFERENCES phxclaw_ai_portfolios(tenant_uuid,portfolio_uuid), CHECK(created_at<expires_at), CHECK(automatic_allowed <> requires_human_approval)
);
CREATE TABLE IF NOT EXISTS phxclaw_ai_autopilot_executions (
 tenant_uuid uuid NOT NULL, execution_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, controller_uuid uuid NOT NULL, leader_epoch bigint NOT NULL CHECK(leader_epoch>0), source_state_sha256 text NOT NULL CHECK(source_state_sha256 ~ '^[0-9a-f]{64}$'), result_state_sha256 text NOT NULL CHECK(result_state_sha256 ~ '^[0-9a-f]{64}$'), execution_sha256 text NOT NULL CHECK(execution_sha256 ~ '^[0-9a-f]{64}$'), executed_at timestamptz NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,execution_uuid), UNIQUE(tenant_uuid,plan_uuid), FOREIGN KEY(tenant_uuid,plan_uuid) REFERENCES phxclaw_ai_autopilot_plans(tenant_uuid,plan_uuid), FOREIGN KEY(tenant_uuid,controller_uuid) REFERENCES phxclaw_controller_members(tenant_uuid,controller_uuid)
);

CREATE OR REPLACE FUNCTION phxclaw_ai_autopilot_execute(
 p_tenant uuid,p_execution uuid,p_plan uuid,p_controller uuid,p_epoch bigint,p_source_sha text,p_result_sha text,p_execution_sha text
) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE p phxclaw_ai_autopilot_plans%ROWTYPE; e phxclaw_ai_autopilot_executions%ROWTYPE;
BEGIN
 IF p_source_sha !~ '^[0-9a-f]{64}$' OR p_result_sha !~ '^[0-9a-f]{64}$' OR p_execution_sha !~ '^[0-9a-f]{64}$' THEN RETURN false; END IF;
 IF NOT phxclaw_assert_controller_fence(p_tenant,p_controller,p_epoch) THEN RETURN false; END IF;
 SELECT * INTO p FROM phxclaw_ai_autopilot_plans WHERE tenant_uuid=p_tenant AND plan_uuid=p_plan;
 IF NOT FOUND OR NOT p.automatic_allowed OR p.requires_human_approval OR p.source_state_sha256<>p_source_sha OR p.expires_at<=clock_timestamp() THEN RETURN false; END IF;
 INSERT INTO phxclaw_ai_autopilot_executions(tenant_uuid,execution_uuid,plan_uuid,controller_uuid,leader_epoch,source_state_sha256,result_state_sha256,execution_sha256,executed_at)
 VALUES(p_tenant,p_execution,p_plan,p_controller,p_epoch,p_source_sha,p_result_sha,p_execution_sha,clock_timestamp()) ON CONFLICT (tenant_uuid,plan_uuid) DO NOTHING;
 SELECT * INTO e FROM phxclaw_ai_autopilot_executions WHERE tenant_uuid=p_tenant AND plan_uuid=p_plan;
 RETURN e.controller_uuid=p_controller AND e.leader_epoch=p_epoch AND e.source_state_sha256=p_source_sha AND e.result_state_sha256=p_result_sha AND e.execution_sha256=p_execution_sha;
END $$;

DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY[
 'phxclaw_ai_sre_policies','phxclaw_ai_slo_samples','phxclaw_ai_slo_evaluations','phxclaw_ai_demand_samples','phxclaw_ai_demand_forecasts','phxclaw_ai_rate_limit_snapshots','phxclaw_ai_admission_decisions','phxclaw_ai_budget_snapshots','phxclaw_ai_cost_forecasts','phxclaw_ai_incident_events','phxclaw_ai_autopilot_plans','phxclaw_ai_autopilot_executions'
] LOOP
 EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t); EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t);
 EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t);
END LOOP; END $$;

CREATE OR REPLACE FUNCTION phxclaw_ai_sre_no_mutation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'AI SRE evidence/configuration is append-only'; END $$;
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY[
 'phxclaw_ai_sre_policies','phxclaw_ai_slo_samples','phxclaw_ai_slo_evaluations','phxclaw_ai_demand_samples','phxclaw_ai_demand_forecasts','phxclaw_ai_rate_limit_snapshots','phxclaw_ai_admission_decisions','phxclaw_ai_budget_snapshots','phxclaw_ai_cost_forecasts','phxclaw_ai_incident_events','phxclaw_ai_autopilot_plans','phxclaw_ai_autopilot_executions'
] LOOP
 EXECUTE format('DROP TRIGGER IF EXISTS ai_sre_append_only ON %I',t); EXECUTE format('CREATE TRIGGER ai_sre_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phxclaw_ai_sre_no_mutation()',t);
END LOOP; END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (35,'0035_ai_sre_autopilot.sql','34a41049aa66fdaa486c50777678336832973ed3000e2e794b6b9c10accd2986','1dc84852db03b98fd76b706c4e9b0e1ebb79114389c089ed6a75fff7086d924d','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0036: 0036_incident_chaos_skill_router.sql
-- source_sha256:    4c2c38956526579425f470c45cd4fd8eb3ea30ecddef0213a42d90dbd17162a9
-- effective_sha256: 0aa4f7d72af8548fc5f25b38b4635cd771fc8c3badc841ece63b0787145f77aa
-- source_status:    original
-- ============================================================================

CREATE TABLE IF NOT EXISTS phx_skill_sources_v036 (
  tenant_uuid uuid NOT NULL,
  source_uuid uuid NOT NULL,
  skill_id text NOT NULL,
  source_url text NOT NULL,
  license_expression text NOT NULL,
  license_status text NOT NULL,
  source_sha256 text NOT NULL CHECK (source_sha256 ~ '^[0-9a-fA-F]{64}$'),
  observed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  supersedes_source_uuid uuid NULL,
  PRIMARY KEY (tenant_uuid, source_uuid),
  UNIQUE (tenant_uuid, skill_id, source_sha256)
);
CREATE TABLE IF NOT EXISTS phx_skill_routes_v036 (
  tenant_uuid uuid NOT NULL, route_uuid uuid NOT NULL, request_uuid uuid NOT NULL,
  catalog_sha256 text NOT NULL CHECK (catalog_sha256 ~ '^[0-9a-fA-F]{64}$'),
  plan_sha256 text NOT NULL CHECK (plan_sha256 ~ '^[0-9a-fA-F]{64}$'),
  plan_json jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, route_uuid), UNIQUE (tenant_uuid, request_uuid, plan_sha256)
);
CREATE TABLE IF NOT EXISTS phx_skill_route_evidence_v036 (
  tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, route_uuid uuid NOT NULL,
  skill_id text NOT NULL, input_sha256 text NOT NULL CHECK (input_sha256 ~ '^[0-9a-fA-F]{64}$'),
  output_sha256 text NOT NULL CHECK (output_sha256 ~ '^[0-9a-fA-F]{64}$'), evidence_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,evidence_uuid),
  FOREIGN KEY (tenant_uuid,route_uuid) REFERENCES phx_skill_routes_v036(tenant_uuid,route_uuid)
);
CREATE TABLE IF NOT EXISTS phx_incidents_v036 (
  tenant_uuid uuid NOT NULL, incident_uuid uuid NOT NULL, severity text NOT NULL,
  title text NOT NULL, detection_evidence_sha256 text NOT NULL CHECK (detection_evidence_sha256 ~ '^[0-9a-fA-F]{64}$'),
  declared_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,incident_uuid)
);
CREATE TABLE IF NOT EXISTS phx_incident_events_v036 (
  tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, incident_uuid uuid NOT NULL,
  event_type text NOT NULL, evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-fA-F]{64}$'), payload jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,event_uuid),
  FOREIGN KEY (tenant_uuid,incident_uuid) REFERENCES phx_incidents_v036(tenant_uuid,incident_uuid)
);
CREATE TABLE IF NOT EXISTS phx_incident_actions_v036 (
  tenant_uuid uuid NOT NULL, action_uuid uuid NOT NULL, incident_uuid uuid NOT NULL,
  capability text NOT NULL, approval_uuid uuid NULL, leader_epoch bigint NULL, fencing_token bigint NULL,
  input_sha256 text NOT NULL CHECK (input_sha256 ~ '^[0-9a-fA-F]{64}$'), status text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,action_uuid),
  FOREIGN KEY (tenant_uuid,incident_uuid) REFERENCES phx_incidents_v036(tenant_uuid,incident_uuid)
);
CREATE TABLE IF NOT EXISTS phx_resilience_runbooks_v036 (
  tenant_uuid uuid NOT NULL, runbook_uuid uuid NOT NULL, name text NOT NULL, version text NOT NULL,
  content_sha256 text NOT NULL CHECK (content_sha256 ~ '^[0-9a-fA-F]{64}$'), content_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,runbook_uuid), UNIQUE(tenant_uuid,name,version)
);
CREATE TABLE IF NOT EXISTS phx_chaos_experiments_v036 (
  tenant_uuid uuid NOT NULL, experiment_uuid uuid NOT NULL, environment text NOT NULL, fault_kind text NOT NULL,
  blast_radius_basis_points integer NOT NULL CHECK (blast_radius_basis_points BETWEEN 0 AND 10000),
  steady_state_sha256 text NOT NULL CHECK (steady_state_sha256 ~ '^[0-9a-fA-F]{64}$'),
  abort_conditions_sha256 text NOT NULL CHECK (abort_conditions_sha256 ~ '^[0-9a-fA-F]{64}$'),
  recovery_runbook_uuid uuid NOT NULL, approval_uuid uuid NULL, leader_epoch bigint NULL, fencing_token bigint NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,experiment_uuid),
  FOREIGN KEY (tenant_uuid,recovery_runbook_uuid) REFERENCES phx_resilience_runbooks_v036(tenant_uuid,runbook_uuid)
);
CREATE TABLE IF NOT EXISTS phx_chaos_events_v036 (
  tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, experiment_uuid uuid NOT NULL, event_type text NOT NULL,
  evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-fA-F]{64}$'), payload jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,event_uuid),
  FOREIGN KEY (tenant_uuid,experiment_uuid) REFERENCES phx_chaos_experiments_v036(tenant_uuid,experiment_uuid)
);
CREATE TABLE IF NOT EXISTS phx_recovery_evidence_v036 (
  tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, experiment_uuid uuid NOT NULL,
  before_sha256 text NOT NULL CHECK (before_sha256 ~ '^[0-9a-fA-F]{64}$'), after_sha256 text NOT NULL CHECK (after_sha256 ~ '^[0-9a-fA-F]{64}$'),
  steady_state_restored boolean NOT NULL, evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-fA-F]{64}$'),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,evidence_uuid),
  FOREIGN KEY (tenant_uuid,experiment_uuid) REFERENCES phx_chaos_experiments_v036(tenant_uuid,experiment_uuid)
);
CREATE TABLE IF NOT EXISTS phx_postmortems_v036 (
  tenant_uuid uuid NOT NULL, postmortem_uuid uuid NOT NULL, incident_uuid uuid NOT NULL,
  summary_sha256 text NOT NULL CHECK (summary_sha256 ~ '^[0-9a-fA-F]{64}$'), report_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,postmortem_uuid),
  FOREIGN KEY (tenant_uuid,incident_uuid) REFERENCES phx_incidents_v036(tenant_uuid,incident_uuid)
);

CREATE OR REPLACE FUNCTION phxclaw_v036_append_only() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'PhxClaw v0.36 evidence/event tables are append-only'; END $$;

DO $$ DECLARE t text; BEGIN
  FOREACH t IN ARRAY ARRAY['phx_skill_sources_v036','phx_skill_route_evidence_v036','phx_incident_events_v036','phx_chaos_events_v036','phx_recovery_evidence_v036','phx_postmortems_v036'] LOOP
    EXECUTE format('DROP TRIGGER IF EXISTS trg_v036_append_only ON %I',t);
    EXECUTE format('CREATE TRIGGER trg_v036_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phxclaw_v036_append_only()',t);
  END LOOP;
END $$;

DO $$ DECLARE t text; BEGIN
  FOREACH t IN ARRAY ARRAY['phx_skill_sources_v036','phx_skill_routes_v036','phx_skill_route_evidence_v036','phx_incidents_v036','phx_incident_events_v036','phx_incident_actions_v036','phx_resilience_runbooks_v036','phx_chaos_experiments_v036','phx_chaos_events_v036','phx_recovery_evidence_v036','phx_postmortems_v036'] LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
    EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_v036 ON %I',t);
    EXECUTE format($cmd$CREATE POLICY tenant_isolation_v036 ON %I USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)$cmd$,t);
  END LOOP;
END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (36,'0036_incident_chaos_skill_router.sql','4c2c38956526579425f470c45cd4fd8eb3ea30ecddef0213a42d90dbd17162a9','0aa4f7d72af8548fc5f25b38b4635cd771fc8c3badc841ece63b0787145f77aa','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0037: 0037_autonomous_engineering_workflow.sql
-- source_sha256:    6f1195ac6dec98c4cd13d822308ad00ae382d5f89e88e7f867adf9ef7d082566
-- effective_sha256: 0b5a319658f646c928865815166413637daac87af0b01945c0373f9514cae617
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.37 autonomous engineering workflow
CREATE TABLE IF NOT EXISTS engineering_workflows (
 tenant_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, objective text NOT NULL, source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9A-Fa-f]{64}$'), policy_sha256 text NOT NULL CHECK (policy_sha256 ~ '^[0-9A-Fa-f]{64}$'), skill_catalog_sha256 text NOT NULL CHECK (skill_catalog_sha256 ~ '^[0-9A-Fa-f]{64}$'), risk text NOT NULL CHECK (risk IN ('low','medium','high','critical')), spec jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid, workflow_uuid));
CREATE TABLE IF NOT EXISTS engineering_stage_runs (
 tenant_uuid uuid NOT NULL, run_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, stage text NOT NULL, attempt integer NOT NULL CHECK (attempt>0), idempotency_key text NOT NULL, actor_uuid uuid NOT NULL, started_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid, run_uuid), UNIQUE (tenant_uuid, workflow_uuid, stage, attempt), UNIQUE (tenant_uuid,idempotency_key), FOREIGN KEY (tenant_uuid,workflow_uuid) REFERENCES engineering_workflows(tenant_uuid,workflow_uuid));
CREATE TABLE IF NOT EXISTS engineering_stage_evidence (
 tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, run_uuid uuid NOT NULL, stage text NOT NULL, source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9A-Fa-f]{64}$'), input_sha256 text NOT NULL CHECK (input_sha256 ~ '^[0-9A-Fa-f]{64}$'), output_sha256 text NOT NULL CHECK (output_sha256 ~ '^[0-9A-Fa-f]{64}$'), evidence_refs jsonb NOT NULL DEFAULT '[]'::jsonb, actor_uuid uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,evidence_uuid), FOREIGN KEY (tenant_uuid,workflow_uuid) REFERENCES engineering_workflows(tenant_uuid,workflow_uuid), FOREIGN KEY (tenant_uuid,run_uuid) REFERENCES engineering_stage_runs(tenant_uuid,run_uuid));
CREATE TABLE IF NOT EXISTS engineering_checkpoints (
 tenant_uuid uuid NOT NULL, checkpoint_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, stage text NOT NULL CHECK (stage='implement'), source_state_sha256 text NOT NULL, workspace_state_sha256 text NOT NULL, git_commit text, fencing_token bigint NOT NULL CHECK (fencing_token>=0), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,checkpoint_uuid), FOREIGN KEY (tenant_uuid,workflow_uuid) REFERENCES engineering_workflows(tenant_uuid,workflow_uuid));
CREATE TABLE IF NOT EXISTS engineering_transition_events (
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, stage text NOT NULL, outcome text NOT NULL CHECK (outcome IN ('completed','failed','rolled_back','skipped')), attempt integer NOT NULL CHECK (attempt>0), evidence_uuid uuid, checkpoint_uuid uuid, idempotency_key text NOT NULL, fencing_token bigint NOT NULL CHECK (fencing_token>=0), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,event_uuid), UNIQUE (tenant_uuid,idempotency_key), FOREIGN KEY (tenant_uuid,workflow_uuid) REFERENCES engineering_workflows(tenant_uuid,workflow_uuid), FOREIGN KEY (tenant_uuid,evidence_uuid) REFERENCES engineering_stage_evidence(tenant_uuid,evidence_uuid), FOREIGN KEY (tenant_uuid,checkpoint_uuid) REFERENCES engineering_checkpoints(tenant_uuid,checkpoint_uuid));
CREATE TABLE IF NOT EXISTS engineering_approvals (
 tenant_uuid uuid NOT NULL, approval_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, stage text NOT NULL, approver_uuid uuid NOT NULL, actor_uuid uuid NOT NULL, plan_sha256 text NOT NULL CHECK (plan_sha256 ~ '^[0-9A-Fa-f]{64}$'), approved boolean NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,approval_uuid), FOREIGN KEY (tenant_uuid,workflow_uuid) REFERENCES engineering_workflows(tenant_uuid,workflow_uuid));
CREATE TABLE IF NOT EXISTS engineering_rollbacks (
 tenant_uuid uuid NOT NULL, rollback_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, checkpoint_uuid uuid NOT NULL, reason text NOT NULL, before_sha256 text NOT NULL, after_sha256 text NOT NULL, verified boolean NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,rollback_uuid), FOREIGN KEY (tenant_uuid,workflow_uuid) REFERENCES engineering_workflows(tenant_uuid,workflow_uuid), FOREIGN KEY (tenant_uuid,checkpoint_uuid) REFERENCES engineering_checkpoints(tenant_uuid,checkpoint_uuid));
CREATE TABLE IF NOT EXISTS engineering_delivery_intents (
 tenant_uuid uuid NOT NULL, intent_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, artifact_sha256 text NOT NULL CHECK (artifact_sha256 ~ '^[0-9A-Fa-f]{64}$'), destination_kind text NOT NULL, external_side_effect boolean NOT NULL DEFAULT false, approval_uuid uuid, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,intent_uuid), FOREIGN KEY (tenant_uuid,workflow_uuid) REFERENCES engineering_workflows(tenant_uuid,workflow_uuid), FOREIGN KEY (tenant_uuid,approval_uuid) REFERENCES engineering_approvals(tenant_uuid,approval_uuid));
CREATE OR REPLACE FUNCTION phxclaw_prevent_mutation_0037() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only table: %', TG_TABLE_NAME; END $$;
ALTER TABLE engineering_workflows ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_workflows FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_workflows;
CREATE POLICY tenant_isolation ON engineering_workflows USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
ALTER TABLE engineering_stage_runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_stage_runs FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_stage_runs;
CREATE POLICY tenant_isolation ON engineering_stage_runs USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
ALTER TABLE engineering_stage_evidence ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_stage_evidence FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_stage_evidence;
CREATE POLICY tenant_isolation ON engineering_stage_evidence USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
ALTER TABLE engineering_checkpoints ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_checkpoints FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_checkpoints;
CREATE POLICY tenant_isolation ON engineering_checkpoints USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
ALTER TABLE engineering_transition_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_transition_events FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_transition_events;
CREATE POLICY tenant_isolation ON engineering_transition_events USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
ALTER TABLE engineering_approvals ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_approvals FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_approvals;
CREATE POLICY tenant_isolation ON engineering_approvals USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
ALTER TABLE engineering_rollbacks ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_rollbacks FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_rollbacks;
CREATE POLICY tenant_isolation ON engineering_rollbacks USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
ALTER TABLE engineering_delivery_intents ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_delivery_intents FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_delivery_intents;
CREATE POLICY tenant_isolation ON engineering_delivery_intents USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
DROP TRIGGER IF EXISTS engineering_stage_evidence_append_only ON engineering_stage_evidence;
CREATE TRIGGER engineering_stage_evidence_append_only BEFORE UPDATE OR DELETE ON engineering_stage_evidence FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0037();
DROP TRIGGER IF EXISTS engineering_checkpoints_append_only ON engineering_checkpoints;
CREATE TRIGGER engineering_checkpoints_append_only BEFORE UPDATE OR DELETE ON engineering_checkpoints FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0037();
DROP TRIGGER IF EXISTS engineering_transition_events_append_only ON engineering_transition_events;
CREATE TRIGGER engineering_transition_events_append_only BEFORE UPDATE OR DELETE ON engineering_transition_events FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0037();
DROP TRIGGER IF EXISTS engineering_approvals_append_only ON engineering_approvals;
CREATE TRIGGER engineering_approvals_append_only BEFORE UPDATE OR DELETE ON engineering_approvals FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0037();
DROP TRIGGER IF EXISTS engineering_rollbacks_append_only ON engineering_rollbacks;
CREATE TRIGGER engineering_rollbacks_append_only BEFORE UPDATE OR DELETE ON engineering_rollbacks FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0037();
DROP TRIGGER IF EXISTS engineering_delivery_intents_append_only ON engineering_delivery_intents;
CREATE TRIGGER engineering_delivery_intents_append_only BEFORE UPDATE OR DELETE ON engineering_delivery_intents FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0037();

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (37,'0037_autonomous_engineering_workflow.sql','6f1195ac6dec98c4cd13d822308ad00ae382d5f89e88e7f867adf9ef7d082566','0b5a319658f646c928865815166413637daac87af0b01945c0373f9514cae617','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0038: 0038_autonomous_engineering_swarm.sql
-- source_sha256:    73ba9be645176a36011eed77814c4adbbdc1ae9b9c94d96b39c4a9ca20036310
-- effective_sha256: 6e9c5dc6ff1b241c8d5c5eda98254cc3b5a89251e36589fbe94495ba742fb440
-- source_status:    original
-- ============================================================================
CREATE TABLE IF NOT EXISTS engineering_swarms (
 tenant_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, objective text NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9A-Fa-f]{64}$'), policy_sha256 text NOT NULL CHECK (policy_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 skill_catalog_sha256 text NOT NULL CHECK (skill_catalog_sha256 ~ '^[0-9A-Fa-f]{64}$'), max_parallel_teams integer NOT NULL CHECK (max_parallel_teams BETWEEN 1 AND 6), spec jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_team_assignments (
 tenant_uuid uuid NOT NULL, assignment_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, team text NOT NULL CHECK (team IN ('research','architecture','coding','qa','security','documentation')),
 actor_uuid uuid NOT NULL, base_source_sha256 text NOT NULL CHECK (base_source_sha256 ~ '^[0-9A-Fa-f]{64}$'), worktree_path text NOT NULL, branch text NOT NULL,
 lease_uuid uuid NOT NULL, fencing_token bigint NOT NULL CHECK (fencing_token>=0), dependencies jsonb NOT NULL DEFAULT '[]'::jsonb, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,assignment_uuid), UNIQUE (tenant_uuid,swarm_uuid,team), UNIQUE (tenant_uuid,swarm_uuid,worktree_path), UNIQUE (tenant_uuid,swarm_uuid,branch),
 FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_team_leases (
 tenant_uuid uuid NOT NULL, lease_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, team text NOT NULL, holder_uuid uuid NOT NULL, fencing_token bigint NOT NULL CHECK (fencing_token>=0),
 expires_at timestamptz NOT NULL, last_heartbeat_at timestamptz NOT NULL DEFAULT clock_timestamp(), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,lease_uuid), UNIQUE (tenant_uuid,swarm_uuid,team,fencing_token), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_team_checkpoints (
 tenant_uuid uuid NOT NULL, checkpoint_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, team text NOT NULL, workspace_state_sha256 text NOT NULL CHECK (workspace_state_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 git_commit text, fencing_token bigint NOT NULL CHECK (fencing_token>=0), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,checkpoint_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_team_evidence (
 tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, team text NOT NULL, actor_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9A-Fa-f]{64}$'), artifact_sha256 text NOT NULL CHECK (artifact_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 evidence_refs jsonb NOT NULL, passed boolean NOT NULL, highest_severity text NOT NULL CHECK (highest_severity IN ('info','low','medium','high','critical')),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,evidence_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_conflicts (
 tenant_uuid uuid NOT NULL, conflict_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, kind text NOT NULL CHECK (kind IN ('source_drift','overlapping_write','evidence_conflict','architecture_conflict','qa_failure','security_finding','documentation_mismatch')),
 teams jsonb NOT NULL, subject text NOT NULL, evidence_refs jsonb NOT NULL DEFAULT '[]'::jsonb, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,conflict_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_conflict_resolutions (
 tenant_uuid uuid NOT NULL, resolution_uuid uuid NOT NULL, conflict_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, resolver_uuid uuid NOT NULL,
 resolution_sha256 text NOT NULL CHECK (resolution_sha256 ~ '^[0-9A-Fa-f]{64}$'), rationale text NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,resolution_uuid), UNIQUE (tenant_uuid,conflict_uuid), FOREIGN KEY (tenant_uuid,conflict_uuid) REFERENCES swarm_conflicts(tenant_uuid,conflict_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_merge_candidates (
 tenant_uuid uuid NOT NULL, candidate_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, base_source_sha256 text NOT NULL CHECK (base_source_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 integration_tree_sha256 text NOT NULL CHECK (integration_tree_sha256 ~ '^[0-9A-Fa-f]{64}$'), manifest jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,candidate_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_merge_gate_events (
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, candidate_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, decision text NOT NULL CHECK (decision IN ('allow','block','needs_approval')),
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9A-Fa-f]{64}$'), fencing_token bigint NOT NULL CHECK (fencing_token>=0), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,event_uuid), FOREIGN KEY (tenant_uuid,candidate_uuid) REFERENCES swarm_merge_candidates(tenant_uuid,candidate_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_team_rollbacks (
 tenant_uuid uuid NOT NULL, rollback_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, team text NOT NULL, checkpoint_uuid uuid NOT NULL, before_sha256 text NOT NULL CHECK (before_sha256 ~ '^[0-9A-Fa-f]{64}$'), after_sha256 text NOT NULL CHECK (after_sha256 ~ '^[0-9A-Fa-f]{64}$'), verified boolean NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,rollback_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid), FOREIGN KEY (tenant_uuid,checkpoint_uuid) REFERENCES swarm_team_checkpoints(tenant_uuid,checkpoint_uuid));
CREATE OR REPLACE FUNCTION phxclaw_prevent_mutation_0038() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only table: %', TG_TABLE_NAME; END $$;
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['engineering_swarms','swarm_team_assignments','swarm_team_leases','swarm_team_checkpoints','swarm_team_evidence','swarm_conflicts','swarm_conflict_resolutions','swarm_merge_candidates','swarm_merge_gate_events','swarm_team_rollbacks'] LOOP
 EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
 EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t);
 EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t);
 END LOOP; END $$;
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['swarm_team_checkpoints','swarm_team_evidence','swarm_conflicts','swarm_conflict_resolutions','swarm_merge_candidates','swarm_merge_gate_events','swarm_team_rollbacks'] LOOP
 EXECUTE format('DROP TRIGGER IF EXISTS %I_append_only ON %I',t,t); EXECUTE format('CREATE TRIGGER %I_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0038()',t,t);
 END LOOP; END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (38,'0038_autonomous_engineering_swarm.sql','73ba9be645176a36011eed77814c4adbbdc1ae9b9c94d96b39c4a9ca20036310','6e9c5dc6ff1b241c8d5c5eda98254cc3b5a89251e36589fbe94495ba742fb440','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0039: 0039_swarm_consensus_merge_intelligence.sql
-- source_sha256:    ec5325ff8ce16073318ef9e43c000ab3bce1c3f32bf33f8452670f5a4559423f
-- effective_sha256: 44fff33de8f526c5307474d6f9178acbd5fed1c273ce3ed75ce2dd709075339d
-- source_status:    original
-- ============================================================================
CREATE TABLE IF NOT EXISTS swarm_change_intents (
 tenant_uuid uuid NOT NULL, intent_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, team text NOT NULL, actor_uuid uuid NOT NULL,
 base_source_sha256 text NOT NULL CHECK (base_source_sha256 ~ '^[0-9A-Fa-f]{64}$'), artifact_sha256 text NOT NULL CHECK (artifact_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 changed_paths jsonb NOT NULL, changed_symbols jsonb NOT NULL, contract_changes jsonb NOT NULL, depends_on_intents jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,intent_uuid), UNIQUE (tenant_uuid,swarm_uuid,intent_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_contract_surfaces (
 tenant_uuid uuid NOT NULL, contract_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, kind text NOT NULL CHECK (kind IN ('api','database','schema','behavior','security_policy','event','config')),
 name text NOT NULL, version text NOT NULL, schema_sha256 text NOT NULL CHECK (schema_sha256 ~ '^[0-9A-Fa-f]{64}$'), breaking boolean NOT NULL DEFAULT false, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,contract_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_semantic_conflicts_v039 (
 tenant_uuid uuid NOT NULL, conflict_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, class text NOT NULL CHECK (class IN ('symbol_collision','contract_violation','dependency_order','schema_migration_conflict','behavioral_divergence','security_policy_conflict','test_expectation_conflict','evidence_conflict')),
 severity text NOT NULL CHECK (severity IN ('info','low','medium','high','critical')), intent_uuids jsonb NOT NULL, subject text NOT NULL,
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9A-Fa-f]{64}$'), blocking boolean NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,conflict_uuid), UNIQUE (tenant_uuid,swarm_uuid,conflict_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_merge_dependencies_v039 (
 tenant_uuid uuid NOT NULL, dependency_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, before_intent_uuid uuid NOT NULL, after_intent_uuid uuid NOT NULL,
 reason text NOT NULL, evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9A-Fa-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,dependency_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid,before_intent_uuid) REFERENCES swarm_change_intents(tenant_uuid,swarm_uuid,intent_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid,after_intent_uuid) REFERENCES swarm_change_intents(tenant_uuid,swarm_uuid,intent_uuid));
CREATE TABLE IF NOT EXISTS swarm_merge_plans_v039 (
 tenant_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, base_source_sha256 text NOT NULL CHECK (base_source_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 ordered_intents jsonb NOT NULL, plan_sha256 text NOT NULL CHECK (plan_sha256 ~ '^[0-9A-Fa-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,plan_uuid), UNIQUE (tenant_uuid,swarm_uuid,plan_uuid), UNIQUE (tenant_uuid,swarm_uuid,plan_sha256), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_integration_replays_v039 (
 tenant_uuid uuid NOT NULL, replay_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, plan_uuid uuid NOT NULL,
 base_source_sha256 text NOT NULL CHECK (base_source_sha256 ~ '^[0-9A-Fa-f]{64}$'), merge_plan_sha256 text NOT NULL CHECK (merge_plan_sha256 ~ '^[0-9A-Fa-f]{64}$'), candidate_artifacts jsonb NOT NULL,
 replay_tree_sha256 text NOT NULL CHECK (replay_tree_sha256 ~ '^[0-9A-Fa-f]{64}$'), test_suite_sha256 text NOT NULL CHECK (test_suite_sha256 ~ '^[0-9A-Fa-f]{64}$'), status text NOT NULL CHECK (status IN ('passed','failed','diverged','side_effect_mismatch')),
 side_effects_sha256 text NOT NULL CHECK (side_effects_sha256 ~ '^[0-9A-Fa-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,replay_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid,plan_uuid) REFERENCES swarm_merge_plans_v039(tenant_uuid,swarm_uuid,plan_uuid));
CREATE TABLE IF NOT EXISTS swarm_mutation_evidence_v039 (
 tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 total_mutants integer NOT NULL CHECK (total_mutants>=0), killed_mutants integer NOT NULL CHECK (killed_mutants>=0 AND killed_mutants<=total_mutants), critical_survivors integer NOT NULL CHECK (critical_survivors>=0), report_sha256 text NOT NULL CHECK (report_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,evidence_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_resolution_documents_v039 (
 tenant_uuid uuid NOT NULL, resolution_uuid uuid NOT NULL, conflict_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, signer_key_id text NOT NULL,
 conflict_evidence_sha256 text NOT NULL CHECK (conflict_evidence_sha256 ~ '^[0-9A-Fa-f]{64}$'), chosen_resolution_sha256 text NOT NULL CHECK (chosen_resolution_sha256 ~ '^[0-9A-Fa-f]{64}$'), rationale_sha256 text NOT NULL CHECK (rationale_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 document_sha256 text NOT NULL CHECK (document_sha256 ~ '^[0-9A-Fa-f]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,resolution_uuid), UNIQUE (tenant_uuid,conflict_uuid,resolution_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid,conflict_uuid) REFERENCES swarm_semantic_conflicts_v039(tenant_uuid,swarm_uuid,conflict_uuid));
CREATE TABLE IF NOT EXISTS swarm_consensus_decisions_v039 (
 tenant_uuid uuid NOT NULL, decision_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, decision text NOT NULL CHECK (decision IN ('allow','block','needs_approval')),
 evidence_bundle_sha256 text NOT NULL CHECK (evidence_bundle_sha256 ~ '^[0-9A-Fa-f]{64}$'), fencing_token bigint NOT NULL CHECK (fencing_token>=0), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,decision_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid,plan_uuid) REFERENCES swarm_merge_plans_v039(tenant_uuid,swarm_uuid,plan_uuid));
CREATE TABLE IF NOT EXISTS swarm_merge_execution_events_v039 (
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, event_type text NOT NULL CHECK (event_type IN ('planned','replayed','approved','committed','rolled_back','failed')),
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9A-Fa-f]{64}$'), integration_tree_sha256 text NOT NULL CHECK (integration_tree_sha256 ~ '^[0-9A-Fa-f]{64}$'), fencing_token bigint NOT NULL CHECK (fencing_token>=0), payload_sha256 text NOT NULL CHECK (payload_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,event_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid,plan_uuid) REFERENCES swarm_merge_plans_v039(tenant_uuid,swarm_uuid,plan_uuid));
CREATE OR REPLACE FUNCTION phxclaw_prevent_mutation_0039() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only table: %', TG_TABLE_NAME; END $$;
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['swarm_change_intents','swarm_contract_surfaces','swarm_semantic_conflicts_v039','swarm_merge_dependencies_v039','swarm_merge_plans_v039','swarm_integration_replays_v039','swarm_mutation_evidence_v039','swarm_resolution_documents_v039','swarm_consensus_decisions_v039','swarm_merge_execution_events_v039'] LOOP
 EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
 EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t);
 EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t);
 EXECUTE format('DROP TRIGGER IF EXISTS %I_append_only ON %I',t,t); EXECUTE format('CREATE TRIGGER %I_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0039()',t,t);
 END LOOP; END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (39,'0039_swarm_consensus_merge_intelligence.sql','ec5325ff8ce16073318ef9e43c000ab3bce1c3f32bf33f8452670f5a4559423f','44fff33de8f526c5307474d6f9178acbd5fed1c273ce3ed75ce2dd709075339d','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0040: 0040_unified_configuration_and_factory.sql
-- source_sha256:    591569af912d323a6e78fcae3847d3b6709a61f7b1c82970b62378e47a5bdf15
-- effective_sha256: aba242cca390dd0972a9a3e124e170d6279f5532b0017e4485e21be5a7b7d756
-- source_status:    original
-- ============================================================================
CREATE TABLE IF NOT EXISTS phoenix_config_revisions (
 tenant_uuid uuid NOT NULL,
 config_uuid uuid NOT NULL,
 revision bigint NOT NULL CHECK (revision > 0),
 config_sha256 text NOT NULL CHECK (config_sha256 ~ '^[0-9a-f]{64}$'),
 config_json jsonb NOT NULL,
 actor_uuid uuid,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, config_uuid, revision)
);
CREATE TABLE IF NOT EXISTS phoenix_factory_runs (
 tenant_uuid uuid NOT NULL,
 run_uuid uuid NOT NULL,
 requirement_id text NOT NULL CHECK (length(requirement_id)>0),
 stage text NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 checkpoint_uuid uuid,
 approved boolean NOT NULL DEFAULT false,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, run_uuid)
);
ALTER TABLE phoenix_config_revisions ENABLE ROW LEVEL SECURITY;
ALTER TABLE phoenix_config_revisions FORCE ROW LEVEL SECURITY;
ALTER TABLE phoenix_factory_runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE phoenix_factory_runs FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS phoenix_config_revisions_tenant ON phoenix_config_revisions;
CREATE POLICY phoenix_config_revisions_tenant ON phoenix_config_revisions USING (tenant_uuid = NULLIF(current_setting('phoenix.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phoenix.tenant_uuid', true), '')::uuid);
DROP POLICY IF EXISTS phoenix_factory_runs_tenant ON phoenix_factory_runs;
CREATE POLICY phoenix_factory_runs_tenant ON phoenix_factory_runs USING (tenant_uuid = NULLIF(current_setting('phoenix.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting('phoenix.tenant_uuid', true), '')::uuid);
CREATE OR REPLACE FUNCTION phoenix_block_config_revision_mutation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'configuration revision history is append-only'; END $$;
DROP TRIGGER IF EXISTS trg_config_revision_append_only ON phoenix_config_revisions;
CREATE TRIGGER trg_config_revision_append_only BEFORE UPDATE OR DELETE ON phoenix_config_revisions FOR EACH ROW EXECUTE FUNCTION phoenix_block_config_revision_mutation();

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (40,'0040_unified_configuration_and_factory.sql','591569af912d323a6e78fcae3847d3b6709a61f7b1c82970b62378e47a5bdf15','aba242cca390dd0972a9a3e124e170d6279f5532b0017e4485e21be5a7b7d756','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0041 — NO-OP
-- v0.41 global rename — sem DDL persistente
-- ============================================================================
INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (41,'0041_NO_OP','7597349e413cc5b8c4607e5033668dd3b7c06f97859f229389ca662b285ccc05','7597349e413cc5b8c4607e5033668dd3b7c06f97859f229389ca662b285ccc05','no_op','v0.41 global rename — sem DDL persistente') ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0042: 0042_project_management_suite.sql
-- source_sha256:    29eec7c369acfac42160694e66c6eb12ccec83c9b2165279ffe23ece933e392c
-- effective_sha256: 0cfd20489830eeaaa675c771f95809f180cbcbaa2138e51b9cd93d2a08d23ce4
-- source_status:    original
-- repair_notes:     reparo sintático current_setting/NULLIF: 2 ocorrência(s)
-- ============================================================================
-- PhxClaw v0.42 Project Management Suite

CREATE TABLE IF NOT EXISTS pm_projects (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, name text NOT NULL, methodology text NOT NULL DEFAULT 'hybrid', status text NOT NULL DEFAULT 'active', revision bigint NOT NULL DEFAULT 1, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), updated_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid, project_uuid)
);
CREATE TABLE IF NOT EXISTS pm_work_items (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, work_item_uuid uuid NOT NULL, parent_uuid uuid NULL, title text NOT NULL, state text NOT NULL, priority integer NOT NULL DEFAULT 0, story_points numeric NULL, planned_start timestamptz NULL, planned_finish timestamptz NULL, actual_start timestamptz NULL, actual_finish timestamptz NULL, planned_cost numeric NOT NULL DEFAULT 0, actual_cost numeric NOT NULL DEFAULT 0, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid, project_uuid, work_item_uuid), FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES pm_projects(tenant_uuid, project_uuid)
);
CREATE TABLE IF NOT EXISTS pm_sprints (tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, sprint_uuid uuid NOT NULL, name text NOT NULL, start_at timestamptz, end_at timestamptz, goal text, status text NOT NULL DEFAULT 'planned', PRIMARY KEY(tenant_uuid, project_uuid, sprint_uuid), FOREIGN KEY(tenant_uuid,project_uuid) REFERENCES pm_projects(tenant_uuid,project_uuid));
CREATE TABLE IF NOT EXISTS pm_risks (tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, risk_uuid uuid NOT NULL, title text NOT NULL, probability numeric NOT NULL, impact numeric NOT NULL, response text, status text NOT NULL DEFAULT 'open', PRIMARY KEY(tenant_uuid,project_uuid,risk_uuid), FOREIGN KEY(tenant_uuid,project_uuid) REFERENCES pm_projects(tenant_uuid,project_uuid), CHECK(probability BETWEEN 0 AND 1), CHECK(impact BETWEEN 0 AND 1));
CREATE TABLE IF NOT EXISTS pm_cost_snapshots (tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL, pv numeric NOT NULL, ev numeric NOT NULL, ac numeric NOT NULL, bac numeric NOT NULL, captured_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,project_uuid,snapshot_uuid), FOREIGN KEY(tenant_uuid,project_uuid) REFERENCES pm_projects(tenant_uuid,project_uuid));
CREATE TABLE IF NOT EXISTS pm_pdca_cycles (tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, cycle_uuid uuid NOT NULL, phase text NOT NULL, objective text, metric jsonb NOT NULL DEFAULT '{}'::jsonb, evidence_sha256 text, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,project_uuid,cycle_uuid), FOREIGN KEY(tenant_uuid,project_uuid) REFERENCES pm_projects(tenant_uuid,project_uuid));
CREATE TABLE IF NOT EXISTS pm_stakeholders (tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, stakeholder_uuid uuid NOT NULL, display_name text NOT NULL, role text, influence integer, interest integer, communication_plan jsonb NOT NULL DEFAULT '{}'::jsonb, PRIMARY KEY(tenant_uuid,project_uuid,stakeholder_uuid), FOREIGN KEY(tenant_uuid,project_uuid) REFERENCES pm_projects(tenant_uuid,project_uuid));
CREATE TABLE IF NOT EXISTS pm_events (tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, event_uuid uuid NOT NULL, event_type text NOT NULL, payload jsonb NOT NULL, evidence_sha256 text NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,project_uuid,event_uuid), FOREIGN KEY(tenant_uuid,project_uuid) REFERENCES pm_projects(tenant_uuid,project_uuid));

DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['pm_projects','pm_work_items','pm_sprints','pm_risks','pm_cost_snapshots','pm_pdca_cycles','pm_stakeholders','pm_events'] LOOP EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t); EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t); EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t); END LOOP; END $$;

CREATE OR REPLACE FUNCTION pm_events_no_update_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'pm_events is append-only'; END $$;
DROP TRIGGER IF EXISTS pm_events_no_update ON pm_events; CREATE TRIGGER pm_events_no_update BEFORE UPDATE OR DELETE ON pm_events FOR EACH ROW EXECUTE FUNCTION pm_events_no_update_delete();

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (42,'0042_project_management_suite.sql','29eec7c369acfac42160694e66c6eb12ccec83c9b2165279ffe23ece933e392c','0cfd20489830eeaaa675c771f95809f180cbcbaa2138e51b9cd93d2a08d23ce4','original','reparo sintático current_setting/NULLIF: 2 ocorrência(s)') ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0043: 0043_project_workspace_lifecycle.sql
-- source_sha256:    69e68d16a49c3f3620b3bcfa4c1060af5abc6f38896b1b458c79db7a40113dce
-- effective_sha256: 5b8dafac2620a233b8a256952c2cfba9affb2f649b8162b65debb9c4dd4f8a7f
-- source_status:    original
-- repair_notes:     reparo sintático current_setting/NULLIF: 2 ocorrência(s)
-- ============================================================================
CREATE TABLE IF NOT EXISTS phx_projects (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, name text NOT NULL, slug text NOT NULL,
 manifest_sha256 text NOT NULL CHECK (manifest_sha256 ~ '^[0-9a-f]{64}$'), root_path text NOT NULL,
 template_id text, archived boolean NOT NULL DEFAULT false, created_at timestamptz NOT NULL DEFAULT now(), last_opened_at timestamptz,
 PRIMARY KEY (tenant_uuid, project_uuid), UNIQUE (tenant_uuid, slug)
);
CREATE TABLE IF NOT EXISTS phx_project_versions (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, version_uuid uuid NOT NULL, semver text, git_commit text,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'), config_revision bigint NOT NULL,
 backup_uuid uuid, created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, project_uuid, version_uuid),
 FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid)
);
CREATE TABLE IF NOT EXISTS phx_project_log_events (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, log_uuid uuid NOT NULL, severity text NOT NULL,
 category text NOT NULL, message_redacted text NOT NULL, correlation_id text, source_path text NOT NULL,
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'), observed_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, project_uuid, log_uuid),
 FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid)
);
CREATE TABLE IF NOT EXISTS phx_project_backups (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, backup_uuid uuid NOT NULL, source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 backup_path text NOT NULL, manifest_sha256 text NOT NULL CHECK (manifest_sha256 ~ '^[0-9a-f]{64}$'), file_count bigint NOT NULL CHECK(file_count>=0), byte_count bigint NOT NULL CHECK(byte_count>=0),
 verified boolean NOT NULL DEFAULT false, created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, project_uuid, backup_uuid), FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid)
);
CREATE TABLE IF NOT EXISTS phx_project_activity_bindings (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, binding_uuid uuid NOT NULL, activity_pattern text NOT NULL,
 agent_uuid uuid, skill_id text, model_route text, enabled boolean NOT NULL DEFAULT true, created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, project_uuid, binding_uuid), FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid)
);
CREATE TABLE IF NOT EXISTS phx_project_events (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, event_uuid uuid NOT NULL, event_type text NOT NULL,
 payload_sha256 text NOT NULL CHECK(payload_sha256 ~ '^[0-9a-f]{64}$'), payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, project_uuid, event_uuid), FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid)
);
DO $$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['phx_projects','phx_project_versions','phx_project_log_events','phx_project_backups','phx_project_activity_bindings','phx_project_events'] LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
  EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
  EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I', t);
  EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)', t);
 END LOOP;
END $$;
CREATE OR REPLACE FUNCTION phx_forbid_project_event_update() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only table'; END $$;
DROP TRIGGER IF EXISTS trg_phx_project_events_append_only ON phx_project_events;
CREATE TRIGGER trg_phx_project_events_append_only BEFORE UPDATE OR DELETE ON phx_project_events FOR EACH ROW EXECUTE FUNCTION phx_forbid_project_event_update();

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (43,'0043_project_workspace_lifecycle.sql','69e68d16a49c3f3620b3bcfa4c1060af5abc6f38896b1b458c79db7a40113dce','5b8dafac2620a233b8a256952c2cfba9affb2f649b8162b65debb9c4dd4f8a7f','original','reparo sintático current_setting/NULLIF: 2 ocorrência(s)') ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0044: 0044_agent_control_pdca_learning.sql
-- source_sha256:    357e1f41dd97d7a7576081df2015cfd8e4b6eb4323fca09c4178b1845d9f0214
-- effective_sha256: 761bca32815c6b0a70aff81b53e042922ca67d9799a2bad0dac07210aac35a00
-- source_status:    original
-- ============================================================================

CREATE TABLE IF NOT EXISTS phx_agent_runtime_profiles (
    tenant_uuid uuid NOT NULL,
    project_uuid uuid NOT NULL,
    agent_uuid uuid NOT NULL,
    name text NOT NULL,
    capability text NOT NULL,
    default_complexity text NOT NULL CHECK (default_complexity IN ('low','medium','high','extreme')),
    model_hint text,
    local_first boolean NOT NULL DEFAULT true,
    active boolean NOT NULL DEFAULT true,
    registry_sha256 text NOT NULL CHECK (registry_sha256 ~ '^[0-9a-fA-F]{64}$'),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_uuid, project_uuid, agent_uuid),
    FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS phx_agent_task_assignments (
    tenant_uuid uuid NOT NULL,
    project_uuid uuid NOT NULL,
    assignment_uuid uuid NOT NULL,
    task_uuid uuid NOT NULL,
    agent_uuid uuid NOT NULL,
    complexity text NOT NULL CHECK (complexity IN ('low','medium','high','extreme')),
    capability text NOT NULL,
    model_profile_uuid uuid,
    routing_decision_sha256 text CHECK (routing_decision_sha256 IS NULL OR routing_decision_sha256 ~ '^[0-9a-fA-F]{64}$'),
    lease_uuid uuid,
    fencing_token bigint NOT NULL DEFAULT 0,
    state text NOT NULL DEFAULT 'planned' CHECK (state IN ('planned','leased','running','checking','acting','completed','failed','cancelled')),
    source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-fA-F]{64}$'),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_uuid, project_uuid, assignment_uuid),
    UNIQUE (tenant_uuid, project_uuid, task_uuid, agent_uuid),
    FOREIGN KEY (tenant_uuid, project_uuid, agent_uuid) REFERENCES phx_agent_runtime_profiles(tenant_uuid, project_uuid, agent_uuid) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS phx_model_routing_decisions (
    tenant_uuid uuid NOT NULL,
    project_uuid uuid NOT NULL,
    decision_uuid uuid NOT NULL,
    task_uuid uuid NOT NULL,
    agent_uuid uuid NOT NULL,
    model_profile_uuid uuid NOT NULL,
    provider text NOT NULL,
    model text NOT NULL,
    local_execution boolean NOT NULL,
    estimated_cost_usd numeric(18,8) NOT NULL DEFAULT 0 CHECK (estimated_cost_usd >= 0),
    quality_floor numeric(9,6) NOT NULL DEFAULT 0,
    decision_sha256 text NOT NULL CHECK (decision_sha256 ~ '^[0-9a-fA-F]{64}$'),
    evidence jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_uuid, project_uuid, decision_uuid),
    FOREIGN KEY (tenant_uuid, project_uuid, agent_uuid) REFERENCES phx_agent_runtime_profiles(tenant_uuid, project_uuid, agent_uuid) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS phx_agent_cost_ledger (
    tenant_uuid uuid NOT NULL,
    project_uuid uuid NOT NULL,
    entry_uuid uuid NOT NULL,
    task_uuid uuid NOT NULL,
    agent_uuid uuid NOT NULL,
    model_profile_uuid uuid,
    provider text,
    estimated_cost_usd numeric(18,8) NOT NULL DEFAULT 0 CHECK (estimated_cost_usd >= 0),
    actual_cost_usd numeric(18,8) CHECK (actual_cost_usd IS NULL OR actual_cost_usd >= 0),
    input_tokens bigint CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens bigint CHECK (output_tokens IS NULL OR output_tokens >= 0),
    idempotency_key text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    settled_at timestamptz,
    PRIMARY KEY (tenant_uuid, project_uuid, entry_uuid),
    UNIQUE (tenant_uuid, project_uuid, idempotency_key),
    FOREIGN KEY (tenant_uuid, project_uuid, agent_uuid) REFERENCES phx_agent_runtime_profiles(tenant_uuid, project_uuid, agent_uuid) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS phx_pdca_cycles (
    tenant_uuid uuid NOT NULL,
    project_uuid uuid NOT NULL,
    cycle_uuid uuid NOT NULL,
    task_uuid uuid NOT NULL,
    phase text NOT NULL CHECK (phase IN ('plan','do','check','act','closed')),
    objective text NOT NULL,
    expected_evidence jsonb NOT NULL DEFAULT '[]'::jsonb,
    source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-fA-F]{64}$'),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_uuid, project_uuid, cycle_uuid)
);

CREATE TABLE IF NOT EXISTS phx_pdca_events (
    tenant_uuid uuid NOT NULL,
    project_uuid uuid NOT NULL,
    event_uuid uuid NOT NULL,
    cycle_uuid uuid NOT NULL,
    phase text NOT NULL,
    event_type text NOT NULL,
    payload jsonb NOT NULL,
    evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-fA-F]{64}$'),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_uuid, project_uuid, event_uuid),
    FOREIGN KEY (tenant_uuid, project_uuid, cycle_uuid) REFERENCES phx_pdca_cycles(tenant_uuid, project_uuid, cycle_uuid) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS phx_knowledge_fruitful (
    tenant_uuid uuid NOT NULL,
    project_uuid uuid NOT NULL,
    knowledge_uuid uuid NOT NULL,
    task_class text NOT NULL,
    context_fingerprint text NOT NULL,
    source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-fA-F]{64}$'),
    pattern_summary text NOT NULL,
    agent_uuid uuid NOT NULL,
    model_profile_uuid uuid,
    skill_versions jsonb NOT NULL DEFAULT '{}'::jsonb,
    quality_score numeric(9,6) NOT NULL DEFAULT 0,
    actual_cost_usd numeric(18,8) NOT NULL DEFAULT 0,
    evidence_refs jsonb NOT NULL DEFAULT '[]'::jsonb,
    confidence numeric(9,6) NOT NULL DEFAULT 0,
    reuse_count bigint NOT NULL DEFAULT 0,
    promotion_state text NOT NULL DEFAULT 'candidate' CHECK (promotion_state IN ('candidate','accepted','governed','superseded','expired')),
    fresh_until timestamptz,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_uuid, project_uuid, knowledge_uuid),
    FOREIGN KEY (tenant_uuid, project_uuid, agent_uuid) REFERENCES phx_agent_runtime_profiles(tenant_uuid, project_uuid, agent_uuid) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS phx_knowledge_unfruitful (
    tenant_uuid uuid NOT NULL,
    project_uuid uuid NOT NULL,
    knowledge_uuid uuid NOT NULL,
    task_class text NOT NULL,
    context_fingerprint text NOT NULL,
    source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-fA-F]{64}$'),
    failure_signature text NOT NULL,
    root_cause text NOT NULL,
    remediation text,
    avoidance_rule text NOT NULL,
    agent_uuid uuid NOT NULL,
    model_profile_uuid uuid,
    skill_versions jsonb NOT NULL DEFAULT '{}'::jsonb,
    evidence_refs jsonb NOT NULL DEFAULT '[]'::jsonb,
    occurrence_count bigint NOT NULL DEFAULT 1,
    promotion_state text NOT NULL DEFAULT 'candidate' CHECK (promotion_state IN ('candidate','accepted','governed','superseded','expired')),
    retry_after timestamptz,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_uuid, project_uuid, knowledge_uuid),
    FOREIGN KEY (tenant_uuid, project_uuid, agent_uuid) REFERENCES phx_agent_runtime_profiles(tenant_uuid, project_uuid, agent_uuid) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS phx_knowledge_reuse_events (
    tenant_uuid uuid NOT NULL,
    project_uuid uuid NOT NULL,
    event_uuid uuid NOT NULL,
    task_uuid uuid NOT NULL,
    knowledge_uuid uuid NOT NULL,
    knowledge_kind text NOT NULL CHECK (knowledge_kind IN ('fruitful','unfruitful')),
    action text NOT NULL CHECK (action IN ('reused','warned','blocked','ignored','superseded')),
    context_fingerprint text NOT NULL,
    evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-fA-F]{64}$'),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_uuid, project_uuid, event_uuid)
);

-- RLS is static and tenant-scoped. FORCE RLS prevents table owners from silently bypassing policies.
DO $$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY[
    'phx_agent_runtime_profiles','phx_agent_task_assignments','phx_model_routing_decisions',
    'phx_agent_cost_ledger','phx_pdca_cycles','phx_pdca_events','phx_knowledge_fruitful',
    'phx_knowledge_unfruitful','phx_knowledge_reuse_events'
  ] LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
  END LOOP;
END $$;

DROP POLICY IF EXISTS phx_agent_runtime_profiles_tenant ON phx_agent_runtime_profiles;
CREATE POLICY phx_agent_runtime_profiles_tenant ON phx_agent_runtime_profiles
USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
DROP POLICY IF EXISTS phx_agent_task_assignments_tenant ON phx_agent_task_assignments;
CREATE POLICY phx_agent_task_assignments_tenant ON phx_agent_task_assignments
USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
DROP POLICY IF EXISTS phx_model_routing_decisions_tenant ON phx_model_routing_decisions;
CREATE POLICY phx_model_routing_decisions_tenant ON phx_model_routing_decisions
USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
DROP POLICY IF EXISTS phx_agent_cost_ledger_tenant ON phx_agent_cost_ledger;
CREATE POLICY phx_agent_cost_ledger_tenant ON phx_agent_cost_ledger
USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
DROP POLICY IF EXISTS phx_pdca_cycles_tenant ON phx_pdca_cycles;
CREATE POLICY phx_pdca_cycles_tenant ON phx_pdca_cycles
USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
DROP POLICY IF EXISTS phx_pdca_events_tenant ON phx_pdca_events;
CREATE POLICY phx_pdca_events_tenant ON phx_pdca_events
USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
DROP POLICY IF EXISTS phx_knowledge_fruitful_tenant ON phx_knowledge_fruitful;
CREATE POLICY phx_knowledge_fruitful_tenant ON phx_knowledge_fruitful
USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
DROP POLICY IF EXISTS phx_knowledge_unfruitful_tenant ON phx_knowledge_unfruitful;
CREATE POLICY phx_knowledge_unfruitful_tenant ON phx_knowledge_unfruitful
USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
DROP POLICY IF EXISTS phx_knowledge_reuse_events_tenant ON phx_knowledge_reuse_events;
CREATE POLICY phx_knowledge_reuse_events_tenant ON phx_knowledge_reuse_events
USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);

-- Evidence/event tables are append-only. Updates/deletes are denied explicitly.
CREATE OR REPLACE FUNCTION phx_deny_update_delete() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'append-only table'; END $$;

DROP TRIGGER IF EXISTS phx_pdca_events_append_only ON phx_pdca_events;
CREATE TRIGGER phx_pdca_events_append_only BEFORE UPDATE OR DELETE ON phx_pdca_events
FOR EACH ROW EXECUTE FUNCTION phx_deny_update_delete();
DROP TRIGGER IF EXISTS phx_routing_decisions_append_only ON phx_model_routing_decisions;
CREATE TRIGGER phx_routing_decisions_append_only BEFORE UPDATE OR DELETE ON phx_model_routing_decisions
FOR EACH ROW EXECUTE FUNCTION phx_deny_update_delete();
DROP TRIGGER IF EXISTS phx_reuse_events_append_only ON phx_knowledge_reuse_events;
CREATE TRIGGER phx_reuse_events_append_only BEFORE UPDATE OR DELETE ON phx_knowledge_reuse_events
FOR EACH ROW EXECUTE FUNCTION phx_deny_update_delete();

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (44,'0044_agent_control_pdca_learning.sql','357e1f41dd97d7a7576081df2015cfd8e4b6eb4323fca09c4178b1845d9f0214','761bca32815c6b0a70aff81b53e042922ca67d9799a2bad0dac07210aac35a00','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0045: 0045_active_project_runtime.sql
-- source_sha256:    9ea49afb3d0e04eecd971982ade64b61e4092e886cf6ef9063edc5092d4730cf
-- effective_sha256: 692f448628d01336e1426e55c9488ce89a2790c36dbd6c105a5055cb6e278ef3
-- source_status:    original
-- ============================================================================

CREATE TABLE IF NOT EXISTS phx_active_project_runtime (
    tenant_uuid uuid NOT NULL,
    project_uuid uuid NOT NULL,
    runtime_uuid uuid NOT NULL,
    source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-fA-F]{64}$'),
    state text NOT NULL CHECK (state IN ('active','paused','closed')),
    max_concurrency integer NOT NULL CHECK (max_concurrency > 0),
    max_budget_usd numeric(18,8) NOT NULL CHECK (max_budget_usd >= 0),
    current_fencing_token bigint NOT NULL DEFAULT 0 CHECK (current_fencing_token >= 0),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_uuid, project_uuid, runtime_uuid),
    UNIQUE (tenant_uuid, project_uuid),
    FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS phx_project_task_queue (
    tenant_uuid uuid NOT NULL,
    project_uuid uuid NOT NULL,
    task_uuid uuid NOT NULL,
    task_class text NOT NULL,
    capability text NOT NULL,
    complexity text NOT NULL CHECK (complexity IN ('low','medium','high','extreme')),
    data_class text NOT NULL CHECK (data_class IN ('public','internal','confidential','restricted')),
    priority integer NOT NULL DEFAULT 50,
    dependencies jsonb NOT NULL DEFAULT '[]'::jsonb,
    context_fingerprint text NOT NULL,
    source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-fA-F]{64}$'),
    state text NOT NULL CHECK (state IN ('queued','ready','leased','running','checking','acting','completed','failed','cancelled','blocked')),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    max_attempts integer NOT NULL DEFAULT 3 CHECK (max_attempts > 0),
    required_agent_uuid uuid,
    quality_floor numeric(9,6) NOT NULL DEFAULT 0 CHECK (quality_floor >= 0 AND quality_floor <= 1),
    max_estimated_cost_usd numeric(18,8) NOT NULL DEFAULT 0 CHECK (max_estimated_cost_usd >= 0),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_uuid, project_uuid, task_uuid),
    FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS phx_agent_execution_leases (
    tenant_uuid uuid NOT NULL,
    project_uuid uuid NOT NULL,
    lease_uuid uuid NOT NULL,
    task_uuid uuid NOT NULL,
    agent_uuid uuid NOT NULL,
    run_uuid uuid NOT NULL,
    model_profile_uuid uuid NOT NULL,
    decision_sha256 text NOT NULL CHECK (decision_sha256 ~ '^[0-9a-fA-F]{64}$'),
    lease_sha256 text NOT NULL CHECK (lease_sha256 ~ '^[0-9a-fA-F]{64}$'),
    fencing_token bigint NOT NULL CHECK (fencing_token > 0),
    estimated_cost_usd numeric(18,8) NOT NULL CHECK (estimated_cost_usd >= 0),
    expires_at timestamptz NOT NULL,
    state text NOT NULL CHECK (state IN ('leased','running','checking','acting','completed','failed','cancelled','expired')),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_uuid, project_uuid, lease_uuid),
    UNIQUE (tenant_uuid, project_uuid, run_uuid),
    UNIQUE (tenant_uuid, project_uuid, fencing_token),
    FOREIGN KEY (tenant_uuid, project_uuid, task_uuid) REFERENCES phx_project_task_queue(tenant_uuid, project_uuid, task_uuid) ON DELETE RESTRICT,
    FOREIGN KEY (tenant_uuid, project_uuid, agent_uuid) REFERENCES phx_agent_runtime_profiles(tenant_uuid, project_uuid, agent_uuid) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS phx_project_budget_reservations (
    tenant_uuid uuid NOT NULL,
    project_uuid uuid NOT NULL,
    reservation_uuid uuid NOT NULL,
    run_uuid uuid NOT NULL,
    task_uuid uuid NOT NULL,
    estimated_usd numeric(18,8) NOT NULL CHECK (estimated_usd >= 0),
    actual_usd numeric(18,8),
    state text NOT NULL CHECK (state IN ('reserved','settled','released')),
    idempotency_key text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    settled_at timestamptz,
    PRIMARY KEY (tenant_uuid, project_uuid, reservation_uuid),
    UNIQUE (tenant_uuid, project_uuid, idempotency_key),
    FOREIGN KEY (tenant_uuid, project_uuid, task_uuid) REFERENCES phx_project_task_queue(tenant_uuid, project_uuid, task_uuid) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS phx_execution_runtime_events (
    tenant_uuid uuid NOT NULL,
    project_uuid uuid NOT NULL,
    event_uuid uuid NOT NULL,
    run_uuid uuid,
    task_uuid uuid,
    agent_uuid uuid,
    event_type text NOT NULL,
    payload jsonb NOT NULL,
    evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-fA-F]{64}$'),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_uuid, project_uuid, event_uuid)
);

-- Tenant isolation is mandatory and forced even for table owners.
DO $$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY[
    'phx_active_project_runtime','phx_project_task_queue','phx_agent_execution_leases',
    'phx_project_budget_reservations','phx_execution_runtime_events'
  ] LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
  END LOOP;
END $$;

DROP POLICY IF EXISTS phx_active_project_runtime_tenant ON phx_active_project_runtime;
CREATE POLICY phx_active_project_runtime_tenant ON phx_active_project_runtime
USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
DROP POLICY IF EXISTS phx_project_task_queue_tenant ON phx_project_task_queue;
CREATE POLICY phx_project_task_queue_tenant ON phx_project_task_queue
USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
DROP POLICY IF EXISTS phx_agent_execution_leases_tenant ON phx_agent_execution_leases;
CREATE POLICY phx_agent_execution_leases_tenant ON phx_agent_execution_leases
USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
DROP POLICY IF EXISTS phx_project_budget_reservations_tenant ON phx_project_budget_reservations;
CREATE POLICY phx_project_budget_reservations_tenant ON phx_project_budget_reservations
USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
DROP POLICY IF EXISTS phx_execution_runtime_events_tenant ON phx_execution_runtime_events;
CREATE POLICY phx_execution_runtime_events_tenant ON phx_execution_runtime_events
USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid)
WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);

-- Atomic fencing token allocation for one active project runtime.
CREATE OR REPLACE FUNCTION phx_next_project_fencing(p_tenant uuid, p_project uuid)
RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE v_next bigint;
BEGIN
  UPDATE phx_active_project_runtime
     SET current_fencing_token = current_fencing_token + 1,
         updated_at = clock_timestamp()
   WHERE tenant_uuid = p_tenant AND project_uuid = p_project AND state = 'active'
   RETURNING current_fencing_token INTO v_next;
  IF v_next IS NULL THEN RAISE EXCEPTION 'active project runtime not found'; END IF;
  RETURN v_next;
END $$;

CREATE OR REPLACE FUNCTION phx_deny_runtime_event_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'append-only runtime event'; END $$;
DROP TRIGGER IF EXISTS phx_runtime_events_append_only ON phx_execution_runtime_events;
CREATE TRIGGER phx_runtime_events_append_only BEFORE UPDATE OR DELETE ON phx_execution_runtime_events
FOR EACH ROW EXECUTE FUNCTION phx_deny_runtime_event_mutation();

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (45,'0045_active_project_runtime.sql','9ea49afb3d0e04eecd971982ade64b61e4092e886cf6ef9063edc5092d4730cf','692f448628d01336e1426e55c9488ce89a2790c36dbd6c105a5055cb6e278ef3','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0046: 0046_predictive_project_intelligence.sql
-- source_sha256:    2bf8902453805dcb9167598f4c2a5c183dea1662fa133e62fd53a8533510a9f1
-- effective_sha256: 5ad3ae5877760a5b0125a9d2e40318b5de3e3ce609465269d6f05df7f466274b
-- source_status:    original
-- repair_notes:     reparo sintático current_setting/NULLIF: 2 ocorrência(s)
-- ============================================================================

CREATE TABLE IF NOT EXISTS phx_predictive_route_forecasts (
  tenant_uuid uuid NOT NULL,
  project_uuid uuid NOT NULL,
  forecast_uuid uuid NOT NULL,
  task_class text NOT NULL,
  context_fingerprint text NOT NULL,
  agent_uuid uuid NOT NULL,
  model_profile_uuid uuid NOT NULL,
  provider text NOT NULL,
  local boolean NOT NULL,
  sample_count integer NOT NULL CHECK (sample_count > 0),
  success_probability numeric(9,6) NOT NULL CHECK (success_probability BETWEEN 0 AND 1),
  expected_quality numeric(9,6) NOT NULL CHECK (expected_quality BETWEEN 0 AND 1),
  expected_cost_usd numeric(18,8) NOT NULL CHECK (expected_cost_usd >= 0),
  expected_duration_ms bigint NOT NULL CHECK (expected_duration_ms >= 0),
  expected_retries numeric(12,6) NOT NULL CHECK (expected_retries >= 0),
  effective_cost_usd numeric(18,8) NOT NULL CHECK (effective_cost_usd >= 0),
  confidence numeric(9,6) NOT NULL CHECK (confidence BETWEEN 0 AND 1),
  evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-fA-F]{64}$'),
  forecast_sha256 text NOT NULL CHECK (forecast_sha256 ~ '^[0-9a-fA-F]{64}$'),
  generated_at timestamptz NOT NULL,
  expires_at timestamptz NOT NULL,
  PRIMARY KEY (tenant_uuid, project_uuid, forecast_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS phx_predictive_recommendations (
  tenant_uuid uuid NOT NULL,
  project_uuid uuid NOT NULL,
  recommendation_uuid uuid NOT NULL,
  task_uuid uuid NOT NULL,
  forecast_uuid uuid NOT NULL,
  agent_uuid uuid NOT NULL,
  model_profile_uuid uuid NOT NULL,
  provider text NOT NULL,
  local boolean NOT NULL,
  predicted_cost_usd numeric(18,8) NOT NULL CHECK (predicted_cost_usd >= 0),
  predicted_quality numeric(9,6) NOT NULL CHECK (predicted_quality BETWEEN 0 AND 1),
  predicted_success_probability numeric(9,6) NOT NULL CHECK (predicted_success_probability BETWEEN 0 AND 1),
  recommendation_sha256 text NOT NULL CHECK (recommendation_sha256 ~ '^[0-9a-fA-F]{64}$'),
  rationale jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, recommendation_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid, task_uuid) REFERENCES phx_project_task_queue(tenant_uuid, project_uuid, task_uuid) ON DELETE RESTRICT,
  FOREIGN KEY (tenant_uuid, project_uuid, forecast_uuid) REFERENCES phx_predictive_route_forecasts(tenant_uuid, project_uuid, forecast_uuid) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS phx_project_predictive_forecasts (
  tenant_uuid uuid NOT NULL,
  project_uuid uuid NOT NULL,
  forecast_uuid uuid NOT NULL,
  task_count integer NOT NULL CHECK (task_count >= 0),
  expected_remaining_cost_usd numeric(18,8) NOT NULL CHECK (expected_remaining_cost_usd >= 0),
  expected_remaining_duration_ms bigint NOT NULL CHECK (expected_remaining_duration_ms >= 0),
  expected_rework_cost_usd numeric(18,8) NOT NULL CHECK (expected_rework_cost_usd >= 0),
  deadline_risk numeric(9,6) NOT NULL CHECK (deadline_risk BETWEEN 0 AND 1),
  budget_overrun_risk numeric(9,6) NOT NULL CHECK (budget_overrun_risk BETWEEN 0 AND 1),
  source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-fA-F]{64}$'),
  forecast_sha256 text NOT NULL CHECK (forecast_sha256 ~ '^[0-9a-fA-F]{64}$'),
  generated_at timestamptz NOT NULL,
  expires_at timestamptz NOT NULL,
  PRIMARY KEY (tenant_uuid, project_uuid, forecast_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS phx_forecast_feedback (
  tenant_uuid uuid NOT NULL,
  project_uuid uuid NOT NULL,
  feedback_uuid uuid NOT NULL,
  forecast_uuid uuid NOT NULL,
  run_uuid uuid NOT NULL,
  actual_success boolean NOT NULL,
  actual_quality numeric(9,6) NOT NULL CHECK (actual_quality BETWEEN 0 AND 1),
  actual_cost_usd numeric(18,8) NOT NULL CHECK (actual_cost_usd >= 0),
  actual_duration_ms bigint NOT NULL CHECK (actual_duration_ms >= 0),
  cost_absolute_error numeric(18,8) NOT NULL CHECK (cost_absolute_error >= 0),
  duration_absolute_error_ms bigint NOT NULL CHECK (duration_absolute_error_ms >= 0),
  quality_absolute_error numeric(9,6) NOT NULL CHECK (quality_absolute_error >= 0),
  evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-fA-F]{64}$'),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, feedback_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid, forecast_uuid) REFERENCES phx_predictive_route_forecasts(tenant_uuid, project_uuid, forecast_uuid) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS phx_predictive_events (
  tenant_uuid uuid NOT NULL,
  project_uuid uuid NOT NULL,
  event_uuid uuid NOT NULL,
  event_type text NOT NULL,
  object_uuid uuid,
  payload jsonb NOT NULL,
  evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-fA-F]{64}$'),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, event_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid) REFERENCES phx_projects(tenant_uuid, project_uuid) ON DELETE CASCADE
);

DO $$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['phx_predictive_route_forecasts','phx_predictive_recommendations','phx_project_predictive_forecasts','phx_forecast_feedback','phx_predictive_events'] LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
    EXECUTE format('DROP POLICY IF EXISTS %I ON %I', t || '_tenant', t);
    EXECUTE format('CREATE POLICY %I ON %I USING (tenant_uuid = nullif(current_setting(''phx.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phx.tenant_uuid'', true), '''')::uuid)', t || '_tenant', t);
  END LOOP;
END $$;

CREATE OR REPLACE FUNCTION phx_deny_predictive_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'predictive evidence is append-only'; END $$;
DO $$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['phx_predictive_route_forecasts','phx_predictive_recommendations','phx_project_predictive_forecasts','phx_forecast_feedback','phx_predictive_events'] LOOP
    EXECUTE format('DROP TRIGGER IF EXISTS %I ON %I', t || '_append_only', t);
    EXECUTE format('CREATE TRIGGER %I BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phx_deny_predictive_mutation()', t || '_append_only', t);
  END LOOP;
END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (46,'0046_predictive_project_intelligence.sql','2bf8902453805dcb9167598f4c2a5c183dea1662fa133e62fd53a8533510a9f1','5ad3ae5877760a5b0125a9d2e40318b5de3e3ce609465269d6f05df7f466274b','original','reparo sintático current_setting/NULLIF: 2 ocorrência(s)') ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0047: 0047_autonomous_project_supervisor.sql
-- source_sha256:    0e1fda91e13eddd2134d4c79ca9885a638008a1dbe785cd2fac3664066d9037d
-- effective_sha256: e5fbde91ffbbacd423fd686b643883f1c66c1d65bf8da08b91628668fcb00fc9
-- source_status:    original
-- repair_notes:     reparo sintático current_setting/NULLIF: 2 ocorrência(s)
-- ============================================================================
-- PhxClaw v0.47 Autonomous Project Supervisor
CREATE TABLE IF NOT EXISTS phx_project_supervisor_health (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL, source_state_sha256 text NOT NULL, payload jsonb NOT NULL, evidence_sha256 text NOT NULL, snapshot_sha256 text NOT NULL, generated_at timestamptz NOT NULL, expires_at timestamptz NOT NULL, PRIMARY KEY (tenant_uuid, project_uuid, snapshot_uuid));
CREATE TABLE IF NOT EXISTS phx_project_supervisor_cycles (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, cycle_uuid uuid NOT NULL, source_state_sha256 text NOT NULL, phase text NOT NULL, payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY (tenant_uuid, project_uuid, cycle_uuid));
CREATE TABLE IF NOT EXISTS phx_project_supervisor_plans (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL, source_state_sha256 text NOT NULL, policy_sha256 text NOT NULL, plan_sha256 text NOT NULL, payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(), expires_at timestamptz NOT NULL, PRIMARY KEY (tenant_uuid, project_uuid, plan_uuid), FOREIGN KEY (tenant_uuid, project_uuid, snapshot_uuid) REFERENCES phx_project_supervisor_health(tenant_uuid, project_uuid, snapshot_uuid));
CREATE TABLE IF NOT EXISTS phx_project_supervisor_actions (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, action_uuid uuid NOT NULL, kind text NOT NULL, status text NOT NULL, requires_approval boolean NOT NULL, evidence_sha256 text, created_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY (tenant_uuid, project_uuid, action_uuid), FOREIGN KEY (tenant_uuid, project_uuid, plan_uuid) REFERENCES phx_project_supervisor_plans(tenant_uuid, project_uuid, plan_uuid));
CREATE TABLE IF NOT EXISTS phx_project_supervisor_events (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, event_uuid uuid PRIMARY KEY, correlation_uuid uuid NOT NULL, event_type text NOT NULL, payload jsonb NOT NULL, evidence_sha256 text NOT NULL, created_at timestamptz NOT NULL DEFAULT now());

DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['phx_project_supervisor_health','phx_project_supervisor_cycles','phx_project_supervisor_plans','phx_project_supervisor_actions','phx_project_supervisor_events'] LOOP
 EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
 EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
 EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I', t);
 EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)', t);
END LOOP; END $$;

CREATE OR REPLACE FUNCTION phx_supervisor_append_only() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only supervisor record'; END $$;
DROP TRIGGER IF EXISTS phx_supervisor_events_append_only ON phx_project_supervisor_events; CREATE TRIGGER phx_supervisor_events_append_only BEFORE UPDATE OR DELETE ON phx_project_supervisor_events FOR EACH ROW EXECUTE FUNCTION phx_supervisor_append_only();
DROP TRIGGER IF EXISTS phx_supervisor_health_append_only ON phx_project_supervisor_health; CREATE TRIGGER phx_supervisor_health_append_only BEFORE UPDATE OR DELETE ON phx_project_supervisor_health FOR EACH ROW EXECUTE FUNCTION phx_supervisor_append_only();

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (47,'0047_autonomous_project_supervisor.sql','0e1fda91e13eddd2134d4c79ca9885a638008a1dbe785cd2fac3664066d9037d','e5fbde91ffbbacd423fd686b643883f1c66c1d65bf8da08b91628668fcb00fc9','original','reparo sintático current_setting/NULLIF: 2 ocorrência(s)') ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0048: 0048_executive_project_control_tower.sql
-- source_sha256:    8be832ebc5c5d80e7a183753609713e7678860f4c2ce5c3395b17ceb31e54baa
-- effective_sha256: 8be832ebc5c5d80e7a183753609713e7678860f4c2ce5c3395b17ceb31e54baa
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.48 Executive Project Control Tower
CREATE TABLE IF NOT EXISTS phx_executive_portfolio_snapshots (
 tenant_uuid uuid NOT NULL,
 snapshot_uuid uuid NOT NULL,
 generated_at timestamptz NOT NULL DEFAULT now(),
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 snapshot_sha256 text NOT NULL CHECK (snapshot_sha256 ~ '^[0-9a-f]{64}$'),
 payload jsonb NOT NULL,
 PRIMARY KEY (tenant_uuid, snapshot_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_project_snapshots (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 snapshot_uuid uuid NOT NULL,
 portfolio_snapshot_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 health_score double precision NOT NULL CHECK (health_score BETWEEN 0 AND 1),
 attention_score double precision NOT NULL,
 payload jsonb NOT NULL,
 PRIMARY KEY (tenant_uuid, project_uuid, snapshot_uuid),
 FOREIGN KEY (tenant_uuid, portfolio_snapshot_uuid) REFERENCES phx_executive_portfolio_snapshots(tenant_uuid, snapshot_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_alerts (
 tenant_uuid uuid NOT NULL,
 alert_uuid uuid NOT NULL,
 project_uuid uuid NULL,
 severity text NOT NULL CHECK (severity IN ('low','medium','high','critical')),
 kind text NOT NULL,
 title text NOT NULL,
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT now(),
 acknowledged_at timestamptz NULL,
 PRIMARY KEY (tenant_uuid, alert_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_decision_requests (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 request_uuid uuid NOT NULL,
 supervisor_plan_uuid uuid NOT NULL,
 action_kind text NOT NULL,
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 requires_approval boolean NOT NULL DEFAULT true,
 status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected','expired')),
 created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, project_uuid, request_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_events (
 tenant_uuid uuid NOT NULL,
 event_uuid uuid NOT NULL,
 project_uuid uuid NULL,
 event_type text NOT NULL,
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 payload jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, event_uuid)
);

DO $$ DECLARE t text; BEGIN
 FOR t IN SELECT unnest(ARRAY['phx_executive_portfolio_snapshots','phx_executive_project_snapshots','phx_executive_alerts','phx_executive_decision_requests','phx_executive_events']) LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
  EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
  EXECUTE format('DROP POLICY IF EXISTS phx_tenant_isolation ON %I', t);
  EXECUTE format($policy$CREATE POLICY phx_tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)$policy$, t);
 END LOOP;
END $$;

CREATE OR REPLACE FUNCTION phx_executive_append_only() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only executive record'; END $$;
DROP TRIGGER IF EXISTS phx_executive_portfolio_append_only ON phx_executive_portfolio_snapshots;
CREATE TRIGGER phx_executive_portfolio_append_only BEFORE UPDATE OR DELETE ON phx_executive_portfolio_snapshots FOR EACH ROW EXECUTE FUNCTION phx_executive_append_only();
DROP TRIGGER IF EXISTS phx_executive_project_append_only ON phx_executive_project_snapshots;
CREATE TRIGGER phx_executive_project_append_only BEFORE UPDATE OR DELETE ON phx_executive_project_snapshots FOR EACH ROW EXECUTE FUNCTION phx_executive_append_only();
DROP TRIGGER IF EXISTS phx_executive_events_append_only ON phx_executive_events;
CREATE TRIGGER phx_executive_events_append_only BEFORE UPDATE OR DELETE ON phx_executive_events FOR EACH ROW EXECUTE FUNCTION phx_executive_append_only();

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (48,'0048_executive_project_control_tower.sql','8be832ebc5c5d80e7a183753609713e7678860f4c2ce5c3395b17ceb31e54baa','8be832ebc5c5d80e7a183753609713e7678860f4c2ce5c3395b17ceb31e54baa','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0049: 0049_executive_decision_center.sql
-- source_sha256:    479fe5f7c635fe075356b9473625e020af22e6c8c083ecb1c6b17c042c52144d
-- effective_sha256: 479fe5f7c635fe075356b9473625e020af22e6c8c083ecb1c6b17c042c52144d
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.49 Executive Decision Center
CREATE TABLE IF NOT EXISTS phx_executive_decision_cases (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 case_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 portfolio_snapshot_sha256 text NOT NULL CHECK (portfolio_snapshot_sha256 ~ '^[0-9a-f]{64}$'),
 trigger_kind text NOT NULL,
 trigger_evidence_sha256 text NOT NULL CHECK (trigger_evidence_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT now(),
 expires_at timestamptz NOT NULL,
 PRIMARY KEY (tenant_uuid, project_uuid, case_uuid),
 UNIQUE (tenant_uuid, case_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_decision_scenarios (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 case_uuid uuid NOT NULL,
 scenario_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 kind text NOT NULL,
 scenario_sha256 text NOT NULL CHECK (scenario_sha256 ~ '^[0-9a-f]{64}$'),
 model_evidence_sha256 text NOT NULL CHECK (model_evidence_sha256 ~ '^[0-9a-f]{64}$'),
 payload jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 expires_at timestamptz NOT NULL,
 PRIMARY KEY (tenant_uuid, project_uuid, case_uuid, scenario_uuid),
 UNIQUE (tenant_uuid, scenario_uuid),
 FOREIGN KEY (tenant_uuid, project_uuid, case_uuid) REFERENCES phx_executive_decision_cases(tenant_uuid, project_uuid, case_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_decision_evaluations (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 case_uuid uuid NOT NULL,
 scenario_uuid uuid NOT NULL,
 evaluation_uuid uuid NOT NULL,
 feasible boolean NOT NULL,
 approval_required boolean NOT NULL,
 utility_score double precision NOT NULL,
 evaluation_sha256 text NOT NULL CHECK (evaluation_sha256 ~ '^[0-9a-f]{64}$'),
 payload jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, evaluation_uuid),
 FOREIGN KEY (tenant_uuid, project_uuid, case_uuid, scenario_uuid) REFERENCES phx_executive_decision_scenarios(tenant_uuid, project_uuid, case_uuid, scenario_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_decision_approvals (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 case_uuid uuid NOT NULL,
 scenario_uuid uuid NOT NULL,
 approval_uuid uuid NOT NULL,
 signer_uuid uuid NOT NULL,
 approval_sha256 text NOT NULL CHECK (approval_sha256 ~ '^[0-9a-f]{64}$'),
 public_key_hex text NOT NULL,
 signature_hex text NOT NULL,
 issued_at timestamptz NOT NULL,
 expires_at timestamptz NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, approval_uuid),
 FOREIGN KEY (tenant_uuid, project_uuid, case_uuid, scenario_uuid) REFERENCES phx_executive_decision_scenarios(tenant_uuid, project_uuid, case_uuid, scenario_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_decision_executions (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 case_uuid uuid NOT NULL,
 scenario_uuid uuid NOT NULL,
 execution_uuid uuid NOT NULL,
 supervisor_plan_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 scenario_sha256 text NOT NULL CHECK (scenario_sha256 ~ '^[0-9a-f]{64}$'),
 approval_sha256 text NULL,
 controller_epoch bigint NOT NULL CHECK (controller_epoch > 0),
 fencing_token bigint NOT NULL CHECK (fencing_token > 0),
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, execution_uuid),
 FOREIGN KEY (tenant_uuid, project_uuid, case_uuid, scenario_uuid) REFERENCES phx_executive_decision_scenarios(tenant_uuid, project_uuid, case_uuid, scenario_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_decision_events (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 case_uuid uuid NOT NULL,
 event_uuid uuid NOT NULL,
 event_type text NOT NULL,
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 payload jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, event_uuid),
 FOREIGN KEY (tenant_uuid, project_uuid, case_uuid) REFERENCES phx_executive_decision_cases(tenant_uuid, project_uuid, case_uuid)
);

DO $$ DECLARE t text; BEGIN
 FOR t IN SELECT unnest(ARRAY['phx_executive_decision_cases','phx_executive_decision_scenarios','phx_executive_decision_evaluations','phx_executive_decision_approvals','phx_executive_decision_executions','phx_executive_decision_events']) LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
  EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
  EXECUTE format('DROP POLICY IF EXISTS phx_tenant_isolation ON %I', t);
  EXECUTE format($policy$CREATE POLICY phx_tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)$policy$, t);
 END LOOP;
END $$;

CREATE OR REPLACE FUNCTION phx_executive_decision_append_only() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only executive decision record'; END $$;
DO $$ DECLARE t text; trig text; BEGIN
 FOR t IN SELECT unnest(ARRAY['phx_executive_decision_cases','phx_executive_decision_scenarios','phx_executive_decision_evaluations','phx_executive_decision_approvals','phx_executive_decision_executions','phx_executive_decision_events']) LOOP
  trig := t || '_append_only';
  EXECUTE format('DROP TRIGGER IF EXISTS %I ON %I',trig,t);
  EXECUTE format('CREATE TRIGGER %I BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phx_executive_decision_append_only()',trig,t);
 END LOOP;
END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (49,'0049_executive_decision_center.sql','479fe5f7c635fe075356b9473625e020af22e6c8c083ecb1c6b17c042c52144d','479fe5f7c635fe075356b9473625e020af22e6c8c083ecb1c6b17c042c52144d','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0050: 0050_portfolio_digital_twin.sql
-- source_sha256:    b4bd978b5e8ddfdd568d6e959d50152559ab40b5c30b63bcd0cddefd966804b3
-- effective_sha256: b4bd978b5e8ddfdd568d6e959d50152559ab40b5c30b63bcd0cddefd966804b3
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.50 Portfolio Digital Twin & Monte Carlo Planning
CREATE TABLE IF NOT EXISTS phx_portfolio_twins (
  tenant_uuid uuid NOT NULL,
  twin_uuid uuid NOT NULL,
  source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
  created_at timestamptz NOT NULL DEFAULT now(),
  twin_json jsonb NOT NULL,
  twin_sha256 text NOT NULL CHECK (twin_sha256 ~ '^[0-9a-f]{64}$'),
  PRIMARY KEY (tenant_uuid, twin_uuid),
  UNIQUE (tenant_uuid, twin_uuid, source_state_sha256)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_scenarios (
  tenant_uuid uuid NOT NULL,
  twin_uuid uuid NOT NULL,
  scenario_uuid uuid NOT NULL,
  source_state_sha256 text NOT NULL,
  scenario_sha256 text NOT NULL CHECK (scenario_sha256 ~ '^[0-9a-f]{64}$'),
  spec jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, scenario_uuid),
  FOREIGN KEY (tenant_uuid, twin_uuid, source_state_sha256) REFERENCES phx_portfolio_twins(tenant_uuid,twin_uuid,source_state_sha256)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_simulation_runs (
  tenant_uuid uuid NOT NULL,
  scenario_uuid uuid NOT NULL,
  run_uuid uuid NOT NULL,
  source_state_sha256 text NOT NULL,
  iterations integer NOT NULL CHECK (iterations BETWEEN 100 AND 100000),
  seed bigint NOT NULL,
  result_json jsonb NOT NULL,
  evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, run_uuid),
  FOREIGN KEY (tenant_uuid, scenario_uuid) REFERENCES phx_portfolio_scenarios(tenant_uuid,scenario_uuid)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_calibration_evidence (
  tenant_uuid uuid NOT NULL,
  evidence_uuid uuid NOT NULL,
  source_state_sha256 text NOT NULL,
  promoted boolean NOT NULL,
  fresh_until timestamptz NOT NULL,
  sample_count integer NOT NULL CHECK (sample_count > 0),
  evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
  evidence_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid,evidence_uuid)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_decision_delegations (
  tenant_uuid uuid NOT NULL,
  decision_case_uuid uuid NOT NULL,
  scenario_uuid uuid NOT NULL,
  simulation_evidence_sha256 text NOT NULL,
  delegate_to text NOT NULL CHECK (delegate_to='executive_decision_center'),
  direct_mutation boolean NOT NULL CHECK (direct_mutation=false),
  delegation_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid,decision_case_uuid),
  FOREIGN KEY (tenant_uuid,scenario_uuid) REFERENCES phx_portfolio_scenarios(tenant_uuid,scenario_uuid)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_events (
  tenant_uuid uuid NOT NULL,
  event_uuid uuid NOT NULL,
  source_state_sha256 text NOT NULL,
  event_type text NOT NULL,
  event_sha256 text NOT NULL CHECK (event_sha256 ~ '^[0-9a-f]{64}$'),
  payload jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid,event_uuid)
);

DO $phx$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['phx_portfolio_twins','phx_portfolio_scenarios','phx_portfolio_simulation_runs','phx_portfolio_calibration_evidence','phx_portfolio_decision_delegations','phx_portfolio_events'] LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
    EXECUTE format($policy$CREATE POLICY %I_tenant ON %I USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)$policy$,t,t);
  END LOOP;
END $phx$;

CREATE OR REPLACE FUNCTION phx_portfolio_append_only() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'append-only table'; END $$;
DO $$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['phx_portfolio_twins','phx_portfolio_scenarios','phx_portfolio_simulation_runs','phx_portfolio_calibration_evidence','phx_portfolio_decision_delegations','phx_portfolio_events'] LOOP
   EXECUTE format('DROP TRIGGER IF EXISTS %I_append_only ON %I',t,t);
   EXECUTE format('CREATE TRIGGER %I_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phx_portfolio_append_only()',t,t);
 END LOOP;
END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (50,'0050_portfolio_digital_twin.sql','b4bd978b5e8ddfdd568d6e959d50152559ab40b5c30b63bcd0cddefd966804b3','b4bd978b5e8ddfdd568d6e959d50152559ab40b5c30b63bcd0cddefd966804b3','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0051: 0051_portfolio_optimizer.sql
-- source_sha256:    4fdde51ff8dbbb237270986cb785ba67be7c619283840a8eff011df5e7dfe2c0
-- effective_sha256: 4fdde51ff8dbbb237270986cb785ba67be7c619283840a8eff011df5e7dfe2c0
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.51 Portfolio Optimizer & Scenario Search
CREATE TABLE IF NOT EXISTS phx_portfolio_optimization_searches (
  tenant_uuid uuid NOT NULL,
  search_uuid uuid NOT NULL,
  twin_uuid uuid NOT NULL,
  source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
  plan_sha256 text NOT NULL CHECK (plan_sha256 ~ '^[0-9a-f]{64}$'),
  spec jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, search_uuid),
  FOREIGN KEY (tenant_uuid, twin_uuid, source_state_sha256) REFERENCES phx_portfolio_twins(tenant_uuid, twin_uuid, source_state_sha256)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_optimization_candidates (
  tenant_uuid uuid NOT NULL,
  search_uuid uuid NOT NULL,
  candidate_uuid uuid NOT NULL,
  candidate_sha256 text NOT NULL CHECK (candidate_sha256 ~ '^[0-9a-f]{64}$'),
  feasible boolean NOT NULL,
  metrics jsonb NOT NULL,
  simulation_evidence_sha256 text NOT NULL CHECK (simulation_evidence_sha256 ~ '^[0-9a-f]{64}$'),
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, search_uuid, candidate_uuid),
  FOREIGN KEY (tenant_uuid, search_uuid) REFERENCES phx_portfolio_optimization_searches(tenant_uuid, search_uuid)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_optimization_frontier (
  tenant_uuid uuid NOT NULL,
  search_uuid uuid NOT NULL,
  candidate_uuid uuid NOT NULL,
  metrics jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, search_uuid, candidate_uuid),
  FOREIGN KEY (tenant_uuid, search_uuid, candidate_uuid) REFERENCES phx_portfolio_optimization_candidates(tenant_uuid, search_uuid, candidate_uuid)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_optimization_recommendations (
  tenant_uuid uuid NOT NULL,
  recommendation_uuid uuid NOT NULL,
  search_uuid uuid NOT NULL,
  candidate_uuid uuid NOT NULL,
  confidence double precision NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
  rank_stability double precision NOT NULL CHECK (rank_stability >= 0 AND rank_stability <= 1),
  recommendation_sha256 text NOT NULL CHECK (recommendation_sha256 ~ '^[0-9a-f]{64}$'),
  direct_mutation boolean NOT NULL DEFAULT false CHECK (direct_mutation=false),
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, recommendation_uuid),
  FOREIGN KEY (tenant_uuid, search_uuid, candidate_uuid) REFERENCES phx_portfolio_optimization_candidates(tenant_uuid, search_uuid, candidate_uuid)
);
CREATE TABLE IF NOT EXISTS phx_portfolio_optimization_events (
  tenant_uuid uuid NOT NULL,
  event_uuid uuid NOT NULL,
  search_uuid uuid NOT NULL,
  event_type text NOT NULL,
  payload jsonb NOT NULL,
  evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_uuid, event_uuid),
  FOREIGN KEY (tenant_uuid, search_uuid) REFERENCES phx_portfolio_optimization_searches(tenant_uuid, search_uuid)
);

DO $rls$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['phx_portfolio_optimization_searches','phx_portfolio_optimization_candidates','phx_portfolio_optimization_frontier','phx_portfolio_optimization_recommendations','phx_portfolio_optimization_events'] LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
    EXECUTE format($policy$CREATE POLICY %I_tenant ON %I USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)$policy$,t,t);
  END LOOP;
END $rls$;

CREATE OR REPLACE FUNCTION phx_portfolio_optimizer_append_only() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only table'; END $$;
DO $trg$ DECLARE t text; BEGIN
  FOREACH t IN ARRAY ARRAY['phx_portfolio_optimization_candidates','phx_portfolio_optimization_frontier','phx_portfolio_optimization_recommendations','phx_portfolio_optimization_events'] LOOP
    EXECUTE format('DROP TRIGGER IF EXISTS %I_append_only ON %I',t,t);
    EXECUTE format('CREATE TRIGGER %I_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phx_portfolio_optimizer_append_only()',t,t);
  END LOOP;
END $trg$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (51,'0051_portfolio_optimizer.sql','4fdde51ff8dbbb237270986cb785ba67be7c619283840a8eff011df5e7dfe2c0','4fdde51ff8dbbb237270986cb785ba67be7c619283840a8eff011df5e7dfe2c0','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0052: 0052_autonomous_portfolio_planner.sql
-- source_sha256:    84065624ef45d2d0e77a8b80d0b816f356cb312bdcfad673ebd0bef3bf34c6f0
-- effective_sha256: 23d36d17d9e57087df3d4aa98d2ae6c300c534dad4c41b4baaebe467489a5b43
-- source_status:    original
-- ============================================================================
CREATE TABLE IF NOT EXISTS phx_portfolio_plans(
 tenant_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'), selected_candidate_hash text NOT NULL CHECK(selected_candidate_hash~'^[a-f0-9]{64}$'), plan_sha256 text NOT NULL CHECK(plan_sha256~'^[a-f0-9]{64}$'), requires_approval boolean NOT NULL, total_budget_reserved numeric NOT NULL CHECK(total_budget_reserved>=0), direct_mutation boolean NOT NULL DEFAULT false CHECK(direct_mutation=false), payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,plan_uuid), UNIQUE(tenant_uuid,plan_uuid,plan_sha256));
CREATE TABLE IF NOT EXISTS phx_portfolio_plan_items(
 tenant_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, project_uuid uuid NOT NULL, sequence_no integer NOT NULL CHECK(sequence_no>0), planned_start_day integer NOT NULL, planned_finish_day integer NOT NULL CHECK(planned_finish_day>=planned_start_day), recommendation text NOT NULL, budget_reservation numeric NOT NULL CHECK(budget_reservation>=0), payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,plan_uuid,project_uuid), FOREIGN KEY(tenant_uuid,plan_uuid) REFERENCES phx_portfolio_plans(tenant_uuid,plan_uuid));
CREATE TABLE IF NOT EXISTS phx_portfolio_resource_reservations(
 tenant_uuid uuid NOT NULL, reservation_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, project_uuid uuid NOT NULL, resource_kind text NOT NULL, resource_key text NOT NULL, start_day integer NOT NULL, finish_day integer NOT NULL CHECK(finish_day>=start_day), amount numeric NOT NULL CHECK(amount>=0), evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,reservation_uuid), FOREIGN KEY(tenant_uuid,plan_uuid) REFERENCES phx_portfolio_plans(tenant_uuid,plan_uuid));
CREATE TABLE IF NOT EXISTS phx_portfolio_plan_delegations(
 tenant_uuid uuid NOT NULL, delegation_uuid uuid NOT NULL, plan_uuid uuid NOT NULL, plan_sha256 text NOT NULL CHECK(plan_sha256~'^[a-f0-9]{64}$'), delegate_to text NOT NULL CHECK(delegate_to='executive_decision_center'), direct_mutation boolean NOT NULL DEFAULT false CHECK(direct_mutation=false), evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,delegation_uuid), FOREIGN KEY(tenant_uuid,plan_uuid,plan_sha256) REFERENCES phx_portfolio_plans(tenant_uuid,plan_uuid,plan_sha256));
CREATE TABLE IF NOT EXISTS phx_portfolio_planner_events(
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, plan_uuid uuid, event_type text NOT NULL, evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'), payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,event_uuid));

DO $ddl$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['phx_portfolio_plans','phx_portfolio_plan_items','phx_portfolio_resource_reservations','phx_portfolio_plan_delegations','phx_portfolio_planner_events'] LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format($p$CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)$p$,t);
 END LOOP;
END $ddl$;

CREATE OR REPLACE FUNCTION phx_portfolio_planner_append_only() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'append-only table';END$$;
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['phx_portfolio_plans','phx_portfolio_plan_items','phx_portfolio_resource_reservations','phx_portfolio_plan_delegations','phx_portfolio_planner_events'] LOOP EXECUTE format('DROP TRIGGER IF EXISTS zz_append_only ON %I',t); EXECUTE format('CREATE TRIGGER zz_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phx_portfolio_planner_append_only()',t); END LOOP; END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (52,'0052_autonomous_portfolio_planner.sql','84065624ef45d2d0e77a8b80d0b816f356cb312bdcfad673ebd0bef3bf34c6f0','23d36d17d9e57087df3d4aa98d2ae6c300c534dad4c41b4baaebe467489a5b43','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0053: 0053_adaptive_portfolio_execution_controller.sql
-- source_sha256:    a8392e1ab7d2bca83e4167320ca648cb6ad2ae7a473272dd0cd6567933a04775
-- effective_sha256: 11a1d54545f22bd5532c4bc066587c0382ab2edde967eaa8190518ef56da8244
-- source_status:    original
-- ============================================================================
CREATE TABLE IF NOT EXISTS phx_portfolio_execution_controllers(
 tenant_uuid uuid NOT NULL, controller_uuid uuid NOT NULL, portfolio_uuid uuid NOT NULL, active_plan_uuid uuid NOT NULL, active_plan_sha256 text NOT NULL CHECK(active_plan_sha256~'^[a-f0-9]{64}$'), source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'), revision_no bigint NOT NULL CHECK(revision_no>=0), controller_epoch bigint NOT NULL CHECK(controller_epoch>0), fencing_token bigint NOT NULL CHECK(fencing_token>0), direct_mutation boolean NOT NULL DEFAULT false CHECK(direct_mutation=false), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,controller_uuid), UNIQUE(tenant_uuid,portfolio_uuid,controller_uuid));
CREATE TABLE IF NOT EXISTS phx_portfolio_execution_observations(
 tenant_uuid uuid NOT NULL, observation_uuid uuid NOT NULL, controller_uuid uuid NOT NULL, project_uuid uuid NOT NULL, observed_at timestamptz NOT NULL, source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'), evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'), payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,observation_uuid), FOREIGN KEY(tenant_uuid,controller_uuid) REFERENCES phx_portfolio_execution_controllers(tenant_uuid,controller_uuid));
CREATE TABLE IF NOT EXISTS phx_portfolio_drift_events(
 tenant_uuid uuid NOT NULL, drift_uuid uuid NOT NULL, controller_uuid uuid NOT NULL, project_uuid uuid NOT NULL, drift_kind text NOT NULL, severity text NOT NULL, magnitude numeric NOT NULL, confirmed boolean NOT NULL DEFAULT false, evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'), payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,drift_uuid), FOREIGN KEY(tenant_uuid,controller_uuid) REFERENCES phx_portfolio_execution_controllers(tenant_uuid,controller_uuid));
CREATE TABLE IF NOT EXISTS phx_portfolio_replan_requests(
 tenant_uuid uuid NOT NULL, request_uuid uuid NOT NULL, controller_uuid uuid NOT NULL, base_plan_uuid uuid NOT NULL, base_plan_sha256 text NOT NULL CHECK(base_plan_sha256~'^[a-f0-9]{64}$'), base_revision_no bigint NOT NULL, source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'), shadow_only boolean NOT NULL DEFAULT true CHECK(shadow_only=true), request_sha256 text NOT NULL CHECK(request_sha256~'^[a-f0-9]{64}$'), payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,request_uuid), UNIQUE(tenant_uuid,request_uuid,request_sha256), FOREIGN KEY(tenant_uuid,controller_uuid) REFERENCES phx_portfolio_execution_controllers(tenant_uuid,controller_uuid));
CREATE TABLE IF NOT EXISTS phx_portfolio_plan_revision_proposals(
 tenant_uuid uuid NOT NULL, revision_uuid uuid NOT NULL, request_uuid uuid NOT NULL, base_plan_uuid uuid NOT NULL, base_plan_sha256 text NOT NULL CHECK(base_plan_sha256~'^[a-f0-9]{64}$'), candidate_plan_sha256 text NOT NULL CHECK(candidate_plan_sha256~'^[a-f0-9]{64}$'), source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'), proposal_sha256 text NOT NULL CHECK(proposal_sha256~'^[a-f0-9]{64}$'), requires_approval boolean NOT NULL DEFAULT true CHECK(requires_approval=true), direct_mutation boolean NOT NULL DEFAULT false CHECK(direct_mutation=false), payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,revision_uuid), UNIQUE(tenant_uuid,revision_uuid,proposal_sha256), FOREIGN KEY(tenant_uuid,request_uuid) REFERENCES phx_portfolio_replan_requests(tenant_uuid,request_uuid));
CREATE TABLE IF NOT EXISTS phx_portfolio_execution_events(
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, controller_uuid uuid NOT NULL, event_type text NOT NULL, source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'), evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'), payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,event_uuid), FOREIGN KEY(tenant_uuid,controller_uuid) REFERENCES phx_portfolio_execution_controllers(tenant_uuid,controller_uuid));

DO $ddl$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['phx_portfolio_execution_controllers','phx_portfolio_execution_observations','phx_portfolio_drift_events','phx_portfolio_replan_requests','phx_portfolio_plan_revision_proposals','phx_portfolio_execution_events'] LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format($p$CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)$p$,t);
 END LOOP;
END $ddl$;
CREATE OR REPLACE FUNCTION phx_portfolio_execution_append_only() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'append-only table';END$$;
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['phx_portfolio_execution_observations','phx_portfolio_drift_events','phx_portfolio_replan_requests','phx_portfolio_plan_revision_proposals','phx_portfolio_execution_events'] LOOP EXECUTE format('DROP TRIGGER IF EXISTS zz_append_only ON %I',t); EXECUTE format('CREATE TRIGGER zz_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phx_portfolio_execution_append_only()',t); END LOOP; END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (53,'0053_adaptive_portfolio_execution_controller.sql','a8392e1ab7d2bca83e4167320ca648cb6ad2ae7a473272dd0cd6567933a04775','11a1d54545f22bd5532c4bc066587c0382ab2edde967eaa8190518ef56da8244','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0054: 0054_consolidation_performance.sql
-- source_sha256:    e28542f6a8eb459392a5efd97a38eb2aec2786fe88c251f92ce0cdf05b31518e
-- effective_sha256: 2e0441b399750bb42ced3539c47188482196276382028f9206d57f95d1039ea4
-- source_status:    original
-- ============================================================================
CREATE TABLE IF NOT EXISTS phx_model_outcomes_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, outcome_uuid uuid NOT NULL,
 task_uuid uuid NOT NULL, agent_uuid uuid NOT NULL, task_class text NOT NULL,
 model_profile_uuid uuid NOT NULL, success boolean NOT NULL, quality_score double precision NOT NULL,
 actual_cost numeric(18,8) NOT NULL, latency_ms bigint NOT NULL, input_tokens bigint NOT NULL,
 output_tokens bigint NOT NULL, failure_signature text, failure_origin text,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, outcome_uuid)
);
CREATE TABLE IF NOT EXISTS phx_agent_model_affinity_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, affinity_uuid uuid NOT NULL,
 agent_uuid uuid NOT NULL, task_class text NOT NULL, model_profile_uuid uuid NOT NULL,
 sample_count integer NOT NULL CHECK(sample_count>=1), success_rate double precision NOT NULL,
 avg_quality double precision NOT NULL, avg_cost numeric(18,8) NOT NULL, avg_latency_ms double precision NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 promoted boolean NOT NULL DEFAULT false, fresh_until timestamptz NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, affinity_uuid)
);
CREATE TABLE IF NOT EXISTS phx_context_budget_decisions_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, decision_uuid uuid NOT NULL,
 task_uuid uuid NOT NULL, requested_tokens integer NOT NULL, budget_tokens integer NOT NULL,
 model_limit_tokens integer NOT NULL, compressed_locally boolean NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 decision_sha256 text NOT NULL CHECK (decision_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, decision_uuid)
);
CREATE TABLE IF NOT EXISTS phx_prompt_plans_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, prompt_uuid uuid NOT NULL,
 task_uuid uuid NOT NULL, model_profile_uuid uuid NOT NULL, provider text NOT NULL,
 prompt_sha256 text NOT NULL CHECK (prompt_sha256 ~ '^[0-9a-f]{64}$'),
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 policy_sha256 text NOT NULL CHECK (policy_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, prompt_uuid)
);
CREATE TABLE IF NOT EXISTS phx_semantic_cache_entries_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, cache_uuid uuid NOT NULL,
 cache_key_sha256 text NOT NULL CHECK (cache_key_sha256 ~ '^[0-9a-f]{64}$'),
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 policy_sha256 text NOT NULL CHECK (policy_sha256 ~ '^[0-9a-f]{64}$'),
 model_profile_uuid uuid NOT NULL, payload_sha256 text NOT NULL CHECK (payload_sha256 ~ '^[0-9a-f]{64}$'),
 fresh_until timestamptz NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, cache_uuid), UNIQUE(tenant_uuid, project_uuid, cache_key_sha256)
);
CREATE TABLE IF NOT EXISTS phx_semantic_cache_events_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, event_uuid uuid NOT NULL,
 cache_uuid uuid NOT NULL, event_kind text NOT NULL CHECK(event_kind IN ('hit','miss','invalidate')),
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, event_uuid)
);
CREATE TABLE IF NOT EXISTS phx_ollama_runtime_samples_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, sample_uuid uuid NOT NULL,
 model text NOT NULL, loaded boolean NOT NULL, size_vram bigint, queue_depth integer,
 latency_ms bigint, tokens_per_second double precision, context_length integer,
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 sampled_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, sample_uuid)
);
CREATE TABLE IF NOT EXISTS phx_performance_events_v054 (
 tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, event_uuid uuid NOT NULL,
 event_kind text NOT NULL, source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid, project_uuid, event_uuid)
);

DO $do$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['phx_model_outcomes_v054','phx_agent_model_affinity_v054','phx_context_budget_decisions_v054','phx_prompt_plans_v054','phx_semantic_cache_entries_v054','phx_semantic_cache_events_v054','phx_ollama_runtime_samples_v054','phx_performance_events_v054'] LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t);
  EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t);
  EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = NULLIF(current_setting($q$phxclaw.tenant_uuid$q$, true), $q$$q$)::uuid) WITH CHECK (tenant_uuid = NULLIF(current_setting($q$phxclaw.tenant_uuid$q$, true), $q$$q$)::uuid)',t);
 END LOOP;
END $do$;

CREATE OR REPLACE FUNCTION phx_v054_append_only() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'append-only table'; END$$;
DO $do$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['phx_model_outcomes_v054','phx_agent_model_affinity_v054','phx_context_budget_decisions_v054','phx_prompt_plans_v054','phx_semantic_cache_entries_v054','phx_semantic_cache_events_v054','phx_ollama_runtime_samples_v054','phx_performance_events_v054'] LOOP
  EXECUTE format('DROP TRIGGER IF EXISTS trg_append_only_v054 ON %I',t);
  EXECUTE format('CREATE TRIGGER trg_append_only_v054 BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phx_v054_append_only()',t);
 END LOOP;
END $do$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (54,'0054_consolidation_performance.sql','e28542f6a8eb459392a5efd97a38eb2aec2786fe88c251f92ce0cdf05b31518e','2e0441b399750bb42ced3539c47188482196276382028f9206d57f95d1039ea4','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0055: 0055_reconciliation_execution_ledger.sql
-- source_sha256:    7a12285f26af8f62a6c3c0952b28698efc2c5ec81cfaa3788cd1b671ff01e2e5
-- effective_sha256: 171116b0cdade030900865db90bec7eb94741470b8120e4fd95eb95b97da8728
-- source_status:    original
-- ============================================================================
CREATE TABLE IF NOT EXISTS phx_reconciliation_plan_snapshots(
 tenant_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL, project_uuid uuid NOT NULL, plan_revision_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'), plan_sha256 text NOT NULL CHECK(plan_sha256~'^[a-f0-9]{64}$'),
 payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,snapshot_uuid), UNIQUE(tenant_uuid,project_uuid,plan_revision_uuid,plan_sha256));
CREATE TABLE IF NOT EXISTS phx_execution_ledger_facts(
 tenant_uuid uuid NOT NULL, fact_uuid uuid NOT NULL, project_uuid uuid NOT NULL, work_item_uuid uuid NOT NULL, sequence_no bigint NOT NULL CHECK(sequence_no>0),
 controller_epoch bigint NOT NULL CHECK(controller_epoch>0), fencing_token bigint NOT NULL CHECK(fencing_token>0), observed_at timestamptz NOT NULL,
 source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'), fact_kind text NOT NULL, idempotency_key text NOT NULL,
 payload_sha256 text NOT NULL CHECK(payload_sha256~'^[a-f0-9]{64}$'), evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'),
 reason_code text, reason_status text NOT NULL CHECK(reason_status IN ('verified','unverified','missing')), causation_uuid uuid, correlation_uuid uuid NOT NULL, payload jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,fact_uuid), UNIQUE(tenant_uuid,project_uuid,sequence_no), UNIQUE(tenant_uuid,idempotency_key,payload_sha256));
CREATE TABLE IF NOT EXISTS phx_reconciliation_variances(
 tenant_uuid uuid NOT NULL, variance_uuid uuid NOT NULL, project_uuid uuid NOT NULL, work_item_uuid uuid NOT NULL, plan_revision_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'), variance_sha256 text NOT NULL CHECK(variance_sha256~'^[a-f0-9]{64}$'),
 reason_status text NOT NULL CHECK(reason_status IN ('verified','unverified','missing')), reason_code text, reason_evidence_sha256 text,
 payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,variance_uuid), UNIQUE(tenant_uuid,variance_uuid,variance_sha256));
CREATE TABLE IF NOT EXISTS phx_reconciliation_causal_links(
 tenant_uuid uuid NOT NULL, link_uuid uuid NOT NULL, project_uuid uuid NOT NULL, cause_fact_uuid uuid NOT NULL, effect_variance_uuid uuid NOT NULL,
 evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'), reason_code text NOT NULL, verified boolean NOT NULL DEFAULT false,
 payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,link_uuid),
 FOREIGN KEY(tenant_uuid,cause_fact_uuid) REFERENCES phx_execution_ledger_facts(tenant_uuid,fact_uuid), FOREIGN KEY(tenant_uuid,effect_variance_uuid) REFERENCES phx_reconciliation_variances(tenant_uuid,variance_uuid));
CREATE TABLE IF NOT EXISTS phx_sprint_gate_evidence(
 tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, sprint_id text NOT NULL, gate_id text NOT NULL,
 source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'), result text NOT NULL CHECK(result IN ('pass','fail','unavailable')),
 evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'), tool_version text NOT NULL, executed_at timestamptz NOT NULL,
 payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,evidence_uuid));
CREATE TABLE IF NOT EXISTS phx_sprint_gate_status_snapshots(
 tenant_uuid uuid NOT NULL, status_uuid uuid NOT NULL, sprint_id text NOT NULL, source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'),
 color text NOT NULL CHECK(color IN ('green','yellow','red')), evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'), payload jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,status_uuid));

DO $ddl$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['phx_reconciliation_plan_snapshots','phx_execution_ledger_facts','phx_reconciliation_variances','phx_reconciliation_causal_links','phx_sprint_gate_evidence','phx_sprint_gate_status_snapshots'] LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format($p$CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)$p$,t);
 END LOOP;
END $ddl$;
CREATE OR REPLACE FUNCTION phx_reconciliation_append_only() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'append-only table';END$$;
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['phx_reconciliation_plan_snapshots','phx_execution_ledger_facts','phx_reconciliation_variances','phx_reconciliation_causal_links','phx_sprint_gate_evidence','phx_sprint_gate_status_snapshots'] LOOP EXECUTE format('DROP TRIGGER IF EXISTS zz_append_only ON %I',t); EXECUTE format('CREATE TRIGGER zz_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phx_reconciliation_append_only()',t); END LOOP; END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (55,'0055_reconciliation_execution_ledger.sql','7a12285f26af8f62a6c3c0952b28698efc2c5ec81cfaa3788cd1b671ff01e2e5','171116b0cdade030900865db90bec7eb94741470b8120e4fd95eb95b97da8728','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0056 — NO-OP
-- v0.56 native qualification green campaign — sem DDL persistente
-- ============================================================================
INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (56,'0056_NO_OP','3f1917d1d401a4ca35e7b24b220a47fcea4e4b8a144d33c473f656a35a4068b1','3f1917d1d401a4ca35e7b24b220a47fcea4e4b8a144d33c473f656a35a4068b1','no_op','v0.56 native qualification green campaign — sem DDL persistente') ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0057: 0057_self_evolving_intelligence.sql
-- source_sha256:    51b2437f01913907be80fbfc45f35ead0864fc9da29f1626f0820d3183a643e1
-- effective_sha256: 51b2437f01913907be80fbfc45f35ead0864fc9da29f1626f0820d3183a643e1
-- source_status:    original
-- ============================================================================
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

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (57,'0057_self_evolving_intelligence.sql','51b2437f01913907be80fbfc45f35ead0864fc9da29f1626f0820d3183a643e1','51b2437f01913907be80fbfc45f35ead0864fc9da29f1626f0820d3183a643e1','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0058: 0058_project_trace_tree.sql
-- source_sha256:    6613b4e54b1db25871580d98391d5264e03b254de8932d90d50b5be584512993
-- effective_sha256: ea36664c221ee2b2eb2e2411772e17b46eca41d13bd7c4d05d1552699ad73cc0
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.58 — hierarchical project control / trace index
CREATE OR REPLACE FUNCTION phx_v058_deny_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'PhxClaw v0.58 append-only relation'; END $$;

-- Reparo native-v070: array_to_string e STABLE e o PostgreSQL recusa coluna gerada
-- que nao seja IMMUTABLE. Para text[] a saida nao depende de configuracao de sessao.
CREATE OR REPLACE FUNCTION phx_immutable_tags_text(p_tags text[]) RETURNS text
  LANGUAGE sql IMMUTABLE PARALLEL SAFE
  AS $fn$ SELECT coalesce(array_to_string(p_tags, ' '), '') $fn$;

CREATE TABLE phx_project_trace_nodes (
  tenant_uuid uuid NOT NULL,
  project_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL,
  parent_uuid uuid,
  root_uuid uuid NOT NULL,
  depth integer NOT NULL CHECK (depth BETWEEN 0 AND 64),
  path_ids uuid[] NOT NULL,
  node_kind text NOT NULL,
  logical_key text NOT NULL,
  title text NOT NULL,
  object_type text,
  object_uuid uuid,
  source_state_sha256 char(64) NOT NULL,
  evidence_sha256 char(64),
  tags text[] NOT NULL DEFAULT '{}',
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
  search_document tsvector GENERATED ALWAYS AS (
    to_tsvector('simple', coalesce(title,'') || ' ' || coalesce(logical_key,'') || ' ' || phx_immutable_tags_text(tags))
  ) STORED,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid, project_uuid, node_uuid),
  FOREIGN KEY (tenant_uuid, project_uuid, parent_uuid)
    REFERENCES phx_project_trace_nodes(tenant_uuid, project_uuid, node_uuid)
);
CREATE INDEX phx_project_trace_nodes_parent_idx ON phx_project_trace_nodes(tenant_uuid,project_uuid,parent_uuid,created_at);
CREATE INDEX phx_project_trace_nodes_path_gin ON phx_project_trace_nodes USING gin(path_ids);
CREATE INDEX phx_project_trace_nodes_search_gin ON phx_project_trace_nodes USING gin(search_document);
CREATE INDEX phx_project_trace_nodes_meta_gin ON phx_project_trace_nodes USING gin(metadata jsonb_path_ops);
CREATE INDEX phx_project_trace_nodes_object_idx ON phx_project_trace_nodes(tenant_uuid,project_uuid,object_type,object_uuid);
CREATE INDEX phx_project_trace_nodes_state_idx ON phx_project_trace_nodes(tenant_uuid,project_uuid,source_state_sha256,node_kind);

CREATE OR REPLACE FUNCTION phx_v058_trace_node_prepare() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE p phx_project_trace_nodes%ROWTYPE;
BEGIN
  IF NEW.parent_uuid IS NULL THEN
    NEW.root_uuid := NEW.node_uuid; NEW.depth := 0; NEW.path_ids := ARRAY[NEW.node_uuid];
  ELSE
    SELECT * INTO STRICT p FROM phx_project_trace_nodes
      WHERE tenant_uuid=NEW.tenant_uuid AND project_uuid=NEW.project_uuid AND node_uuid=NEW.parent_uuid;
    IF NEW.node_uuid = ANY(p.path_ids) THEN RAISE EXCEPTION 'trace cycle detected'; END IF;
    NEW.root_uuid := p.root_uuid; NEW.depth := p.depth + 1; NEW.path_ids := p.path_ids || NEW.node_uuid;
  END IF;
  IF NEW.depth > 64 THEN RAISE EXCEPTION 'trace depth > 64'; END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER phx_project_trace_nodes_prepare BEFORE INSERT ON phx_project_trace_nodes FOR EACH ROW EXECUTE FUNCTION phx_v058_trace_node_prepare();
CREATE TRIGGER phx_project_trace_nodes_immutable BEFORE UPDATE OR DELETE ON phx_project_trace_nodes FOR EACH ROW EXECUTE FUNCTION phx_v058_deny_mutation();

CREATE TABLE phx_project_trace_links (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, link_uuid uuid NOT NULL,
  from_node_uuid uuid NOT NULL, to_node_uuid uuid NOT NULL,
  link_kind text NOT NULL CHECK (link_kind IN ('supports','refutes','derived_from','caused_by','supersedes','tests','implements','uses','produced_by','decided_by','depends_on','related_to')),
  evidence_sha256 char(64), source_state_sha256 char(64) NOT NULL,
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid,project_uuid,link_uuid),
  FOREIGN KEY (tenant_uuid,project_uuid,from_node_uuid) REFERENCES phx_project_trace_nodes(tenant_uuid,project_uuid,node_uuid),
  FOREIGN KEY (tenant_uuid,project_uuid,to_node_uuid) REFERENCES phx_project_trace_nodes(tenant_uuid,project_uuid,node_uuid)
);
CREATE INDEX phx_project_trace_links_from_idx ON phx_project_trace_links(tenant_uuid,project_uuid,from_node_uuid,link_kind);
CREATE INDEX phx_project_trace_links_to_idx ON phx_project_trace_links(tenant_uuid,project_uuid,to_node_uuid,link_kind);
CREATE TRIGGER phx_project_trace_links_immutable BEFORE UPDATE OR DELETE ON phx_project_trace_links FOR EACH ROW EXECUTE FUNCTION phx_v058_deny_mutation();

CREATE TABLE phx_project_source_documents (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, source_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL, source_kind text NOT NULL,
  uri text NOT NULL, content_sha256 char(64) NOT NULL,
  repository text, commit_sha text, line_start integer, line_end integer,
  captured_by_agent_uuid uuid, source_state_sha256 char(64) NOT NULL,
  provenance jsonb NOT NULL DEFAULT '{}'::jsonb,
  search_document tsvector GENERATED ALWAYS AS (to_tsvector('simple', coalesce(uri,'') || ' ' || coalesce(repository,'') || ' ' || coalesce(commit_sha,''))) STORED,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid,project_uuid,source_uuid),
  FOREIGN KEY (tenant_uuid,project_uuid,node_uuid) REFERENCES phx_project_trace_nodes(tenant_uuid,project_uuid,node_uuid),
  CHECK (line_start IS NULL OR line_start > 0), CHECK (line_end IS NULL OR line_end >= line_start)
);
CREATE INDEX phx_project_source_documents_hash_idx ON phx_project_source_documents(tenant_uuid,project_uuid,content_sha256);
CREATE INDEX phx_project_source_documents_search_gin ON phx_project_source_documents USING gin(search_document);
CREATE INDEX phx_project_source_documents_commit_idx ON phx_project_source_documents(tenant_uuid,project_uuid,commit_sha) WHERE commit_sha IS NOT NULL;
CREATE TRIGGER phx_project_source_documents_immutable BEFORE UPDATE OR DELETE ON phx_project_source_documents FOR EACH ROW EXECUTE FUNCTION phx_v058_deny_mutation();

CREATE TABLE phx_project_trace_events (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, event_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL, event_type text NOT NULL, correlation_uuid uuid, causation_uuid uuid,
  source_state_sha256 char(64) NOT NULL, evidence_sha256 char(64) NOT NULL,
  payload jsonb NOT NULL DEFAULT '{}'::jsonb, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid,project_uuid,event_uuid),
  FOREIGN KEY (tenant_uuid,project_uuid,node_uuid) REFERENCES phx_project_trace_nodes(tenant_uuid,project_uuid,node_uuid)
);
CREATE INDEX phx_project_trace_events_node_idx ON phx_project_trace_events(tenant_uuid,project_uuid,node_uuid,created_at);
CREATE INDEX phx_project_trace_events_corr_idx ON phx_project_trace_events(tenant_uuid,project_uuid,correlation_uuid) WHERE correlation_uuid IS NOT NULL;
CREATE TRIGGER phx_project_trace_events_immutable BEFORE UPDATE OR DELETE ON phx_project_trace_events FOR EACH ROW EXECUTE FUNCTION phx_v058_deny_mutation();

-- Generic fast search/index binding for existing domain objects (v0.43/v0.44/v0.45/v0.55/v0.57 etc.).
CREATE TABLE phx_project_trace_bindings (
  tenant_uuid uuid NOT NULL, project_uuid uuid NOT NULL, binding_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL, object_table text NOT NULL, object_uuid uuid NOT NULL,
  object_sha256 char(64), source_state_sha256 char(64) NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (tenant_uuid,project_uuid,binding_uuid),
  UNIQUE (tenant_uuid,project_uuid,object_table,object_uuid),
  FOREIGN KEY (tenant_uuid,project_uuid,node_uuid) REFERENCES phx_project_trace_nodes(tenant_uuid,project_uuid,node_uuid)
);
CREATE INDEX phx_project_trace_bindings_obj_idx ON phx_project_trace_bindings(tenant_uuid,project_uuid,object_uuid,object_table);
CREATE TRIGGER phx_project_trace_bindings_immutable BEFORE UPDATE OR DELETE ON phx_project_trace_bindings FOR EACH ROW EXECUTE FUNCTION phx_v058_deny_mutation();

-- RLS
ALTER TABLE phx_project_trace_nodes ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_project_trace_nodes FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_project_trace_links ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_project_trace_links FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_project_source_documents ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_project_source_documents FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_project_trace_events ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_project_trace_events FORCE ROW LEVEL SECURITY;
ALTER TABLE phx_project_trace_bindings ENABLE ROW LEVEL SECURITY; ALTER TABLE phx_project_trace_bindings FORCE ROW LEVEL SECURITY;
CREATE POLICY phx_project_trace_nodes_tenant ON phx_project_trace_nodes USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_project_trace_links_tenant ON phx_project_trace_links USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_project_source_documents_tenant ON phx_project_source_documents USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_project_trace_events_tenant ON phx_project_trace_events USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);
CREATE POLICY phx_project_trace_bindings_tenant ON phx_project_trace_bindings USING (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid);

-- Hierarchy queries
CREATE OR REPLACE FUNCTION phx_project_trace_descendants(p_node uuid, p_max_depth integer DEFAULT 64)
RETURNS SETOF phx_project_trace_nodes LANGUAGE sql STABLE AS $$
  WITH anchor AS (
    SELECT tenant_uuid,project_uuid,depth FROM phx_project_trace_nodes
    WHERE tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid AND node_uuid=p_node
  )
  SELECT n FROM phx_project_trace_nodes n JOIN anchor a
    ON a.tenant_uuid=n.tenant_uuid AND a.project_uuid=n.project_uuid
  WHERE n.path_ids @> ARRAY[p_node]::uuid[] AND n.depth <= a.depth + LEAST(GREATEST(p_max_depth,0),64)
  ORDER BY n.depth,n.created_at;
$$;
CREATE OR REPLACE FUNCTION phx_project_trace_ancestors(p_node uuid)
RETURNS SETOF phx_project_trace_nodes LANGUAGE sql STABLE AS $$
  SELECT a FROM phx_project_trace_nodes n
  JOIN phx_project_trace_nodes a ON a.tenant_uuid=n.tenant_uuid AND a.project_uuid=n.project_uuid AND a.node_uuid = ANY(n.path_ids)
  WHERE n.tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid AND n.node_uuid=p_node
  ORDER BY a.depth;
$$;

CREATE OR REPLACE VIEW phx_project_decision_trace_v AS
SELECT n.tenant_uuid,n.project_uuid,n.node_uuid AS decision_node_uuid,n.title,n.source_state_sha256,
       count(DISTINCT s.source_uuid) AS source_count,
       count(DISTINCT l.link_uuid) FILTER (WHERE l.link_kind='supports') AS supports_count,
       count(DISTINCT l.link_uuid) FILTER (WHERE l.link_kind='refutes') AS refutes_count
FROM phx_project_trace_nodes n
LEFT JOIN phx_project_source_documents s ON s.tenant_uuid=n.tenant_uuid AND s.project_uuid=n.project_uuid AND s.node_uuid=n.node_uuid
LEFT JOIN phx_project_trace_links l ON l.tenant_uuid=n.tenant_uuid AND l.project_uuid=n.project_uuid AND (l.from_node_uuid=n.node_uuid OR l.to_node_uuid=n.node_uuid)
WHERE n.node_kind='decision'
GROUP BY n.tenant_uuid,n.project_uuid,n.node_uuid,n.title,n.source_state_sha256;


CREATE OR REPLACE FUNCTION phx_project_trace_search(p_query text, p_limit integer DEFAULT 50)
RETURNS TABLE(node_uuid uuid,node_kind text,title text,rank real,source_state_sha256 char(64))
LANGUAGE sql STABLE AS $$
  SELECT n.node_uuid,n.node_kind,n.title,
         ts_rank_cd(n.search_document, websearch_to_tsquery('simple', p_query)) AS rank,
         n.source_state_sha256
  FROM phx_project_trace_nodes n
  WHERE n.tenant_uuid = nullif(current_setting('phx.tenant_uuid', true), '')::uuid
    AND n.search_document @@ websearch_to_tsquery('simple', p_query)
  ORDER BY rank DESC,n.created_at DESC
  LIMIT LEAST(GREATEST(p_limit,1),200);
$$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (58,'0058_project_trace_tree.sql','6613b4e54b1db25871580d98391d5264e03b254de8932d90d50b5be584512993','ea36664c221ee2b2eb2e2411772e17b46eca41d13bd7c4d05d1552699ad73cc0','original','native-v070: array_to_string e STABLE; coluna gerada exige IMMUTABLE -> wrapper phx_immutable_tags_text(text[])') ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0059: 0059_historical_backfill.sql
-- source_sha256:    bae400458067f7641b78f7cddc3ab5a43ad9296ac22aba6cca7a6eccb6321ae0
-- effective_sha256: dafab4579aa8ca07c06278137f3d1cb5b51573d350cef1fff2d4bf53287c68ab
-- source_status:    original
-- repair_notes:     reparo sintático current_setting/NULLIF: 2 ocorrência(s)
-- ============================================================================
-- PhxClaw v0.59 — Historical Backfill & Knowledge Migration
-- Control metadata is auditable; specialized historical tables remain authoritative.
CREATE OR REPLACE FUNCTION phx_v059_deny_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'PhxClaw v0.59 append-only relation'; END $$;

CREATE TABLE phx_historical_backfill_runs (
  tenant_uuid uuid NOT NULL, run_uuid uuid NOT NULL,
  source_state_sha256 char(64) NOT NULL, registry_sha256 char(64) NOT NULL,
  batch_size integer NOT NULL CHECK(batch_size BETWEEN 1 AND 10000),
  mode text NOT NULL CHECK(mode IN ('backfill','retry_orphans','plan_only')),
  requested_by text, started_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY(tenant_uuid,run_uuid)
);
CREATE TRIGGER phx_historical_backfill_runs_immutable BEFORE UPDATE OR DELETE ON phx_historical_backfill_runs FOR EACH ROW EXECUTE FUNCTION phx_v059_deny_mutation();

CREATE TABLE phx_historical_backfill_checkpoints (
  tenant_uuid uuid NOT NULL, run_uuid uuid NOT NULL, checkpoint_uuid uuid NOT NULL,
  source_table text NOT NULL, checkpoint_seq bigint NOT NULL CHECK(checkpoint_seq>0),
  offset_rows bigint NOT NULL CHECK(offset_rows>=0), processed_count bigint NOT NULL CHECK(processed_count>=0),
  inserted_count bigint NOT NULL CHECK(inserted_count>=0), skipped_count bigint NOT NULL CHECK(skipped_count>=0),
  orphan_count bigint NOT NULL CHECK(orphan_count>=0), cursor_json jsonb NOT NULL DEFAULT '{}'::jsonb,
  evidence_sha256 char(64) NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY(tenant_uuid,run_uuid,checkpoint_uuid), UNIQUE(tenant_uuid,run_uuid,source_table,checkpoint_seq),
  FOREIGN KEY(tenant_uuid,run_uuid) REFERENCES phx_historical_backfill_runs(tenant_uuid,run_uuid)
);
CREATE INDEX phx_historical_backfill_checkpoint_latest_idx ON phx_historical_backfill_checkpoints(tenant_uuid,run_uuid,source_table,checkpoint_seq DESC);
CREATE TRIGGER phx_historical_backfill_checkpoints_immutable BEFORE UPDATE OR DELETE ON phx_historical_backfill_checkpoints FOR EACH ROW EXECUTE FUNCTION phx_v059_deny_mutation();

CREATE TABLE phx_historical_backfill_identity_map (
  tenant_uuid uuid NOT NULL, source_table text NOT NULL, source_key_sha256 char(64) NOT NULL,
  object_uuid uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY(tenant_uuid,source_table,source_key_sha256), UNIQUE(tenant_uuid,object_uuid)
);
CREATE TRIGGER phx_historical_backfill_identity_immutable BEFORE UPDATE OR DELETE ON phx_historical_backfill_identity_map FOR EACH ROW EXECUTE FUNCTION phx_v059_deny_mutation();

CREATE TABLE phx_historical_backfill_scope_map (
  tenant_uuid uuid NOT NULL, scope_kind text NOT NULL CHECK(scope_kind IN ('portfolio','tenant')),
  scope_key text NOT NULL, project_uuid uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY(tenant_uuid,scope_kind,scope_key), UNIQUE(tenant_uuid,project_uuid)
);
CREATE TRIGGER phx_historical_backfill_scope_immutable BEFORE UPDATE OR DELETE ON phx_historical_backfill_scope_map FOR EACH ROW EXECUTE FUNCTION phx_v059_deny_mutation();

CREATE TABLE phx_historical_backfill_orphans (
  tenant_uuid uuid NOT NULL, run_uuid uuid NOT NULL, orphan_uuid uuid NOT NULL,
  source_table text NOT NULL, source_key_sha256 char(64) NOT NULL, row_sha256 char(64) NOT NULL,
  reason_code text NOT NULL, project_uuid uuid, candidate_refs jsonb NOT NULL DEFAULT '{}'::jsonb,
  evidence_sha256 char(64) NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY(tenant_uuid,run_uuid,orphan_uuid),
  FOREIGN KEY(tenant_uuid,run_uuid) REFERENCES phx_historical_backfill_runs(tenant_uuid,run_uuid)
);
CREATE INDEX phx_historical_backfill_orphans_reason_idx ON phx_historical_backfill_orphans(tenant_uuid,run_uuid,reason_code,source_table);
CREATE TRIGGER phx_historical_backfill_orphans_immutable BEFORE UPDATE OR DELETE ON phx_historical_backfill_orphans FOR EACH ROW EXECUTE FUNCTION phx_v059_deny_mutation();

CREATE TABLE phx_historical_backfill_coverage (
  tenant_uuid uuid NOT NULL, run_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL,
  source_table text NOT NULL, source_rows bigint NOT NULL CHECK(source_rows>=0),
  processed_rows bigint NOT NULL CHECK(processed_rows>=0), bound_rows bigint NOT NULL CHECK(bound_rows>=0),
  orphan_rows bigint NOT NULL CHECK(orphan_rows>=0), skipped_rows bigint NOT NULL CHECK(skipped_rows>=0),
  coverage_ratio numeric(8,6) NOT NULL CHECK(coverage_ratio BETWEEN 0 AND 1),
  status text NOT NULL CHECK(status IN ('complete','partial','missing_table','failed')),
  evidence_sha256 char(64) NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY(tenant_uuid,run_uuid,snapshot_uuid),
  FOREIGN KEY(tenant_uuid,run_uuid) REFERENCES phx_historical_backfill_runs(tenant_uuid,run_uuid)
);
CREATE INDEX phx_historical_backfill_coverage_idx ON phx_historical_backfill_coverage(tenant_uuid,run_uuid,source_table,created_at DESC);
CREATE TRIGGER phx_historical_backfill_coverage_immutable BEFORE UPDATE OR DELETE ON phx_historical_backfill_coverage FOR EACH ROW EXECUTE FUNCTION phx_v059_deny_mutation();

CREATE TABLE phx_historical_backfill_events (
  tenant_uuid uuid NOT NULL, run_uuid uuid NOT NULL, event_uuid uuid NOT NULL,
  source_table text, event_type text NOT NULL, evidence_sha256 char(64) NOT NULL,
  payload jsonb NOT NULL DEFAULT '{}'::jsonb, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY(tenant_uuid,run_uuid,event_uuid),
  FOREIGN KEY(tenant_uuid,run_uuid) REFERENCES phx_historical_backfill_runs(tenant_uuid,run_uuid)
);
CREATE INDEX phx_historical_backfill_events_idx ON phx_historical_backfill_events(tenant_uuid,run_uuid,source_table,created_at);
CREATE TRIGGER phx_historical_backfill_events_immutable BEFORE UPDATE OR DELETE ON phx_historical_backfill_events FOR EACH ROW EXECUTE FUNCTION phx_v059_deny_mutation();

-- RLS / FORCE RLS for every v0.59 control relation.
DO $phx$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['phx_historical_backfill_runs','phx_historical_backfill_checkpoints','phx_historical_backfill_identity_map','phx_historical_backfill_scope_map','phx_historical_backfill_orphans','phx_historical_backfill_coverage','phx_historical_backfill_events'] LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
    EXECUTE format('CREATE POLICY %I ON %I USING (tenant_uuid = nullif(current_setting(''phx.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phx.tenant_uuid'', true), '''')::uuid)',t||'_tenant',t);
  END LOOP;
END $phx$;

CREATE OR REPLACE VIEW phx_historical_backfill_latest_coverage_v AS
SELECT DISTINCT ON (tenant_uuid,run_uuid,source_table)
 tenant_uuid,run_uuid,source_table,source_rows,processed_rows,bound_rows,orphan_rows,skipped_rows,coverage_ratio,status,created_at
FROM phx_historical_backfill_coverage ORDER BY tenant_uuid,run_uuid,source_table,created_at DESC;

CREATE OR REPLACE VIEW phx_historical_backfill_progress_v AS
SELECT r.tenant_uuid,r.run_uuid,r.source_state_sha256,r.registry_sha256,r.started_at,
       count(c.source_table) FILTER (WHERE c.status='complete') AS completed_tables,
       count(c.source_table) AS observed_tables,
       coalesce(sum(c.source_rows),0) AS source_rows,
       coalesce(sum(c.bound_rows),0) AS bound_rows,
       coalesce(sum(c.orphan_rows),0) AS orphan_rows
FROM phx_historical_backfill_runs r
LEFT JOIN phx_historical_backfill_latest_coverage_v c ON c.tenant_uuid=r.tenant_uuid AND c.run_uuid=r.run_uuid
GROUP BY r.tenant_uuid,r.run_uuid,r.source_state_sha256,r.registry_sha256,r.started_at;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (59,'0059_historical_backfill.sql','bae400458067f7641b78f7cddc3ab5a43ad9296ac22aba6cca7a6eccb6321ae0','dafab4579aa8ca07c06278137f3d1cb5b51573d350cef1fff2d4bf53287c68ab','original','reparo sintático current_setting/NULLIF: 2 ocorrência(s)') ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0060: 0060_source_provenance.sql
-- source_sha256:    cd48ab814ecdb28e428e6bea6109357d53a8483d5d05bdb9901d70a31255374c
-- effective_sha256: cd48ab814ecdb28e428e6bea6109357d53a8483d5d05bdb9901d70a31255374c
-- source_status:    original
-- ============================================================================
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

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (60,'0060_source_provenance.sql','cd48ab814ecdb28e428e6bea6109357d53a8483d5d05bdb9901d70a31255374c','cd48ab814ecdb28e428e6bea6109357d53a8483d5d05bdb9901d70a31255374c','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0061: 0061_safe_source_harvester.sql
-- source_sha256:    7adcf8e43691bd959e11d66ab7ba47924e447a84ba23c53d053d1082eb9daff5
-- effective_sha256: 7adcf8e43691bd959e11d66ab7ba47924e447a84ba23c53d053d1082eb9daff5
-- source_status:    original
-- ============================================================================
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

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (61,'0061_safe_source_harvester.sql','7adcf8e43691bd959e11d66ab7ba47924e447a84ba23c53d053d1082eb9daff5','7adcf8e43691bd959e11d66ab7ba47924e447a84ba23c53d053d1082eb9daff5','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0062: 0062_canonical_state_knowledge_promotion.sql
-- source_sha256:    2cfc215e6a71abc57b9641c1797da66d5d234205b49cb3ebd829ac0c04b0e615
-- effective_sha256: 2cfc215e6a71abc57b9641c1797da66d5d234205b49cb3ebd829ac0c04b0e615
-- source_status:    original
-- ============================================================================
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

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (62,'0062_canonical_state_knowledge_promotion.sql','2cfc215e6a71abc57b9641c1797da66d5d234205b49cb3ebd829ac0c04b0e615','2cfc215e6a71abc57b9641c1797da66d5d234205b49cb3ebd829ac0c04b0e615','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0063: 0063_repo_intelligence_tree_sitter.sql
-- source_sha256:    ec87fa188036d998c8cc47ec38f17dc1e70c820919cf70f722f8b6e8964e3036
-- effective_sha256: ec87fa188036d998c8cc47ec38f17dc1e70c820919cf70f722f8b6e8964e3036
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.63 — Native Repo Intelligence / Tree-sitter evidence store
CREATE SCHEMA IF NOT EXISTS phxclaw;
CREATE TABLE IF NOT EXISTS phxclaw.repo_analysis_runs(
 run_uuid uuid PRIMARY KEY,tenant_uuid uuid NOT NULL,repository_key text NOT NULL,repository_state_sha256 bytea NOT NULL CHECK(octet_length(repository_state_sha256)=32),
 analyzer_version text NOT NULL,tree_sitter_version text NOT NULL,status text NOT NULL CHECK(status IN('completed','partial','failed')),started_at timestamptz NOT NULL,completed_at timestamptz NOT NULL,
 CHECK(completed_at>=started_at),UNIQUE(tenant_uuid,repository_key,repository_state_sha256,analyzer_version));
CREATE TABLE IF NOT EXISTS phxclaw.repo_analysis_files(
 run_uuid uuid NOT NULL REFERENCES phxclaw.repo_analysis_runs(run_uuid) ON DELETE RESTRICT,file_path text NOT NULL,language text NOT NULL,content_sha256 bytea NOT NULL CHECK(octet_length(content_sha256)=32),
 byte_len bigint NOT NULL CHECK(byte_len>=0),line_count bigint NOT NULL CHECK(line_count>=0),parse_status text NOT NULL CHECK(parse_status IN('parsed','parsed_with_errors','unsupported_language','oversize','read_error','parser_error')),parse_error_nodes integer NOT NULL DEFAULT 0 CHECK(parse_error_nodes>=0),named_nodes bigint NOT NULL DEFAULT 0 CHECK(named_nodes>=0),PRIMARY KEY(run_uuid,file_path));
CREATE TABLE IF NOT EXISTS phxclaw.repo_symbols(
 run_uuid uuid NOT NULL REFERENCES phxclaw.repo_analysis_runs(run_uuid) ON DELETE RESTRICT,symbol_id char(64) NOT NULL CHECK(symbol_id~'^[a-f0-9]{64}$'),file_path text NOT NULL,language text NOT NULL,symbol_kind text NOT NULL,symbol_name text NOT NULL,qualified_hint text NOT NULL,
 start_row bigint NOT NULL,start_column bigint NOT NULL,end_row bigint NOT NULL,end_column bigint NOT NULL,complexity integer NOT NULL CHECK(complexity>=1),PRIMARY KEY(run_uuid,symbol_id),FOREIGN KEY(run_uuid,file_path) REFERENCES phxclaw.repo_analysis_files(run_uuid,file_path) ON DELETE RESTRICT);
CREATE TABLE IF NOT EXISTS phxclaw.repo_calls(
 run_uuid uuid NOT NULL REFERENCES phxclaw.repo_analysis_runs(run_uuid) ON DELETE RESTRICT,call_id char(64) NOT NULL CHECK(call_id~'^[a-f0-9]{64}$'),file_path text NOT NULL,caller_symbol_id char(64) NULL,target_text text NOT NULL,target_name text NOT NULL,resolved_symbol_ids jsonb NOT NULL DEFAULT '[]'::jsonb,start_row bigint NOT NULL,start_column bigint NOT NULL,end_row bigint NOT NULL,end_column bigint NOT NULL,PRIMARY KEY(run_uuid,call_id),FOREIGN KEY(run_uuid,file_path) REFERENCES phxclaw.repo_analysis_files(run_uuid,file_path) ON DELETE RESTRICT);
CREATE TABLE IF NOT EXISTS phxclaw.repo_dependencies(
 run_uuid uuid NOT NULL REFERENCES phxclaw.repo_analysis_runs(run_uuid) ON DELETE RESTRICT,dependency_id char(64) NOT NULL CHECK(dependency_id~'^[a-f0-9]{64}$'),file_path text NOT NULL,dependency_kind text NOT NULL,raw_text text NOT NULL,resolved_file text NULL,start_row bigint NOT NULL,start_column bigint NOT NULL,end_row bigint NOT NULL,end_column bigint NOT NULL,PRIMARY KEY(run_uuid,dependency_id),FOREIGN KEY(run_uuid,file_path) REFERENCES phxclaw.repo_analysis_files(run_uuid,file_path) ON DELETE RESTRICT);
CREATE TABLE IF NOT EXISTS phxclaw.repo_hotspots(
 run_uuid uuid NOT NULL REFERENCES phxclaw.repo_analysis_runs(run_uuid) ON DELETE RESTRICT,file_path text NOT NULL,pagerank_ppm integer NOT NULL CHECK(pagerank_ppm BETWEEN 0 AND 1000000),inbound_edges bigint NOT NULL CHECK(inbound_edges>=0),outbound_edges bigint NOT NULL CHECK(outbound_edges>=0),symbol_count bigint NOT NULL CHECK(symbol_count>=0),complexity bigint NOT NULL CHECK(complexity>=0),hotspot_score bigint NOT NULL CHECK(hotspot_score>=0),PRIMARY KEY(run_uuid,file_path),FOREIGN KEY(run_uuid,file_path) REFERENCES phxclaw.repo_analysis_files(run_uuid,file_path) ON DELETE RESTRICT);
CREATE INDEX IF NOT EXISTS repo_symbols_name_idx ON phxclaw.repo_symbols(run_uuid,symbol_name);
CREATE INDEX IF NOT EXISTS repo_calls_target_idx ON phxclaw.repo_calls(run_uuid,target_name);
CREATE INDEX IF NOT EXISTS repo_hotspots_score_idx ON phxclaw.repo_hotspots(run_uuid,hotspot_score DESC);
CREATE OR REPLACE FUNCTION phxclaw.reject_repo_intelligence_mutation() RETURNS trigger LANGUAGE plpgsql AS $fn$ BEGIN RAISE EXCEPTION 'repo intelligence evidence is append-only; create a new analysis run'; END $fn$;
DO $immut$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['repo_analysis_runs','repo_analysis_files','repo_symbols','repo_calls','repo_dependencies','repo_hotspots'] LOOP EXECUTE format('DROP TRIGGER IF EXISTS %I ON phxclaw.%I','trg_'||t||'_immutable',t);EXECUTE format('CREATE TRIGGER %I BEFORE UPDATE OR DELETE ON phxclaw.%I FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_repo_intelligence_mutation()','trg_'||t||'_immutable',t);END LOOP;END $immut$;
DO $rls$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['repo_analysis_runs','repo_analysis_files','repo_symbols','repo_calls','repo_dependencies','repo_hotspots'] LOOP EXECUTE format('ALTER TABLE phxclaw.%I ENABLE ROW LEVEL SECURITY',t);EXECUTE format('ALTER TABLE phxclaw.%I FORCE ROW LEVEL SECURITY',t);EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_v063 ON phxclaw.%I',t);IF t='repo_analysis_runs' THEN EXECUTE 'CREATE POLICY tenant_isolation_v063 ON phxclaw.repo_analysis_runs USING (tenant_uuid=phxclaw.current_tenant_uuid()) WITH CHECK (tenant_uuid=phxclaw.current_tenant_uuid())';ELSE EXECUTE format('CREATE POLICY tenant_isolation_v063 ON phxclaw.%I USING (EXISTS (SELECT 1 FROM phxclaw.repo_analysis_runs r WHERE r.run_uuid=%I.run_uuid AND r.tenant_uuid=phxclaw.current_tenant_uuid())) WITH CHECK (EXISTS (SELECT 1 FROM phxclaw.repo_analysis_runs r WHERE r.run_uuid=%I.run_uuid AND r.tenant_uuid=phxclaw.current_tenant_uuid()))',t,t,t);END IF;END LOOP;END $rls$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (63,'0063_repo_intelligence_tree_sitter.sql','ec87fa188036d998c8cc47ec38f17dc1e70c820919cf70f722f8b6e8964e3036','ec87fa188036d998c8cc47ec38f17dc1e70c820919cf70f722f8b6e8964e3036','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0064: 0064_mcp_lsp_managed_runtime.sql
-- source_sha256:    85b6c8f7166ca173fa0a43e31f0c3f69676b1212a0f18e84a8b64006da69a836
-- effective_sha256: 85b6c8f7166ca173fa0a43e31f0c3f69676b1212a0f18e84a8b64006da69a836
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.64 — MCP/LSP managed runtime evidence store
CREATE TABLE IF NOT EXISTS phxclaw.protocol_sessions (
    session_uuid uuid PRIMARY KEY,
    tenant_uuid uuid NOT NULL,
    kind text NOT NULL CHECK (kind IN ('mcp_stdio','mcp_streamable_http','lsp_stdio')),
    endpoint_label text NOT NULL,
    transport text NOT NULL CHECK (transport IN ('stdio','streamable_http')),
    protocol_version text,
    state text NOT NULL CHECK (state IN ('starting','ready','degraded','closing','closed','failed')),
    restart_count integer NOT NULL DEFAULT 0 CHECK (restart_count >= 0),
    started_at timestamptz NOT NULL DEFAULT now(),
    closed_at timestamptz,
    source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$')
);

CREATE TABLE IF NOT EXISTS phxclaw.protocol_requests (
    request_uuid uuid PRIMARY KEY,
    session_uuid uuid NOT NULL REFERENCES phxclaw.protocol_sessions(session_uuid),
    tenant_uuid uuid NOT NULL,
    jsonrpc_id text,
    method text NOT NULL,
    capability text NOT NULL,
    status text NOT NULL CHECK (status IN ('requested','succeeded','failed','cancelled','timeout','denied')),
    requested_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz,
    duration_ms bigint CHECK (duration_ms IS NULL OR duration_ms >= 0),
    error_code text,
    evidence_uuid uuid
);

CREATE TABLE IF NOT EXISTS phxclaw.protocol_session_events (
    event_uuid uuid PRIMARY KEY,
    session_uuid uuid NOT NULL REFERENCES phxclaw.protocol_sessions(session_uuid),
    tenant_uuid uuid NOT NULL,
    event_type text NOT NULL,
    payload jsonb NOT NULL DEFAULT '{}'::jsonb,
    occurred_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_protocol_requests_session_time
    ON phxclaw.protocol_requests(session_uuid, requested_at DESC);
CREATE INDEX IF NOT EXISTS idx_protocol_events_session_time
    ON phxclaw.protocol_session_events(session_uuid, occurred_at DESC);

CREATE OR REPLACE FUNCTION phxclaw.protocol_append_only()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'protocol audit rows are append-only';
END $$;

DROP TRIGGER IF EXISTS trg_protocol_requests_append_only ON phxclaw.protocol_requests;
CREATE TRIGGER trg_protocol_requests_append_only
BEFORE UPDATE OR DELETE ON phxclaw.protocol_requests
FOR EACH ROW EXECUTE FUNCTION phxclaw.protocol_append_only();

DROP TRIGGER IF EXISTS trg_protocol_events_append_only ON phxclaw.protocol_session_events;
CREATE TRIGGER trg_protocol_events_append_only
BEFORE UPDATE OR DELETE ON phxclaw.protocol_session_events
FOR EACH ROW EXECUTE FUNCTION phxclaw.protocol_append_only();

ALTER TABLE phxclaw.protocol_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.protocol_sessions FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.protocol_requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.protocol_requests FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.protocol_session_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.protocol_session_events FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS protocol_sessions_tenant ON phxclaw.protocol_sessions;
CREATE POLICY protocol_sessions_tenant ON phxclaw.protocol_sessions
USING (tenant_uuid = phxclaw.current_tenant_uuid())
WITH CHECK (tenant_uuid = phxclaw.current_tenant_uuid());
DROP POLICY IF EXISTS protocol_requests_tenant ON phxclaw.protocol_requests;
CREATE POLICY protocol_requests_tenant ON phxclaw.protocol_requests
USING (tenant_uuid = phxclaw.current_tenant_uuid())
WITH CHECK (tenant_uuid = phxclaw.current_tenant_uuid());
DROP POLICY IF EXISTS protocol_events_tenant ON phxclaw.protocol_session_events;
CREATE POLICY protocol_events_tenant ON phxclaw.protocol_session_events
USING (tenant_uuid = phxclaw.current_tenant_uuid())
WITH CHECK (tenant_uuid = phxclaw.current_tenant_uuid());

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (64,'0064_mcp_lsp_managed_runtime.sql','85b6c8f7166ca173fa0a43e31f0c3f69676b1212a0f18e84a8b64006da69a836','85b6c8f7166ca173fa0a43e31f0c3f69676b1212a0f18e84a8b64006da69a836','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0065: 0065_installer_backup_restore_dr.sql
-- source_sha256:    0a9c93b1580f9c99eafc199c252cc7c58055bd3c23bd55f4a9f0167c54473021
-- effective_sha256: af7ba5b43e33629566ec5eae4ae13946b260e3889b6ad5ba3c09c764c77c5cfa
-- source_status:    original
-- ============================================================================
CREATE SCHEMA IF NOT EXISTS phxclaw;

CREATE TABLE IF NOT EXISTS phxclaw.backup_sets (
  backup_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  backup_kind text NOT NULL CHECK (backup_kind IN ('filesystem','postgresql','pre_upgrade','disaster_recovery')),
  source_version text NOT NULL,
  manifest_sha256 char(64) NOT NULL CHECK (manifest_sha256 ~ '^[a-f0-9]{64}$'),
  object_count bigint NOT NULL CHECK (object_count >= 0),
  total_bytes bigint NOT NULL CHECK (total_bytes >= 0),
  repository_uri text NOT NULL,
  state text NOT NULL CHECK (state IN ('creating','verified','failed','expired')),
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  verified_at timestamptz NULL
);

CREATE TABLE IF NOT EXISTS phxclaw.backup_objects (
  backup_uuid uuid NOT NULL REFERENCES phxclaw.backup_sets(backup_uuid) ON DELETE RESTRICT,
  object_path text NOT NULL,
  blob_sha256 char(64) NOT NULL CHECK (blob_sha256 ~ '^[a-f0-9]{64}$'),
  size_bytes bigint NOT NULL CHECK (size_bytes >= 0),
  readonly boolean NOT NULL DEFAULT false,
  PRIMARY KEY (backup_uuid, object_path)
);

CREATE TABLE IF NOT EXISTS phxclaw.restore_runs (
  restore_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  backup_uuid uuid NOT NULL REFERENCES phxclaw.backup_sets(backup_uuid) ON DELETE RESTRICT,
  target_uri text NOT NULL,
  rollback_uri text NULL,
  state text NOT NULL CHECK (state IN ('planned','staging','committed','rolled_back','failed')),
  verified boolean NOT NULL DEFAULT false,
  error_text text NULL,
  started_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  completed_at timestamptz NULL
);

CREATE TABLE IF NOT EXISTS phxclaw.upgrade_runs (
  upgrade_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  from_version text NOT NULL,
  to_version text NOT NULL,
  pre_upgrade_backup_uuid uuid NOT NULL REFERENCES phxclaw.backup_sets(backup_uuid) ON DELETE RESTRICT,
  state text NOT NULL CHECK (state IN ('planned','applying','verifying','committed','rolling_back','rolled_back','failed')),
  apply_succeeded boolean NOT NULL DEFAULT false,
  verify_succeeded boolean NOT NULL DEFAULT false,
  rollback_attempted boolean NOT NULL DEFAULT false,
  rollback_succeeded boolean NOT NULL DEFAULT false,
  started_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  completed_at timestamptz NULL,
  CHECK (from_version <> to_version)
);

CREATE TABLE IF NOT EXISTS phxclaw.disaster_recovery_drills (
  drill_uuid uuid PRIMARY KEY,
  tenant_uuid uuid NOT NULL,
  backup_uuid uuid NOT NULL REFERENCES phxclaw.backup_sets(backup_uuid) ON DELETE RESTRICT,
  restore_uuid uuid NULL REFERENCES phxclaw.restore_runs(restore_uuid) ON DELETE RESTRICT,
  state text NOT NULL CHECK (state IN ('planned','running','passed','failed')),
  target_rpo_seconds bigint NULL CHECK (target_rpo_seconds IS NULL OR target_rpo_seconds >= 0),
  measured_rpo_seconds bigint NULL CHECK (measured_rpo_seconds IS NULL OR measured_rpo_seconds >= 0),
  target_rto_seconds bigint NULL CHECK (target_rto_seconds IS NULL OR target_rto_seconds >= 0),
  measured_rto_seconds bigint NULL CHECK (measured_rto_seconds IS NULL OR measured_rto_seconds >= 0),
  evidence jsonb NOT NULL DEFAULT '{}'::jsonb,
  started_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  completed_at timestamptz NULL
);

CREATE INDEX IF NOT EXISTS backup_sets_tenant_created_idx ON phxclaw.backup_sets(tenant_uuid, created_at DESC);
CREATE INDEX IF NOT EXISTS restore_runs_tenant_started_idx ON phxclaw.restore_runs(tenant_uuid, started_at DESC);
CREATE INDEX IF NOT EXISTS upgrade_runs_tenant_started_idx ON phxclaw.upgrade_runs(tenant_uuid, started_at DESC);

CREATE OR REPLACE FUNCTION phxclaw.reject_backup_evidence_delete() RETURNS trigger LANGUAGE plpgsql AS $fn$
BEGIN
  RAISE EXCEPTION 'installer/backup evidence cannot be deleted; create a compensating record';
END $fn$;

DO $trg$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['backup_sets','backup_objects','restore_runs','upgrade_runs','disaster_recovery_drills'] LOOP
    EXECUTE format('DROP TRIGGER IF EXISTS %I ON phxclaw.%I','trg_'||t||'_no_delete',t);
    EXECUTE format('CREATE TRIGGER %I BEFORE DELETE ON phxclaw.%I FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_backup_evidence_delete()','trg_'||t||'_no_delete',t);
  END LOOP;
END $trg$;

DO $rls$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['backup_sets','restore_runs','upgrade_runs','disaster_recovery_drills'] LOOP
    EXECUTE format('ALTER TABLE phxclaw.%I ENABLE ROW LEVEL SECURITY',t);
    EXECUTE format('ALTER TABLE phxclaw.%I FORCE ROW LEVEL SECURITY',t);
    EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_v065 ON phxclaw.%I',t);
    EXECUTE format('CREATE POLICY tenant_isolation_v065 ON phxclaw.%I USING (tenant_uuid=phxclaw.current_tenant_uuid()) WITH CHECK (tenant_uuid=phxclaw.current_tenant_uuid())',t);
  END LOOP;
  ALTER TABLE phxclaw.backup_objects ENABLE ROW LEVEL SECURITY;
  ALTER TABLE phxclaw.backup_objects FORCE ROW LEVEL SECURITY;
  DROP POLICY IF EXISTS tenant_isolation_v065 ON phxclaw.backup_objects;
  CREATE POLICY tenant_isolation_v065 ON phxclaw.backup_objects
    USING (EXISTS (SELECT 1 FROM phxclaw.backup_sets b WHERE b.backup_uuid=backup_objects.backup_uuid AND b.tenant_uuid=phxclaw.current_tenant_uuid()))
    WITH CHECK (EXISTS (SELECT 1 FROM phxclaw.backup_sets b WHERE b.backup_uuid=backup_objects.backup_uuid AND b.tenant_uuid=phxclaw.current_tenant_uuid()));
END $rls$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (65,'0065_installer_backup_restore_dr.sql','0a9c93b1580f9c99eafc199c252cc7c58055bd3c23bd55f4a9f0167c54473021','af7ba5b43e33629566ec5eae4ae13946b260e3889b6ad5ba3c09c764c77c5cfa','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0066: 0066_research_hypothesis_persistence.sql
-- source_sha256:    0d097922a8ff4d63fc2fdc9cbb43a7676c07ca5e989c49072ed9bd4e45859fa7
-- effective_sha256: eb7d79e01b54c816148f74da51533f0c475b713e3c52a0bae202ba28ffc5ebc8
-- source_status:    original
-- ============================================================================
CREATE SCHEMA IF NOT EXISTS phxclaw;
CREATE TABLE IF NOT EXISTS phxclaw.research_records(
 research_uuid uuid PRIMARY KEY,tenant_uuid uuid NOT NULL,project_uuid uuid NULL,correlation_uuid uuid NOT NULL,topic text NOT NULL,question text NOT NULL,findings jsonb NOT NULL DEFAULT '[]'::jsonb,suggested_hypotheses jsonb NOT NULL DEFAULT '[]'::jsonb,source_state_sha256 char(64) NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'),created_at timestamptz NOT NULL);
CREATE TABLE IF NOT EXISTS phxclaw.research_record_evidence(
 research_uuid uuid NOT NULL REFERENCES phxclaw.research_records(research_uuid) ON DELETE RESTRICT,evidence_uuid uuid NOT NULL,uri text NOT NULL,source_type text NOT NULL,retrieved_at timestamptz NOT NULL,sha256 char(64) NOT NULL CHECK(sha256~'^[a-f0-9]{64}$'),notes text NULL,PRIMARY KEY(research_uuid,evidence_uuid));
CREATE TABLE IF NOT EXISTS phxclaw.research_graph_links(
 research_uuid uuid NOT NULL REFERENCES phxclaw.research_records(research_uuid) ON DELETE RESTRICT,knowledge_node_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_nodes(node_uuid) ON DELETE RESTRICT,role text NOT NULL CHECK(role IN('raw_source','claim','evidence','decision','related')),PRIMARY KEY(research_uuid,knowledge_node_uuid,role));

CREATE TABLE IF NOT EXISTS phxclaw.hypothesis_records(
 hypothesis_uuid uuid PRIMARY KEY,tenant_uuid uuid NOT NULL,research_uuid uuid NULL REFERENCES phxclaw.research_records(research_uuid) ON DELETE RESTRICT,statement text NOT NULL,rationale text NOT NULL,experiment_uuid uuid NOT NULL UNIQUE,current_status text NOT NULL CHECK(current_status IN('proposed','testing','supported','rejected','inconclusive')),source_state_sha256 char(64) NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'),created_at timestamptz NOT NULL,updated_at timestamptz NOT NULL);
CREATE TABLE IF NOT EXISTS phxclaw.experiment_plans(
 experiment_uuid uuid PRIMARY KEY,hypothesis_uuid uuid NOT NULL UNIQUE REFERENCES phxclaw.hypothesis_records(hypothesis_uuid) ON DELETE RESTRICT,steps jsonb NOT NULL DEFAULT '[]'::jsonb,success_criteria jsonb NOT NULL DEFAULT '[]'::jsonb,failure_criteria jsonb NOT NULL DEFAULT '[]'::jsonb);
CREATE TABLE IF NOT EXISTS phxclaw.hypothesis_evidence(
 hypothesis_uuid uuid NOT NULL REFERENCES phxclaw.hypothesis_records(hypothesis_uuid) ON DELETE RESTRICT,evidence_uuid uuid NOT NULL,uri text NOT NULL,source_type text NOT NULL,retrieved_at timestamptz NOT NULL,sha256 char(64) NOT NULL CHECK(sha256~'^[a-f0-9]{64}$'),notes text NULL,PRIMARY KEY(hypothesis_uuid,evidence_uuid));
CREATE TABLE IF NOT EXISTS phxclaw.hypothesis_decisions(
 decision_uuid uuid PRIMARY KEY,hypothesis_uuid uuid NOT NULL REFERENCES phxclaw.hypothesis_records(hypothesis_uuid) ON DELETE RESTRICT,from_status text NULL CHECK(from_status IS NULL OR from_status IN('proposed','testing','supported','rejected','inconclusive')),to_status text NOT NULL CHECK(to_status IN('proposed','testing','supported','rejected','inconclusive')),note text NOT NULL,actor text NOT NULL,decided_at timestamptz NOT NULL DEFAULT clock_timestamp(),import_order integer NULL,UNIQUE(hypothesis_uuid,import_order));
CREATE TABLE IF NOT EXISTS phxclaw.experiment_runs(
 run_uuid uuid PRIMARY KEY,tenant_uuid uuid NOT NULL,hypothesis_uuid uuid NOT NULL REFERENCES phxclaw.hypothesis_records(hypothesis_uuid) ON DELETE RESTRICT,state text NOT NULL CHECK(state IN('running','completed','failed','cancelled')),success boolean NULL,result_summary jsonb NOT NULL DEFAULT '{}'::jsonb,started_at timestamptz NOT NULL,completed_at timestamptz NULL,CHECK((state='running' AND completed_at IS NULL) OR state<>'running'));
CREATE TABLE IF NOT EXISTS phxclaw.experiment_run_evidence(
 run_uuid uuid NOT NULL REFERENCES phxclaw.experiment_runs(run_uuid) ON DELETE RESTRICT,evidence_uuid uuid NOT NULL,uri text NOT NULL,source_type text NOT NULL,retrieved_at timestamptz NOT NULL,sha256 char(64) NOT NULL CHECK(sha256~'^[a-f0-9]{64}$'),notes text NULL,PRIMARY KEY(run_uuid,evidence_uuid));
CREATE TABLE IF NOT EXISTS phxclaw.hypothesis_graph_links(
 hypothesis_uuid uuid NOT NULL REFERENCES phxclaw.hypothesis_records(hypothesis_uuid) ON DELETE RESTRICT,knowledge_node_uuid uuid NOT NULL REFERENCES phxclaw.knowledge_nodes(node_uuid) ON DELETE RESTRICT,role text NOT NULL CHECK(role IN('hypothesis','claim','evidence','decision','experiment')),PRIMARY KEY(hypothesis_uuid,knowledge_node_uuid,role));

CREATE INDEX IF NOT EXISTS research_records_tenant_created_idx ON phxclaw.research_records(tenant_uuid,created_at DESC);
CREATE INDEX IF NOT EXISTS hypothesis_records_tenant_status_idx ON phxclaw.hypothesis_records(tenant_uuid,current_status,updated_at DESC);
CREATE INDEX IF NOT EXISTS experiment_runs_tenant_started_idx ON phxclaw.experiment_runs(tenant_uuid,started_at DESC);

CREATE OR REPLACE FUNCTION phxclaw.reject_research_hypothesis_delete() RETURNS trigger LANGUAGE plpgsql AS $fn$ BEGIN RAISE EXCEPTION 'research/hypothesis evidence cannot be deleted'; END $fn$;
DO $no_delete$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['research_records','research_record_evidence','research_graph_links','hypothesis_records','experiment_plans','hypothesis_evidence','hypothesis_decisions','experiment_runs','experiment_run_evidence','hypothesis_graph_links'] LOOP EXECUTE format('DROP TRIGGER IF EXISTS %I ON phxclaw.%I','trg_'||t||'_no_delete',t);EXECUTE format('CREATE TRIGGER %I BEFORE DELETE ON phxclaw.%I FOR EACH ROW EXECUTE FUNCTION phxclaw.reject_research_hypothesis_delete()','trg_'||t||'_no_delete',t);END LOOP;END $no_delete$;

DO $rls$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['research_records','hypothesis_records','experiment_runs'] LOOP EXECUTE format('ALTER TABLE phxclaw.%I ENABLE ROW LEVEL SECURITY',t);EXECUTE format('ALTER TABLE phxclaw.%I FORCE ROW LEVEL SECURITY',t);EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_v066 ON phxclaw.%I',t);EXECUTE format('CREATE POLICY tenant_isolation_v066 ON phxclaw.%I USING (tenant_uuid=phxclaw.current_tenant_uuid()) WITH CHECK (tenant_uuid=phxclaw.current_tenant_uuid())',t);END LOOP;
 FOREACH t IN ARRAY ARRAY['research_record_evidence','research_graph_links'] LOOP EXECUTE format('ALTER TABLE phxclaw.%I ENABLE ROW LEVEL SECURITY',t);EXECUTE format('ALTER TABLE phxclaw.%I FORCE ROW LEVEL SECURITY',t);EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_v066 ON phxclaw.%I',t);EXECUTE format('CREATE POLICY tenant_isolation_v066 ON phxclaw.%I USING (EXISTS(SELECT 1 FROM phxclaw.research_records r WHERE r.research_uuid=%I.research_uuid AND r.tenant_uuid=phxclaw.current_tenant_uuid())) WITH CHECK (EXISTS(SELECT 1 FROM phxclaw.research_records r WHERE r.research_uuid=%I.research_uuid AND r.tenant_uuid=phxclaw.current_tenant_uuid()))',t,t,t);END LOOP;
 FOREACH t IN ARRAY ARRAY['experiment_plans','hypothesis_evidence','hypothesis_decisions','hypothesis_graph_links'] LOOP EXECUTE format('ALTER TABLE phxclaw.%I ENABLE ROW LEVEL SECURITY',t);EXECUTE format('ALTER TABLE phxclaw.%I FORCE ROW LEVEL SECURITY',t);EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_v066 ON phxclaw.%I',t);EXECUTE format('CREATE POLICY tenant_isolation_v066 ON phxclaw.%I USING (EXISTS(SELECT 1 FROM phxclaw.hypothesis_records h WHERE h.hypothesis_uuid=%I.hypothesis_uuid AND h.tenant_uuid=phxclaw.current_tenant_uuid())) WITH CHECK (EXISTS(SELECT 1 FROM phxclaw.hypothesis_records h WHERE h.hypothesis_uuid=%I.hypothesis_uuid AND h.tenant_uuid=phxclaw.current_tenant_uuid()))',t,t,t);END LOOP;
 ALTER TABLE phxclaw.experiment_run_evidence ENABLE ROW LEVEL SECURITY;ALTER TABLE phxclaw.experiment_run_evidence FORCE ROW LEVEL SECURITY;DROP POLICY IF EXISTS tenant_isolation_v066 ON phxclaw.experiment_run_evidence;CREATE POLICY tenant_isolation_v066 ON phxclaw.experiment_run_evidence USING(EXISTS(SELECT 1 FROM phxclaw.experiment_runs x WHERE x.run_uuid=experiment_run_evidence.run_uuid AND x.tenant_uuid=phxclaw.current_tenant_uuid())) WITH CHECK(EXISTS(SELECT 1 FROM phxclaw.experiment_runs x WHERE x.run_uuid=experiment_run_evidence.run_uuid AND x.tenant_uuid=phxclaw.current_tenant_uuid()));
END $rls$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (66,'0066_research_hypothesis_persistence.sql','0d097922a8ff4d63fc2fdc9cbb43a7676c07ca5e989c49072ed9bd4e45859fa7','eb7d79e01b54c816148f74da51533f0c475b713e3c52a0bae202ba28ffc5ebc8','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0067: 0067_bpm_crash_safe_runtime.sql
-- source_sha256:    2d4d6d2b5310a7dabf6cd6fec5906b9b9e8f589edac50f1e5967aca52ff79cfc
-- effective_sha256: 7b312370b68aa95bea3284fadd0453849c6e90978f5a4bab2ed487b70cfcf625
-- source_status:    original
-- ============================================================================
CREATE SCHEMA IF NOT EXISTS phxclaw;
CREATE TABLE IF NOT EXISTS phxclaw.bpm_process_definitions(process_uuid uuid PRIMARY KEY,tenant_uuid uuid NOT NULL,external_id text NULL,version integer NOT NULL CHECK(version>0),bpmn_xml text NOT NULL,compiled_graph jsonb NOT NULL,content_sha256 char(64) NOT NULL CHECK(content_sha256~'^[a-f0-9]{64}$'),created_at timestamptz NOT NULL DEFAULT clock_timestamp(),UNIQUE(tenant_uuid,external_id,version));
CREATE TABLE IF NOT EXISTS phxclaw.bpm_instances_v2(instance_uuid uuid PRIMARY KEY,tenant_uuid uuid NOT NULL,process_uuid uuid NOT NULL REFERENCES phxclaw.bpm_process_definitions(process_uuid) ON DELETE RESTRICT,state text NOT NULL CHECK(state IN('running','completed','failed','cancelled')),variables jsonb NOT NULL DEFAULT '{}'::jsonb,started_at timestamptz NOT NULL DEFAULT clock_timestamp(),completed_at timestamptz NULL);
CREATE TABLE IF NOT EXISTS phxclaw.bpm_tokens_v2(token_uuid uuid PRIMARY KEY,tenant_uuid uuid NOT NULL,instance_uuid uuid NOT NULL REFERENCES phxclaw.bpm_instances_v2(instance_uuid) ON DELETE RESTRICT,node_id text NOT NULL,state text NOT NULL CHECK(state IN('ready','leased','completed','failed')),lease_owner text NULL,lease_until timestamptz NULL,fencing_token bigint NOT NULL DEFAULT 0 CHECK(fencing_token>=0),updated_at timestamptz NOT NULL DEFAULT clock_timestamp());
CREATE TABLE IF NOT EXISTS phxclaw.bpm_events_v2(sequence bigserial PRIMARY KEY,event_uuid uuid NOT NULL UNIQUE,tenant_uuid uuid NOT NULL,instance_uuid uuid NOT NULL REFERENCES phxclaw.bpm_instances_v2(instance_uuid) ON DELETE RESTRICT,token_uuid uuid NULL,node_id text NULL,event_type text NOT NULL,fencing_token bigint NULL,payload jsonb NOT NULL DEFAULT '{}'::jsonb,occurred_at timestamptz NOT NULL DEFAULT clock_timestamp());
CREATE TABLE IF NOT EXISTS phxclaw.bpm_checkpoints(checkpoint_uuid uuid PRIMARY KEY,tenant_uuid uuid NOT NULL,instance_uuid uuid NOT NULL REFERENCES phxclaw.bpm_instances_v2(instance_uuid) ON DELETE RESTRICT,through_sequence bigint NOT NULL CHECK(through_sequence>=0),state_snapshot jsonb NOT NULL,created_at timestamptz NOT NULL DEFAULT clock_timestamp(),UNIQUE(instance_uuid,through_sequence));
CREATE INDEX IF NOT EXISTS bpm_tokens_ready_idx ON phxclaw.bpm_tokens_v2(tenant_uuid,state,lease_until,updated_at) WHERE state IN('ready','leased');
CREATE INDEX IF NOT EXISTS bpm_events_instance_seq_idx ON phxclaw.bpm_events_v2(instance_uuid,sequence);
CREATE OR REPLACE FUNCTION phxclaw.bpm_event_append_only() RETURNS trigger LANGUAGE plpgsql AS $fn$ BEGIN RAISE EXCEPTION 'BPM event journal is append-only'; END $fn$;
DROP TRIGGER IF EXISTS trg_bpm_events_v2_append_only ON phxclaw.bpm_events_v2;CREATE TRIGGER trg_bpm_events_v2_append_only BEFORE UPDATE OR DELETE ON phxclaw.bpm_events_v2 FOR EACH ROW EXECUTE FUNCTION phxclaw.bpm_event_append_only();
DO $rls$ DECLARE t text;BEGIN FOREACH t IN ARRAY ARRAY['bpm_process_definitions','bpm_instances_v2','bpm_tokens_v2','bpm_events_v2','bpm_checkpoints'] LOOP EXECUTE format('ALTER TABLE phxclaw.%I ENABLE ROW LEVEL SECURITY',t);EXECUTE format('ALTER TABLE phxclaw.%I FORCE ROW LEVEL SECURITY',t);EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_v067 ON phxclaw.%I',t);EXECUTE format('CREATE POLICY tenant_isolation_v067 ON phxclaw.%I USING (tenant_uuid=phxclaw.current_tenant_uuid()) WITH CHECK (tenant_uuid=phxclaw.current_tenant_uuid())',t);END LOOP;END $rls$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (67,'0067_bpm_crash_safe_runtime.sql','2d4d6d2b5310a7dabf6cd6fec5906b9b9e8f589edac50f1e5967aca52ff79cfc','7b312370b68aa95bea3284fadd0453849c6e90978f5a4bab2ed487b70cfcf625','original',NULL) ON CONFLICT (version) DO NOTHING;


-- ============================================================================
-- MIGRATION 0068: 0068_channel_provider_runtime.sql
-- source_sha256:    4f85d19d6a5ea60788a6bf530f2fb68a7dd9c98cd61a8342cd7a9c7aae4b3211
-- effective_sha256: 4f85d19d6a5ea60788a6bf530f2fb68a7dd9c98cd61a8342cd7a9c7aae4b3211
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.68 / F21 Channel Provider Runtime
CREATE SCHEMA IF NOT EXISTS phxclaw;

CREATE TABLE IF NOT EXISTS phxclaw.channel_provider_accounts (
  tenant_uuid uuid NOT NULL,
  provider_uuid uuid NOT NULL PRIMARY KEY,
  channel text NOT NULL CHECK (channel IN ('telegram','whatsapp','slack','discord','teams')),
  provider_id text NOT NULL,
  account_id text NOT NULL,
  secret_uuid uuid NOT NULL,
  endpoint_origin text NOT NULL,
  state text NOT NULL DEFAULT 'active' CHECK (state IN ('active','disabled','quarantined')),
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_uuid, channel, account_id)
);

CREATE TABLE IF NOT EXISTS phxclaw.channel_delivery_attempts (
  tenant_uuid uuid NOT NULL,
  attempt_uuid uuid PRIMARY KEY,
  outbound_message_uuid uuid NOT NULL,
  provider_uuid uuid NOT NULL REFERENCES phxclaw.channel_provider_accounts(provider_uuid) ON DELETE RESTRICT,
  idempotency_key text NOT NULL,
  state text NOT NULL CHECK (state IN ('queued','sending','sent','retryable','failed','dead_letter')),
  attempt_no integer NOT NULL DEFAULT 0 CHECK (attempt_no >= 0),
  provider_message_id text,
  next_attempt_at timestamptz,
  last_http_status integer,
  last_error_code text,
  last_error_redacted text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (tenant_uuid, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_channel_delivery_ready
  ON phxclaw.channel_delivery_attempts(tenant_uuid, state, next_attempt_at, created_at);

ALTER TABLE phxclaw.channel_provider_accounts ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.channel_provider_accounts FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.channel_delivery_attempts ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.channel_delivery_attempts FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation ON phxclaw.channel_provider_accounts;
CREATE POLICY tenant_isolation ON phxclaw.channel_provider_accounts
  USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.channel_delivery_attempts;
CREATE POLICY tenant_isolation ON phxclaw.channel_delivery_attempts
  USING (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_uuid = NULLIF(current_setting('phxclaw.tenant_id', true), '')::uuid);

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (68,'0068_channel_provider_runtime.sql','4f85d19d6a5ea60788a6bf530f2fb68a7dd9c98cd61a8342cd7a9c7aae4b3211','4f85d19d6a5ea60788a6bf530f2fb68a7dd9c98cd61a8342cd7a9c7aae4b3211','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0069: 0069_device_pairing_transport.sql
-- source_sha256:    c5ec56233461b0817c4262cd8d29ec3ed906f276a8d89981626a67cd623a5912
-- effective_sha256: c5ec56233461b0817c4262cd8d29ec3ed906f276a8d89981626a67cd623a5912
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.69 / F22 Device pairing + transport hardening
CREATE SCHEMA IF NOT EXISTS phxclaw;

CREATE TABLE IF NOT EXISTS phxclaw.device_platform_state (
  tenant_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL,
  platform text NOT NULL CHECK (platform IN ('windows','linux','macos','android','ios')),
  agent_version text NOT NULL,
  transport text NOT NULL DEFAULT 'wss' CHECK (transport IN ('wss')),
  transport_state text NOT NULL DEFAULT 'offline' CHECK (transport_state IN ('offline','connecting','online','degraded','quarantined')),
  connected_at timestamptz,
  disconnected_at timestamptz,
  last_heartbeat_at timestamptz,
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
  PRIMARY KEY (tenant_uuid,node_uuid),
  FOREIGN KEY (tenant_uuid,node_uuid) REFERENCES phxclaw.device_nodes(tenant_uuid,node_uuid) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS phxclaw.device_pairing_audit (
  tenant_uuid uuid NOT NULL,
  pairing_uuid uuid PRIMARY KEY,
  enrollment_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL,
  token_sha256 bytea NOT NULL CHECK (octet_length(token_sha256)=32),
  public_key_ed25519 bytea NOT NULL CHECK (octet_length(public_key_ed25519)=32),
  result text NOT NULL CHECK (result IN ('accepted','rejected','expired','replayed')),
  reason text,
  recorded_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_device_platform_online ON phxclaw.device_platform_state(tenant_uuid,transport_state,last_heartbeat_at);
CREATE INDEX IF NOT EXISTS idx_device_pairing_audit_node ON phxclaw.device_pairing_audit(tenant_uuid,node_uuid,recorded_at DESC);

ALTER TABLE phxclaw.device_platform_state ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_platform_state FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_pairing_audit ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_pairing_audit FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.device_platform_state;
CREATE POLICY tenant_isolation ON phxclaw.device_platform_state USING (tenant_uuid=NULLIF(current_setting('phxclaw.tenant_id',true),'')::uuid) WITH CHECK (tenant_uuid=NULLIF(current_setting('phxclaw.tenant_id',true),'')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.device_pairing_audit;
CREATE POLICY tenant_isolation ON phxclaw.device_pairing_audit USING (tenant_uuid=NULLIF(current_setting('phxclaw.tenant_id',true),'')::uuid) WITH CHECK (tenant_uuid=NULLIF(current_setting('phxclaw.tenant_id',true),'')::uuid);

CREATE OR REPLACE FUNCTION phxclaw.consume_device_enrollment(
  p_tenant uuid,p_token_sha256 bytea,p_node uuid,p_display_name text,p_public_key bytea,p_platform text,p_agent_version text
) RETURNS uuid LANGUAGE plpgsql AS $$
DECLARE v_enrollment uuid; v_session uuid:=gen_random_uuid();
BEGIN
  SELECT enrollment_uuid INTO v_enrollment FROM phxclaw.device_enrollment_tokens
   WHERE tenant_uuid=p_tenant AND token_sha256=p_token_sha256 AND consumed_at IS NULL AND expires_at>now()
   FOR UPDATE;
  IF v_enrollment IS NULL THEN RAISE EXCEPTION 'invalid, expired, or consumed enrollment token'; END IF;
  INSERT INTO phxclaw.device_nodes(node_uuid,tenant_uuid,display_name,state,public_key_ed25519)
    VALUES(p_node,p_tenant,p_display_name,'enrolled',p_public_key)
    ON CONFLICT(node_uuid) DO UPDATE SET display_name=EXCLUDED.display_name,public_key_ed25519=EXCLUDED.public_key_ed25519,updated_at=now();
  UPDATE phxclaw.device_enrollment_tokens SET consumed_at=now(),consumed_by_node_uuid=p_node WHERE enrollment_uuid=v_enrollment;
  INSERT INTO phxclaw.device_platform_state(tenant_uuid,node_uuid,platform,agent_version,transport_state)
    VALUES(p_tenant,p_node,p_platform,p_agent_version,'offline')
    ON CONFLICT(tenant_uuid,node_uuid) DO UPDATE SET platform=EXCLUDED.platform,agent_version=EXCLUDED.agent_version;
  INSERT INTO phxclaw.device_sessions(session_uuid,tenant_uuid,node_uuid,expires_at,fencing_token)
    VALUES(v_session,p_tenant,p_node,now()+interval '12 hours',1);
  INSERT INTO phxclaw.device_pairing_audit(tenant_uuid,pairing_uuid,enrollment_uuid,node_uuid,token_sha256,public_key_ed25519,result)
    VALUES(p_tenant,gen_random_uuid(),v_enrollment,p_node,p_token_sha256,p_public_key,'accepted');
  RETURN v_session;
END $$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (69,'0069_device_pairing_transport.sql','c5ec56233461b0817c4262cd8d29ec3ed906f276a8d89981626a67cd623a5912','c5ec56233461b0817c4262cd8d29ec3ed906f276a8d89981626a67cd623a5912','original',NULL) ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- MIGRATION 0070: 0070_native_qualification_matrix.sql
-- source_sha256:    16044375b74506da99fcbcb64b1872484c080d90764119b45629d140e324b5fb
-- effective_sha256: ca963e019fdfc5f9f94b2ce6c1a51544aa91555687cef6b9f0f5035530e72ba4
-- source_status:    original
-- ============================================================================
-- PhxClaw v0.70 / native qualification evidence matrix
CREATE SCHEMA IF NOT EXISTS phxclaw;
CREATE TABLE IF NOT EXISTS phxclaw.native_verification_runs (
  run_uuid uuid PRIMARY KEY,
  gate_code text NOT NULL,
  sprint_code text,
  platform text NOT NULL,
  environment_fingerprint text NOT NULL,
  status text NOT NULL CHECK(status IN ('running','passed','failed','blocked')),
  evidence_sha256 text,
  artifact_uri text,
  details jsonb NOT NULL DEFAULT '{}'::jsonb,
  started_at timestamptz NOT NULL DEFAULT now(),
  completed_at timestamptz,
  CHECK ((status='running' AND completed_at IS NULL) OR (status<>'running' AND completed_at IS NOT NULL))
);
CREATE INDEX IF NOT EXISTS idx_native_verification_gate ON phxclaw.native_verification_runs(gate_code,platform,started_at DESC);
CREATE TABLE IF NOT EXISTS phxclaw.native_gate_catalog (
  gate_code text PRIMARY KEY,
  sprint_code text,
  required boolean NOT NULL DEFAULT true,
  description text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);
INSERT INTO phxclaw.native_gate_catalog(gate_code,sprint_code,description) VALUES
 ('desktop_os_automation_e2e','F11','Keyboard, mouse, capture and governed shell on physical desktop'),
 ('real_stt_model_e2e','F12','whisper.cpp executable plus verified model transcription'),
 ('native_tauri_e2e','F14','Native Tauri desktop build and UI/live-event bridge'),
 ('channel_provider_credentialed_e2e','F21','Credentialed Telegram/Discord/Slack/WhatsApp/Teams delivery'),
 ('device_pairing_wss_keyring_multiplatform_e2e','F22','Physical WSS pairing and OS-keyring node execution')
ON CONFLICT(gate_code) DO UPDATE SET sprint_code=EXCLUDED.sprint_code,description=EXCLUDED.description;

-- Reparo native-v070 (RLS): 21 politicas (device_*, channel_*, skill_*, release_*) liam so
-- phxclaw.tenant_id, enquanto as outras liam phxclaw.current_tenant_uuid(), que prefere
-- phxclaw.tenant_uuid. Nenhum codigo Rust seta tenant_id: essas tabelas ficavam invisiveis
-- ao app, e uma sessao com as duas GUCs diferentes via tenants diferentes por tabela.
-- A lista sai do catalogo, nao de uma copia: toda politica que ainda le so tenant_id.
DO $rls$
DECLARE p record;
BEGIN
  FOR p IN
    SELECT schemaname, tablename, policyname
      FROM pg_policies
     WHERE coalesce(qual, '') || coalesce(with_check, '') LIKE '%current_setting(''phxclaw.tenant_id''%'
  LOOP
    EXECUTE format(
      'ALTER POLICY %I ON %I.%I USING (tenant_uuid = phxclaw.current_tenant_uuid()) WITH CHECK (tenant_uuid = phxclaw.current_tenant_uuid())',
      p.policyname, p.schemaname, p.tablename);
  END LOOP;
END
$rls$;

INSERT INTO phxclaw_schema_migrations(version,migration_name,source_sha256,effective_sha256,source_status,repair_notes) VALUES (70,'0070_native_qualification_matrix.sql','16044375b74506da99fcbcb64b1872484c080d90764119b45629d140e324b5fb','ca963e019fdfc5f9f94b2ce6c1a51544aa91555687cef6b9f0f5035530e72ba4','original','native-v070: 21 politicas RLS liam so phxclaw.tenant_id; unificadas em phxclaw.current_tenant_uuid()') ON CONFLICT (version) DO NOTHING;

-- ============================================================================
-- FINALIZAÇÃO / SANITY CHECKS
-- ============================================================================
DO $phx_final_check$
DECLARE n integer;
BEGIN
    SELECT count(*) INTO n FROM phxclaw_schema_migrations WHERE version BETWEEN 1 AND 70;
    IF n <> 70 THEN RAISE EXCEPTION 'Ledger incompleto: esperado 70 versões, encontrado %',n; END IF;
    IF to_regclass('phxclaw.channel_provider_accounts') IS NULL THEN RAISE EXCEPTION 'v0.68 ausente: channel_provider_accounts'; END IF;
    IF to_regclass('phxclaw.channel_delivery_attempts') IS NULL THEN RAISE EXCEPTION 'v0.68 ausente: channel_delivery_attempts'; END IF;
    IF to_regclass('phxclaw.device_platform_state') IS NULL THEN RAISE EXCEPTION 'v0.69 ausente: device_platform_state'; END IF;
    IF to_regclass('phxclaw.device_pairing_audit') IS NULL THEN RAISE EXCEPTION 'v0.69 ausente: device_pairing_audit'; END IF;
    IF to_regprocedure('phxclaw.consume_device_enrollment(uuid,bytea,uuid,text,bytea,text,text)') IS NULL THEN RAISE EXCEPTION 'v0.69 ausente: consume_device_enrollment'; END IF;
    IF to_regclass('phxclaw.native_verification_runs') IS NULL THEN RAISE EXCEPTION 'v0.70 ausente: native_verification_runs'; END IF;
    IF to_regclass('phxclaw.native_gate_catalog') IS NULL THEN RAISE EXCEPTION 'v0.70 ausente: native_gate_catalog'; END IF;
    IF (SELECT count(*) FROM phxclaw.native_gate_catalog WHERE gate_code IN ('desktop_os_automation_e2e','real_stt_model_e2e','native_tauri_e2e','channel_provider_credentialed_e2e','device_pairing_wss_keyring_multiplatform_e2e')) <> 5 THEN RAISE EXCEPTION 'v0.70 gate catalog incompleto'; END IF;
END
$phx_final_check$;
COMMIT;

SELECT version,migration_name,source_status,applied_at FROM phxclaw_schema_migrations ORDER BY version;
SELECT current_database() database_name,current_user installed_by,current_setting('server_version') postgres_version,count(*) migration_versions FROM phxclaw_schema_migrations;

BEGIN;

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

COMMIT;

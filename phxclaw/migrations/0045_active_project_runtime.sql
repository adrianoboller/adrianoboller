BEGIN;

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

COMMIT;

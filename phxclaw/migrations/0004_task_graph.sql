BEGIN;

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

COMMIT;

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

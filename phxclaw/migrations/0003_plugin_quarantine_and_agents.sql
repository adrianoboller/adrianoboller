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

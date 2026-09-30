-- PhxClaw v0.21 / F22 Device Nodes — corrected by v0.22
-- original migration_uuid retained in v0.21 source history
-- Repair: valid tenant RLS current_setting expressions; transaction remains all-or-nothing.

BEGIN;
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

COMMIT;

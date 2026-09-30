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

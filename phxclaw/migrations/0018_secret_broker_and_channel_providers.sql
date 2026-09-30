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

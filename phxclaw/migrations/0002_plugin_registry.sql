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

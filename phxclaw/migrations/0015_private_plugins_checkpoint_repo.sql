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

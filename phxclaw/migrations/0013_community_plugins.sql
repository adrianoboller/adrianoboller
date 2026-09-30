BEGIN;

CREATE TABLE IF NOT EXISTS phoenix_plugin_publishers (
    publisher_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    signer TEXT NOT NULL,
    trust_tier TEXT NOT NULL CHECK (trust_tier IN ('builtin','verified','community','local_development')),
    website TEXT,
    repository TEXT,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','suspended','revoked')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS phoenix_plugin_releases (
    plugin_uuid UUID NOT NULL,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    core_api TEXT NOT NULL,
    publisher_id TEXT NOT NULL REFERENCES phoenix_plugin_publishers(publisher_id),
    license TEXT NOT NULL,
    source_repository TEXT NOT NULL,
    package_url TEXT NOT NULL,
    package_sha256 TEXT NOT NULL,
    manifest_sha256 TEXT NOT NULL,
    signature_algorithm TEXT NOT NULL CHECK (signature_algorithm = 'ed25519'),
    signature TEXT NOT NULL,
    signer TEXT NOT NULL,
    provenance TEXT NOT NULL,
    manifest JSONB NOT NULL,
    yanked BOOLEAN NOT NULL DEFAULT FALSE,
    published_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (plugin_uuid, version)
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_phoenix_plugin_releases_name_version
    ON phoenix_plugin_releases(name, version);

CREATE TABLE IF NOT EXISTS phoenix_plugin_installs (
    install_uuid UUID PRIMARY KEY,
    plugin_uuid UUID NOT NULL,
    version TEXT NOT NULL,
    source_registry TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('downloaded','verified','installed','enabled','disabled','quarantined','failed','rolled_back')),
    installed_manifest JSONB NOT NULL,
    package_sha256 TEXT NOT NULL,
    installed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (plugin_uuid, version, install_uuid)
);

CREATE TABLE IF NOT EXISTS phoenix_plugin_capability_routes (
    capability TEXT PRIMARY KEY,
    plugin_uuid UUID NOT NULL,
    plugin_version TEXT NOT NULL,
    pinned_by TEXT NOT NULL,
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS phoenix_plugin_extension_events (
    event_uuid UUID PRIMARY KEY,
    plugin_uuid UUID NOT NULL,
    capability TEXT,
    correlation_uuid UUID,
    causation_uuid UUID,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMIT;

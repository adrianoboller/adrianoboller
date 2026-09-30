BEGIN;

CREATE TABLE IF NOT EXISTS phoenix_model_budget_accounts (
    name text PRIMARY KEY,
    ceiling_microunits bigint NOT NULL CHECK (ceiling_microunits >= 0),
    reserved_microunits bigint NOT NULL DEFAULT 0 CHECK (reserved_microunits >= 0),
    spent_microunits bigint NOT NULL DEFAULT 0 CHECK (spent_microunits >= 0),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (reserved_microunits + spent_microunits <= ceiling_microunits)
);

CREATE TABLE IF NOT EXISTS phoenix_model_route_events (
    uuid uuid PRIMARY KEY,
    request_uuid uuid NOT NULL,
    provider_uuid uuid NOT NULL,
    provider_name text NOT NULL,
    model_id text NOT NULL,
    capability text NOT NULL,
    classification text NOT NULL CHECK (classification IN ('public','internal','confidential','restricted')),
    requires_local boolean NOT NULL,
    estimated_cost_microunits bigint NOT NULL CHECK (estimated_cost_microunits >= 0),
    budget_account text NOT NULL REFERENCES phoenix_model_budget_accounts(name),
    routed_at timestamptz NOT NULL
);

CREATE TABLE IF NOT EXISTS phoenix_model_telemetry (
    uuid uuid PRIMARY KEY,
    request_uuid uuid NOT NULL,
    provider_uuid uuid NOT NULL,
    model_id text NOT NULL,
    latency_ms bigint NOT NULL CHECK (latency_ms >= 0),
    input_tokens bigint NOT NULL CHECK (input_tokens >= 0),
    output_tokens bigint NOT NULL CHECK (output_tokens >= 0),
    cost_microunits bigint NOT NULL CHECK (cost_microunits >= 0),
    success boolean NOT NULL,
    observed_at timestamptz NOT NULL
);

CREATE INDEX IF NOT EXISTS phoenix_model_route_request_idx
    ON phoenix_model_route_events (request_uuid, routed_at);

CREATE INDEX IF NOT EXISTS phoenix_model_telemetry_provider_time_idx
    ON phoenix_model_telemetry (provider_uuid, observed_at DESC);

COMMIT;

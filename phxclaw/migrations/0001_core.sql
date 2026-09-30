CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS phoenix_objects (
  uuid uuid PRIMARY KEY,
  object_type text NOT NULL,
  payload jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_phoenix_objects_type
  ON phoenix_objects(object_type);

CREATE INDEX IF NOT EXISTS idx_phoenix_objects_payload_gin
  ON phoenix_objects USING gin(payload);

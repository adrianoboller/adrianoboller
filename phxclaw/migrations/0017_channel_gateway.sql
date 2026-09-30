-- PhxClaw F21 Channel Gateway
CREATE TABLE IF NOT EXISTS phoenix_channel_identities (
    uuid UUID PRIMARY KEY,
    channel TEXT NOT NULL,
    account_id TEXT NOT NULL,
    external_user_id TEXT NOT NULL,
    principal_uuid UUID NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending','active','blocked')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(channel, account_id, external_user_id)
);

CREATE TABLE IF NOT EXISTS phoenix_channel_sessions (
    uuid UUID PRIMARY KEY,
    principal_uuid UUID NOT NULL,
    channel TEXT NOT NULL,
    account_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(principal_uuid, channel, account_id, conversation_id)
);

CREATE TABLE IF NOT EXISTS phoenix_channel_messages (
    uuid UUID PRIMARY KEY,
    session_uuid UUID NOT NULL REFERENCES phoenix_channel_sessions(uuid),
    external_message_id TEXT,
    direction TEXT NOT NULL CHECK (direction IN ('inbound','outbound')),
    channel TEXT NOT NULL,
    account_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(channel, account_id, external_message_id)
);

CREATE INDEX IF NOT EXISTS idx_phoenix_channel_sessions_principal ON phoenix_channel_sessions(principal_uuid);
CREATE INDEX IF NOT EXISTS idx_phoenix_channel_messages_session ON phoenix_channel_messages(session_uuid, created_at);

-- PhxClaw v0.69 / F22 Device pairing + transport hardening
CREATE SCHEMA IF NOT EXISTS phxclaw;

CREATE TABLE IF NOT EXISTS phxclaw.device_platform_state (
  tenant_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL,
  platform text NOT NULL CHECK (platform IN ('windows','linux','macos','android','ios')),
  agent_version text NOT NULL,
  transport text NOT NULL DEFAULT 'wss' CHECK (transport IN ('wss')),
  transport_state text NOT NULL DEFAULT 'offline' CHECK (transport_state IN ('offline','connecting','online','degraded','quarantined')),
  connected_at timestamptz,
  disconnected_at timestamptz,
  last_heartbeat_at timestamptz,
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
  PRIMARY KEY (tenant_uuid,node_uuid),
  FOREIGN KEY (tenant_uuid,node_uuid) REFERENCES phxclaw.device_nodes(tenant_uuid,node_uuid) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS phxclaw.device_pairing_audit (
  tenant_uuid uuid NOT NULL,
  pairing_uuid uuid PRIMARY KEY,
  enrollment_uuid uuid NOT NULL,
  node_uuid uuid NOT NULL,
  token_sha256 bytea NOT NULL CHECK (octet_length(token_sha256)=32),
  public_key_ed25519 bytea NOT NULL CHECK (octet_length(public_key_ed25519)=32),
  result text NOT NULL CHECK (result IN ('accepted','rejected','expired','replayed')),
  reason text,
  recorded_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_device_platform_online ON phxclaw.device_platform_state(tenant_uuid,transport_state,last_heartbeat_at);
CREATE INDEX IF NOT EXISTS idx_device_pairing_audit_node ON phxclaw.device_pairing_audit(tenant_uuid,node_uuid,recorded_at DESC);

ALTER TABLE phxclaw.device_platform_state ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_platform_state FORCE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_pairing_audit ENABLE ROW LEVEL SECURITY;
ALTER TABLE phxclaw.device_pairing_audit FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.device_platform_state;
CREATE POLICY tenant_isolation ON phxclaw.device_platform_state USING (tenant_uuid=NULLIF(current_setting('phxclaw.tenant_id',true),'')::uuid) WITH CHECK (tenant_uuid=NULLIF(current_setting('phxclaw.tenant_id',true),'')::uuid);
DROP POLICY IF EXISTS tenant_isolation ON phxclaw.device_pairing_audit;
CREATE POLICY tenant_isolation ON phxclaw.device_pairing_audit USING (tenant_uuid=NULLIF(current_setting('phxclaw.tenant_id',true),'')::uuid) WITH CHECK (tenant_uuid=NULLIF(current_setting('phxclaw.tenant_id',true),'')::uuid);

CREATE OR REPLACE FUNCTION phxclaw.consume_device_enrollment(
  p_tenant uuid,p_token_sha256 bytea,p_node uuid,p_display_name text,p_public_key bytea,p_platform text,p_agent_version text
) RETURNS uuid LANGUAGE plpgsql AS $$
DECLARE v_enrollment uuid; v_session uuid:=gen_random_uuid();
BEGIN
  SELECT enrollment_uuid INTO v_enrollment FROM phxclaw.device_enrollment_tokens
   WHERE tenant_uuid=p_tenant AND token_sha256=p_token_sha256 AND consumed_at IS NULL AND expires_at>now()
   FOR UPDATE;
  IF v_enrollment IS NULL THEN RAISE EXCEPTION 'invalid, expired, or consumed enrollment token'; END IF;
  INSERT INTO phxclaw.device_nodes(node_uuid,tenant_uuid,display_name,state,public_key_ed25519)
    VALUES(p_node,p_tenant,p_display_name,'enrolled',p_public_key)
    ON CONFLICT(node_uuid) DO UPDATE SET display_name=EXCLUDED.display_name,public_key_ed25519=EXCLUDED.public_key_ed25519,updated_at=now();
  UPDATE phxclaw.device_enrollment_tokens SET consumed_at=now(),consumed_by_node_uuid=p_node WHERE enrollment_uuid=v_enrollment;
  INSERT INTO phxclaw.device_platform_state(tenant_uuid,node_uuid,platform,agent_version,transport_state)
    VALUES(p_tenant,p_node,p_platform,p_agent_version,'offline')
    ON CONFLICT(tenant_uuid,node_uuid) DO UPDATE SET platform=EXCLUDED.platform,agent_version=EXCLUDED.agent_version;
  INSERT INTO phxclaw.device_sessions(session_uuid,tenant_uuid,node_uuid,expires_at,fencing_token)
    VALUES(v_session,p_tenant,p_node,now()+interval '12 hours',1);
  INSERT INTO phxclaw.device_pairing_audit(tenant_uuid,pairing_uuid,enrollment_uuid,node_uuid,token_sha256,public_key_ed25519,result)
    VALUES(p_tenant,gen_random_uuid(),v_enrollment,p_node,p_token_sha256,p_public_key,'accepted');
  RETURN v_session;
END $$;

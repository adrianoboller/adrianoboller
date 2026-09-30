-- Run as a dedicated NOBYPASSRLS/non-superuser role after migrations through 0035.
-- The test is intentionally non-vacuous: a tenant-A v0.34 portfolio fixture MUST exist.
BEGIN;
SET LOCAL phxclaw.tenant_uuid = '11111111-1111-7111-8111-111111111111';
DO $$ BEGIN
 IF NOT EXISTS(SELECT 1 FROM phxclaw_ai_portfolios WHERE tenant_uuid='11111111-1111-7111-8111-111111111111') THEN
  RAISE EXCEPTION 'missing tenant A portfolio fixture';
 END IF;
END $$;
INSERT INTO phxclaw_ai_sre_policies(tenant_uuid,policy_uuid,service_name,portfolio_uuid,document_sha256,signer_id,signature_hex,policy_json,starts_at,expires_at)
SELECT '11111111-1111-7111-8111-111111111111','11111111-1111-7111-8111-111111111112','rls-fixture',p.portfolio_uuid,repeat('a',64),'fixture',repeat('b',128),'{}'::jsonb,clock_timestamp(),clock_timestamp()+interval '1 hour'
FROM phxclaw_ai_portfolios p WHERE p.tenant_uuid='11111111-1111-7111-8111-111111111111' LIMIT 1;
DO $$ BEGIN
 IF NOT EXISTS(SELECT 1 FROM phxclaw_ai_sre_policies WHERE tenant_uuid='11111111-1111-7111-8111-111111111111' AND policy_uuid='11111111-1111-7111-8111-111111111112') THEN
  RAISE EXCEPTION 'SRE policy fixture insert was vacuous';
 END IF;
END $$;
SET LOCAL phxclaw.tenant_uuid = '22222222-2222-7222-8222-222222222222';
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM phxclaw_ai_sre_policies WHERE tenant_uuid='11111111-1111-7111-8111-111111111111') THEN
  RAISE EXCEPTION 'cross-tenant RLS leak';
 END IF;
END $$;
ROLLBACK;

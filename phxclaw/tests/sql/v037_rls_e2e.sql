\set ON_ERROR_STOP on
BEGIN;
DO $$
DECLARE r record;
BEGIN
 SELECT rolsuper, rolbypassrls INTO r FROM pg_roles WHERE rolname=current_user;
 IF NOT FOUND OR r.rolsuper OR r.rolbypassrls THEN RAISE EXCEPTION 'v0.37 RLS E2E requires NOSUPERUSER + NOBYPASSRLS role'; END IF;
END $$;
SELECT set_config('phxclaw.tenant_uuid','00000000-0000-7000-8000-000000000037',true);
INSERT INTO engineering_workflows(tenant_uuid,workflow_uuid,objective,source_state_sha256,policy_sha256,skill_catalog_sha256,risk,spec)
VALUES ('00000000-0000-7000-8000-000000000037','00000000-0000-7000-8000-000000003701','fixture',repeat('a',64),repeat('b',64),repeat('c',64),'low','{}') ON CONFLICT DO NOTHING;
DO $$ DECLARE n int; BEGIN SELECT count(*) INTO n FROM engineering_workflows WHERE workflow_uuid='00000000-0000-7000-8000-000000003701'; IF n<>1 THEN RAISE EXCEPTION 'tenant A fixture missing'; END IF; END $$;
SELECT set_config('phxclaw.tenant_uuid','00000000-0000-7000-8000-000000000038',true);
DO $$ DECLARE n int; BEGIN SELECT count(*) INTO n FROM engineering_workflows WHERE workflow_uuid='00000000-0000-7000-8000-000000003701'; IF n<>0 THEN RAISE EXCEPTION 'cross-tenant RLS leak'; END IF; END $$;
ROLLBACK;

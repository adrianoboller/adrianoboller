\set ON_ERROR_STOP on
DO $$ BEGIN IF current_setting('is_superuser')='on' THEN RAISE EXCEPTION 'RLS E2E must use NOSUPERUSER'; END IF; IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname=current_user AND rolbypassrls) THEN RAISE EXCEPTION 'RLS E2E role must be NOBYPASSRLS'; END IF; END $$;
SELECT set_config('phxclaw.tenant_uuid','00000000-0000-7000-8000-000000000038',false);
INSERT INTO engineering_swarms(tenant_uuid,swarm_uuid,workflow_uuid,objective,source_state_sha256,policy_sha256,skill_catalog_sha256,max_parallel_teams,spec)
VALUES('00000000-0000-7000-8000-000000000038','00000000-0000-7000-8000-000000000138','00000000-0000-7000-8000-000000000238','fixture',repeat('a',64),repeat('b',64),repeat('c',64),6,'{}');
SELECT set_config('phxclaw.tenant_uuid','00000000-0000-7000-8000-000000000039',false);
DO $$ BEGIN IF EXISTS (SELECT 1 FROM engineering_swarms WHERE swarm_uuid='00000000-0000-7000-8000-000000000138') THEN RAISE EXCEPTION 'cross-tenant read leaked'; END IF; END $$;

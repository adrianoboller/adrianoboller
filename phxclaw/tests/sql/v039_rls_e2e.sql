\set ON_ERROR_STOP on
DO $$ BEGIN
 IF current_setting('phxclaw.tenant_uuid', true) IS NULL THEN RAISE EXCEPTION 'phxclaw.tenant_uuid must be set'; END IF;
END $$;
DO $$ DECLARE r record; BEGIN
 SELECT rolsuper, rolbypassrls INTO r FROM pg_roles WHERE rolname=current_user;
 IF r.rolsuper OR r.rolbypassrls THEN RAISE EXCEPTION 'RLS E2E must use NOSUPERUSER + NOBYPASSRLS role'; END IF;
END $$;
SELECT count(*) AS visible_merge_plans FROM swarm_merge_plans_v039;

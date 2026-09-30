\set ON_ERROR_STOP on
DO $$
DECLARE r record;
BEGIN
  SELECT rolsuper, rolbypassrls INTO r FROM pg_roles WHERE rolname = current_user;
  IF NOT FOUND THEN RAISE EXCEPTION 'current role not found'; END IF;
  IF r.rolsuper THEN RAISE EXCEPTION 'RLS proof role must not be superuser'; END IF;
  IF r.rolbypassrls THEN RAISE EXCEPTION 'RLS proof role must not have BYPASSRLS'; END IF;
END $$;
\echo 'V025_RLS_ROLE_OK'

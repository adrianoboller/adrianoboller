\set ON_ERROR_STOP on
DO $$ BEGIN
 IF current_setting('is_superuser')='on' THEN RAISE EXCEPTION 'RLS proof must not run as superuser'; END IF;
 IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname=current_user AND rolbypassrls) THEN RAISE EXCEPTION 'RLS proof role must be NOBYPASSRLS'; END IF;
END $$;
SELECT set_config('phxclaw.tenant_uuid','018f0000-0000-7000-8000-000000000036',false);
INSERT INTO phx_incidents_v036(tenant_uuid,incident_uuid,severity,title,detection_evidence_sha256)
VALUES('018f0000-0000-7000-8000-000000000036','018f0000-0000-7000-8000-000000003601','low','fixture',repeat('a',64));
DO $$ BEGIN
 IF (SELECT count(*) FROM phx_incidents_v036 WHERE incident_uuid='018f0000-0000-7000-8000-000000003601') <> 1 THEN RAISE EXCEPTION 'tenant A fixture missing'; END IF;
 PERFORM set_config('phxclaw.tenant_uuid','018f0000-0000-7000-8000-000000000037',false);
 IF (SELECT count(*) FROM phx_incidents_v036 WHERE incident_uuid='018f0000-0000-7000-8000-000000003601') <> 0 THEN RAISE EXCEPTION 'cross-tenant RLS leak'; END IF;
END $$;
ROLLBACK;

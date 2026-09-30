\set ON_ERROR_STOP on
BEGIN;
SET LOCAL phxclaw.tenant_uuid = '0199a5f1-0065-7065-8abc-000000000065';
INSERT INTO phxclaw.backup_sets(backup_uuid,tenant_uuid,backup_kind,source_version,manifest_sha256,object_count,total_bytes,repository_uri,state,verified_at)
VALUES ('0199a5f1-0065-7165-8abc-000000000065','0199a5f1-0065-7065-8abc-000000000065','filesystem','0.65.0',repeat('a',64),1,5,'file:///tmp/phx','verified',clock_timestamp());
INSERT INTO phxclaw.backup_objects(backup_uuid,object_path,blob_sha256,size_bytes)
VALUES ('0199a5f1-0065-7165-8abc-000000000065','a.txt',repeat('b',64),5);
DO $$ BEGIN
 IF (SELECT count(*) FROM phxclaw.backup_sets WHERE tenant_uuid=phxclaw.current_tenant_uuid()) <> 1 THEN RAISE EXCEPTION 'tenant backup visibility failed'; END IF;
 IF (SELECT count(*) FROM phxclaw.backup_objects) <> 1 THEN RAISE EXCEPTION 'backup object RLS failed'; END IF;
END $$;
ROLLBACK;

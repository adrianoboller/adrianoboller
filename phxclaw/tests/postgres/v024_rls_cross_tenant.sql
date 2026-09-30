\set ON_ERROR_STOP on
BEGIN;
-- Run after migrations 0021..0024 on a disposable database using a non-superuser role subject to RLS.

SET LOCAL phxclaw.tenant_id = '11111111-1111-7111-8111-111111111111';
INSERT INTO phxclaw.device_nodes(node_uuid,tenant_uuid,display_name,state,public_key_ed25519)
VALUES ('aaaaaaaa-aaaa-7aaa-8aaa-aaaaaaaaaaaa','11111111-1111-7111-8111-111111111111','tenant-a','active',decode(repeat('00',32),'hex'));
INSERT INTO phxclaw.knowledge_nodes(node_uuid,tenant_uuid,kind,epistemic_state,content_sha256,source_state_sha256)
VALUES ('bbbbbbbb-bbbb-7bbb-8bbb-bbbbbbbbbbbb','11111111-1111-7111-8111-111111111111','claim','unverified',decode(repeat('11',32),'hex'),decode(repeat('22',32),'hex'));

SET LOCAL phxclaw.tenant_id = '22222222-2222-7222-8222-222222222222';
DO $$ DECLARE n integer; BEGIN
  SELECT count(*) INTO n FROM phxclaw.device_nodes WHERE node_uuid='aaaaaaaa-aaaa-7aaa-8aaa-aaaaaaaaaaaa';
  IF n <> 0 THEN RAISE EXCEPTION 'RLS leak: tenant B can see tenant A device'; END IF;
END $$;

INSERT INTO phxclaw.device_nodes(node_uuid,tenant_uuid,display_name,state,public_key_ed25519)
VALUES ('cccccccc-cccc-7ccc-8ccc-cccccccccccc','22222222-2222-7222-8222-222222222222','tenant-b','active',decode(repeat('00',32),'hex'));
INSERT INTO phxclaw.knowledge_nodes(node_uuid,tenant_uuid,kind,epistemic_state,content_sha256,source_state_sha256)
VALUES ('dddddddd-dddd-7ddd-8ddd-dddddddddddd','22222222-2222-7222-8222-222222222222','claim','unverified',decode(repeat('33',32),'hex'),decode(repeat('44',32),'hex'));

DO $$ BEGIN
  BEGIN
    INSERT INTO phxclaw.knowledge_edges(edge_uuid,tenant_uuid,from_node_uuid,to_node_uuid,kind)
    VALUES ('eeeeeeee-eeee-7eee-8eee-eeeeeeeeeeee','22222222-2222-7222-8222-222222222222',
            'dddddddd-dddd-7ddd-8ddd-dddddddddddd','bbbbbbbb-bbbb-7bbb-8bbb-bbbbbbbbbbbb','relates_to');
    RAISE EXCEPTION 'expected cross-tenant composite FK rejection';
  EXCEPTION WHEN foreign_key_violation THEN NULL;
  END;
END $$;

DO $$ BEGIN
  BEGIN
    INSERT INTO phxclaw.device_commands(command_uuid,tenant_uuid,node_uuid,capability,idempotency_key,risk,state,fencing_token,submitted_at,not_before,expires_at)
    VALUES ('ffffffff-ffff-7fff-8fff-ffffffffffff','22222222-2222-7222-8222-222222222222',
            'aaaaaaaa-aaaa-7aaa-8aaa-aaaaaaaaaaaa','device.health','cross-tenant','low','queued',0,now(),now(),now()+interval '1 minute');
    RAISE EXCEPTION 'expected device cross-tenant composite FK rejection';
  EXCEPTION WHEN foreign_key_violation THEN NULL;
  END;
END $$;

ROLLBACK;
\echo 'V024_RLS_CROSS_TENANT_PASS'

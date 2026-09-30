\set ON_ERROR_STOP on
-- Reparo native-v070: as tabelas device_*/channel_*/skill_*/release_* liam so phxclaw.tenant_id.
-- O app (Rust) seta phxclaw.tenant_uuid. Esta prova usa a convencao do app e tem de:
--   (1) enxergar a linha do proprio tenant; (2) NAO enxergar a do outro.
-- Com as politicas antigas, (1) falha: o WITH CHECK le tenant_id, que esta vazio.
BEGIN;
SET LOCAL phxclaw.tenant_uuid = '11111111-1111-7111-8111-111111111111';
INSERT INTO phxclaw.device_nodes(node_uuid,tenant_uuid,display_name,state,public_key_ed25519)
VALUES ('aaaaaaaa-aaaa-7aaa-8aaa-aaaaaaaa0070','11111111-1111-7111-8111-111111111111','guc-a','active',decode(repeat('00',32),'hex'));
DO $a$ BEGIN
  IF (SELECT count(*) FROM phxclaw.device_nodes WHERE node_uuid='aaaaaaaa-aaaa-7aaa-8aaa-aaaaaaaa0070') <> 1 THEN
    RAISE EXCEPTION 'tenant A nao enxerga o proprio device pela GUC tenant_uuid';
  END IF;
END $a$;
SET LOCAL phxclaw.tenant_uuid = '22222222-2222-7222-8222-222222222222';
DO $b$ BEGIN
  IF (SELECT count(*) FROM phxclaw.device_nodes WHERE node_uuid='aaaaaaaa-aaaa-7aaa-8aaa-aaaaaaaa0070') <> 0 THEN
    RAISE EXCEPTION 'RLS leak: tenant B enxerga device do tenant A';
  END IF;
END $b$;
ROLLBACK;

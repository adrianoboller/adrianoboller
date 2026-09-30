\set ON_ERROR_STOP on
BEGIN;SET LOCAL phxclaw.tenant_uuid='0199a5f1-0067-7067-8abc-000000000067';
INSERT INTO phxclaw.bpm_process_definitions(process_uuid,tenant_uuid,external_id,version,bpmn_xml,compiled_graph,content_sha256) VALUES('0199a5f1-0067-7167-8abc-000000000067','0199a5f1-0067-7067-8abc-000000000067','p',1,'<xml/>','{}',repeat('a',64));
INSERT INTO phxclaw.bpm_instances_v2(instance_uuid,tenant_uuid,process_uuid,state) VALUES('0199a5f1-0067-7267-8abc-000000000067','0199a5f1-0067-7067-8abc-000000000067','0199a5f1-0067-7167-8abc-000000000067','running');
INSERT INTO phxclaw.bpm_tokens_v2(token_uuid,tenant_uuid,instance_uuid,node_id,state) VALUES('0199a5f1-0067-7367-8abc-000000000067','0199a5f1-0067-7067-8abc-000000000067','0199a5f1-0067-7267-8abc-000000000067','start','ready');
INSERT INTO phxclaw.bpm_events_v2(event_uuid,tenant_uuid,instance_uuid,token_uuid,event_type,node_id,fencing_token) VALUES('0199a5f1-0067-7467-8abc-000000000067','0199a5f1-0067-7067-8abc-000000000067','0199a5f1-0067-7267-8abc-000000000067','0199a5f1-0067-7367-8abc-000000000067','token_created','start',0);
DO $$BEGIN IF (SELECT count(*) FROM phxclaw.bpm_events_v2)<>1 THEN RAISE EXCEPTION 'BPM event/RLS fail';END IF;END$$;ROLLBACK;

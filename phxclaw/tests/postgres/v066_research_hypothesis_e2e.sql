\set ON_ERROR_STOP on
BEGIN;
SET LOCAL phxclaw.tenant_uuid='0199a5f1-0066-7066-8abc-000000000066';
INSERT INTO phxclaw.research_records(research_uuid,tenant_uuid,correlation_uuid,topic,question,source_state_sha256,created_at) VALUES('0199a5f1-0066-7166-8abc-000000000066','0199a5f1-0066-7066-8abc-000000000066','0199a5f1-0066-7266-8abc-000000000066','test','works?',repeat('a',64),clock_timestamp());
INSERT INTO phxclaw.research_record_evidence(research_uuid,evidence_uuid,uri,source_type,retrieved_at,sha256) VALUES('0199a5f1-0066-7166-8abc-000000000066','0199a5f1-0066-7366-8abc-000000000066','file://e','fixture',clock_timestamp(),repeat('b',64));
INSERT INTO phxclaw.hypothesis_records(hypothesis_uuid,tenant_uuid,research_uuid,statement,rationale,experiment_uuid,current_status,source_state_sha256,created_at,updated_at) VALUES('0199a5f1-0066-7466-8abc-000000000066','0199a5f1-0066-7066-8abc-000000000066','0199a5f1-0066-7166-8abc-000000000066','x','r','0199a5f1-0066-7566-8abc-000000000066','proposed',repeat('c',64),clock_timestamp(),clock_timestamp());
INSERT INTO phxclaw.experiment_plans(experiment_uuid,hypothesis_uuid,steps,success_criteria,failure_criteria) VALUES('0199a5f1-0066-7566-8abc-000000000066','0199a5f1-0066-7466-8abc-000000000066','["step"]','["pass"]','["fail"]');
INSERT INTO phxclaw.hypothesis_decisions(decision_uuid,hypothesis_uuid,from_status,to_status,note,actor) VALUES('0199a5f1-0066-7666-8abc-000000000066','0199a5f1-0066-7466-8abc-000000000066','proposed','testing','start','fixture');
INSERT INTO phxclaw.experiment_runs(run_uuid,tenant_uuid,hypothesis_uuid,state,started_at) VALUES('0199a5f1-0066-7766-8abc-000000000066','0199a5f1-0066-7066-8abc-000000000066','0199a5f1-0066-7466-8abc-000000000066','running',clock_timestamp());
DO $$BEGIN IF (SELECT count(*) FROM phxclaw.research_records)<>1 THEN RAISE EXCEPTION 'research RLS failed';END IF;IF (SELECT count(*) FROM phxclaw.hypothesis_records)<>1 THEN RAISE EXCEPTION 'hypothesis RLS failed';END IF;IF (SELECT count(*) FROM phxclaw.experiment_runs)<>1 THEN RAISE EXCEPTION 'experiment persistence failed';END IF;END$$;
ROLLBACK;

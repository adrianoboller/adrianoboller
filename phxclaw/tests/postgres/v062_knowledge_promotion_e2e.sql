-- PhxClaw v0.62 — native PostgreSQL E2E fixture
-- Run ONLY on a disposable test database after FULL INSTALL v0.62.
-- Expected: all ASSERT blocks pass; final ROLLBACK leaves no fixture rows.

BEGIN;
SET LOCAL phxclaw.tenant_uuid = '018f0000-0000-7000-8000-000000000062';

-- Provenance + ALLOW harvest candidate.
INSERT INTO public.phx_source_artifact(
  artifact_uuid,source_name,sha256,origin_kind,origin_known,license_class,license_spdx,
  local_license_evidence,declared_leak,redistribution_prohibited,decision,reasons
) VALUES (
  '018f0000-0000-7000-8000-000000000601','v062-e2e-source',repeat('a',64),'first_party',true,'permissive','Apache-2.0',
  true,false,false,'ALLOW','[]'::jsonb
);

INSERT INTO public.phx_source_harvest_run(
  run_uuid,tenant_uuid,artifact_uuid,actor,provenance_decision,final_decision,provenance_reasons,security_reasons,
  files_seen,bytes_seen,candidate_count,started_at,completed_at
) VALUES (
  '018f0000-0000-7000-8000-000000000602','018f0000-0000-7000-8000-000000000062','018f0000-0000-7000-8000-000000000601',
  'v062-e2e','ALLOW','ALLOW','[]','[]',1,16,1,clock_timestamp()-interval '1 second',clock_timestamp()
);

INSERT INTO public.phx_source_knowledge_candidate(
  candidate_uuid,tenant_uuid,run_uuid,artifact_uuid,source_path,content_sha256,source_state_sha256,mechanism,excerpt,collected_at
) VALUES (
  '018f0000-0000-7000-8000-000000000603','018f0000-0000-7000-8000-000000000062','018f0000-0000-7000-8000-000000000602',
  '018f0000-0000-7000-8000-000000000601','docs/e2e.md',repeat('b',64),repeat('f',64),'v062-e2e-harvest','fixture',clock_timestamp()
);

-- Claim + two independent evidence nodes.
INSERT INTO phxclaw.knowledge_nodes(node_uuid,tenant_uuid,kind,epistemic_state,content_sha256,source_state_sha256,subject_key,predicate_key,value_sha256,confidence_ppm,provenance)
VALUES
('018f0000-0000-7000-8000-000000000610','018f0000-0000-7000-8000-000000000062','claim','unverified',decode(repeat('1',64),'hex'),decode(repeat('f',64),'hex'),'runtime.postgresql','major',decode(repeat('2',64),'hex'),700000,'{}'),
('018f0000-0000-7000-8000-000000000611','018f0000-0000-7000-8000-000000000062','evidence','accepted',decode(repeat('3',64),'hex'),decode(repeat('f',64),'hex'),NULL,NULL,NULL,900000,'{}'),
('018f0000-0000-7000-8000-000000000612','018f0000-0000-7000-8000-000000000062','evidence','accepted',decode(repeat('4',64),'hex'),decode(repeat('f',64),'hex'),NULL,NULL,NULL,900000,'{}');

INSERT INTO phxclaw.knowledge_evidence_bindings(binding_uuid,tenant_uuid,claim_node_uuid,evidence_node_uuid,relation,evidence_sha256,source_state_sha256,mechanism,collected_at)
VALUES
('018f0000-0000-7000-8000-000000000621','018f0000-0000-7000-8000-000000000062','018f0000-0000-7000-8000-000000000610','018f0000-0000-7000-8000-000000000611','supports',decode(repeat('3',64),'hex'),decode(repeat('f',64),'hex'),'test-suite',clock_timestamp()),
('018f0000-0000-7000-8000-000000000622','018f0000-0000-7000-8000-000000000062','018f0000-0000-7000-8000-000000000610','018f0000-0000-7000-8000-000000000612','supports',decode(repeat('4',64),'hex'),decode(repeat('f',64),'hex'),'documentation',clock_timestamp());

INSERT INTO phxclaw.knowledge_promotion_requests(
  request_uuid,tenant_uuid,candidate_uuid,claim_node_uuid,target_state,expected_source_state_sha256,min_supporting_evidence,min_independent_mechanisms,max_evidence_age_seconds,requested_by
) VALUES (
  '018f0000-0000-7000-8000-000000000630','018f0000-0000-7000-8000-000000000062','018f0000-0000-7000-8000-000000000603',
  '018f0000-0000-7000-8000-000000000610','accepted',decode(repeat('f',64),'hex'),2,2,2592000,'v062-e2e'
);
INSERT INTO phxclaw.knowledge_promotion_evidence(request_uuid,evidence_uuid,relation,evidence_sha256,source_state_sha256,mechanism,collected_at)
VALUES
('018f0000-0000-7000-8000-000000000630','018f0000-0000-7000-8000-000000000611','supports',decode(repeat('3',64),'hex'),decode(repeat('f',64),'hex'),'test-suite',clock_timestamp()),
('018f0000-0000-7000-8000-000000000630','018f0000-0000-7000-8000-000000000612','supports',decode(repeat('4',64),'hex'),decode(repeat('f',64),'hex'),'documentation',clock_timestamp());

-- ACCEPTED may be approved by system after all gates pass.
INSERT INTO phxclaw.knowledge_promotion_reviews(review_uuid,tenant_uuid,request_uuid,authority,reviewer,decision,decision_sha256)
VALUES ('018f0000-0000-7000-8000-000000000631','018f0000-0000-7000-8000-000000000062','018f0000-0000-7000-8000-000000000630','system','v062-e2e','approved',decode(repeat('5',64),'hex'));

INSERT INTO phxclaw.knowledge_nodes(node_uuid,tenant_uuid,kind,epistemic_state,content_sha256,source_state_sha256,subject_key,predicate_key,value_sha256,confidence_ppm,provenance)
VALUES ('018f0000-0000-7000-8000-000000000640','018f0000-0000-7000-8000-000000000062','claim','accepted',decode(repeat('1',64),'hex'),decode(repeat('f',64),'hex'),'runtime.postgresql','major',decode(repeat('2',64),'hex'),700000,'{}');
INSERT INTO phxclaw.knowledge_edges(edge_uuid,tenant_uuid,from_node_uuid,to_node_uuid,kind,attributes)
VALUES ('018f0000-0000-7000-8000-000000000641','018f0000-0000-7000-8000-000000000062','018f0000-0000-7000-8000-000000000640','018f0000-0000-7000-8000-000000000610','supersedes','{}');
INSERT INTO phxclaw.knowledge_promotion_receipts(promotion_uuid,tenant_uuid,request_uuid,previous_node_uuid,promoted_node_uuid,target_state,review_uuid)
VALUES ('018f0000-0000-7000-8000-000000000642','018f0000-0000-7000-8000-000000000062','018f0000-0000-7000-8000-000000000630','018f0000-0000-7000-8000-000000000610','018f0000-0000-7000-8000-000000000640','accepted','018f0000-0000-7000-8000-000000000631');
INSERT INTO public.phx_source_candidate_promotion(promotion_uuid,tenant_uuid,candidate_uuid,knowledge_node_uuid,authority,evidence_sha256)
VALUES ('018f0000-0000-7000-8000-000000000643','018f0000-0000-7000-8000-000000000062','018f0000-0000-7000-8000-000000000603','018f0000-0000-7000-8000-000000000640','system',repeat('5',64));

DO $assert$
DECLARE n integer;
BEGIN
  SELECT count(*) INTO n FROM phxclaw.knowledge_promotion_receipts WHERE request_uuid='018f0000-0000-7000-8000-000000000630';
  IF n<>1 THEN RAISE EXCEPTION 'E2E FAIL: accepted promotion receipt missing'; END IF;
  SELECT count(*) INTO n FROM public.phx_source_candidate_promotion WHERE candidate_uuid='018f0000-0000-7000-8000-000000000603';
  IF n<>1 THEN RAISE EXCEPTION 'E2E FAIL: v0.61 bridge promotion missing'; END IF;
END $assert$;

-- GOVERNED must reject system authority.
INSERT INTO phxclaw.knowledge_promotion_requests(request_uuid,tenant_uuid,claim_node_uuid,target_state,expected_source_state_sha256,min_supporting_evidence,min_independent_mechanisms,max_evidence_age_seconds,requested_by)
VALUES ('018f0000-0000-7000-8000-000000000650','018f0000-0000-7000-8000-000000000062','018f0000-0000-7000-8000-000000000610','governed',decode(repeat('f',64),'hex'),2,2,2592000,'v062-e2e');
INSERT INTO phxclaw.knowledge_promotion_evidence(request_uuid,evidence_uuid,relation,evidence_sha256,source_state_sha256,mechanism,collected_at)
SELECT '018f0000-0000-7000-8000-000000000650',evidence_uuid,relation,evidence_sha256,source_state_sha256,mechanism,collected_at FROM phxclaw.knowledge_promotion_evidence WHERE request_uuid='018f0000-0000-7000-8000-000000000630';
DO $must_fail$
BEGIN
  BEGIN
    INSERT INTO phxclaw.knowledge_promotion_reviews(review_uuid,tenant_uuid,request_uuid,authority,reviewer,decision,decision_sha256)
    VALUES ('018f0000-0000-7000-8000-000000000651','018f0000-0000-7000-8000-000000000062','018f0000-0000-7000-8000-000000000650','system','v062-e2e','approved',decode(repeat('6',64),'hex'));
    RAISE EXCEPTION 'E2E FAIL: governed system approval unexpectedly succeeded';
  EXCEPTION WHEN OTHERS THEN
    IF SQLERRM='E2E FAIL: governed system approval unexpectedly succeeded' THEN RAISE; END IF;
    RAISE NOTICE 'PASS expected governed/system denial: %',SQLERRM;
  END;
END $must_fail$;

-- Direct candidate promotion without gated receipt must fail.
DO $must_fail_bridge$
BEGIN
  BEGIN
    INSERT INTO public.phx_source_candidate_promotion(promotion_uuid,tenant_uuid,candidate_uuid,knowledge_node_uuid,authority,evidence_sha256)
    VALUES ('018f0000-0000-7000-8000-000000000660','018f0000-0000-7000-8000-000000000062','018f0000-0000-7000-8000-000000000603','018f0000-0000-7000-8000-000000000610','system',repeat('7',64));
    RAISE EXCEPTION 'E2E FAIL: ungated bridge promotion unexpectedly succeeded';
  EXCEPTION WHEN OTHERS THEN
    IF SQLERRM='E2E FAIL: ungated bridge promotion unexpectedly succeeded' THEN RAISE; END IF;
    RAISE NOTICE 'PASS expected ungated bridge denial: %',SQLERRM;
  END;
END $must_fail_bridge$;

RAISE NOTICE 'PASS: PhxClaw v0.62 knowledge promotion native E2E fixture.';
ROLLBACK;

\set ON_ERROR_STOP on
BEGIN;
-- Run after migrations 0021..0024 on a disposable DB with a role subject to RLS.
SET LOCAL phxclaw.tenant_id = '33333333-3333-7333-8333-333333333333';

INSERT INTO phxclaw.release_gate_runs(
  run_uuid, tenant_uuid, release_uuid, version, workspace_sha256, state, started_at, finished_at
) VALUES (
  '25000000-0000-7000-8000-000000000001',
  '33333333-3333-7333-8333-333333333333',
  '25000000-0000-7000-8000-000000000002',
  '0.25.0', decode(repeat('aa',32),'hex'), 'complete', now(), now()
);

INSERT INTO phxclaw.release_gate_evidence(
  evidence_uuid, tenant_uuid, run_uuid, gate_name, status, source_state_sha256,
  evidence_sha256, tool_name, tool_version, evidence_ref, verified_at
) VALUES (
  '25000000-0000-7000-8000-000000000003',
  '33333333-3333-7333-8333-333333333333',
  '25000000-0000-7000-8000-000000000001',
  'workspace_static', 'verified', decode(repeat('aa',32),'hex'), decode(repeat('bb',32),'hex'),
  'phxclaw-v025-test', '0.25.0', 'fixture', now()
);

INSERT INTO phxclaw.release_attestations(
  attestation_uuid, tenant_uuid, run_uuid, source_ready, static_verified,
  runtime_verified, e2e_verified, release_ready, attestation_sha256
) VALUES (
  '25000000-0000-7000-8000-000000000004',
  '33333333-3333-7333-8333-333333333333',
  '25000000-0000-7000-8000-000000000001',
  true, false, false, false, false, decode(repeat('cc',32),'hex')
);

DO $$ BEGIN
  BEGIN
    UPDATE phxclaw.release_gate_evidence
      SET tool_version = 'mutated'
      WHERE evidence_uuid = '25000000-0000-7000-8000-000000000003';
    RAISE EXCEPTION 'expected append-only release evidence rejection';
  EXCEPTION WHEN raise_exception THEN
    IF SQLERRM = 'expected append-only release evidence rejection' THEN RAISE; END IF;
  END;
END $$;

DO $$ BEGIN
  BEGIN
    UPDATE phxclaw.release_attestations
      SET source_ready = false
      WHERE attestation_uuid = '25000000-0000-7000-8000-000000000004';
    RAISE EXCEPTION 'expected append-only attestation rejection';
  EXCEPTION WHEN raise_exception THEN
    IF SQLERRM = 'expected append-only attestation rejection' THEN RAISE; END IF;
  END;
END $$;

ROLLBACK;
\echo 'V025_RELEASE_ATTESTATION_PASS'

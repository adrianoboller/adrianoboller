-- PhxClaw v0.70 — PostgreSQL Post-Install Verification
-- Run with: psql -v ON_ERROR_STOP=1 -f PhxClaw_PostgreSQL_POST_INSTALL_VERIFY_v0.70.sql
\set ON_ERROR_STOP on
DO $phx_verify$
DECLARE
  n integer; r text;
  required_relations text[] := ARRAY[
    'public.phx_project_trace_nodes','public.phx_project_trace_links','public.phx_project_trace_events','public.phx_project_trace_bindings',
    'public.phx_historical_backfill_runs','public.phx_historical_backfill_checkpoints','public.phx_historical_backfill_orphans','public.phx_historical_backfill_coverage',
    'public.phx_source_artifact','public.phx_source_evidence','public.phx_source_harvest_run','public.phx_source_harvest_file','public.phx_source_knowledge_candidate','public.phx_source_candidate_promotion',
    'phxclaw.project_state_snapshots','phxclaw.knowledge_promotion_requests','phxclaw.knowledge_promotion_evidence','phxclaw.knowledge_promotion_reviews','phxclaw.knowledge_promotion_receipts','phxclaw.knowledge_revocations','phxclaw.knowledge_promotion_events',
    'phxclaw.repo_analysis_runs','phxclaw.repo_analysis_files','phxclaw.repo_symbols','phxclaw.repo_calls','phxclaw.repo_dependencies','phxclaw.repo_hotspots',
    'phxclaw.protocol_sessions','phxclaw.protocol_requests','phxclaw.protocol_session_events',
    'phxclaw.backup_sets','phxclaw.backup_objects','phxclaw.restore_runs','phxclaw.upgrade_runs','phxclaw.disaster_recovery_drills',
    'phxclaw.research_records','phxclaw.research_record_evidence','phxclaw.research_graph_links','phxclaw.hypothesis_records','phxclaw.experiment_plans','phxclaw.hypothesis_evidence','phxclaw.hypothesis_decisions','phxclaw.experiment_runs','phxclaw.experiment_run_evidence','phxclaw.hypothesis_graph_links',
    'phxclaw.bpm_process_definitions','phxclaw.bpm_instances_v2','phxclaw.bpm_tokens_v2','phxclaw.bpm_events_v2','phxclaw.bpm_checkpoints',
    'phxclaw.channel_provider_accounts','phxclaw.channel_delivery_attempts',
    'phxclaw.device_platform_state','phxclaw.device_pairing_audit',
    'phxclaw.native_verification_runs','phxclaw.native_gate_catalog'
  ];
BEGIN
  IF to_regclass('public.phxclaw_schema_migrations') IS NULL THEN RAISE EXCEPTION 'FAIL: phxclaw_schema_migrations absent'; END IF;
  SELECT count(*) INTO n FROM public.phxclaw_schema_migrations WHERE version BETWEEN 1 AND 70;
  IF n<>70 THEN RAISE EXCEPTION 'FAIL: expected ledger 1..70, found %',n; END IF;
  IF NOT EXISTS (SELECT 1 FROM public.phxclaw_schema_migrations WHERE version=34 AND source_status='reconstructed') THEN RAISE EXCEPTION 'FAIL: v0.34 reconstructed marker absent'; END IF;
  FOR n IN 60..70 LOOP
    IF NOT EXISTS (SELECT 1 FROM public.phxclaw_schema_migrations WHERE version=n) THEN RAISE EXCEPTION 'FAIL: migration % absent',n; END IF;
  END LOOP;
  FOREACH r IN ARRAY required_relations LOOP IF to_regclass(r) IS NULL THEN RAISE EXCEPTION 'FAIL: relation absent: %',r; END IF; END LOOP;

  IF to_regprocedure('public.phx_source_candidate_must_be_allowed()') IS NULL THEN RAISE EXCEPTION 'FAIL: ALLOW-only source gate absent'; END IF;
  IF to_regprocedure('phxclaw.current_tenant_uuid()') IS NULL THEN RAISE EXCEPTION 'FAIL: current_tenant_uuid absent'; END IF;
  IF to_regprocedure('phxclaw.validate_knowledge_promotion_review()') IS NULL THEN RAISE EXCEPTION 'FAIL: promotion review gate absent'; END IF;
  IF to_regprocedure('phxclaw.reject_repo_intelligence_mutation()') IS NULL THEN RAISE EXCEPTION 'FAIL: repo intelligence immutability absent'; END IF;
  IF to_regprocedure('phxclaw.protocol_append_only()') IS NULL THEN RAISE EXCEPTION 'FAIL: protocol append-only absent'; END IF;
  IF to_regprocedure('phxclaw.reject_backup_evidence_delete()') IS NULL THEN RAISE EXCEPTION 'FAIL: backup evidence protection absent'; END IF;
  IF to_regprocedure('phxclaw.reject_research_hypothesis_delete()') IS NULL THEN RAISE EXCEPTION 'FAIL: research/hypothesis protection absent'; END IF;
  IF to_regprocedure('phxclaw.bpm_event_append_only()') IS NULL THEN RAISE EXCEPTION 'FAIL: BPM append-only absent'; END IF;
  IF to_regprocedure('phxclaw.consume_device_enrollment(uuid,bytea,uuid,text,bytea,text,text)') IS NULL THEN RAISE EXCEPTION 'FAIL: device enrollment function absent'; END IF;

  SELECT count(*) INTO n FROM pg_class c JOIN pg_namespace ns ON ns.oid=c.relnamespace
   WHERE ns.nspname='phxclaw' AND c.relname IN (
    'bpm_process_definitions','bpm_instances_v2','bpm_tokens_v2','bpm_events_v2','bpm_checkpoints',
    'channel_provider_accounts','channel_delivery_attempts','device_platform_state','device_pairing_audit'
   ) AND c.relrowsecurity AND c.relforcerowsecurity;
  IF n<>9 THEN RAISE EXCEPTION 'FAIL: v0.67-v0.69 RLS/FORCE RLS expected 9 found %',n; END IF;

  IF NOT EXISTS (SELECT 1 FROM pg_trigger t JOIN pg_class c ON c.oid=t.tgrelid JOIN pg_namespace ns ON ns.oid=c.relnamespace WHERE ns.nspname='phxclaw' AND c.relname='bpm_events_v2' AND t.tgname='trg_bpm_events_v2_append_only' AND NOT t.tgisinternal) THEN RAISE EXCEPTION 'FAIL: BPM append-only trigger absent'; END IF;
  SELECT count(*) INTO n FROM phxclaw.native_gate_catalog WHERE gate_code IN ('desktop_os_automation_e2e','real_stt_model_e2e','native_tauri_e2e','channel_provider_credentialed_e2e','device_pairing_wss_keyring_multiplatform_e2e') AND required;
  IF n<>5 THEN RAISE EXCEPTION 'FAIL: native gate catalog expected 5 core remaining gates, found %',n; END IF;
  RAISE NOTICE 'PASS: PhxClaw PostgreSQL v0.70 structurally verified.';
END $phx_verify$;
SELECT current_database() database_name,current_user verified_by,current_setting('server_version') postgres_version,count(*) migration_versions,min(version) min_version,max(version) max_version FROM public.phxclaw_schema_migrations;

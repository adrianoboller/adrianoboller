BEGIN;
CREATE TABLE IF NOT EXISTS phx_reconciliation_plan_snapshots(
 tenant_uuid uuid NOT NULL, snapshot_uuid uuid NOT NULL, project_uuid uuid NOT NULL, plan_revision_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'), plan_sha256 text NOT NULL CHECK(plan_sha256~'^[a-f0-9]{64}$'),
 payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,snapshot_uuid), UNIQUE(tenant_uuid,project_uuid,plan_revision_uuid,plan_sha256));
CREATE TABLE IF NOT EXISTS phx_execution_ledger_facts(
 tenant_uuid uuid NOT NULL, fact_uuid uuid NOT NULL, project_uuid uuid NOT NULL, work_item_uuid uuid NOT NULL, sequence_no bigint NOT NULL CHECK(sequence_no>0),
 controller_epoch bigint NOT NULL CHECK(controller_epoch>0), fencing_token bigint NOT NULL CHECK(fencing_token>0), observed_at timestamptz NOT NULL,
 source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'), fact_kind text NOT NULL, idempotency_key text NOT NULL,
 payload_sha256 text NOT NULL CHECK(payload_sha256~'^[a-f0-9]{64}$'), evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'),
 reason_code text, reason_status text NOT NULL CHECK(reason_status IN ('verified','unverified','missing')), causation_uuid uuid, correlation_uuid uuid NOT NULL, payload jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,fact_uuid), UNIQUE(tenant_uuid,project_uuid,sequence_no), UNIQUE(tenant_uuid,idempotency_key,payload_sha256));
CREATE TABLE IF NOT EXISTS phx_reconciliation_variances(
 tenant_uuid uuid NOT NULL, variance_uuid uuid NOT NULL, project_uuid uuid NOT NULL, work_item_uuid uuid NOT NULL, plan_revision_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'), variance_sha256 text NOT NULL CHECK(variance_sha256~'^[a-f0-9]{64}$'),
 reason_status text NOT NULL CHECK(reason_status IN ('verified','unverified','missing')), reason_code text, reason_evidence_sha256 text,
 payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,variance_uuid), UNIQUE(tenant_uuid,variance_uuid,variance_sha256));
CREATE TABLE IF NOT EXISTS phx_reconciliation_causal_links(
 tenant_uuid uuid NOT NULL, link_uuid uuid NOT NULL, project_uuid uuid NOT NULL, cause_fact_uuid uuid NOT NULL, effect_variance_uuid uuid NOT NULL,
 evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'), reason_code text NOT NULL, verified boolean NOT NULL DEFAULT false,
 payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,link_uuid),
 FOREIGN KEY(tenant_uuid,cause_fact_uuid) REFERENCES phx_execution_ledger_facts(tenant_uuid,fact_uuid), FOREIGN KEY(tenant_uuid,effect_variance_uuid) REFERENCES phx_reconciliation_variances(tenant_uuid,variance_uuid));
CREATE TABLE IF NOT EXISTS phx_sprint_gate_evidence(
 tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, sprint_id text NOT NULL, gate_id text NOT NULL,
 source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'), result text NOT NULL CHECK(result IN ('pass','fail','unavailable')),
 evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'), tool_version text NOT NULL, executed_at timestamptz NOT NULL,
 payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,evidence_uuid));
CREATE TABLE IF NOT EXISTS phx_sprint_gate_status_snapshots(
 tenant_uuid uuid NOT NULL, status_uuid uuid NOT NULL, sprint_id text NOT NULL, source_state_sha256 text NOT NULL CHECK(source_state_sha256~'^[a-f0-9]{64}$'),
 color text NOT NULL CHECK(color IN ('green','yellow','red')), evidence_sha256 text NOT NULL CHECK(evidence_sha256~'^[a-f0-9]{64}$'), payload jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(tenant_uuid,status_uuid));

DO $ddl$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['phx_reconciliation_plan_snapshots','phx_execution_ledger_facts','phx_reconciliation_variances','phx_reconciliation_causal_links','phx_sprint_gate_evidence','phx_sprint_gate_status_snapshots'] LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format($p$CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)$p$,t);
 END LOOP;
END $ddl$;
CREATE OR REPLACE FUNCTION phx_reconciliation_append_only() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'append-only table';END$$;
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['phx_reconciliation_plan_snapshots','phx_execution_ledger_facts','phx_reconciliation_variances','phx_reconciliation_causal_links','phx_sprint_gate_evidence','phx_sprint_gate_status_snapshots'] LOOP EXECUTE format('DROP TRIGGER IF EXISTS zz_append_only ON %I',t); EXECUTE format('CREATE TRIGGER zz_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phx_reconciliation_append_only()',t); END LOOP; END $$;
COMMIT;

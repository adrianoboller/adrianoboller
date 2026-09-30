-- PhxClaw v0.37 autonomous engineering workflow
BEGIN;
CREATE TABLE IF NOT EXISTS engineering_workflows (
 tenant_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, objective text NOT NULL, source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9A-Fa-f]{64}$'), policy_sha256 text NOT NULL CHECK (policy_sha256 ~ '^[0-9A-Fa-f]{64}$'), skill_catalog_sha256 text NOT NULL CHECK (skill_catalog_sha256 ~ '^[0-9A-Fa-f]{64}$'), risk text NOT NULL CHECK (risk IN ('low','medium','high','critical')), spec jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid, workflow_uuid));
CREATE TABLE IF NOT EXISTS engineering_stage_runs (
 tenant_uuid uuid NOT NULL, run_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, stage text NOT NULL, attempt integer NOT NULL CHECK (attempt>0), idempotency_key text NOT NULL, actor_uuid uuid NOT NULL, started_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid, run_uuid), UNIQUE (tenant_uuid, workflow_uuid, stage, attempt), UNIQUE (tenant_uuid,idempotency_key), FOREIGN KEY (tenant_uuid,workflow_uuid) REFERENCES engineering_workflows(tenant_uuid,workflow_uuid));
CREATE TABLE IF NOT EXISTS engineering_stage_evidence (
 tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, run_uuid uuid NOT NULL, stage text NOT NULL, source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9A-Fa-f]{64}$'), input_sha256 text NOT NULL CHECK (input_sha256 ~ '^[0-9A-Fa-f]{64}$'), output_sha256 text NOT NULL CHECK (output_sha256 ~ '^[0-9A-Fa-f]{64}$'), evidence_refs jsonb NOT NULL DEFAULT '[]'::jsonb, actor_uuid uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,evidence_uuid), FOREIGN KEY (tenant_uuid,workflow_uuid) REFERENCES engineering_workflows(tenant_uuid,workflow_uuid), FOREIGN KEY (tenant_uuid,run_uuid) REFERENCES engineering_stage_runs(tenant_uuid,run_uuid));
CREATE TABLE IF NOT EXISTS engineering_checkpoints (
 tenant_uuid uuid NOT NULL, checkpoint_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, stage text NOT NULL CHECK (stage='implement'), source_state_sha256 text NOT NULL, workspace_state_sha256 text NOT NULL, git_commit text, fencing_token bigint NOT NULL CHECK (fencing_token>=0), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,checkpoint_uuid), FOREIGN KEY (tenant_uuid,workflow_uuid) REFERENCES engineering_workflows(tenant_uuid,workflow_uuid));
CREATE TABLE IF NOT EXISTS engineering_transition_events (
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, stage text NOT NULL, outcome text NOT NULL CHECK (outcome IN ('completed','failed','rolled_back','skipped')), attempt integer NOT NULL CHECK (attempt>0), evidence_uuid uuid, checkpoint_uuid uuid, idempotency_key text NOT NULL, fencing_token bigint NOT NULL CHECK (fencing_token>=0), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,event_uuid), UNIQUE (tenant_uuid,idempotency_key), FOREIGN KEY (tenant_uuid,workflow_uuid) REFERENCES engineering_workflows(tenant_uuid,workflow_uuid), FOREIGN KEY (tenant_uuid,evidence_uuid) REFERENCES engineering_stage_evidence(tenant_uuid,evidence_uuid), FOREIGN KEY (tenant_uuid,checkpoint_uuid) REFERENCES engineering_checkpoints(tenant_uuid,checkpoint_uuid));
CREATE TABLE IF NOT EXISTS engineering_approvals (
 tenant_uuid uuid NOT NULL, approval_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, stage text NOT NULL, approver_uuid uuid NOT NULL, actor_uuid uuid NOT NULL, plan_sha256 text NOT NULL CHECK (plan_sha256 ~ '^[0-9A-Fa-f]{64}$'), approved boolean NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,approval_uuid), FOREIGN KEY (tenant_uuid,workflow_uuid) REFERENCES engineering_workflows(tenant_uuid,workflow_uuid));
CREATE TABLE IF NOT EXISTS engineering_rollbacks (
 tenant_uuid uuid NOT NULL, rollback_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, checkpoint_uuid uuid NOT NULL, reason text NOT NULL, before_sha256 text NOT NULL, after_sha256 text NOT NULL, verified boolean NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,rollback_uuid), FOREIGN KEY (tenant_uuid,workflow_uuid) REFERENCES engineering_workflows(tenant_uuid,workflow_uuid), FOREIGN KEY (tenant_uuid,checkpoint_uuid) REFERENCES engineering_checkpoints(tenant_uuid,checkpoint_uuid));
CREATE TABLE IF NOT EXISTS engineering_delivery_intents (
 tenant_uuid uuid NOT NULL, intent_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, artifact_sha256 text NOT NULL CHECK (artifact_sha256 ~ '^[0-9A-Fa-f]{64}$'), destination_kind text NOT NULL, external_side_effect boolean NOT NULL DEFAULT false, approval_uuid uuid, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,intent_uuid), FOREIGN KEY (tenant_uuid,workflow_uuid) REFERENCES engineering_workflows(tenant_uuid,workflow_uuid), FOREIGN KEY (tenant_uuid,approval_uuid) REFERENCES engineering_approvals(tenant_uuid,approval_uuid));
CREATE OR REPLACE FUNCTION phxclaw_prevent_mutation_0037() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only table: %', TG_TABLE_NAME; END $$;
ALTER TABLE engineering_workflows ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_workflows FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_workflows;
CREATE POLICY tenant_isolation ON engineering_workflows USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
ALTER TABLE engineering_stage_runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_stage_runs FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_stage_runs;
CREATE POLICY tenant_isolation ON engineering_stage_runs USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
ALTER TABLE engineering_stage_evidence ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_stage_evidence FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_stage_evidence;
CREATE POLICY tenant_isolation ON engineering_stage_evidence USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
ALTER TABLE engineering_checkpoints ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_checkpoints FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_checkpoints;
CREATE POLICY tenant_isolation ON engineering_checkpoints USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
ALTER TABLE engineering_transition_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_transition_events FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_transition_events;
CREATE POLICY tenant_isolation ON engineering_transition_events USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
ALTER TABLE engineering_approvals ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_approvals FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_approvals;
CREATE POLICY tenant_isolation ON engineering_approvals USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
ALTER TABLE engineering_rollbacks ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_rollbacks FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_rollbacks;
CREATE POLICY tenant_isolation ON engineering_rollbacks USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
ALTER TABLE engineering_delivery_intents ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_delivery_intents FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation ON engineering_delivery_intents;
CREATE POLICY tenant_isolation ON engineering_delivery_intents USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid);
DROP TRIGGER IF EXISTS engineering_stage_evidence_append_only ON engineering_stage_evidence;
CREATE TRIGGER engineering_stage_evidence_append_only BEFORE UPDATE OR DELETE ON engineering_stage_evidence FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0037();
DROP TRIGGER IF EXISTS engineering_checkpoints_append_only ON engineering_checkpoints;
CREATE TRIGGER engineering_checkpoints_append_only BEFORE UPDATE OR DELETE ON engineering_checkpoints FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0037();
DROP TRIGGER IF EXISTS engineering_transition_events_append_only ON engineering_transition_events;
CREATE TRIGGER engineering_transition_events_append_only BEFORE UPDATE OR DELETE ON engineering_transition_events FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0037();
DROP TRIGGER IF EXISTS engineering_approvals_append_only ON engineering_approvals;
CREATE TRIGGER engineering_approvals_append_only BEFORE UPDATE OR DELETE ON engineering_approvals FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0037();
DROP TRIGGER IF EXISTS engineering_rollbacks_append_only ON engineering_rollbacks;
CREATE TRIGGER engineering_rollbacks_append_only BEFORE UPDATE OR DELETE ON engineering_rollbacks FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0037();
DROP TRIGGER IF EXISTS engineering_delivery_intents_append_only ON engineering_delivery_intents;
CREATE TRIGGER engineering_delivery_intents_append_only BEFORE UPDATE OR DELETE ON engineering_delivery_intents FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0037();
COMMIT;

-- PhxClaw v0.49 Executive Decision Center
CREATE TABLE IF NOT EXISTS phx_executive_decision_cases (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 case_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 portfolio_snapshot_sha256 text NOT NULL CHECK (portfolio_snapshot_sha256 ~ '^[0-9a-f]{64}$'),
 trigger_kind text NOT NULL,
 trigger_evidence_sha256 text NOT NULL CHECK (trigger_evidence_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT now(),
 expires_at timestamptz NOT NULL,
 PRIMARY KEY (tenant_uuid, project_uuid, case_uuid),
 UNIQUE (tenant_uuid, case_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_decision_scenarios (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 case_uuid uuid NOT NULL,
 scenario_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 kind text NOT NULL,
 scenario_sha256 text NOT NULL CHECK (scenario_sha256 ~ '^[0-9a-f]{64}$'),
 model_evidence_sha256 text NOT NULL CHECK (model_evidence_sha256 ~ '^[0-9a-f]{64}$'),
 payload jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 expires_at timestamptz NOT NULL,
 PRIMARY KEY (tenant_uuid, project_uuid, case_uuid, scenario_uuid),
 UNIQUE (tenant_uuid, scenario_uuid),
 FOREIGN KEY (tenant_uuid, project_uuid, case_uuid) REFERENCES phx_executive_decision_cases(tenant_uuid, project_uuid, case_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_decision_evaluations (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 case_uuid uuid NOT NULL,
 scenario_uuid uuid NOT NULL,
 evaluation_uuid uuid NOT NULL,
 feasible boolean NOT NULL,
 approval_required boolean NOT NULL,
 utility_score double precision NOT NULL,
 evaluation_sha256 text NOT NULL CHECK (evaluation_sha256 ~ '^[0-9a-f]{64}$'),
 payload jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, evaluation_uuid),
 FOREIGN KEY (tenant_uuid, project_uuid, case_uuid, scenario_uuid) REFERENCES phx_executive_decision_scenarios(tenant_uuid, project_uuid, case_uuid, scenario_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_decision_approvals (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 case_uuid uuid NOT NULL,
 scenario_uuid uuid NOT NULL,
 approval_uuid uuid NOT NULL,
 signer_uuid uuid NOT NULL,
 approval_sha256 text NOT NULL CHECK (approval_sha256 ~ '^[0-9a-f]{64}$'),
 public_key_hex text NOT NULL,
 signature_hex text NOT NULL,
 issued_at timestamptz NOT NULL,
 expires_at timestamptz NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, approval_uuid),
 FOREIGN KEY (tenant_uuid, project_uuid, case_uuid, scenario_uuid) REFERENCES phx_executive_decision_scenarios(tenant_uuid, project_uuid, case_uuid, scenario_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_decision_executions (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 case_uuid uuid NOT NULL,
 scenario_uuid uuid NOT NULL,
 execution_uuid uuid NOT NULL,
 supervisor_plan_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9a-f]{64}$'),
 scenario_sha256 text NOT NULL CHECK (scenario_sha256 ~ '^[0-9a-f]{64}$'),
 approval_sha256 text NULL,
 controller_epoch bigint NOT NULL CHECK (controller_epoch > 0),
 fencing_token bigint NOT NULL CHECK (fencing_token > 0),
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, execution_uuid),
 FOREIGN KEY (tenant_uuid, project_uuid, case_uuid, scenario_uuid) REFERENCES phx_executive_decision_scenarios(tenant_uuid, project_uuid, case_uuid, scenario_uuid)
);
CREATE TABLE IF NOT EXISTS phx_executive_decision_events (
 tenant_uuid uuid NOT NULL,
 project_uuid uuid NOT NULL,
 case_uuid uuid NOT NULL,
 event_uuid uuid NOT NULL,
 event_type text NOT NULL,
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9a-f]{64}$'),
 payload jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY (tenant_uuid, event_uuid),
 FOREIGN KEY (tenant_uuid, project_uuid, case_uuid) REFERENCES phx_executive_decision_cases(tenant_uuid, project_uuid, case_uuid)
);

DO $$ DECLARE t text; BEGIN
 FOR t IN SELECT unnest(ARRAY['phx_executive_decision_cases','phx_executive_decision_scenarios','phx_executive_decision_evaluations','phx_executive_decision_approvals','phx_executive_decision_executions','phx_executive_decision_events']) LOOP
  EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
  EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
  EXECUTE format('DROP POLICY IF EXISTS phx_tenant_isolation ON %I', t);
  EXECUTE format($policy$CREATE POLICY phx_tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid)$policy$, t);
 END LOOP;
END $$;

CREATE OR REPLACE FUNCTION phx_executive_decision_append_only() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only executive decision record'; END $$;
DO $$ DECLARE t text; trig text; BEGIN
 FOR t IN SELECT unnest(ARRAY['phx_executive_decision_cases','phx_executive_decision_scenarios','phx_executive_decision_evaluations','phx_executive_decision_approvals','phx_executive_decision_executions','phx_executive_decision_events']) LOOP
  trig := t || '_append_only';
  EXECUTE format('DROP TRIGGER IF EXISTS %I ON %I',trig,t);
  EXECUTE format('CREATE TRIGGER %I BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phx_executive_decision_append_only()',trig,t);
 END LOOP;
END $$;

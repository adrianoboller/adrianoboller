BEGIN;
CREATE TABLE IF NOT EXISTS engineering_swarms (
 tenant_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, workflow_uuid uuid NOT NULL, objective text NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9A-Fa-f]{64}$'), policy_sha256 text NOT NULL CHECK (policy_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 skill_catalog_sha256 text NOT NULL CHECK (skill_catalog_sha256 ~ '^[0-9A-Fa-f]{64}$'), max_parallel_teams integer NOT NULL CHECK (max_parallel_teams BETWEEN 1 AND 6), spec jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_team_assignments (
 tenant_uuid uuid NOT NULL, assignment_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, team text NOT NULL CHECK (team IN ('research','architecture','coding','qa','security','documentation')),
 actor_uuid uuid NOT NULL, base_source_sha256 text NOT NULL CHECK (base_source_sha256 ~ '^[0-9A-Fa-f]{64}$'), worktree_path text NOT NULL, branch text NOT NULL,
 lease_uuid uuid NOT NULL, fencing_token bigint NOT NULL CHECK (fencing_token>=0), dependencies jsonb NOT NULL DEFAULT '[]'::jsonb, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,assignment_uuid), UNIQUE (tenant_uuid,swarm_uuid,team), UNIQUE (tenant_uuid,swarm_uuid,worktree_path), UNIQUE (tenant_uuid,swarm_uuid,branch),
 FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_team_leases (
 tenant_uuid uuid NOT NULL, lease_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, team text NOT NULL, holder_uuid uuid NOT NULL, fencing_token bigint NOT NULL CHECK (fencing_token>=0),
 expires_at timestamptz NOT NULL, last_heartbeat_at timestamptz NOT NULL DEFAULT clock_timestamp(), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,lease_uuid), UNIQUE (tenant_uuid,swarm_uuid,team,fencing_token), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_team_checkpoints (
 tenant_uuid uuid NOT NULL, checkpoint_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, team text NOT NULL, workspace_state_sha256 text NOT NULL CHECK (workspace_state_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 git_commit text, fencing_token bigint NOT NULL CHECK (fencing_token>=0), created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,checkpoint_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_team_evidence (
 tenant_uuid uuid NOT NULL, evidence_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, team text NOT NULL, actor_uuid uuid NOT NULL,
 source_state_sha256 text NOT NULL CHECK (source_state_sha256 ~ '^[0-9A-Fa-f]{64}$'), artifact_sha256 text NOT NULL CHECK (artifact_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 evidence_refs jsonb NOT NULL, passed boolean NOT NULL, highest_severity text NOT NULL CHECK (highest_severity IN ('info','low','medium','high','critical')),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,evidence_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_conflicts (
 tenant_uuid uuid NOT NULL, conflict_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, kind text NOT NULL CHECK (kind IN ('source_drift','overlapping_write','evidence_conflict','architecture_conflict','qa_failure','security_finding','documentation_mismatch')),
 teams jsonb NOT NULL, subject text NOT NULL, evidence_refs jsonb NOT NULL DEFAULT '[]'::jsonb, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,conflict_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_conflict_resolutions (
 tenant_uuid uuid NOT NULL, resolution_uuid uuid NOT NULL, conflict_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, resolver_uuid uuid NOT NULL,
 resolution_sha256 text NOT NULL CHECK (resolution_sha256 ~ '^[0-9A-Fa-f]{64}$'), rationale text NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,resolution_uuid), UNIQUE (tenant_uuid,conflict_uuid), FOREIGN KEY (tenant_uuid,conflict_uuid) REFERENCES swarm_conflicts(tenant_uuid,conflict_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_merge_candidates (
 tenant_uuid uuid NOT NULL, candidate_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, base_source_sha256 text NOT NULL CHECK (base_source_sha256 ~ '^[0-9A-Fa-f]{64}$'),
 integration_tree_sha256 text NOT NULL CHECK (integration_tree_sha256 ~ '^[0-9A-Fa-f]{64}$'), manifest jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,candidate_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_merge_gate_events (
 tenant_uuid uuid NOT NULL, event_uuid uuid NOT NULL, candidate_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, decision text NOT NULL CHECK (decision IN ('allow','block','needs_approval')),
 evidence_sha256 text NOT NULL CHECK (evidence_sha256 ~ '^[0-9A-Fa-f]{64}$'), fencing_token bigint NOT NULL CHECK (fencing_token>=0), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY (tenant_uuid,event_uuid), FOREIGN KEY (tenant_uuid,candidate_uuid) REFERENCES swarm_merge_candidates(tenant_uuid,candidate_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid));
CREATE TABLE IF NOT EXISTS swarm_team_rollbacks (
 tenant_uuid uuid NOT NULL, rollback_uuid uuid NOT NULL, swarm_uuid uuid NOT NULL, team text NOT NULL, checkpoint_uuid uuid NOT NULL, before_sha256 text NOT NULL CHECK (before_sha256 ~ '^[0-9A-Fa-f]{64}$'), after_sha256 text NOT NULL CHECK (after_sha256 ~ '^[0-9A-Fa-f]{64}$'), verified boolean NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (tenant_uuid,rollback_uuid), FOREIGN KEY (tenant_uuid,swarm_uuid) REFERENCES engineering_swarms(tenant_uuid,swarm_uuid), FOREIGN KEY (tenant_uuid,checkpoint_uuid) REFERENCES swarm_team_checkpoints(tenant_uuid,checkpoint_uuid));
CREATE OR REPLACE FUNCTION phxclaw_prevent_mutation_0038() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only table: %', TG_TABLE_NAME; END $$;
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['engineering_swarms','swarm_team_assignments','swarm_team_leases','swarm_team_checkpoints','swarm_team_evidence','swarm_conflicts','swarm_conflict_resolutions','swarm_merge_candidates','swarm_merge_gate_events','swarm_team_rollbacks'] LOOP
 EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY',t); EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY',t);
 EXECUTE format('DROP POLICY IF EXISTS tenant_isolation ON %I',t);
 EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t);
 END LOOP; END $$;
DO $$ DECLARE t text; BEGIN FOREACH t IN ARRAY ARRAY['swarm_team_checkpoints','swarm_team_evidence','swarm_conflicts','swarm_conflict_resolutions','swarm_merge_candidates','swarm_merge_gate_events','swarm_team_rollbacks'] LOOP
 EXECUTE format('DROP TRIGGER IF EXISTS %I_append_only ON %I',t,t); EXECUTE format('CREATE TRIGGER %I_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION phxclaw_prevent_mutation_0038()',t,t);
 END LOOP; END $$;
COMMIT;

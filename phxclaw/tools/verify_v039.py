#!/usr/bin/env python3
from pathlib import Path
import json, sys, tomllib
ROOT=Path(__file__).resolve().parents[1]
BASE=ROOT/'overlay' if (ROOT/'overlay').is_dir() else ROOT
checks=[]
def ck(name, cond, detail=''): checks.append({'name':name,'pass':bool(cond),'detail':detail})
for rel in ['crates/phxclaw-swarm-merge-intelligence/Cargo.toml','crates/phxclaw-swarm-merge-intelligence/src/lib.rs','config/swarm-merge-intelligence-policy.v039.json','config/swarm-contract-policy.v039.json','migrations/0039_swarm_consensus_merge_intelligence.sql','tests/sql/v039_rls_e2e.sql']:
 ck('exists_'+rel,(BASE/rel).is_file())
rust=(BASE/'crates/phxclaw-swarm-merge-intelligence/src/lib.rs').read_text()
for s in ['ContractKind','ConflictClass','ChangeIntent','SemanticConflict','MergeOrderPlan','IntegrationReplayEvidence','MutationEvidence','SignedResolutionDocument','VerifiedResolution','ConsensusEvidence','detect_semantic_conflicts','build_merge_order','contract_gate','replay_gate','mutation_gate','verify_signed_resolution','consensus_gate','merge_bundle_hash','no_majority_override','DependencyCycle','BlockingConflict','UntrustedSigner']:
 ck('rust_'+s,s in rust)
ck('rust_ed25519_verify','key.verify(&payload,&signature)' in rust)
ck('rust_no_majority_comment','Counts/votes are intentionally ignored' in rust)
pol=json.loads((BASE/'config/swarm-merge-intelligence-policy.v039.json').read_text())
ck('policy_majority_override_false',pol['resolution']['majority_override_allowed'] is False)
ck('policy_mutation_gate',pol['mutation_gate']['enabled'] and pol['mutation_gate']['minimum_score_bps']>=7000)
ck('policy_replay_required',pol['require_integration_replay'] is True)
sql=(BASE/'migrations/0039_swarm_consensus_merge_intelligence.sql').read_text()
for t in ['swarm_change_intents','swarm_contract_surfaces','swarm_semantic_conflicts_v039','swarm_merge_dependencies_v039','swarm_merge_plans_v039','swarm_integration_replays_v039','swarm_mutation_evidence_v039','swarm_resolution_documents_v039','swarm_consensus_decisions_v039','swarm_merge_execution_events_v039']:
 ck('sql_table_'+t,('CREATE TABLE IF NOT EXISTS '+t) in sql and t in sql)
ck('sql_force_rls','FORCE ROW LEVEL SECURITY' in sql)
rls_needle = "nullif(current_setting(''phxclaw.tenant_uuid'', true), " + chr(39)*4 + ")::uuid"
ck('sql_rls_quoting', rls_needle in sql)
ck('sql_append_only','BEFORE UPDATE OR DELETE' in sql)
ck('sql_tenant_fk','FOREIGN KEY (tenant_uuid,swarm_uuid)' in sql)
ck('sql_swarm_fk_intents','FOREIGN KEY (tenant_uuid,swarm_uuid,before_intent_uuid)' in sql and 'FOREIGN KEY (tenant_uuid,swarm_uuid,after_intent_uuid)' in sql)
ck('sql_swarm_fk_plan','FOREIGN KEY (tenant_uuid,swarm_uuid,plan_uuid)' in sql)
ck('sql_mutant_bound','killed_mutants<=total_mutants' in sql)
sh=(BASE/'ci/run-v039-native.sh').read_text()
ck('native_locked','cargo check --workspace --locked' in sh and 'cargo test --workspace --locked' in sh)
ck('native_separate_rls_url','PHXCLAW_RLS_DATABASE_URL' in sh)
ck('native_no_fake_pass','|| true' not in sh)
# repairs only exist in package layout
if (ROOT/'repairs/V038_RLS_REPAIR.json').is_file():
 rep=json.loads((ROOT/'repairs/V038_RLS_REPAIR.json').read_text()); ck('repair_hash_guard',rep['affected_sha256']=='4a3096b46a95ed0d5a72083c9ee999665ae4a46c55111902316a318ab85bd3a2')
report={'suite':'PhxClaw v0.39 static verifier','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}
out=(ROOT/'reports/V039_STATIC_VERIFY_REPORT.json') if (ROOT/'reports').is_dir() else (ROOT/'reports/V039_STATIC_VERIFY_REPORT.json')
out.parent.mkdir(parents=True,exist_ok=True); out.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'pass':report['pass'],'fail':report['fail']})); sys.exit(1 if report['fail'] else 0)

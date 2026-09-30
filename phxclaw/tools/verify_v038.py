#!/usr/bin/env python3
from pathlib import Path
import json, re, sys
HERE=Path(__file__).resolve().parent
PKG=HERE.parent
ROOT=(PKG/'overlay') if (PKG/'overlay/crates/phxclaw-engineering-swarm').is_dir() else PKG
checks=[]
def ck(name,cond,detail=''): checks.append({'name':name,'pass':bool(cond),'detail':detail})
def txt(rel): return (ROOT/rel).read_text()
for rel in ['crates/phxclaw-engineering-swarm/Cargo.toml','crates/phxclaw-engineering-swarm/src/lib.rs','config/engineering-swarm-policy.v038.json','config/engineering-swarm-team-map.v038.json','migrations/0038_autonomous_engineering_swarm.sql','tests/sql/v038_rls_e2e.sql']:
 ck('exists_'+rel,(ROOT/rel).is_file())
lib=txt('crates/phxclaw-engineering-swarm/src/lib.rs'); sql=txt('migrations/0038_autonomous_engineering_swarm.sql'); pol=json.loads(txt('config/engineering-swarm-policy.v038.json'))
for token in ['TeamRole','TeamAssignment','TeamEvidence','TeamCheckpoint','MergeConflict','MergeCandidate','ready_teams','verify_assignment','detect_write_conflicts','consensus_gate','verify_approval','rollback_allowed']:
 ck('rust_'+token,token in lib)
for token in ['SharedWritableWorktree','IndependenceRequired','UnresolvedConflict','QualityGateFailed','SecurityGateFailed','StaleFencing']:
 ck('rust_error_'+token,token in lib)
ck('no_majority_vote',pol['evidence']['majority_vote_for_facts'] is False)
ck('no_shared_worktree',pol['workspace']['shared_writable_worktree'] is False)
ck('qa_independent',pol['independence']['qa_must_be_independent_from_coding'] is True)
ck('security_independent',pol['independence']['security_must_be_independent_from_coding'] is True)
ck('human_main_prod',pol['merge_gate']['human_approval_for_main_or_production'] is True)
for table in ['engineering_swarms','swarm_team_assignments','swarm_team_leases','swarm_team_checkpoints','swarm_team_evidence','swarm_conflicts','swarm_conflict_resolutions','swarm_merge_candidates','swarm_merge_gate_events','swarm_team_rollbacks']:
 ck('sql_table_'+table,('CREATE TABLE IF NOT EXISTS '+table) in sql)
ck('sql_force_rls_loop','FORCE ROW LEVEL SECURITY' in sql and 'tenant_isolation' in sql and "current_setting(''phxclaw.tenant_uuid'', true)" in sql)
ck('sql_append_only','phxclaw_prevent_mutation_0038' in sql and "swarm_merge_gate_events" in sql)
ck('sql_compound_tenant_fk',sql.count('FOREIGN KEY (tenant_uuid,')>=9,str(sql.count('FOREIGN KEY (tenant_uuid,')))
e2e=txt('tests/sql/v038_rls_e2e.sql')
ck('e2e_non_superuser',"NOSUPERUSER" in e2e and "NOBYPASSRLS" in e2e)
ck('e2e_cross_tenant','cross-tenant read leaked' in e2e)
cap=json.loads(txt('capabilities/V038_CAPABILITIES_DELTA.json'))
ck('cap_count',len(cap['capabilities'])==54,str(len(cap['capabilities'])))
ck('cap_total',cap['projected_total']==698)
# parse all json
for p in ROOT.rglob('*.json'):
 if 'reports' in p.relative_to(ROOT).parts: continue
 try: json.loads(p.read_text()); ck('json_'+str(p.relative_to(ROOT)),True)
 except Exception as e: ck('json_'+str(p.relative_to(ROOT)),False,str(e))
report={'suite':'PhxClaw v0.38 static verifier','pass':sum(c['pass'] for c in checks),'fail':sum(not c['pass'] for c in checks),'checks':checks}
out=(PKG/'reports/V038_STATIC_VERIFY_REPORT.json') if (PKG/'overlay').is_dir() else (ROOT/'reports/V038_STATIC_VERIFY_REPORT.json')
out.parent.mkdir(parents=True,exist_ok=True); out.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'pass':report['pass'],'fail':report['fail']})); sys.exit(1 if report['fail'] else 0)

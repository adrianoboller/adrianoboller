#!/usr/bin/env python3
from pathlib import Path
import json, tempfile, subprocess, shutil, sys
HERE=Path(__file__).resolve().parent; PKG=HERE.parent
checks=[]
def ck(n,c,d=''): checks.append({'name':n,'pass':bool(c),'detail':d})
# static semantics via source
lib=(PKG/'overlay/crates/phxclaw-engineering-swarm/src/lib.rs').read_text()
pol=json.loads((PKG/'overlay/config/engineering-swarm-policy.v038.json').read_text())
ck('parallel_qa_security','TeamRole::Qa|TeamRole::Security' in lib)
ck('isolated_worktree','SharedWritableWorktree' in lib and pol['workspace']['isolated_worktree_per_team'])
ck('unresolved_conflict_blocks','UnresolvedConflict' in lib and pol['merge_gate']['unresolved_conflicts_allowed'] is False)
ck('qa_security_independent','qa.actor_uuid==coding_actor || sec.actor_uuid==coding_actor' in lib)
ck('security_high_blocks','sec.highest_severity>=Severity::High' in lib)
ck('main_needs_approval','candidate.target_main_or_production' in lib and 'NeedsApproval' in lib)
ck('rollback_checkpoint','rollback_allowed' in lib and 'CheckpointRequired' in lib)
# apply test
with tempfile.TemporaryDirectory() as td:
 t=Path(td)/'base'; t.mkdir(); (t/'config/overlays').mkdir(parents=True)
 (t/'config/overlays/v0.37.applied.json').write_text('{}')
 for rel in ['crates/phxclaw-autonomous-engineering/Cargo.toml','config/autonomous-engineering-policy.v037.json','migrations/0037_autonomous_engineering_workflow.sql']:
  p=t/rel; p.parent.mkdir(parents=True,exist_ok=True); p.write_text('[package]\nname="x"\nversion="0"\n' if rel.endswith('Cargo.toml') else ('{}' if rel.endswith('.json') else 'BEGIN; COMMIT;'))
 (t/'Cargo.toml').write_text('[workspace]\nresolver="2"\nmembers=[\n  "crates/existing"\n]\n'); (t/'crates/existing').mkdir(parents=True); (t/'crates/existing/Cargo.toml').write_text('[package]\nname="existing"\nversion="0.1.0"\n')
 p=subprocess.run([sys.executable,str(PKG/'tools/apply_overlay.py'),str(t)],capture_output=True,text=True); ck('apply_first',p.returncode==0,p.stderr+p.stdout)
 p2=subprocess.run([sys.executable,str(PKG/'tools/apply_overlay.py'),str(t)],capture_output=True,text=True); ck('apply_idempotent',p2.returncode==0 and 'already applied' in p2.stdout,p2.stderr+p2.stdout)
 ck('crate_once',(t/'Cargo.toml').read_text().count('crates/phxclaw-engineering-swarm')==1)
 pv=subprocess.run([sys.executable,str(t/'tools/verify_v038.py')],capture_output=True,text=True); ck('installed_verifier',pv.returncode==0,pv.stderr+pv.stdout)
 # conflict test
 marker=t/'config/overlays/v0.38.applied.json'; marker.unlink(); (t/'config/engineering-swarm-policy.v038.json').write_text('{"unknown":true}')
 pc=subprocess.run([sys.executable,str(PKG/'tools/apply_overlay.py'),str(t)],capture_output=True,text=True); ck('unknown_conflict_fail_closed',pc.returncode!=0 and 'conflict:' in (pc.stderr+pc.stdout),pc.stderr+pc.stdout)
report={'suite':'PhxClaw v0.38 local/application','pass':sum(c['pass'] for c in checks),'fail':sum(not c['pass'] for c in checks),'checks':checks}
(PKG/'reports/V038_LOCAL_TEST_REPORT.json').write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps({'pass':report['pass'],'fail':report['fail']})); sys.exit(1 if report['fail'] else 0)

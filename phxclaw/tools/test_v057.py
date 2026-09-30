#!/usr/bin/env python3
from pathlib import Path
import json, tempfile, subprocess, sys, shutil, hashlib
P=Path(__file__).resolve().parents[1]; checks=[]
def c(n,v,d=''): checks.append({'name':n,'pass':bool(v),'detail':d})
# Behavioral contract tests mirrored from Rust policy.
def can_promote(kind,risk,reversible,evidence,evaluators,tests,security,regressed,contradictions,stale,touches_core,level=2):
    if evidence!='production': return (False,False)
    if touches_core or kind=='core_policy': return (False,False)
    if stale or evaluators<2 or not tests or security>0 or regressed or contradictions>0: return (False,False)
    auto=(kind=='prompt' and risk=='low' and reversible and level>=2)
    return (True,auto)
c('fixture_blocked',can_promote('prompt','low',True,'fixture',2,True,0,False,0,False,False)==(False,False))
c('core_blocked',can_promote('core_policy','low',True,'production',2,True,0,False,0,False,True)==(False,False))
c('prompt_low_auto',can_promote('prompt','low',True,'production',2,True,0,False,0,False,False)==(True,True))
c('skill_not_auto',can_promote('skill','low',True,'production',2,True,0,False,0,False,False)==(True,False))
c('contradiction_blocks',can_promote('prompt','low',True,'production',2,True,0,False,1,False,False)==(False,False))
c('stale_blocks',can_promote('prompt','low',True,'production',2,True,0,False,0,True,False)==(False,False))
# Apply/idempotence test.
with tempfile.TemporaryDirectory() as td:
    root=Path(td)
    (root/'config').mkdir(); (root/'crates/phxclaw-skill-evolution').mkdir(parents=True); (root/'crates/phxclaw-knowledge-evidence-graph').mkdir(); (root/'crates/phxclaw-performance-fabric').mkdir()
    (root/'config/phxclaw.config.json').write_text('{}\n'); (root/'config/sprint-gate-matrix.v056.json').write_text('{"sprints":[]}\n')
    (root/'Cargo.toml').write_text('[workspace]\nmembers = [\n]\n')
    r1=subprocess.run([sys.executable,str(P/'tools/apply_v057.py'),str(root)],capture_output=True,text=True)
    c('apply_first',r1.returncode==0,r1.stderr)
    r2=subprocess.run([sys.executable,str(P/'tools/apply_v057.py'),str(root)],capture_output=True,text=True)
    c('apply_idempotent',r2.returncode==0 and json.loads(r2.stdout)['changed']==[],r2.stderr)
    rv=subprocess.run([sys.executable,str(root/'tools/verify_v057.py')],cwd=root,capture_output=True,text=True)
    c('installed_verify',rv.returncode==0,rv.stdout[-1000:])
    cfg=json.loads((root/'config/phxclaw.config.json').read_text())
    c('config_merged','continuous_learning' in cfg and cfg['continuous_learning']['autonomy']['core_auto_merge'] is False)
    # conflict fail-closed
    q=root/'schemas/experience-episode-v057.schema.json'; q.write_text('different')
    r3=subprocess.run([sys.executable,str(P/'tools/apply_v057.py'),str(root)],capture_output=True,text=True)
    c('conflict_fail_closed',r3.returncode!=0 and 'refusing to overwrite' in r3.stderr)
report={'suite':'PhxClaw v0.57 behavior/application','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}; print(json.dumps(report,indent=2)); sys.exit(1 if report['fail'] else 0)

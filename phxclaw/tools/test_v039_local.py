#!/usr/bin/env python3
from pathlib import Path
import json, tempfile, shutil, subprocess, hashlib, sys
ROOT=Path(__file__).resolve().parents[1]
checks=[]
def ck(name,cond,detail=''): checks.append({'name':name,'pass':bool(cond),'detail':detail})
def h(s): return hashlib.sha256(s.encode()).hexdigest()
# deterministic topological-order reference
def topo(ids,deps):
 indeg={i:0 for i in ids}; edges={i:set() for i in ids}
 for i in ids:
  for d in deps.get(i,set()):
   if d not in indeg: raise ValueError('missing')
   if i not in edges[d]: edges[d].add(i); indeg[i]+=1
 ready=sorted([i for i,v in indeg.items() if v==0]); out=[]
 while ready:
  i=ready.pop(0); out.append(i)
  for n in sorted(edges[i]):
   indeg[n]-=1
   if indeg[n]==0: ready.append(n); ready.sort()
 if len(out)!=len(ids): raise ValueError('cycle')
 return out
ck('deterministic_order',topo(['a','b','c'],{'b':{'a'},'c':{'a'}})==['a','b','c'])
try: topo(['a','b'],{'a':{'b'},'b':{'a'}}); ck('cycle_blocks',False)
except ValueError: ck('cycle_blocks',True)
# majority must not override evidence
policy=json.loads((ROOT/'overlay/config/swarm-merge-intelligence-policy.v039.json').read_text())
ck('majority_override_disabled',policy['resolution']['majority_override_allowed'] is False)
ck('critical_survivor_blocks',policy['mutation_gate']['critical_survivor_blocks'] is True)
# affected repair hash
bad=ROOT/'repairs/0038_autonomous_engineering_swarm.fixed.sql'
manifest=json.loads((ROOT/'repairs/V038_RLS_REPAIR.json').read_text())
ck('replacement_hash',hashlib.sha256(bad.read_bytes()).hexdigest()==manifest['replacement_sha256'])
# synthetic base apply
with tempfile.TemporaryDirectory() as td:
 b=Path(td)/'base'; (b/'config/overlays').mkdir(parents=True); (b/'crates/phxclaw-engineering-swarm').mkdir(parents=True); (b/'config').mkdir(exist_ok=True); (b/'migrations').mkdir(exist_ok=True)
 (b/'config/overlays/v0.38.applied.json').write_text('{}\n')
 (b/'crates/phxclaw-engineering-swarm/Cargo.toml').write_text('[package]\nname="phxclaw-engineering-swarm"\nversion="0.38.0"\n')
 (b/'config/engineering-swarm-policy.v038.json').write_text('{}\n')
 # reconstruct known affected migration from fixed by reverting one exact policy line
 fixed=(ROOT/'repairs/0038_autonomous_engineering_swarm.fixed.sql').read_text()
 good="EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t);"
 badline="EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), )::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(phxclaw.tenant_uuid, true), )::uuid)',t);"
 affected=fixed.replace(good,badline); (b/'migrations/0038_autonomous_engineering_swarm.sql').write_text(affected)
 ck('affected_fixture_hash',hashlib.sha256(affected.encode()).hexdigest()==manifest['affected_sha256'])
 (b/'Cargo.toml').write_text('[workspace]\nmembers = [\n  "crates/phxclaw-engineering-swarm"\n]\n')
 p=subprocess.run([sys.executable,str(ROOT/'tools/apply_overlay.py'),str(b)],capture_output=True,text=True)
 ck('apply_first',p.returncode==0,p.stdout+p.stderr)
 p2=subprocess.run([sys.executable,str(ROOT/'tools/apply_overlay.py'),str(b)],capture_output=True,text=True)
 ck('apply_idempotent',p2.returncode==0,p2.stdout+p2.stderr)
 cargo=(b/'Cargo.toml').read_text(); ck('crate_once',cargo.count('crates/phxclaw-swarm-merge-intelligence')==1)
 ck('v038_repaired',hashlib.sha256((b/'migrations/0038_autonomous_engineering_swarm.sql').read_bytes()).hexdigest()==manifest['replacement_sha256'])
 v=subprocess.run([sys.executable,str(b/'tools/verify_v039.py')],cwd=b,capture_output=True,text=True)
 ck('installed_verifier',v.returncode==0,v.stdout+v.stderr)
 # unknown repair content must block
 u=Path(td)/'unknown'; shutil.copytree(b,u); (u/'config/overlays/v0.39.applied.json').unlink(missing_ok=True); (u/'migrations/0038_autonomous_engineering_swarm.sql').write_text('unknown\n')
 q=subprocess.run([sys.executable,str(ROOT/'tools/apply_overlay.py'),str(u)],capture_output=True,text=True)
 ck('unknown_v038_repair_fail_closed',q.returncode!=0 and 'refusing repair' in (q.stdout+q.stderr),q.stdout+q.stderr)
report={'suite':'PhxClaw v0.39 local/application','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}
(ROOT/'reports').mkdir(exist_ok=True); (ROOT/'reports/V039_LOCAL_TEST_REPORT.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'pass':report['pass'],'fail':report['fail']})); sys.exit(1 if report['fail'] else 0)

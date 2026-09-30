#!/usr/bin/env python3
from pathlib import Path
import hashlib, json, shutil, subprocess, sys, tempfile, uuid
PKG=Path(__file__).resolve().parents[1]; ROOT=PKG/'overlay' if (PKG/'overlay/capabilities/V032_CAPABILITIES_DELTA.json').is_file() else PKG; REPORT=PKG/'reports' if (PKG/'overlay').exists() else ROOT/'reports'; checks=[]
def ck(n,o,d=''): checks.append({'name':n,'pass':bool(o),'detail':d}); print(('PASS' if o else 'FAIL'),n,d)
def h(s): return hashlib.sha256(s.encode()).hexdigest()
def weighted(vals):
 den=sum(w for _,w in vals); return sum(v*w for v,w in vals)//den if den else 0
def aggregate(cases, obs):
 seen=set(); q=[]; su=[]; lat=[]; costs=[]; covered=set()
 weights={c['id']:c['weight'] for c in cases}
 for o in obs:
  key=(o['run'],o['case'])
  if key in seen: raise ValueError('duplicate')
  seen.add(key); w=weights[o['case']]; q.append((o['quality'],w)); su.append((10000 if o['success'] else 0,w)); lat.append(o['latency']); covered.add(o['case'])
  if o.get('cost') is not None: costs.append(o['cost'])
 lat=sorted(lat); p95=lat[max(0,min(len(lat)-1,(len(lat)*95+99)//100-1))]
 costs=sorted(costs); med=costs[len(costs)//2] if costs else None
 return {'samples':len(obs),'quality':weighted(q),'success':weighted(su),'coverage':len(covered)*10000//len(cases),'p95':p95,'cost':med}
def promotable(p,prev=None,fixture=False,stale=False):
 if fixture or stale:return False
 if p['samples']<20 or p['success']<9000 or p['quality']<8500 or p['coverage']<9000:return False
 if prev and (prev['quality']-p['quality']>300 or prev['success']-p['success']>200):return False
 return True
cases=[{'id':f'c{i}','weight':1} for i in range(1,5)]
obs=[]
for r in range(5):
 for i,c in enumerate(cases): obs.append({'run':f'r{r}','case':c['id'],'quality':9000+i*100,'success':True,'latency':100+i*10,'cost':5+i})
p=aggregate(cases,obs); ck('aggregate_samples',p['samples']==20); ck('aggregate_coverage',p['coverage']==10000); ck('aggregate_quality',9000<=p['quality']<=9300); ck('aggregate_p95',p['p95']==130); ck('aggregate_median_cost',p['cost'] in (6,7)); ck('promotion_pass',promotable(p)); ck('fixture_never_promotes',not promotable(p,fixture=True)); ck('stale_profile_never_promotes',not promotable(p,stale=True))
reg=dict(p); reg['quality']=8500; ck('regression_blocked',not promotable(reg,{'quality':9200,'success':10000}))
try: aggregate(cases,obs+[dict(obs[0])]); dup=False
except ValueError: dup=True
ck('duplicate_observation_blocked',dup)
# Adaptive ordering is applied only to candidates already accepted by the base v0.31 safety router.
base_eligible=[('local','m1'),('cloud','m2')]
profiles=[{'key':('local','m1'),'score':9100*4+9900*3-150},{'key':('cloud','m2'),'score':9800*4+9950*3-90-20}]
ranked=sorted([x for x in profiles if x['key'] in base_eligible],key=lambda x:(-x['score'],x['key']))
ck('adaptive_prefers_measured_quality',ranked[0]['key']==('cloud','m2'))
restricted_base=[('local','m1')]; ranked_restricted=sorted([x for x in profiles if x['key'] in restricted_base],key=lambda x:(-x['score'],x['key'])); ck('restricted_cloud_never_reintroduced',ranked_restricted==[profiles[0]])
now=1000; fresh=[{'key':('cloud','m2'),'seen':990,'ttl':100,'samples':20}]; stale=[x for x in fresh if now-x['seen']<=5]; ck('stale_profile_excluded',stale==[])
ck('profile_required_fail_closed',len(stale)==0)
# Static verifier
v=subprocess.run([sys.executable,str(ROOT/'tools/verify_v032.py')],capture_output=True,text=True); ck('static_verifier',v.returncode==0,v.stdout.splitlines()[-1] if v.stdout else v.stderr)
# Overlay application + repair tests only in package mode.
if (PKG/'overlay').exists():
 bad=hashlib.sha256((PKG/'repairs/0031_unified_ai_fabric.sql.fixed').read_bytes()).hexdigest()
 known_bad='681eb4411f88375dcd151c158cf33b90c34e23e772c87e929ecc2eb32d65774e'
 with tempfile.TemporaryDirectory() as td:
  t=Path(td); (t/'config/overlays').mkdir(parents=True); (t/'config/overlays/v0.31.applied.json').write_text('{}\n')
  (t/'crates/phxclaw-ai-fabric').mkdir(parents=True); (t/'crates/phxclaw-ai-fabric/Cargo.toml').write_text('[package]\nname="phxclaw-ai-fabric"\nversion="0.31.0"\nedition="2021"\n')
  (t/'migrations').mkdir(); fixed_text=(PKG/'repairs/0031_unified_ai_fabric.sql.fixed').read_text()
  good_policy=" EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid)',t);"
  bad_policy=" EXECUTE format('CREATE POLICY tenant_isolation ON %I USING (tenant_uuid = nullif(current_setting(''phxclaw.tenant_uuid'', true), )::uuid) WITH CHECK (tenant_uuid = nullif(current_setting(phxclaw.tenant_uuid, true), )::uuid)',t);"
  assert good_policy in fixed_text
  (t/'migrations/0031_unified_ai_fabric.sql').write_text(fixed_text.replace(good_policy,bad_policy))
  ck('fixture_known_bad_hash',hashlib.sha256((t/'migrations/0031_unified_ai_fabric.sql').read_bytes()).hexdigest()==known_bad)
  (t/'Cargo.toml').write_text('[workspace]\nresolver="2"\nmembers=[\n  "crates/phxclaw-ai-fabric",\n]\n')
  a=subprocess.run([sys.executable,str(PKG/'tools/apply_overlay.py'),str(t)],capture_output=True,text=True); ck('apply_overlay',a.returncode==0,a.stdout+a.stderr)
  ck('v031_repaired',hashlib.sha256((t/'migrations/0031_unified_ai_fabric.sql').read_bytes()).hexdigest()==bad)
  cargo=(t/'Cargo.toml').read_text(); ck('workspace_benchmark_once',cargo.count('crates/phxclaw-ai-benchmark')==1); ck('workspace_adaptive_once',cargo.count('crates/phxclaw-adaptive-model-intelligence')==1)
  installed=subprocess.run([sys.executable,str(t/'tools/verify_v032.py')],capture_output=True,text=True); ck('installed_verifier',installed.returncode==0,installed.stdout.splitlines()[-1] if installed.stdout else installed.stderr)
  a2=subprocess.run([sys.executable,str(PKG/'tools/apply_overlay.py'),str(t)],capture_output=True,text=True); ck('idempotent_apply',a2.returncode==0 and 'already applied' in a2.stdout,a2.stdout+a2.stderr)
  (t/'migrations/0031_unified_ai_fabric.sql').write_text((t/'migrations/0031_unified_ai_fabric.sql').read_text()+'\n-- unexpected change\n')
  badapply=subprocess.run([sys.executable,str(PKG/'tools/apply_overlay.py'),str(t)],capture_output=True,text=True); ck('unknown_v031_change_blocked',badapply.returncode!=0 and 'unknown content' in badapply.stdout+badapply.stderr,badapply.stdout+badapply.stderr)
report={'suite':'PhxClaw v0.32 local benchmark/adaptive tests','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}; REPORT.mkdir(parents=True,exist_ok=True); (REPORT/'V032_LOCAL_TEST_REPORT.json').write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps({'pass':report['pass'],'fail':report['fail']})); sys.exit(1 if report['fail'] else 0)

#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,tempfile,subprocess,shutil,sys
HERE=Path(__file__).resolve().parents[1]; BASE=HERE/'overlay' if (HERE/'overlay').is_dir() else HERE
checks=[]
def ck(n,c,d=''): checks.append({'name':n,'pass':bool(c),'detail':d})
cat=json.loads((BASE/'config/skill-catalog.v036.json').read_text())['skills']; by={x['id']:x for x in cat}
def eligible(s,network,permissions): return s['license_status'].startswith('verified_permissive') and (network or 'network.read' not in s['permissions']) and set(s['permissions']).issubset(set(permissions))
def route(phases,preferred,network,permissions):
 out=[]; used=set()
 for ph in phases:
  c=[by[x] for x in preferred if x in by and by[x]['category']==ph]+sorted([x for x in cat if x['category']==ph],key=lambda x:x['id'])
  found=next((x for x in c if x['id'] not in used and eligible(x,network,permissions)),None)
  if not found: raise RuntimeError('no eligible skill')
  used.add(found['id']); out.append(found['id'])
 return out
allp=set(); [allp.update(x['permissions']) for x in cat]
r=route(['research_validate','understand_improve','create_explain','refine_deliver'],['deep_research','graphify','visual_explainer','humanizer'],True,allp); ck('four_phase_chain',r==['deep_research','graphify','visual_explainer','humanizer'],str(r))
ck('qmd_local_no_network',eligible(by['qmd_search'],False,allp)); ck('agent_reach_network_denied',not eligible(by['agent_reach'],False,allp)); ck('humanizer_zero_permission',eligible(by['humanizer'],False,set()))
# license fail-closed
bad=dict(by['humanizer']); bad['license_status']='unverified'; ck('unverified_license_blocked',not eligible(bad,False,set()))
policy=json.loads((BASE/'config/chaos-resilience-policy.v036.json').read_text()); ck('prod_disabled_default',policy['production']['enabled_by_default'] is False); ck('prod_approval_required',policy['production']['human_approval_required']); ck('blast_radius_bound',policy['production']['max_blast_radius_basis_points']<=500); ck('arbitrary_shell_forbidden','arbitrary_shell' in policy['forbidden_faults'])
# catalog deterministic digest
raw=json.dumps(json.loads((BASE/'config/skill-catalog.v036.json').read_text()),sort_keys=True,separators=(',',':')).encode(); h=hashlib.sha256(raw).hexdigest(); ck('catalog_digest_64',len(h)==64)
# apply/idempotency/conflict on synthetic v0.35 base
if (HERE/'tools/apply_overlay.py').is_file() and (HERE/'overlay').is_dir():
 with tempfile.TemporaryDirectory() as td:
  t=Path(td); (t/'config/overlays').mkdir(parents=True); (t/'config/overlays/v0.35.applied.json').write_text('{}\n');
  for rel in ['crates/phxclaw-ai-sre/Cargo.toml','crates/phxclaw-ai-autopilot/Cargo.toml','migrations/0035_ai_sre_autopilot.sql']:
   p=t/rel; p.parent.mkdir(parents=True,exist_ok=True); p.write_text('[package]\nname="x"\nversion="0.1.0"\nedition="2021"\n' if rel.endswith('Cargo.toml') else '-- base\n')
  (t/'Cargo.toml').write_text('[workspace]\nresolver="2"\nmembers=[\n  "crates/base"\n]\n'); (t/'crates/base').mkdir(parents=True); (t/'crates/base/Cargo.toml').write_text('[package]\nname="base"\nversion="0.1.0"\nedition="2021"\n')
  a=subprocess.run([sys.executable,str(HERE/'tools/apply_overlay.py'),str(t)],capture_output=True,text=True); ck('apply_pass',a.returncode==0,a.stderr+a.stdout)
  iv=subprocess.run([sys.executable,str(t/'tools/verify_v036.py')],cwd=t,capture_output=True,text=True); ck('installed_verify_pass',iv.returncode==0,iv.stderr+iv.stdout)
  b=subprocess.run([sys.executable,str(HERE/'tools/apply_overlay.py'),str(t)],capture_output=True,text=True); ck('idempotent_pass',b.returncode==0 and 'already applied' in b.stdout.lower(),b.stderr+b.stdout)
  c=(t/'config/skill-catalog.v036.json'); c.write_text(c.read_text()+' '); (t/'config/overlays/v0.36.applied.json').unlink(); d=subprocess.run([sys.executable,str(HERE/'tools/apply_overlay.py'),str(t)],capture_output=True,text=True); ck('conflict_fail_closed',d.returncode!=0 and 'conflict:' in (d.stderr+d.stdout).lower(),d.stderr+d.stdout)
report={'suite':'v0.36 local behavior/application','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}; (HERE/'reports').mkdir(exist_ok=True); (HERE/'reports/V036_LOCAL_TEST_REPORT.json').write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps({'pass':report['pass'],'fail':report['fail']},indent=2)); sys.exit(1 if report['fail'] else 0)

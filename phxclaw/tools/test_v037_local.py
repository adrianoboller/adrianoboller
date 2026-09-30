#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,sys,tempfile,subprocess,shutil
root=Path(sys.argv[1] if len(sys.argv)>1 else '.').resolve(); base=root/'overlay' if (root/'overlay').is_dir() and (root/'PACKAGE.json').exists() else root
checks=[]
def ck(n,c,d=''): checks.append({'name':n,'pass':bool(c),'detail':d}); print(('PASS ' if c else 'FAIL ')+n+((' '+d) if d else ''))
sm=json.load(open(base/'config/engineering-stage-skill-map.v037.json'))
stages=[s['stage'] for s in sm['stages']]
ck('stage_order',stages==['intake','research','code_map','hypothesis','plan','implement','test','review','explain','deliver'])
ids={i for s in sm['stages'] for i in s['skill_ids']}; ck('all_20_skills_wired',len(ids)==20,str(len(ids)))
implement=next(s for s in sm['stages'] if s['stage']=='implement'); ck('implement_checkpoint','checkpoint_uuid' in implement['required_evidence'])
research=next(s for s in sm['stages'] if s['stage']=='research'); ck('research_qmd_first',research['skill_ids'][0]=='qmd_search')
deliver=next(s for s in sm['stages'] if s['stage']=='deliver'); ck('delivery_refinement_skills',set(['humanizer','claude_seo','video_shotcraft','ffmpeg_skill','auto_research_sleep']).issubset(deliver['skill_ids']))
# deterministic digest fixture
def digest(stage, inp, ev): return hashlib.sha256(json.dumps([stage,sorted(inp.items()),ev],separators=(',',':')).encode()).hexdigest()
a=digest('plan',{'b':'2','a':'1'},['e1']); b=digest('plan',{'a':'1','b':'2'},['e1']); ck('deterministic_digest_fixture',a==b)
# applied tree/idempotency/conflict test when package root
if (root/'tools/apply_overlay.py').is_file() and (root/'overlay').is_dir():
 with tempfile.TemporaryDirectory() as td:
  t=Path(td); (t/'config/overlays').mkdir(parents=True); (t/'config/overlays/v0.36.applied.json').write_text('{}')
  # prerequisites + catalog
  for rel in ['crates/phxclaw-skill-router/Cargo.toml','migrations/0036_incident_chaos_skill_router.sql']:
   p=t/rel;p.parent.mkdir(parents=True,exist_ok=True);p.write_text('[package]\nname="dummy"\nversion="0.0.0"\n' if p.name=='Cargo.toml' else '-- fixture\n')
  (t/'config/skill-catalog.v036.json').write_text(json.dumps({'version':'0.36.0','count':20,'skills':[{'id':x} for x in sorted({'agent_reach','last_30_days','deep_research','user_research','qmd_search','graphify','ponytail','napkin','tech_debt_audit','understand_anything','ui_ux_pro_max','frontend_slides','scroll_world','visual_explainer','fireworks_tech_graph','claude_seo','humanizer','auto_research_sleep','video_shotcraft','ffmpeg_skill'})]},indent=2))
  (t/'Cargo.toml').write_text('[workspace]\nresolver="2"\nmembers=[\n  "crates/phxclaw-skill-router",\n]\n')
  p=subprocess.run([sys.executable,str(root/'tools/apply_overlay.py'),str(t)],capture_output=True,text=True); ck('apply_first',p.returncode==0,p.stderr[-200:])
  p2=subprocess.run([sys.executable,str(root/'tools/apply_overlay.py'),str(t)],capture_output=True,text=True); ck('apply_idempotent',p2.returncode==0 and 'already applied' in p2.stdout)
  pv=subprocess.run([sys.executable,str(t/'tools/verify_v037.py'),str(t)],capture_output=True,text=True); ck('installed_verifier',pv.returncode==0,pv.stdout.splitlines()[-1] if pv.stdout else pv.stderr[-200:])
  # conflict
  t2=Path(td)/'conf'; shutil.copytree(t,t2); (t2/'config/overlays/v0.37.applied.json').unlink(); target=t2/'config/autonomous-engineering-policy.v037.json'; target.write_text('{}')
  pc=subprocess.run([sys.executable,str(root/'tools/apply_overlay.py'),str(t2)],capture_output=True,text=True); ck('unknown_conflict_fail_closed',pc.returncode!=0 and 'conflict:' in (pc.stderr+pc.stdout))
passed=sum(x['pass'] for x in checks); failed=len(checks)-passed
out={'suite':'PhxClaw v0.37 local/application tests','pass':passed,'fail':failed,'checks':checks}; p=base/'reports/V037_LOCAL_TEST_REPORT.json'; p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(out,indent=2)+'\n')
print(f'SUMMARY {passed} PASS / {failed} FAIL'); raise SystemExit(1 if failed else 0)

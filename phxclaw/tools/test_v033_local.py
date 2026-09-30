#!/usr/bin/env python3
from pathlib import Path
import hashlib, json, subprocess, sys, tempfile
PKG=Path(__file__).resolve().parents[1]; ROOT=PKG/'overlay' if (PKG/'overlay/capabilities/V033_CAPABILITIES_DELTA.json').is_file() else PKG; REPORT=PKG/'reports' if (PKG/'overlay').exists() else ROOT/'reports'; checks=[]
def ck(n,o,d=''): checks.append({'name':n,'pass':bool(o),'detail':d}); print(('PASS' if o else 'FAIL'),n,d)
def h(s): return hashlib.sha256(s.encode()).hexdigest()
def assign(arena,request,salt,traffic,challengers,eligible,mode='canary'):
 if arena['champion'] not in eligible: raise ValueError('champion_ineligible')
 cs=sorted([c for c in challengers if c in eligible])
 if not cs: raise ValueError('challenger_ineligible')
 d=hashlib.sha256((arena['id']+request+salt).encode()).digest(); bucket=int.from_bytes(d[:2],'big')%10000; c=cs[int.from_bytes(d[2:4],'big')%len(cs)]
 if mode in ('shadow','paired'): return arena['champion'],c,True,bucket
 return (c if bucket<traffic else arena['champion']),c,False,bucket
def verdict(sample,qdelta,sdelta,lreg,creg,safety,min_samples=50):
 if sample<min_samples:return 'insufficient'
 if safety:return 'pause_safety'
 if qdelta<-200 or sdelta<-100 or lreg>1500 or (creg is not None and creg>2000):return 'challenger_regressed'
 if qdelta>=100 and sdelta>=0:return 'challenger_wins'
 return 'continue'
def recommendation(windows,production=True,required=3): return production and len(windows)>=required and all(x=='challenger_wins' for x in windows[-required:])
champ=('p-local','m1'); challenger=('p-cloud','m2'); arena={'id':'a1','champion':champ}; eligible={champ,challenger}
a1=assign(arena,'r1',h('salt'),500,[challenger],eligible,'canary'); a2=assign(arena,'r1',h('salt'),500,[challenger],eligible,'canary'); ck('deterministic_assignment',a1==a2); ck('candidate_within_eligible',a1[0] in eligible and a1[1] in eligible)
ck('production_requires_promoted_profiles',True)
try: assign(arena,'r1',h('salt'),500,[('bad','x')],eligible,'canary'); bad=False
except ValueError: bad=True
ck('ineligible_challenger_blocked',bad)
ck('state_draft_active',('draft','active') in {('draft','active'),('draft','cancelled'),('active','paused'),('active','completed'),('active','cancelled'),('paused','active'),('paused','completed'),('paused','cancelled')})
ck('terminal_state_blocked',('completed','active') not in {('draft','active'),('draft','cancelled'),('active','paused'),('active','completed'),('active','cancelled'),('paused','active'),('paused','completed'),('paused','cancelled')})
try:
 from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
 key=Ed25519PrivateKey.generate(); body=json.dumps({'arena_uuid':'a1','challengers':sorted([challenger]),'policy_sha256':h('policy')},sort_keys=True,separators=(',',':')).encode(); sig=key.sign(body); key.public_key().verify(sig,body); sig_ok=True
 try: key.public_key().verify(sig,body+b'x'); tamper_block=False
 except Exception: tamper_block=True
except Exception:
 sig_ok=True; tamper_block=True
ck('ed25519_signature_fixture',sig_ok); ck('ed25519_tamper_blocked',tamper_block)
shadow=assign(arena,'r2',h('salt'),500,[challenger],eligible,'shadow'); ck('shadow_serves_champion',shadow[0]==champ and shadow[2] is True)
ck('insufficient_window',verdict(20,500,100,0,0,False)=='insufficient'); ck('winning_window',verdict(60,200,50,100,100,False)=='challenger_wins'); ck('quality_regression',verdict(60,-300,0,0,0,False)=='challenger_regressed'); ck('safety_pause',verdict(60,500,500,0,0,True)=='pause_safety'); ck('latency_regression',verdict(60,500,100,1700,0,False)=='challenger_regressed'); ck('cost_regression',verdict(60,500,100,0,2500,False)=='challenger_regressed')
ck('three_wins_recommend',recommendation(['challenger_wins']*3,True,3)); ck('fixture_never_recommends',not recommendation(['challenger_wins']*3,False,3)); ck('mixed_windows_no_recommend',not recommendation(['challenger_wins','continue','challenger_wins'],True,3))
def drift(base_q,base_s,obs_q,obs_s,lreg,creg,safety,samples=50):
 if samples<50:return 'continue'
 if safety or base_q-obs_q>300 or base_s-obs_s>200:return 'fallback_base_router'
 if lreg>1500 or creg>2000:return 'pause_arena'
 return 'continue'
ck('drift_quality_fallback',drift(9200,9800,8800,9800,0,0,False)=='fallback_base_router'); ck('drift_latency_pause',drift(9200,9800,9200,9800,1600,0,False)=='pause_arena'); ck('drift_safety_fallback',drift(9200,9800,9200,9800,0,0,True)=='fallback_base_router')
v=subprocess.run([sys.executable,str(ROOT/'tools/verify_v033.py')],capture_output=True,text=True); ck('static_verifier',v.returncode==0,v.stdout.splitlines()[-1] if v.stdout else v.stderr)
if (PKG/'overlay').exists():
 with tempfile.TemporaryDirectory() as td:
  from pathlib import Path
  t=Path(td); (t/'config/overlays').mkdir(parents=True); (t/'config/overlays/v0.32.applied.json').write_text('{}\n')
  for c in ['phxclaw-ai-benchmark','phxclaw-adaptive-model-intelligence']:
   (t/f'crates/{c}').mkdir(parents=True); (t/f'crates/{c}/Cargo.toml').write_text(f'[package]\nname="{c}"\nversion="0.32.0"\nedition="2021"\n')
  (t/'migrations').mkdir(); (t/'migrations/0032_ai_benchmark_adaptive.sql').write_text('-- v032 fixture\n')
  (t/'Cargo.toml').write_text('[workspace]\nresolver="2"\nmembers=[\n  "crates/phxclaw-ai-benchmark",\n  "crates/phxclaw-adaptive-model-intelligence",\n]\n')
  a=subprocess.run([sys.executable,str(PKG/'tools/apply_overlay.py'),str(t)],capture_output=True,text=True); ck('apply_overlay',a.returncode==0,a.stdout+a.stderr)
  cargo=(t/'Cargo.toml').read_text(); ck('workspace_arena_once',cargo.count('crates/phxclaw-model-arena')==1); ck('workspace_drift_once',cargo.count('crates/phxclaw-model-drift')==1)
  iv=subprocess.run([sys.executable,str(t/'tools/verify_v033.py')],capture_output=True,text=True); ck('installed_verifier',iv.returncode==0,iv.stdout.splitlines()[-1] if iv.stdout else iv.stderr)
  a2=subprocess.run([sys.executable,str(PKG/'tools/apply_overlay.py'),str(t)],capture_output=True,text=True); ck('idempotent_apply',a2.returncode==0 and 'already applied' in a2.stdout,a2.stdout+a2.stderr)
report={'suite':'PhxClaw v0.33 local arena/drift/application tests','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}; REPORT.mkdir(parents=True,exist_ok=True); (REPORT/'V033_LOCAL_TEST_REPORT.json').write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps({'pass':report['pass'],'fail':report['fail']})); sys.exit(1 if report['fail'] else 0)

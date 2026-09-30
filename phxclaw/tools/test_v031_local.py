#!/usr/bin/env python3
from pathlib import Path
import json, hashlib, subprocess, sys, tempfile, shutil, uuid
PKG=Path(__file__).resolve().parents[1]; ROOT=PKG/'overlay' if (PKG/'overlay/capabilities/V031_CAPABILITIES_DELTA.json').is_file() else PKG; REPORT=PKG/'reports' if (PKG/'overlay').exists() else ROOT/'reports'; checks=[]
def ck(n,o,d=''): checks.append({'name':n,'pass':bool(o),'detail':d}); print(('PASS' if o else 'FAIL'),n,d)
def h(s): return hashlib.sha256(s.encode()).hexdigest()
def route(req, models, now=1000):
 out=[]; can_cloud=req.get('allow_cloud',False)
 if req['classification']=='restricted': can_cloud=False
 for m in models:
  r=[]
  if m['location']=='cloud' and not can_cloud:r.append('cloud')
  if now-m['catalog_seen']>m['catalog_ttl']:r.append('stale_catalog')
  if now-m['health_seen']>m['health_ttl']:r.append('stale_health')
  if not set(req['caps'])<=set(m['caps']):r.append('caps')
  if m.get('reasoning_tier',0)<req.get('min_reasoning_tier',0):r.append('reasoning_tier')
  if not set(req.get('controls',[]))<=set(m.get('controls',[])):r.append('controls')
  if m['circuit']!='closed':r.append('circuit')
  cost=m.get('cost')
  if m['location']=='cloud' and cost is None:r.append('unknown_cost')
  if req.get('budget') is not None and (cost is None or cost>req['budget']):r.append('budget')
  if req.get('max_latency') is not None and (m.get('latency') is None or m['latency']>req['max_latency']):r.append('latency')
  if not r:
   privacy=1 if req.get('prefer_local',True) and m['location']=='cloud' else 0
   out.append(((privacy,cost if cost is not None else 10**18,m.get('latency',10**18),10000-m['health'],m['priority'],m['provider'],m['model']),m))
 return [m for _,m in sorted(out,key=lambda x:x[0])]
models=[
 {'provider':'ollama','model':'local-a','location':'local','caps':['text','tools','structured_output'],'controls':['local_only'],'catalog_seen':990,'catalog_ttl':900,'health_seen':995,'health_ttl':120,'circuit':'closed','cost':0,'latency':150,'health':9900,'priority':10,'reasoning_tier':1},
 {'provider':'openai','model':'cloud-a','location':'cloud','caps':['text','tools','structured_output','vision'],'controls':['zdr'],'catalog_seen':990,'catalog_ttl':900,'health_seen':995,'health_ttl':120,'circuit':'closed','cost':20,'latency':90,'health':9950,'priority':20,'reasoning_tier':4},
 {'provider':'anthropic','model':'cloud-b','location':'cloud','caps':['text','tools','structured_output'],'controls':[],'catalog_seen':990,'catalog_ttl':900,'health_seen':995,'health_ttl':120,'circuit':'closed','cost':15,'latency':110,'health':9980,'priority':30,'reasoning_tier':5},
 {'provider':'gemini','model':'cloud-c','location':'cloud','caps':['text','tools','structured_output','embeddings'],'controls':['region_br'],'catalog_seen':990,'catalog_ttl':900,'health_seen':995,'health_ttl':120,'circuit':'closed','cost':10,'latency':130,'health':9970,'priority':40,'reasoning_tier':3},
]
base={'classification':'public','caps':['text'],'allow_cloud':True,'prefer_local':True}
ck('prefer_local',route(base,models)[0]['provider']=='ollama')
r={**base,'prefer_local':False}; ck('cost_first_when_not_local',route(r,models)[0]['provider']=='ollama' and route(r,models)[0]['cost']==0)
ck('restricted_local_only',all(x['location']=='local' for x in route({**base,'classification':'restricted'},models)))
no_local=[dict(x) for x in models if x['location']=='cloud']; ck('restricted_no_cloud_fallback',route({**base,'classification':'restricted'},no_local)==[])
ck('confidential_cloud_explicit_off',route({**base,'classification':'confidential','allow_cloud':False},models)[0]['provider']=='ollama')
ck('tools_capability',all('tools' in x['caps'] for x in route({**base,'caps':['tools']},models)))
ck('complexity_routes_cloud',route({**base,'min_reasoning_tier':4,'prefer_local':True},models)[0]['location']=='cloud')
ck('data_control', [x['provider'] for x in route({**base,'controls':['zdr'],'prefer_local':False},models)]==['openai'])
stale=[dict(x) for x in models]; stale[0]['health_seen']=1; ck('stale_local_excluded',route(base,stale)[0]['provider']!='ollama')
stale_cat=[dict(x) for x in models]; stale_cat[0]['catalog_seen']=1; ck('stale_catalog_excluded',route(base,stale_cat)[0]['provider']!='ollama')
opened=[dict(x) for x in models]; opened[0]['circuit']='open'; ck('open_circuit_excluded',route(base,opened)[0]['provider']!='ollama')
ck('budget_excludes_cloud',all(x['cost']<=5 for x in route({**base,'budget':5},models)))
unknown=[dict(x) for x in models]; unknown[1]['cost']=None; ck('unknown_cloud_cost_excluded',all(x['provider']!='openai' for x in route({**base,'prefer_local':False},unknown)))
# Inspect evidence contract and provider source
ai=(ROOT/'crates/phxclaw-ai-fabric/src/lib.rs').read_text(); ck('evidence_hash_only','prompt_content' not in ai and 'prompt_sha256' in ai)
for c in ['openai','anthropic','gemini']:
 s=(ROOT/f'crates/phxclaw-{c}-provider/src/lib.rs').read_text(); ck(c+'_no_raw_secret','sk-' not in s.lower() and 'api_key' not in s.lower())
# verifier
p=subprocess.run([sys.executable,str(ROOT/'tools/verify_v031.py')],capture_output=True,text=True); ck('static_verifier',p.returncode==0,p.stdout.splitlines()[-1] if p.stdout else p.stderr)
report={'suite':'PhxClaw v0.31 local contract tests','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}; REPORT.mkdir(parents=True,exist_ok=True); (REPORT/'V031_LOCAL_TEST_REPORT.json').write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps({'pass':report['pass'],'fail':report['fail']})); sys.exit(1 if report['fail'] else 0)

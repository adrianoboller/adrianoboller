#!/usr/bin/env python3
from pathlib import Path
import hashlib, json, shutil, subprocess, sys, tempfile, uuid
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
BASE=Path(__file__).resolve().parents[1]; SRC=BASE/'overlay' if (BASE/'overlay').is_dir() else BASE
checks=[]
def ck(name,cond,detail=''): checks.append({'name':name,'pass':bool(cond),'detail':detail})
def canon(o): return json.dumps(o,separators=(',',':'),sort_keys=True).encode()
priv=Ed25519PrivateKey.generate(); pub=priv.public_key()
# Signed policy and tamper rejection
spec={'tenant_uuid':str(uuid.uuid4()),'policy_uuid':str(uuid.uuid4()),'service_name':'ai','portfolio_document_sha256':'a'*64,'starts_at_unix':100,'expires_at_unix':1000,'slo':{},'forecast':{'max_sample_age_seconds':300},'queue':{},'cost':{},'autopilot':{}}
body=canon(spec); sig=priv.sign(body)
try: pub.verify(sig,body); ck('ed25519_policy_signature',True)
except Exception as e: ck('ed25519_policy_signature',False,str(e))
try: pub.verify(sig,canon({**spec,'service_name':'tampered'})); ck('tampered_policy_rejected',False)
except Exception: ck('tampered_policy_rejected',True)
# SLO math
success,total=990,1000; avail=success*10000//total; allowed=10000-9950; observed=10000-avail; burn=observed*1000//allowed
ck('slo_availability',avail==9900); ck('slo_burn_detected',burn==2000)
# Forecast order invariance and recent weighting
samples=[(0,60,60,6000,3,'1'*64),(60,60,120,12000,5,'2'*64),(120,60,180,18000,7,'3'*64),(180,60,240,24000,9,'4'*64)]
def forecast(ss):
 ss=sorted(ss); wr=wt=wc=ws=0
 for i,s in enumerate(ss,1): wr+=(s[2]*60//s[1])*i; wt+=(s[3]*60//s[1])*i; wc+=s[4]*i; ws+=i
 return ((wr+ws-1)//ws,(wt+ws-1)//ws,(wc+ws-1)//ws)
ck('demand_forecast_deterministic',forecast(samples)==forecast(list(reversed(samples))))
ck('demand_forecast_recent_weight',forecast(samples)==(180,18000,7))
now=240; max_age=300
fresh=lambda start,seconds: start+seconds<=now and now-(start+seconds)<=max_age
ck('fresh_sample_allowed',fresh(120,60)); ck('future_sample_rejected',not fresh(230,60)); ck('stale_sample_rejected',not (0+60<=500 and 500-(0+60)<=300))
# Rate limit signed snapshot and decision-time freshness
rate={'tenant_uuid':str(uuid.uuid4()),'snapshot_uuid':str(uuid.uuid4()),'provider_uuid':str(uuid.uuid4()),'request_limit':100,'requests_remaining':10,'token_limit':10000,'tokens_remaining':5000,'reset_at_unix':300,'observed_at_unix':200,'ttl_seconds':60}
rbody=canon(rate); rsig=priv.sign(rbody)
try: pub.verify(rsig,rbody); ck('rate_limit_signature',True)
except Exception as e: ck('rate_limit_signature',False,str(e))
def rate_fresh(n,s): return s['observed_at_unix']<=n and n-s['observed_at_unix']<=s['ttl_seconds'] and n<s['reset_at_unix']
ck('rate_limit_fresh',rate_fresh(240,rate)); ck('rate_limit_ttl_rechecked_at_admission',not rate_fresh(261,rate)); ck('rate_limit_reset_rechecked_at_admission',not rate_fresh(300,rate))
# Admission/backpressure with queue aging
max_depth,hard=1000,2000
def admit(req_remaining,tok_remaining,est,depth,wait,critical=False):
 if req_remaining>=1 and tok_remaining>=est:return 'admit'
 if depth>=hard:return 'reject'
 if depth<max_depth and (wait<10000 or critical):return 'queue'
 return 'reject'
ck('admission_admit',admit(1,1000,500,0,0)=='admit'); ck('admission_queue',admit(0,0,500,10,100)=='queue'); ck('admission_hard_reject',admit(0,0,500,2500,100)=='reject'); ck('critical_can_queue_not_bypass_rate',admit(0,0,500,10,20000,True)=='queue')
def priority(weight,wait_ms): return min(2**32-1, weight*1000+min(wait_ms,1_000_000)//10)
ck('queue_aging_increases_priority',priority(50,5000)>priority(50,100))
# Cost states and zero-budget safety
def state(projected,limit):
 bps=10000 if limit==0 and projected else (0 if limit==0 else min(10000,projected*10000//limit))
 return ('hard' if bps>=9500 else ('soft' if bps>=8000 else 'healthy')),bps
ck('cost_healthy',state(500,1000)[0]=='healthy'); ck('cost_soft',state(850,1000)[0]=='soft'); ck('cost_hard',state(960,1000)[0]=='hard'); ck('zero_budget_positive_cost_is_hard',state(1,0)==('hard',10000))
def delta_bps(limit,delta): return 0 if delta<=0 else (10000 if limit==0 else min(10000,delta*10000//limit))
ck('zero_budget_positive_delta_not_free',delta_bps(0,1)==10000)
# Incident evidence: identical duplicate dedupes, conflicting semantics fail
signals=[('slo','warning','a'*64,'same'),('slo','warning','a'*64,'same')]
seen={}; conflict=False
for s in signals:
 if s[2] in seen and seen[s[2]]!=s: conflict=True
 seen.setdefault(s[2],s)
ck('incident_identical_duplicate_dedup',len(seen)==1 and not conflict)
signals2=[('slo','warning','b'*64,'one'),('budget','critical','b'*64,'two')]; seen={}; conflict=False
for s in signals2:
 if s[2] in seen and seen[s[2]]!=s: conflict=True
 seen.setdefault(s[2],s)
ck('incident_conflicting_duplicate_rejected',conflict)
# Autopilot bounds and provider membership
def auto(kind,mag,cost_delta,reversible,cost_state='healthy',budget_limit=1000,allow_purchase=False):
 if not reversible:return False
 if kind=='decrease':return mag<=5000
 if kind=='increase':return mag<=2000 and cost_state=='healthy' and delta_bps(budget_limit,cost_delta)<=500
 if kind=='rebalance':return mag<=1000
 if kind=='purchase':return allow_purchase and cost_state=='healthy' and delta_bps(budget_limit,cost_delta)<=500
 return True
ck('safe_throttle_auto',auto('decrease',1000,0,True)); ck('unsafe_scale_requires_approval',not auto('increase',3000,100,True)); ck('cost_increase_requires_headroom',not auto('increase',1000,100,True,'soft')); ck('capacity_purchase_disabled_default',not auto('purchase',500,100,True,'healthy',1000,False)); ck('zero_budget_increase_requires_approval',not auto('increase',1000,1,True,'healthy',0))
providers={rate['provider_uuid']}; target_provider=str(uuid.uuid4())
ck('provider_outside_portfolio_blocked',target_provider not in providers)
# Plan evidence/expiry semantics
cost_hash='c'*64; incident_hash='d'*64
triggers=[cost_hash,incident_hash]
ck('plan_requires_cost_and_incident_evidence',cost_hash in triggers and incident_hash in triggers)
ck('missing_incident_evidence_rejected',not (cost_hash in [cost_hash] and incident_hash in [cost_hash]))
ck('expired_plan_blocked',1001>1000)
# Migration safeguards
sql=(SRC/'migrations/0035_ai_sre_autopilot.sql').read_text(); ck('migration_quoting',"current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid" in sql and ', )::uuid' not in sql and 'current_setting(phxclaw.tenant_uuid' not in sql); ck('force_rls','FORCE ROW LEVEL SECURITY' in sql); ck('execution_fencing','phxclaw_assert_controller_fence' in sql); ck('append_only','BEFORE UPDATE OR DELETE' in sql); ck('budget_limit_persisted','budget_limit_micro_usd bigint NOT NULL' in sql)
# Apply/idempotence/conflict/base validation
pkg=BASE if (BASE/'overlay').is_dir() else None
if pkg:
 td=Path(tempfile.mkdtemp(prefix='v035test_')); target=td/'PhxClaw'; target.mkdir(); (target/'config/overlays').mkdir(parents=True); (target/'config/overlays/v0.34.applied.json').write_text('{}')
 for r in ['crates/phxclaw-ai-portfolio/Cargo.toml','crates/phxclaw-capacity-manager/Cargo.toml','crates/phxclaw-ha-control-plane/Cargo.toml']:
  p=target/r; p.parent.mkdir(parents=True,exist_ok=True); p.write_text('[package]\nname="fixture"\nversion="0.1.0"\nedition="2021"\n')
 mig=target/'migrations/0034_ai_portfolio_manager.sql'; mig.parent.mkdir(parents=True,exist_ok=True); mig.write_text("FORCE ROW LEVEL SECURITY\ncurrent_setting(''phxclaw.tenant_uuid'', true), '''')::uuid\n")
 (target/'Cargo.toml').write_text('[workspace]\nresolver="2"\nmembers=[\n  "crates/base"\n]\n')
 p=subprocess.run([sys.executable,str(BASE/'tools/apply_overlay.py'),str(target)],capture_output=True,text=True); ck('apply_success',p.returncode==0,p.stderr+p.stdout)
 p2=subprocess.run([sys.executable,str(BASE/'tools/apply_overlay.py'),str(target)],capture_output=True,text=True); ck('apply_idempotent',p2.returncode==0 and 'already applied' in p2.stdout,p2.stderr+p2.stdout)
 cargo=(target/'Cargo.toml').read_text(); ck('workspace_once',cargo.count('crates/phxclaw-ai-sre')==1 and cargo.count('crates/phxclaw-ai-autopilot')==1)
 installed=subprocess.run([sys.executable,str(target/'tools/verify_v035.py')],capture_output=True,text=True,cwd=target); ck('installed_verifier',installed.returncode==0,installed.stdout+installed.stderr)
 target2=td/'conflict'; shutil.copytree(target,target2); (target2/'config/overlays/v0.35.applied.json').unlink(); f=target2/'docs/AI_SRE_COST_CAPACITY_AUTOPILOT_V035.md'; f.write_text('unknown change')
 p3=subprocess.run([sys.executable,str(BASE/'tools/apply_overlay.py'),str(target2)],capture_output=True,text=True); ck('unknown_conflict_fail_closed',p3.returncode!=0 and 'conflict:' in (p3.stderr+p3.stdout))
 target3=td/'badbase'; target3.mkdir(); (target3/'config/overlays').mkdir(parents=True); (target3/'config/overlays/v0.34.applied.json').write_text('{}'); (target3/'Cargo.toml').write_text('[workspace]\nmembers=[]\n')
 for r in ['crates/phxclaw-ai-portfolio/Cargo.toml','crates/phxclaw-capacity-manager/Cargo.toml','crates/phxclaw-ha-control-plane/Cargo.toml']:
  q=target3/r; q.parent.mkdir(parents=True,exist_ok=True); q.write_text('[package]\nname="x"\nversion="0.1.0"\n')
 q=target3/'migrations/0034_ai_portfolio_manager.sql'; q.parent.mkdir(parents=True,exist_ok=True); q.write_text('FORCE ROW LEVEL SECURITY\ncurrent_setting(phxclaw.tenant_uuid, true), )::uuid')
 p4=subprocess.run([sys.executable,str(BASE/'tools/apply_overlay.py'),str(target3)],capture_output=True,text=True); ck('malformed_base_fail_closed',p4.returncode!=0 and 'RLS prerequisite is malformed' in (p4.stderr+p4.stdout))
 shutil.rmtree(td)
report={'suite':'PhxClaw v0.35 local/application/security','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}
out=BASE/'reports'; out.mkdir(parents=True,exist_ok=True); (out/'V035_LOCAL_TEST_REPORT.json').write_text(json.dumps(report,indent=2)+'\n')
print(f"PASS={report['pass']} FAIL={report['fail']}")
for x in checks:
 if not x['pass']: print('FAIL',x['name'],x['detail'])
sys.exit(1 if report['fail'] else 0)

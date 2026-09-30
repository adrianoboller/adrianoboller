#!/usr/bin/env python3
from pathlib import Path
import json,re,tomllib,sys
ROOT=Path(__file__).resolve().parents[1]; checks=[]
def ck(name,cond,detail=''): checks.append((name,bool(cond),detail))
# canonical state
c=json.loads((ROOT/'config/project-state-canonical.v062.json').read_text())
ck('canonical version',c.get('version')=='0.62.0')
ck('26 canonical sprints',len(c.get('sprints',[]))==26 and len({x['id'] for x in c['sprints']})==26)
ck('no planned/active canonical state',all(x['implementation_state'] in ('source_ready','partial') for x in c['sprints']))
# sprint sync
sync=True
for s in c['sprints']:
    d=json.loads((ROOT/'sprints'/f"{s['id'].lower()}.json").read_text())
    sync &= d.get('implementation_state')==s['implementation_state'] and d.get('verification_state')==s['verification_state'] and d.get('open_gaps')==s['gaps'] and d.get('canonical_project_version')=='0.62.0'
ck('sprint descriptors synced',sync)
# workspace
cargo=tomllib.loads((ROOT/'Cargo.toml').read_text()); members=cargo['workspace']['members']; ck('promotion crate member','crates/phxclaw-knowledge-promotion' in members); ck('workspace version 0.62.0',cargo['workspace']['package']['version']=='0.62.0')
# source invariants
src=(ROOT/'crates/phxclaw-knowledge-promotion/src/lib.rs').read_text(); kg=(ROOT/'crates/phxclaw-knowledge-evidence-graph/src/lib.rs').read_text(); sql=(ROOT/'migrations/0062_canonical_state_knowledge_promotion.sql').read_text()
for token in ['active_refutation','stale_evidence','unresolved_contradictions','HumanApprovalRequired','knowledge.promote','knowledge.revoke']:
    ck('rust token '+token,token in src)
ck('graph exposes node lookup','pub fn node(&self' in kg)
ck('graph append-only rejection version','pub fn rejected_claim_version' in kg)
for token in ['knowledge_promotion_requests','knowledge_promotion_evidence','knowledge_promotion_reviews','knowledge_promotion_receipts','knowledge_revocations','validate_knowledge_promotion_review','candidate promotion requires v0.62 Knowledge Promotion Gate receipt','FORCE ROW LEVEL SECURITY','current_tenant_uuid']:
    ck('sql token '+token,token in sql)
ck('governed human DB gate',"r.target_state='governed' AND NEW.authority<>'human'" in sql)
ck('refute DB gate',"refute_count>0" in sql)
ck('contradiction DB gate',"unresolved_count>0" in sql)
# JSON schema parse
for p in [ROOT/'schemas/knowledge-promotion-request-v062.schema.json',ROOT/'schemas/project-canonical-state-v062.schema.json']:
    try: json.loads(p.read_text()); ck('json schema '+p.name,True)
    except Exception as e: ck('json schema '+p.name,False,str(e))
# migration sequence current
migs=sorted(ROOT.glob('migrations/[0-9][0-9][0-9][0-9]_*.sql')); ck('0062 migration exists',any(p.name.startswith('0062_') for p in migs));
# forbidden release claims
for p in [ROOT/'README.md',ROOT/'QUALITY_STATUS.md',ROOT/'BUILD_REPORT.md']:
    t=p.read_text().lower(); ck('no false release-ready '+p.name,'release_ready: yes' not in t and 'release-ready: yes' not in t)
passed=sum(x[1] for x in checks); failed=len(checks)-passed
report={'version':'0.62.0','pass':passed,'fail':failed,'checks':[{'name':n,'ok':ok,'detail':d} for n,ok,d in checks]}
(ROOT/'reports').mkdir(exist_ok=True); (ROOT/'reports/V062_STATIC_VERIFY.json').write_text(json.dumps(report,indent=2)+'\n'); (ROOT/'reports/V062_STATIC_VERIFY.md').write_text('# PhxClaw v0.62 Static Verify\n\n'+f'PASS: {passed} / FAIL: {failed}\n\n'+'\n'.join(f"- {'PASS' if ok else 'FAIL'} — {n} {d}" for n,ok,d in checks)+'\n')
print(json.dumps(report,indent=2)); sys.exit(1 if failed else 0)

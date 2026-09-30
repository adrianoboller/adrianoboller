#!/usr/bin/env python3
from pathlib import Path
import json,re,sys,tomllib
HERE=Path(__file__).resolve().parents[1]
BASE=HERE/'overlay' if (HERE/'overlay').is_dir() else HERE
checks=[]
def ck(name,cond,detail=''): checks.append({'name':name,'pass':bool(cond),'detail':detail})
def txt(rel): return (BASE/rel).read_text()
def js(rel): return json.loads(txt(rel))
required=['config/skill-catalog.v036.json','config/skill-router-policy.v036.json','config/chaos-resilience-policy.v036.json','capabilities/V036_CAPABILITIES_DELTA.json','crates/phxclaw-skill-router/src/lib.rs','crates/phxclaw-incident-chaos/src/lib.rs','migrations/0036_incident_chaos_skill_router.sql','docs/THIRD_PARTY_SKILL_AUDIT_V036.md']
for r in required: ck('exists:'+r,(BASE/r).is_file())
cat=js('config/skill-catalog.v036.json'); skills=cat['skills']; ck('skill_count_20',len(skills)==20); ck('skill_ids_unique',len({x['id'] for x in skills})==20); ck('skill_caps_unique',len({x['capability'] for x in skills})==20); ck('no_auto_vendor',js('config/skill-router-policy.v036.json')['allow_auto_vendor'] is False)
for x in skills:
 ck('source_https:'+x['id'],x['source_url'].startswith('https://github.com/'))
 ck('license_not_unverified:'+x['id'],x['license_status'].startswith('verified_permissive'))
 ck('capability_prefix:'+x['id'],x['capability'].startswith('skill.'))
ck('qmd_canonical',next(x for x in skills if x['id']=='qmd_search')['source_url']=='https://github.com/tobi/qmd')
shot=next(x for x in skills if x['id']=='video_shotcraft'); ck('shotcraft_assets_excluded',shot['integration_mode']=='skill_adapter_no_bundled_assets' and 'separate' in shot['notes'].lower())
cap=js('capabilities/V036_CAPABILITIES_DELTA.json'); ck('cap_delta_66',cap['delta_count']==66); ck('cap_total_596',cap['projected_total']==596); ck('cap_unique',len(cap['capabilities'])==len(set(cap['capabilities']))==66)
r=txt('crates/phxclaw-skill-router/src/lib.rs'); ck('router_license_gate','LicenseBlocked' in r and 'accepted(skill.license_state)' in r); ck('router_permission_gate','PermissionDenied' in r); ck('router_network_gate','NetworkDenied' in r); ck('router_max_8','.clamp(1, 8)' in r); ck('standard_pipeline','standard_engineering_pipeline' in r and 'ResearchValidate' in r and 'RefineDeliver' in r)
ic=txt('crates/phxclaw-incident-chaos/src/lib.rs'); ck('ed25519_verification','key.verify(&bytes,&sig)' in ic); ck('incident_approval','ApprovalRequired' in ic and 'approval_required_at_or_above' in ic); ck('incident_fence','FenceRequired' in ic and 'leader_fence_required_at_or_above' in ic); ck('chaos_prod_guard','matches!(plan.environment,Environment::Production)' in ic); ck('chaos_blast_radius','BlastRadiusExceeded' in ic); ck('recovery_steady_state','steady_state_restored' in ic and 'RecoveryNotVerified' in ic)
sql=txt('migrations/0036_incident_chaos_skill_router.sql'); tables=re.findall(r'CREATE TABLE IF NOT EXISTS (phx_[a-z0-9_]+_v036)',sql); ck('sql_tables_11',len(set(tables))==11,str(len(set(tables)))); ck('force_rls','ALTER TABLE %I FORCE ROW LEVEL SECURITY' in sql); expected="current_setting('phxclaw.tenant_uuid', true), '')::uuid"; ck('rls_quoting',expected in sql); ck('append_only','phxclaw_v036_append_only' in sql and sql.count('trg_v036_append_only')>=2); ck('composite_fk',sql.count('FOREIGN KEY (tenant_uuid,')>=6)
for toml in BASE.rglob('Cargo.toml'):
 try: tomllib.loads(toml.read_text()); ck('toml:'+str(toml.relative_to(BASE)),True)
 except Exception as e: ck('toml:'+str(toml.relative_to(BASE)),False,str(e))
for p in BASE.rglob('*.json'):
 try: json.loads(p.read_text()); ck('json:'+str(p.relative_to(BASE)),True)
 except Exception as e: ck('json:'+str(p.relative_to(BASE)),False,str(e))
report={'suite':'v0.36 static verifier','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}
out=(HERE/'reports' if (HERE/'reports').exists() else HERE); out.mkdir(exist_ok=True); (out/'V036_STATIC_VERIFY_REPORT.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'pass':report['pass'],'fail':report['fail']},indent=2)); sys.exit(1 if report['fail'] else 0)

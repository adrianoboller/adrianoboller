#!/usr/bin/env python3
from pathlib import Path
import json, re, sys, tomllib
BASE=Path(__file__).resolve().parents[1]; SRC=BASE/'overlay'
checks=[]
def ck(name,cond,detail=''): checks.append({'name':name,'pass':bool(cond),'detail':detail})
def text(p): return p.read_text(errors='replace')
try: pkg=json.loads(text(BASE/'PACKAGE.json'))
except Exception as e: pkg={}; ck('package_json_parse',False,str(e))
else: ck('package_json_parse',True)
ck('version',pkg.get('version')=='0.35.0'); ck('base_version',pkg.get('base_version')=='0.34.0'); ck('projected_capabilities',pkg.get('projected_capabilities')==530); ck('release_not_claimed',pkg.get('release_ready') is False and pkg.get('native_build_proven') is False)
cap=json.loads(text(SRC/'capabilities/V035_CAPABILITIES_DELTA.json')); caps=cap.get('capabilities',[])
ck('capability_math',cap.get('base')==488 and cap.get('delta_count')==42 and cap.get('projected_total')==530); ck('capability_unique',len(caps)==42 and len(caps)==len(set(caps)))
files=[p for p in BASE.rglob('*') if p.is_file()]
ck('no_symlinks',not any(p.is_symlink() for p in BASE.rglob('*'))); ck('no_pycache',not any('__pycache__' in p.parts for p in BASE.rglob('*')))
# parse all JSON/TOML currently in package
bad=[]
for p in files:
 try:
  if p.suffix=='.json': json.loads(text(p))
  elif p.suffix=='.toml': tomllib.loads(text(p))
 except Exception as e: bad.append(f'{p.relative_to(BASE)}: {e}')
ck('all_json_toml_parse',not bad,'; '.join(bad[:5]))
# expected source and migration shape
for rel in ['crates/phxclaw-ai-sre/src/lib.rs','crates/phxclaw-ai-autopilot/src/lib.rs','migrations/0035_ai_sre_autopilot.sql','tests/sql/v035_rls_e2e.sql','ci/run-v035-native.sh','ci/run-v035-native.ps1']:
 ck('present:'+rel,(SRC/rel).is_file())
sre=text(SRC/'crates/phxclaw-ai-sre/src/lib.rs'); auto=text(SRC/'crates/phxclaw-ai-autopilot/src/lib.rs'); sql=text(SRC/'migrations/0035_ai_sre_autopilot.sql'); rls=text(SRC/'tests/sql/v035_rls_e2e.sql'); ci=text(SRC/'ci/run-v035-native.sh')
ck('rust_unsafe_forbidden','unsafe {' not in sre and 'unsafe {' not in auto and 'unsafe fn' not in sre and 'unsafe fn' not in auto)
ck('no_raw_prompt_persistence','raw_prompt' not in sql.lower() and 'raw_output' not in sql.lower())
ck('freshness_contract','max_sample_age_seconds' in sre and 'rate_limit_is_fresh' in sre and 'StaleSample' in sre)
ck('budget_limit_bound','budget_limit_micro_usd' in sre and 'budget_limit_micro_usd' in sql)
ck('incident_conflict_guard','ContradictoryIncidentEvidence' in sre)
ck('provider_membership_guard','portfolio_providers' in auto and 'TargetOutsidePortfolio' in auto)
ck('plan_evidence_binding','cost.forecast_sha256' in auto and 'incident.detection_sha256' in auto)
ck('execution_expiry_guard','PlanExpired' in auto and 'plan.spec.expires_at_unix' in auto)
ck('leader_fence_guard','authorize_mutation' in auto and 'phxclaw_assert_controller_fence' in sql)
ck('no_shell_escape','std::process::Command' not in auto and 'system.command.execute' not in auto)
ck('no_model_promotion','PromoteModel' not in auto and 'promote_model' not in auto)
ck('twelve_tables',sql.count('CREATE TABLE IF NOT EXISTS phxclaw_ai_')==12)
ck('force_rls','FORCE ROW LEVEL SECURITY' in sql)
ck('rls_quoting_safe',"current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid" in sql and ', )::uuid' not in sql)
ck('append_only','BEFORE UPDATE OR DELETE' in sql and 'phxclaw_ai_sre_no_mutation' in sql)
ck('tenant_composite_fks',sql.count('FOREIGN KEY(tenant_uuid,')>=10)
ck('execution_idempotent','UNIQUE(tenant_uuid,plan_uuid)' in sql and 'ON CONFLICT (tenant_uuid,plan_uuid) DO NOTHING' in sql)
ck('rls_non_vacuous','missing tenant A portfolio fixture' in rls and 'SRE policy fixture insert was vacuous' in rls)
ck('rls_cross_tenant_assertion','cross-tenant RLS leak' in rls)
ck('separate_rls_connection','PHXCLAW_RLS_DATABASE_URL' in ci)
ck('rls_role_guard','rolsuper OR rolbypassrls' in ci and 'NOSUPERUSER + NOBYPASSRLS' in ci)
ck('cargo_locked','cargo check --workspace --locked' in ci and 'cargo test --workspace --locked' in ci and 'cargo clippy --workspace --all-targets --locked -- -D warnings' in ci)
# no obvious real secrets/tokens
secret_re=re.compile(r'(?i)(sk-[A-Za-z0-9_-]{20,}|ghp_[A-Za-z0-9]{20,}|AIza[0-9A-Za-z_-]{20,}|AKIA[0-9A-Z]{16})')
hits=[]
for p in files:
 if p.suffix.lower() in {'.json','.toml','.md','.rs','.py','.sql','.sh','.ps1'}:
  m=secret_re.search(text(p))
  if m: hits.append(str(p.relative_to(BASE)))
ck('no_obvious_secrets',not hits,','.join(hits[:5]))
ck('third_party_delta',(BASE/'THIRD_PARTY_NOTICES_DELTA.md').is_file() and 'No new third-party source code' in text(BASE/'THIRD_PARTY_NOTICES_DELTA.md'))
report={'suite':'PhxClaw v0.35 package checks','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}
(BASE/'reports').mkdir(exist_ok=True); (BASE/'reports/V035_PACKAGE_CHECKS.json').write_text(json.dumps(report,indent=2)+'\n')
print(f"PASS={report['pass']} FAIL={report['fail']}")
for x in checks:
 if not x['pass']: print('FAIL',x['name'],x['detail'])
sys.exit(1 if report['fail'] else 0)

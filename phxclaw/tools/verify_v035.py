#!/usr/bin/env python3
from pathlib import Path
import json, sys, tomllib
BASE=Path(__file__).resolve().parents[1]
SRC=BASE/'overlay' if (BASE/'overlay').is_dir() else BASE
checks=[]
def ck(name,cond,detail=''): checks.append({'name':name,'pass':bool(cond),'detail':detail})
def txt(rel):
 p=SRC/rel
 return p.read_text() if p.is_file() else ''
required=[
'crates/phxclaw-ai-sre/Cargo.toml','crates/phxclaw-ai-sre/src/lib.rs',
'crates/phxclaw-ai-autopilot/Cargo.toml','crates/phxclaw-ai-autopilot/src/lib.rs',
'migrations/0035_ai_sre_autopilot.sql','config/ai-sre-policy.v035.json',
'capabilities/V035_CAPABILITIES_DELTA.json','docs/AI_SRE_COST_CAPACITY_AUTOPILOT_V035.md',
'ci/run-v035-native.sh','ci/run-v035-native.ps1','tests/sql/v035_rls_e2e.sql']
for r in required: ck('file:'+r,(SRC/r).is_file())
for p in sorted((SRC/'schemas').glob('*v035.schema.json')):
 try: json.loads(p.read_text()); ck('schema:'+p.name,True)
 except Exception as e: ck('schema:'+p.name,False,str(e))
cap=json.loads(txt('capabilities/V035_CAPABILITIES_DELTA.json') or '{}')
ck('cap_base',cap.get('base')==488); ck('cap_delta',cap.get('delta_count')==42); ck('cap_total',cap.get('projected_total')==530)
ck('cap_unique',len(cap.get('capabilities',[]))==len(set(cap.get('capabilities',[])))==42)
pol=json.loads(txt('config/ai-sre-policy.v035.json') or '{}')
ck('no_raw_prompts',pol.get('store_raw_prompts') is False); ck('no_raw_outputs',pol.get('store_raw_outputs') is False)
ck('no_auto_capacity_purchase_default',pol.get('autopilot',{}).get('allow_auto_capacity_purchase') is False)
ck('v030_fence_authority',pol.get('authority',{}).get('production_mutation_fencing')=='v0.30')
ck('forecast_sample_ttl',isinstance(pol.get('forecast',{}).get('max_sample_age_seconds'),int) and pol['forecast']['max_sample_age_seconds']>0)
sre=txt('crates/phxclaw-ai-sre/src/lib.rs'); auto=txt('crates/phxclaw-ai-autopilot/src/lib.rs'); sql=txt('migrations/0035_ai_sre_autopilot.sql'); rls=txt('tests/sql/v035_rls_e2e.sql')
patterns_sre=['SignedSrePolicyDocument','VerifiedSrePolicyDocument','verify_signed_policy','SloEvaluation','evaluate_slo','DemandForecast','forecast_demand','SignedRateLimitSnapshot','verify_rate_limit_snapshot','AdmissionDecision','admission_decision','CostForecast','forecast_cost','IncidentDetection','detect_incident','ed25519_dalek','ttl_seconds','max_sample_age_seconds','rate_limit_is_fresh','budget_limit_micro_usd','StaleSample','ContradictoryIncidentEvidence']
for x in patterns_sre: ck('sre:'+x,x in sre)
ck('sre_admission_rechecks_rate','rate_limit_is_fresh(now_unix, &rate.snapshot)' in sre)
ck('sre_policy_rechecked_at_decision','policy_active(now_unix, policy)' in sre)
ck('sre_queue_aging','queue_wait_ms' in sre and 'saturating_add' in sre)
patterns_auto=['SignedAutopilotPlanDocument','VerifiedAutopilotPlanDocument','recompute_automatic','verify_signed_plan','authorize_automatic_execution','TargetOutsidePortfolio','AutomaticBoundsExceeded','HumanApprovalRequired','authorize_mutation','LeaderLease','OperationFence','RequestCapacityIncrease','RebalanceShare','portfolio_providers','PlanExpired']
for x in patterns_auto: ck('autopilot:'+x,x in auto)
ck('autopilot_no_promote','PromoteModel' not in auto and 'promote_model' not in auto)
ck('autopilot_no_shell','std::process::Command' not in auto and 'system.command.execute' not in auto)
ck('autopilot_provider_membership','!providers.contains(&a.provider_uuid)' in auto and '!providers.contains(&tp)' in auto)
ck('autopilot_zero_budget_not_free','cost.budget_limit_micro_usd == 0' in auto and 'return 10_000' in auto)
ck('autopilot_binds_cost_incident','contains(&cost.forecast_sha256)' in auto and 'contains(&incident.detection_sha256)' in auto)
ck('autopilot_expiry_execution','db_now.timestamp() > plan.spec.expires_at_unix' in auto)
for table in ['phxclaw_ai_sre_policies','phxclaw_ai_slo_samples','phxclaw_ai_slo_evaluations','phxclaw_ai_demand_samples','phxclaw_ai_demand_forecasts','phxclaw_ai_rate_limit_snapshots','phxclaw_ai_admission_decisions','phxclaw_ai_budget_snapshots','phxclaw_ai_cost_forecasts','phxclaw_ai_incident_events','phxclaw_ai_autopilot_plans','phxclaw_ai_autopilot_executions']:
 ck('sql_table:'+table,table in sql)
ck('sql_budget_limit_snapshot','budget_limit_micro_usd bigint NOT NULL' in sql)
ck('sql_force_rls','FORCE ROW LEVEL SECURITY' in sql)
ck('sql_correct_setting',"current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid" in sql)
ck('sql_no_bad_setting',', )::uuid' not in sql and 'current_setting(phxclaw.tenant_uuid' not in sql)
ck('sql_append_only','BEFORE UPDATE OR DELETE' in sql and 'phxclaw_ai_sre_no_mutation' in sql)
ck('sql_fence','phxclaw_assert_controller_fence' in sql)
ck('sql_idempotent_execution','ON CONFLICT (tenant_uuid,plan_uuid) DO NOTHING' in sql)
ck('sql_composite_tenant_fks',sql.count('FOREIGN KEY(tenant_uuid,')>=10)
ck('native_locked','cargo check --workspace --locked' in txt('ci/run-v035-native.sh') and 'cargo clippy --workspace --all-targets --locked -- -D warnings' in txt('ci/run-v035-native.sh'))
ck('native_separate_rls_role','PHXCLAW_RLS_DATABASE_URL' in txt('ci/run-v035-native.sh') and 'rolsuper OR rolbypassrls' in txt('ci/run-v035-native.sh'))
ck('rls_test_cross_tenant','cross-tenant RLS leak' in rls)
ck('rls_test_non_vacuous','missing tenant A portfolio fixture' in rls and 'SRE policy fixture insert was vacuous' in rls)
for cargo in ['crates/phxclaw-ai-sre/Cargo.toml','crates/phxclaw-ai-autopilot/Cargo.toml']:
 try: tomllib.loads(txt(cargo)); ck('toml:'+cargo,True)
 except Exception as e: ck('toml:'+cargo,False,str(e))
report={'suite':'PhxClaw v0.35 static verifier','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}
out=BASE/'reports'; out.mkdir(parents=True,exist_ok=True); (out/'V035_STATIC_VERIFY_REPORT.json').write_text(json.dumps(report,indent=2)+'\n')
print(f"PASS={report['pass']} FAIL={report['fail']}")
for x in checks:
 if not x['pass']: print('FAIL',x['name'],x['detail'])
sys.exit(1 if report['fail'] else 0)

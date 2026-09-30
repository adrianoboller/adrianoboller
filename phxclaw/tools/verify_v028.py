#!/usr/bin/env python3
from pathlib import Path
import json,sys,tomllib
ROOT=Path(__file__).resolve().parents[1]; OVER=(ROOT/'overlay' if (ROOT/'overlay/crates/phxclaw-fleet-updater').exists() else ROOT); checks=[]
def ck(n,c,d=''): checks.append({'name':n,'pass':bool(c),'detail':str(d)})
def tx(r): return (OVER/r).read_text(encoding='utf-8')
def main():
    req=['crates/phxclaw-fleet-updater/Cargo.toml','crates/phxclaw-fleet-updater/src/lib.rs','config/fleet-rollout-policy.v028.json','config/trusted-fleet-signers.v028.json','config/component-update-policy.v028.json','migrations/0028_secure_fleet_rollout.sql','tools/v028_common.py','tools/create_rollout_plan.py','tools/create_component_plan.py','tools/create_health_evidence.py','tools/evaluate_rollout.py','tools/select_fleet_nodes.py','tools/verify_component_plan.py','tools/node_update_preflight.py','schemas/fleet-rollout-plan-v028.schema.json','schemas/fleet-health-evidence-v028.schema.json','schemas/fleet-state-v028.schema.json','schemas/component-update-plan-v028.schema.json','schemas/fleet-rollback-authorization-v028.schema.json','schemas/database-contract-approval-v028.schema.json','tools/create_fleet_rollback_authorization.py','tools/verify_fleet_rollback_authorization.py','tools/create_database_contract_approval.py','capabilities/V028_CAPABILITIES_DELTA.json','docs/PROJECT_STATUS_V028.json','docs/SECURE_FLEET_ROLLOUT_V028.md','.github/workflows/phxclaw-fleet-rollout.yml','ci/run-fleet-rollout.sh','ci/run-fleet-rollout.ps1']
    for r in req: ck('file_'+r,(OVER/r).is_file())
    for p in OVER.rglob('*.json'):
        try:json.loads(p.read_text());ck('json_'+str(p.relative_to(OVER)),True)
        except Exception as e:ck('json_'+str(p.relative_to(OVER)),False,e)
    try:tomllib.loads(tx('crates/phxclaw-fleet-updater/Cargo.toml'));ck('crate_toml',True)
    except Exception as e:ck('crate_toml',False,e)
    lib=tx('crates/phxclaw-fleet-updater/src/lib.rs')
    for t in ['ComponentPlan','component_plan_payload','verify_component_plan_signature','HealthThresholds','CriticalThresholds','policy_sha256','verify_fleet_signature','cohort_bucket','selected_for_percent','verify_health','verify_node_sequence','component_order','allow_database_contract','SignerNotTrusted','AntiRollback','DatabaseContractBlocked'] : ck('rust_'+t,t in lib)
    ck('trust_separation','trusted-fleet-signers.v028.json' in tx('docs/SECURE_FLEET_ROLLOUT_V028.md'))
    common=tx('tools/v028_common.py');
    for t in ['phxclaw-fleet-rollout-v028','phxclaw-fleet-health-v028','phxclaw-fleet-state-v028']: ck('common_'+t,t in common)
    ck('purpose_rollout',"'fleet.rollout'" in tx('tools/create_rollout_plan.py'))
    ck('purpose_health',"'fleet.health'" in tx('tools/create_health_evidence.py'))
    pol=json.loads(tx('config/fleet-rollout-policy.v028.json')); ck('channels',pol['allowed_channels']==['canary','beta','stable']); ck('auto_pause',pol['auto_pause_on_health_failure'] is True); ck('db_contract_100',pol['database_contract_requires_100_percent_compatible'] is True); ck('anti_rollback',pol['anti_rollback_sequence'] is True)
    sql=tx('migrations/0028_secure_fleet_rollout.sql'); ck('rls_force',sql.count('FORCE ROW LEVEL SECURITY')>=5); ck('composite_fks',sql.count('FOREIGN KEY (tenant_uuid,')>=4); ck('atomic_fencing',all(x in sql for x in ['phoenix_fleet_transition','p_expected_fencing_token','stale fleet fencing token','SECURITY INVOKER'])); ck('append_only',sql.count('phoenix_append_only_guard')>=3); ck('current_setting',"current_setting('phxclaw.tenant_uuid', true)" in sql)
    ev=tx('tools/evaluate_rollout.py'); ck('pause_and_rollback',all(x in ev for x in ["new_state='paused'","new_state='rollback_required'","new_state='completed'","advance_stage_verified"])); ck('signed_health_policy',all(x in ev for x in ['policy_sha256','health_thresholds','critical_thresholds','rollout policy changed after plan signing'])); ck('soak',"soak period incomplete" in ev); ck('min_samples',"insufficient health samples" in ev)
    comp=tx('tools/verify_component_plan.py'); ck('signed_component_plan',all(x in comp for x in ['fleet.component_plan','component_plan_payload','component policy changed after plan signing'])); ck('expand_contract',all(x in comp for x in ['database_expand','core','plugin','database_contract'])); ck('signed_contract','100% compatible + signed approval' in comp and 'fleet.database_contract' in comp and 'database contract approval scope mismatch' in comp); ck('cycle_guard','component dependency cycle' in comp)
    node=tx('tools/node_update_preflight.py'); ck('node_antirollback','anti-rollback sequence rejected' in node); ck('node_cohort','node not selected for rollout stage' in node); ck('node_v027_verify','verify_update_manifest.py' in node)
    wf=tx('.github/workflows/phxclaw-fleet-rollout.yml'); ck('workflow_dispatch','workflow_dispatch' in wf); ck('workflow_environment','environment: fleet-production' in wf); ck('workflow_no_autorun','push:' not in wf and 'schedule:' not in wf)
    rb=tx('tools/verify_fleet_rollback_authorization.py'); ck('rollback_scope',all(x in rb for x in ['from_version','to_version','from_sequence','to_sequence','fleet.rollback'])); ck('rollback_lifetime','timedelta(hours=24)' in rb)
    db=tx('tools/create_database_contract_approval.py'); ck('db_approval_completed',"state['state']!='completed'" in db); ck('db_approval_purpose','fleet.database_contract' in db)
    cap=json.loads(tx('capabilities/V028_CAPABILITIES_DELTA.json')); ck('cap_total',cap['base_total']==290 and cap['projected_total']==308 and len(cap['added'])==18); ck('cap_unique',len(cap['added'])==len(set(cap['added'])))
    st=json.loads(tx('docs/PROJECT_STATUS_V028.json')); ck('no_fake_release',st['release_ready'] is False and st['e2e_verified'] is False)
    rep={'suite':'PhxClaw v0.28 static verifier','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}; (ROOT/'reports').mkdir(exist_ok=True); (ROOT/'reports/V028_STATIC_VERIFY_REPORT.json').write_text(json.dumps(rep,indent=2)+'\n'); print(json.dumps({'pass':rep['pass'],'fail':rep['fail'],'report':str(ROOT/'reports/V028_STATIC_VERIFY_REPORT.json')},indent=2));
    if rep['fail']:
        for x in checks:
            if not x['pass']: print('FAIL',x['name'],x['detail'],file=sys.stderr)
    return 1 if rep['fail'] else 0
if __name__=='__main__': raise SystemExit(main())

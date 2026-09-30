#!/usr/bin/env python3
from pathlib import Path
import json,re,sys
HERE=Path(__file__).resolve(); ROOT=HERE.parents[1]; OV=ROOT/'overlay' if (ROOT/'overlay').exists() else ROOT
checks=[]
def ck(n,c,d=''):checks.append({'name':n,'pass':bool(c),'detail':d})
def txt(p):return (OV/p).read_text(encoding='utf-8')
for p in ['config/capabilities-v049.json','schemas/executive-decision-case.schema.json','schemas/what-if-scenario.schema.json','schemas/decision-approval.schema.json','schemas/decision-execution-envelope.schema.json']:
    try:json.loads(txt(p));ck('json_'+p,True)
    except Exception as e:ck('json_'+p,False,str(e))
cap=json.loads(txt('config/capabilities-v049.json'));ck('capability_count',cap.get('count')==63 and len(cap.get('capabilities',[]))==63)
ck('uuidv7_caps',all(re.match(r'^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$',x['uuid']) for x in cap['capabilities']))
rust=txt('crates/phxclaw-executive-decision-center/src/lib.rs')
for token in ['DecisionCase','WhatIfScenario','ImpactVector','DecisionConstraints','ScenarioEvaluation','SignedApproval','VerifiedApproval','DecisionExecutionEnvelope','scenario_hash','evaluate','compare','sensitivity','verify_approval','build_execution_envelope','DelegateRequired']:
    ck('rust_'+token,token in rust)
for phrase in ['does not mutate project state directly','delegated to v0.47','never override security/privacy/release/knowledge/fencing/hard-budget gates']:
    ck('invariant_'+phrase[:18],phrase in rust.lower())
cfg_path='config/executive-decision-center.v049.json' if (ROOT/'overlay').exists() else 'config/phxclaw.config.json';cfg=json.loads(txt(cfg_path));e=cfg['executive_decision_center']
ck('mode',e['mode']=='simulate_compare_approve_delegate');ck('direct_mutation_false',e['execution']['direct_mutation'] is False);ck('delegate_v047',e['execution']['delegate_to']=='autonomous_project_supervisor')
for gate in ['security_gate','privacy_gate','release_gate','knowledge_promotion_gate','fencing','budget_hard_limit']:ck('never_'+gate,gate in e['never_override'])
sql=txt('migrations/0049_executive_decision_center.sql');ck('sql_force_rls','FORCE ROW LEVEL SECURITY' in sql);ck('sql_rls_exact',"nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid" in sql);ck('sql_append_only','phx_executive_decision_append_only' in sql)
ck('ui_exists',(OV/'ui/executive-decision-center.html').exists())
if (ROOT/'repairs/0048_executive_project_control_tower.sql').exists(): ck('repair_0048_fixed',"nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid" in (ROOT/'repairs/0048_executive_project_control_tower.sql').read_text())
ck('no_legacy_brand',not any(x in rust for x in ['PhoenixClaw','phoenixclaw','PHOENIXCLAW']))
passed=sum(x['pass'] for x in checks);failed=len(checks)-passed;rep={'suite':'v0.49 static','pass':passed,'fail':failed,'checks':checks};(ROOT/'reports').mkdir(exist_ok=True);(ROOT/'reports/V049_STATIC_REPORT.json').write_text(json.dumps(rep,indent=2)+'\n');print(json.dumps({'pass':passed,'fail':failed},indent=2));sys.exit(1 if failed else 0)

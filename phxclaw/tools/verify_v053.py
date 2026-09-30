#!/usr/bin/env python3
from pathlib import Path
import json,re,sys
HERE=Path(__file__).resolve();ROOT=HERE.parents[1];OV=ROOT/'overlay' if (ROOT/'overlay').exists() else ROOT
checks=[]
def ck(n,c,d=''):checks.append({'name':n,'pass':bool(c),'detail':d})
def txt(p):return (OV/p).read_text(encoding='utf-8')
for p in ['config/capabilities-v053.json','schemas/adaptive-execution-telemetry.schema.json','schemas/portfolio-drift-event.schema.json','schemas/portfolio-replan-request.schema.json','schemas/portfolio-plan-revision-proposal.schema.json']:
 try:json.loads(txt(p));ck('json_'+p,True)
 except Exception as e:ck('json_'+p,False,str(e))
cap=json.loads(txt('config/capabilities-v053.json'));ck('capability_count',cap.get('count')==100 and len(cap.get('capabilities',[]))==100);ck('uuidv7_caps',all(re.match(r'^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$',x['uuid']) for x in cap['capabilities']))
rust=txt('crates/phxclaw-adaptive-portfolio-execution-controller/src/lib.rs')
for token in ['DriftThresholds','ControllerPolicy','ActivePlanRef','ExecutionTelemetry','DriftObservation','detect_drifts','confirmed_drifts','authorize_replan','ReplanRequest','build_replan_request','PlanRevisionProposal','compare_revision','DecisionDelegation','delegate_revision','assert_no_direct_mutation']:
 ck('rust_'+token,token in rust)
for phrase in ['DIRECT_MUTATION_ALLOWED: bool = false','executive_decision_center','portfolio_digital_twin','portfolio_optimizer','autonomous_portfolio_planner','Cooldown','RateLimit','StaleFencing','Ollama local-first when eligible']:
 ck('authority_'+phrase[:24],phrase in rust)
cfgp='config/adaptive-portfolio-execution-controller.v053.json' if (ROOT/'overlay').exists() else 'config/phxclaw.config.json';cfg=json.loads(txt(cfgp));e=cfg['adaptive_portfolio_execution_controller'];ck('direct_mutation_false',e['direct_mutation'] is False);ck('shadow_first',e['replan']['shadow_first'] is True);ck('auto_execute_false',e['authority']['auto_execute_reversible_preapproved'] is False);ck('cooldown',e['monitoring']['cooldown_seconds']>0);ck('replan_limit',e['monitoring']['max_replans_per_hour']>0);ck('ollama_local_first',e['models']['ollama_local_first_when_eligible'] is True)
sql=txt('migrations/0053_adaptive_portfolio_execution_controller.sql');ck('sql_force_rls',sql.count('FORCE ROW LEVEL SECURITY')>=1);ck('sql_rls_exact',"nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid" in sql);ck('sql_append_only','phx_portfolio_execution_append_only' in sql);ck('sql_direct_mutation_false','direct_mutation boolean NOT NULL DEFAULT false CHECK(direct_mutation=false)' in sql);ck('sql_shadow_only','shadow_only boolean NOT NULL DEFAULT true CHECK(shadow_only=true)' in sql);ck('sql_approval_true','requires_approval boolean NOT NULL DEFAULT true CHECK(requires_approval=true)' in sql)
ck('ui_exists',(OV/'ui/adaptive-portfolio-execution-controller.html').exists());ck('no_legacy_brand',not any(x in rust for x in ['PhoenixClaw','phoenixclaw','PHOENIXCLAW']))
passed=sum(x['pass'] for x in checks);failed=len(checks)-passed;rep={'suite':'v0.53 static','pass':passed,'fail':failed,'checks':checks};(ROOT/'reports').mkdir(exist_ok=True);(ROOT/'reports/V053_STATIC_REPORT.json').write_text(json.dumps(rep,indent=2)+'\n');print(json.dumps({'pass':passed,'fail':failed},indent=2));sys.exit(1 if failed else 0)

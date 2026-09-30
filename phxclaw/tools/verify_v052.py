#!/usr/bin/env python3
from pathlib import Path
import json,re,sys
HERE=Path(__file__).resolve();ROOT=HERE.parents[1];OV=ROOT/'overlay' if (ROOT/'overlay').exists() else ROOT
checks=[]
def ck(n,c,d=''):checks.append({'name':n,'pass':bool(c),'detail':d})
def txt(p):return (OV/p).read_text(encoding='utf-8')
for p in ['config/capabilities-v052.json','schemas/portfolio-roadmap-plan.schema.json','schemas/portfolio-plan-execution-envelope.schema.json','schemas/portfolio-project-demand.schema.json','schemas/portfolio-capacity-snapshot.schema.json']:
 try:json.loads(txt(p));ck('json_'+p,True)
 except Exception as e:ck('json_'+p,False,str(e))
cap=json.loads(txt('config/capabilities-v052.json'));ck('capability_count',cap.get('count')==82 and len(cap.get('capabilities',[]))==82);ck('uuidv7_caps',all(re.match(r'^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$',x['uuid']) for x in cap['capabilities']))
rust=txt('crates/phxclaw-autonomous-portfolio-planner/src/lib.rs')
for token in ['OptimizerSelection','ProjectDemand','ProjectDependency','CapacitySnapshot','PlannerPolicy','ProjectRoadmapItem','PortfolioPlan','DecisionExecutionEnvelope','dependency_order','dependency_bounds','window_feasible','build_plan','execution_envelope','assert_no_direct_mutation']:
 ck('rust_'+token,token in rust)
for phrase in ['DIRECT_MUTATION_ALLOWED: bool = false','executive_decision_center','SelectedCandidateRequired','CandidateNotPareto','DependencyCycle','allow_budget_increase_without_approval||self.allow_scope_change||self.allow_baseline_change||self.auto_execute']:
 ck('authority_'+phrase[:20],phrase in rust)
cfgp='config/autonomous-portfolio-planner.v052.json' if (ROOT/'overlay').exists() else 'config/phxclaw.config.json';cfg=json.loads(txt(cfgp));e=cfg['autonomous_portfolio_planner'];ck('direct_mutation_false',e['direct_mutation'] is False);ck('selected_candidate_required',e['require_explicit_selected_pareto_candidate'] is True);ck('auto_execute_false',e['authority']['auto_execute'] is False);ck('budget_increase_false',e['authority']['increase_budget_without_approval'] is False);ck('ollama_local_first',e['resources']['ollama_local_first_when_eligible'] is True)
sql=txt('migrations/0052_autonomous_portfolio_planner.sql');ck('sql_force_rls',sql.count('FORCE ROW LEVEL SECURITY')>=1);ck('sql_rls_exact',"nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid" in sql);ck('sql_append_only','phx_portfolio_planner_append_only' in sql);ck('sql_direct_mutation_false','direct_mutation boolean NOT NULL DEFAULT false CHECK(direct_mutation=false)' in sql);ck('sql_delegate_fk','REFERENCES phx_portfolio_plans(tenant_uuid,plan_uuid,plan_sha256)' in sql)
ck('ui_exists',(OV/'ui/autonomous-portfolio-planner.html').exists());ck('no_legacy_brand',not any(x in rust for x in ['PhoenixClaw','phoenixclaw','PHOENIXCLAW']))
passed=sum(x['pass'] for x in checks);failed=len(checks)-passed;rep={'suite':'v0.52 static','pass':passed,'fail':failed,'checks':checks};(ROOT/'reports').mkdir(exist_ok=True);(ROOT/'reports/V052_STATIC_REPORT.json').write_text(json.dumps(rep,indent=2)+'\n');print(json.dumps({'pass':passed,'fail':failed},indent=2));sys.exit(1 if failed else 0)

#!/usr/bin/env python3
from pathlib import Path
import json,re,sys
HERE=Path(__file__).resolve();ROOT=HERE.parents[1];OV=ROOT/'overlay' if (ROOT/'overlay').exists() else ROOT
checks=[]
def ck(n,c,d=''):checks.append({'name':n,'pass':bool(c),'detail':d})
def txt(p):return (OV/p).read_text(encoding='utf-8')
for p in ['config/capabilities-v050.json','schemas/portfolio-digital-twin.schema.json','schemas/portfolio-scenario.schema.json','schemas/portfolio-simulation-result.schema.json','schemas/portfolio-decision-delegation.schema.json']:
    try:json.loads(txt(p));ck('json_'+p,True)
    except Exception as e:ck('json_'+p,False,str(e))
cap=json.loads(txt('config/capabilities-v050.json'));ck('capability_count',cap.get('count')==64 and len(cap.get('capabilities',[]))==64);ck('uuidv7_caps',all(re.match(r'^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$',x['uuid']) for x in cap['capabilities']))
rust=txt('crates/phxclaw-portfolio-digital-twin/src/lib.rs')
for token in ['PortfolioTwin','ProjectTwin','ProjectDependency','Distribution','CalibrationEvidence','ScenarioSpec','PortfolioSimulationResult','ProjectSimulationResult','simulate','effective_seed','DependencyCycle','DecisionDelegation','assert_no_direct_mutation']:
    ck('rust_'+token,token in rust)
for phrase in ['simulation is advisory/read-only','direct_mutation_allowed: bool = false','executive_decision_center','autonomous_project_supervisor']:
    ck('authority_'+phrase[:20],phrase in rust.lower())
cfgp='config/portfolio-digital-twin.v050.json' if (ROOT/'overlay').exists() else 'config/phxclaw.config.json';cfg=json.loads(txt(cfgp));e=cfg['portfolio_digital_twin'];ck('direct_mutation_false',e['direct_mutation'] is False);ck('delegate_v049',e['delegate_decisions_to']=='executive_decision_center');ck('aggregates_only',e['simulation']['store_aggregates_only'] is True);ck('raw_samples_false',e['simulation']['store_raw_samples'] is False);ck('knowledge_promoted',e['knowledge']['use_fruitful_promoted_only'] is True)
sql=txt('migrations/0050_portfolio_digital_twin.sql');ck('sql_force_rls',sql.count('FORCE ROW LEVEL SECURITY')>=1);ck('sql_rls_exact',"nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid" in sql);ck('sql_append_only','phx_portfolio_append_only' in sql);ck('sql_no_update_mutation','direct_mutation boolean NOT NULL CHECK (direct_mutation=false)' in sql)
ck('ui_exists',(OV/'ui/portfolio-digital-twin.html').exists())
ck('no_legacy_brand',not any(x in rust for x in ['PhoenixClaw','phoenixclaw','PHOENIXCLAW']))
passed=sum(x['pass'] for x in checks);failed=len(checks)-passed;rep={'suite':'v0.50 static','pass':passed,'fail':failed,'checks':checks};(ROOT/'reports').mkdir(exist_ok=True);(ROOT/'reports/V050_STATIC_REPORT.json').write_text(json.dumps(rep,indent=2)+'\n');print(json.dumps({'pass':passed,'fail':failed},indent=2));sys.exit(1 if failed else 0)

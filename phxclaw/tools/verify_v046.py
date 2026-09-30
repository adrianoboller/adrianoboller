#!/usr/bin/env python3
from pathlib import Path
import json,re,sys
ROOT=Path(__file__).resolve().parents[1]
O=ROOT/'overlay' if (ROOT/'overlay').exists() else ROOT
checks=[]
def ck(name,cond,detail=''): checks.append({'name':name,'pass':bool(cond),'detail':detail})
lib=(O/'crates/phxclaw-predictive-project-intelligence/src/lib.rs').read_text()
sql=(O/'migrations/0046_predictive_project_intelligence.sql').read_text()
patch_path=O/'config/predictive-project-intelligence.v046.json'
if patch_path.exists():
    cfg=json.loads(patch_path.read_text())
else:
    cfg=json.loads((O/'config/phxclaw.config.json').read_text())
    cfg={'predictive_project_intelligence': cfg['predictive_project_intelligence']}
caps=json.loads((O/'config/capabilities-v046.json').read_text())['capabilities']
ck('crate_exists',(O/'crates/phxclaw-predictive-project-intelligence/Cargo.toml').exists())
ck('forecast_route','pub fn forecast_route' in lib)
ck('recommend_route','pub fn recommend_route' in lib)
ck('forecast_project','pub fn forecast_project' in lib)
ck('backtest','pub fn backtest_route' in lib)
ck('governed_only','UngovernedEvidence' in lib and '!h.governed' in lib)
ck('exact_context','context_fingerprint == context_fingerprint' in lib)
ck('ollama_local_first','Ollama local-first' in lib)
ck('v045_boundary','v0.45 routing, budget, lease and fencing gates' in lib)
ck('confidence','confidence_floor' in json.dumps(cfg))
ck('raw_prompt_false',cfg['predictive_project_intelligence']['raw_prompt_storage'] is False)
ck('raw_output_false',cfg['predictive_project_intelligence']['raw_output_storage'] is False)
ck('cloud_evidence',cfg['predictive_project_intelligence']['local_first']['cloud_escalation_requires_evidence'] is True)
ck('promotion_f24f25',cfg['predictive_project_intelligence']['learning']['promotion_authority']=='F24/F25')
ck('capability_count',len(caps)==47,str(len(caps)))
ck('capability_unique',len(caps)==len(set(caps)))
for table in ['phx_predictive_route_forecasts','phx_predictive_recommendations','phx_project_predictive_forecasts','phx_forecast_feedback','phx_predictive_events']:
    ck('table_'+table,('CREATE TABLE IF NOT EXISTS '+table) in sql)
ck('force_rls','FORCE ROW LEVEL SECURITY' in sql)
ck('tenant_setting',"current_setting(''phx.tenant_uuid'', true)" in sql)
ck('append_only','phx_deny_predictive_mutation' in sql)
ck('project_fk','REFERENCES phx_projects(tenant_uuid, project_uuid)' in sql)
for s in ['route-forecast.schema.json','project-forecast.schema.json','predictive-recommendation.schema.json']:
    p=O/'schemas'/s; ck('schema_'+s,p.exists() and json.loads(p.read_text())['type']=='object')
ck('ui',(O/'ui/predictive-project-control.html').exists())
# No legacy brand in source overlay
legacy=[]
for p in O.rglob('*'):
    if p.is_file() and p.suffix.lower() in {'.rs','.json','.sql','.html','.toml','.md'}:
        t=p.read_text(errors='ignore')
        if re.search(r'PhoenixClaw|phoenixclaw|PHOENIXCLAW',t): legacy.append(str(p))
ck('no_legacy_brand',not legacy,';'.join(legacy))
passed=sum(1 for x in checks if x['pass']); failed=len(checks)-passed
report={'suite':'v0.46 static','pass':passed,'fail':failed,'checks':checks}
path=(ROOT/'reports/V046_STATIC_REPORT.json') if (ROOT/'reports').exists() else Path('/tmp/V046_STATIC_REPORT.json')
path.parent.mkdir(parents=True,exist_ok=True); path.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'pass':passed,'fail':failed},indent=2)); sys.exit(1 if failed else 0)

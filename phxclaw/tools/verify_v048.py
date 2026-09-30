#!/usr/bin/env python3
from pathlib import Path
import json,re,sys
HERE=Path(__file__).resolve(); ROOT=HERE.parents[1]; OV=ROOT/'overlay' if (ROOT/'overlay').exists() else ROOT
checks=[]
def ck(n,c,d=''):checks.append({'name':n,'pass':bool(c),'detail':d})
def txt(p):return (OV/p).read_text(encoding='utf-8')
for p in ['config/capabilities-v048.json','schemas/executive-portfolio-snapshot.schema.json','schemas/executive-alert.schema.json','schemas/executive-decision-request.schema.json']:
    try:json.loads(txt(p));ck('json_'+p,True)
    except Exception as e:ck('json_'+p,False,str(e))
cap=json.loads(txt('config/capabilities-v048.json'));ck('capability_count',cap.get('count')==56 and len(cap.get('capabilities',[]))==56)
ck('uuidv7_caps',all(re.match(r'^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$',x['uuid']) for x in cap['capabilities']))
rust=txt('crates/phxclaw-executive-control-tower/src/lib.rs')
for token in ['PortfolioExecutiveSnapshot','ProjectExecutiveRow','rank_attention','rollup_portfolio','ExecutiveAlert','ExecutiveDecisionRequest','DelegateRequired','never']:
    ck('rust_'+token,token.lower() in rust.lower())
ck('read_only_rule','read-only-by-default' in rust.lower())
ck('delegate_supervisor','Operational mutations' in rust and 'v0.47' in rust)
cfg_path='config/executive-control-tower.v048.json' if (ROOT/'overlay').exists() else 'config/phxclaw.config.json';cfg=json.loads(txt(cfg_path));e=cfg['executive_control_tower']
ck('config_read_only',e['mode']=='read_only_default')
for gate in ['security_gate','privacy_gate','release_gate','knowledge_promotion_gate','fencing','budget_hard_limit']:ck('never_'+gate,gate in e['authority']['never_override'])
sql=txt('migrations/0048_executive_project_control_tower.sql')
ck('sql_force_rls','FORCE ROW LEVEL SECURITY' in sql)
ck('sql_rls_exact',"current_setting(''phxclaw.tenant_uuid'', true)" in sql)
ck('sql_append_only','phx_executive_append_only' in sql)
ck('ui_exists',(OV/'ui/executive-project-control-tower.html').exists())
ck('no_legacy_brand',not any(x in rust for x in ['PhoenixClaw','phoenixclaw','PHOENIXCLAW']))
passed=sum(x['pass'] for x in checks);failed=len(checks)-passed;rep={'suite':'v0.48 static','pass':passed,'fail':failed,'checks':checks}
(ROOT/'reports').mkdir(exist_ok=True);(ROOT/'reports/V048_STATIC_REPORT.json').write_text(json.dumps(rep,indent=2)+'\n');print(json.dumps({'pass':passed,'fail':failed},indent=2));sys.exit(1 if failed else 0)

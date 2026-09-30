#!/usr/bin/env python3
from pathlib import Path
import json,re,sys
HERE=Path(__file__).resolve(); ROOT=HERE.parents[1]
OV=ROOT/'overlay' if (ROOT/'overlay').exists() else ROOT
checks=[]
def ck(n,c,d=''): checks.append({'name':n,'pass':bool(c),'detail':d})
def txt(p): return (OV/p).read_text(encoding='utf-8')
# parse core artifacts
json_paths=['config/capabilities-v047.json','schemas/project-health-snapshot.schema.json','schemas/supervisor-intervention-plan.schema.json']
json_paths.append('config/autonomous-project-supervisor.v047.json' if (ROOT/'overlay').exists() else 'config/phxclaw.config.json')
for p in json_paths:
    try: json.loads(txt(p)); ck('json_'+p,True)
    except Exception as e: ck('json_'+p,False,str(e))
cap=json.loads(txt('config/capabilities-v047.json')); ck('capability_count',cap.get('count')==52 and len(cap.get('capabilities',[]))==52)
ck('uuidv7_caps',all(re.match(r'^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$',x['uuid']) for x in cap['capabilities']))
rust=txt('crates/phxclaw-autonomous-project-supervisor/src/lib.rs')
for token in ['ProjectHealthSnapshot','InterventionPlan','PdcaPhase','propose_interventions','can_execute','HardGate','ApprovalRequired','Ollama local-first']:
    ck('rust_'+token,token in rust)
cfg_path='config/autonomous-project-supervisor.v047.json' if (ROOT/'overlay').exists() else 'config/phxclaw.config.json'
cfg=json.loads(txt(cfg_path))
for gate in ['security_gate','privacy_gate','release_gate','knowledge_promotion_gate','fencing','budget_hard_limit']:
    ck('never_override_'+gate,gate in cfg['autonomous_project_supervisor']['authority']['never_override'])
sql=txt('migrations/0047_autonomous_project_supervisor.sql')
ck('sql_force_rls','FORCE ROW LEVEL SECURITY' in sql)
ck('sql_rls_exact',"current_setting(''phxclaw.tenant_uuid'', true)" in sql)
ck('sql_append_only','phx_supervisor_append_only' in sql)
ck('sql_fk_snapshot','REFERENCES phx_project_supervisor_health(tenant_uuid, project_uuid, snapshot_uuid)' in sql)
ck('ui_exists',(OV/'ui/autonomous-project-supervisor.html').exists())
ck('no_legacy_brand',not any(x in rust for x in ['PhoenixClaw','phoenixclaw','PHOENIXCLAW']))
passed=sum(x['pass'] for x in checks); failed=len(checks)-passed
rep={'suite':'v0.47 static','pass':passed,'fail':failed,'checks':checks}
out=(ROOT/'reports' if (ROOT/'reports').exists() else ROOT/'reports'); out.mkdir(exist_ok=True); (out/'V047_STATIC_REPORT.json').write_text(json.dumps(rep,indent=2)+'\n')
print(json.dumps({'pass':passed,'fail':failed},indent=2)); sys.exit(1 if failed else 0)

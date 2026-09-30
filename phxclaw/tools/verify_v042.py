#!/usr/bin/env python3
import json, sys, tomllib, re
from pathlib import Path
root=Path(sys.argv[1] if len(sys.argv)>1 else Path(__file__).resolve().parents[1])
checks=[]
def ck(name,cond,detail=''): checks.append((name,bool(cond),detail))
catp=root/'config/project-management-skill-catalog.v042.json'
if not catp.exists(): catp=root/'overlay/config/project-management-skill-catalog.v042.json'
cat=json.loads(catp.read_text())
ck('skill_count_77',cat.get('specialized_skill_count')==77)
ck('connector_count_2',cat.get('connector_adapter_count')==2)
ck('total_79',len(cat.get('skills',[]))==79)
ids=[x['id'] for x in cat['skills']]; ck('unique_skill_ids',len(ids)==len(set(ids)))
ck('orchestrator',cat['canonical_orchestrator']=='project-management-orchestrator')
ck('hybrid',cat['hybrid_engine']=='hybrid-project-management')
ck('all_uuidv7',all(re.match(r'^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$',x['uuid']) for x in cat['skills']))
ck('all_evidence',all(x['evidence_required'] for x in cat['skills']))
ck('high_approval',all(x['approval_required'] for x in cat['skills'] if x['risk']=='high'))
cp=root/'capabilities/V042_CAPABILITIES_DELTA.json'; cp=cp if cp.exists() else root/'overlay/capabilities/V042_CAPABILITIES_DELTA.json'
caps=json.loads(cp.read_text()); ck('cap_added_90',caps['added']==90); ck('projected_892',caps['projected_total']==892)
sqlp=root/'migrations/0042_project_management_suite.sql'; sqlp=sqlp if sqlp.exists() else root/'overlay/migrations/0042_project_management_suite.sql'; sql=sqlp.read_text()
for t in ['pm_projects','pm_work_items','pm_sprints','pm_risks','pm_cost_snapshots','pm_pdca_cycles','pm_stakeholders','pm_events']: ck('sql_'+t, t in sql)
ck('force_rls','FORCE ROW LEVEL SECURITY' in sql); ck('tenant_setting',"current_setting(''phxclaw.tenant_uuid'', true)" in sql); ck('append_only','append-only' in sql)
ui=root/'ui/project-management.html'; ui=ui if ui.exists() else root/'overlay/ui/project-management.html'; u=ui.read_text(); ck('ui_phxclaw','PhxClaw' in u); ck('ui_13_screens',all(x in u for x in ['Portfolio','WBS & Gantt','Scrum','Kanban','PDCA','Custos & EVM','Calendário & Docs']))
crate=root/'crates/phxclaw-project-management/Cargo.toml'; crate=crate if crate.exists() else root/'overlay/crates/phxclaw-project-management/Cargo.toml'; tomllib.loads(crate.read_text()); ck('crate_toml',True)
lib=(crate.parent/'src/lib.rs').read_text(); ck('evm_code','compute_evm' in lib); ck('health_code','evaluate_health' in lib); ck('dag_code','topological_order' in lib)
fail=[x for x in checks if not x[1]]
print(f'{len(checks)-len(fail)}/{len(checks)} PASS')
for x in fail: print('FAIL',x[0],x[2])
raise SystemExit(1 if fail else 0)

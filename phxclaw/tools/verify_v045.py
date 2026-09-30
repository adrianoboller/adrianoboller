#!/usr/bin/env python3
from __future__ import annotations
import json, sys, re
from pathlib import Path
root=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else Path(__file__).resolve().parents[1]
package=(root/'overlay').exists()
def p(rel): return (root/'overlay'/rel) if package else (root/rel)
checks=[]
def ck(n,c,d=''): checks.append({'name':n,'pass':bool(c),'detail':d})

lib=p(Path('crates/phxclaw-active-project-runtime/src/lib.rs')).read_text(encoding='utf-8')
for token in ['plan_dispatch','knowledge_guard','route_model','KnownFailedRoute','RetryDecisionRequired','DispatchLease','fencing_token','close_pdca_and_classify','Ollama','remaining_budget']:
    ck('rust_'+token, token in lib)
ck('rust_candidate_not_authority','promotion_state != "candidate"' in lib)
ck('rust_extreme_review','Complexity::Extreme' in lib and 'require_independent_review' in lib)

sql=p(Path('migrations/0045_active_project_runtime.sql')).read_text(encoding='utf-8')
for t in ['phx_active_project_runtime','phx_project_task_queue','phx_agent_execution_leases','phx_project_budget_reservations','phx_execution_runtime_events']:
    ck('sql_'+t,f'CREATE TABLE IF NOT EXISTS {t}' in sql)
ck('sql_force_rls','FORCE ROW LEVEL SECURITY' in sql)
ck('sql_quoting',"nullif(current_setting('phx.tenant_uuid', true), '')::uuid" in sql)
ck('sql_fencing_function','phx_next_project_fencing' in sql)
ck('sql_append_only','phx_runtime_events_append_only' in sql)
ck('sql_project_fk','REFERENCES phx_projects(tenant_uuid, project_uuid)' in sql)
ck('sql_agent_fk','REFERENCES phx_agent_runtime_profiles(tenant_uuid, project_uuid, agent_uuid)' in sql)
ck('sql_idempotency','UNIQUE (tenant_uuid, project_uuid, idempotency_key)' in sql)

caps=json.loads(p(Path('config/capabilities-v045.json')).read_text(encoding='utf-8'))
ck('capabilities_48',caps.get('count')==48 and len(caps.get('capabilities',[]))==48)
ck('capabilities_unique',len(set(caps['capabilities']))==48)
for s in ['execution-ticket.schema.json','dispatch-lease.schema.json','pdca-runtime-result.schema.json']:
    obj=json.loads(p(Path('schemas')/s).read_text(encoding='utf-8')); ck('schema_'+s,obj.get('type')=='object')
ui=p(Path('ui/active-project-control.html')).read_text(encoding='utf-8')
ck('ui_phxclaw','PhxClaw' in ui and 'PDCA' in ui and 'Ollama' in ui)

if not package:
    cfg=json.loads((root/'config/phxclaw.config.json').read_text(encoding='utf-8')); ar=cfg.get('active_project_runtime',{})
    ck('cfg_enabled',ar.get('enabled') is True)
    ck('cfg_local_first',ar.get('cost_optimization',{}).get('local_first') is True)
    ck('cfg_pdca',ar.get('pdca',{}).get('required') is True)
    ck('cfg_promotion',ar.get('knowledge_feedback',{}).get('promotion_via_f24_f25_only') is True)
    cargo=(root/'Cargo.toml').read_text(encoding='utf-8'); ck('cargo_member',cargo.count('"crates/phxclaw-active-project-runtime"')==1)

fail=[x for x in checks if not x['pass']]
rep={'suite':'PhxClaw v0.45 Active Project Scheduler','pass':len(checks)-len(fail),'fail':len(fail),'checks':checks}
print(json.dumps(rep,indent=2,ensure_ascii=False)); raise SystemExit(1 if fail else 0)

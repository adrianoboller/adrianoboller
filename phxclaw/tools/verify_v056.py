#!/usr/bin/env python3
from pathlib import Path
import json,sys
P=Path(__file__).resolve().parents[1];O=P/'overlay' if (P/'overlay').exists() else P;checks=[]
def c(n,v,d=''):checks.append({'name':n,'pass':bool(v),'detail':d})
mat=json.loads((O/'config/sprint-gate-matrix.v056.json').read_text());cfg=json.loads((O/'config/native-qualification.v056.json').read_text())
c('matrix_26',len(mat['sprints'])==26)
c('unique_sprints',len({s['sprint_id'] for s in mat['sprints']})==26)
c('target_10',mat['campaign']['target_green_count']==10 and len(mat['campaign']['priority_sprints'])==10)
c('unavailable_not_pass',mat['policy']['unavailable_is_pass'] is False)
c('same_source_required',mat['policy']['source_state_must_match'] is True)
c('native_e2e_not_replaced',mat['policy']['static_evidence_cannot_substitute_native_e2e'] is True)
c('global_cargo_all',all(all(g in s['required_gates'] for g in ['cargo_fmt','cargo_check','cargo_test','cargo_clippy']) for s in mat['sprints']))
c('db_rls_priority',all(any(g in s['required_gates'] for g in ['rls_e2e','checkpoint_rollback_e2e','repo_intelligence_e2e','sandbox_e2e']) for s in mat['sprints'] if s['sprint_id'] in ['F04','F05','F06','F09','F15','F17','F19','F20']))
c('cfg_no_mock',cfg['release']['forbid_mock_pass'] is True)
c('cfg_rls_role',cfg['database']['require_rls_nonsuperuser'] and cfg['database']['require_rls_nobypassrls'])
for f in ['qualify_v056.py','preflight_v056.py','source_state_v056.py','sprint_gate_reconcile_v056.py','apply_v056.py']:
 c('tool_'+f,(P/'tools'/f).exists())
report={'suite':'v0.56 static','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks};print(json.dumps(report,indent=2));sys.exit(1 if report['fail'] else 0)

#!/usr/bin/env python3
from pathlib import Path
import json,re,sys
P=Path(__file__).resolve().parents[1]
def loc(rel):
 a=P/'overlay'/rel
 if a.exists():return a
 return P/rel
checks=[]
def ck(name,cond,detail=''):
 checks.append({'name':name,'pass':bool(cond),'detail':detail})
lib=loc(Path('crates/phxclaw-reconciliation-ledger/src/lib.rs')).read_text()
sql=loc(Path('migrations/0055_reconciliation_execution_ledger.sql')).read_text()
caps=json.loads(loc(Path('config/capabilities-v055.json')).read_text())
cfgp=loc(Path('config/reconciliation-execution-ledger.v055.json'))
if not cfgp.exists(): cfgp=P/'config/phxclaw.config.json'
cfg=json.loads(cfgp.read_text())
ck('capability_count',caps.get('count')==84 and len(caps.get('capabilities',[]))==84)
ck('direct_mutation_false','DIRECT_MUTATION_ALLOWED: bool = false' in lib)
ck('append_fact','pub fn append_fact' in lib and 'NonMonotonicSequence' in lib and 'StaleFencing' in lib and 'IdempotencyConflict' in lib)
ck('reconcile','pub fn reconcile(' in lib and 'agent_changed' in lib and 'model_changed' in lib and 'skills_changed' in lib)
ck('reason_never_inferred','ReasonStatus::Missing' in lib and 'UnprovenReason' in lib)
ck('sprint_gate_reconciler','reconcile_sprint_gates' in lib and 'UNAVAILABLE_IS_PASS: bool = false' in lib)
ck('same_source_state','e.source_state_sha256!=source_state' in lib)
ck('sql_tables',all(x in sql for x in ['phx_reconciliation_plan_snapshots','phx_execution_ledger_facts','phx_reconciliation_variances','phx_reconciliation_causal_links','phx_sprint_gate_evidence','phx_sprint_gate_status_snapshots']))
ck('rls_force',sql.count('FORCE ROW LEVEL SECURITY')>=1 and "current_setting('phxclaw.tenant_uuid', true)" in sql)
ck('append_only','phx_reconciliation_append_only' in sql and 'BEFORE UPDATE OR DELETE' in sql)
ck('config_enabled',('reconciliation_execution_ledger' in cfg) or ('reconciliation_execution_ledger' in cfg.get('reconciliation_execution_ledger',{})))
ck('ui',loc(Path('ui/reconciliation-execution-ledger.html')).exists())
for p in (P/'overlay/schemas').glob('*.json') if (P/'overlay/schemas').exists() else (P/'schemas').glob('*.json'):
 try:json.loads(p.read_text());ck('schema_'+p.name,True)
 except Exception as e:ck('schema_'+p.name,False,str(e))
report={'suite':'v0.55 static','pass':sum(c['pass'] for c in checks),'fail':sum(not c['pass'] for c in checks),'checks':checks}
print(json.dumps(report,indent=2))
if report['fail']:sys.exit(2)

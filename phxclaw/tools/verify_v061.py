#!/usr/bin/env python3
from pathlib import Path
import json,tomllib,re,sys,hashlib
R=Path(__file__).resolve().parents[1]
checks=[]
def ck(n,c,d=''): checks.append((n,bool(c),d))
req=['crates/phxclaw-safe-source-harvester/Cargo.toml','crates/phxclaw-safe-source-harvester/src/lib.rs','migrations/0061_safe_source_harvester.sql','config/source-harvest-policy.v061.json','schemas/source-harvest-receipt-v061.schema.json','docs/ADR-0061-safe-source-harvester.md','docs/INTEGRATION_V061.md']
for p in req: ck('file:'+p,(R/p).is_file())
ws=tomllib.loads((R/'Cargo.toml').read_text()); members=ws['workspace']['members']; ck('workspace_member','crates/phxclaw-safe-source-harvester' in members)
lib=(R/'crates/phxclaw-safe-source-harvester/src/lib.rs').read_text(); sql=(R/'migrations/0061_safe_source_harvester.sql').read_text(); pol=json.loads((R/'config/source-harvest-policy.v061.json').read_text()); sch=json.loads((R/'schemas/source-harvest-receipt-v061.schema.json').read_text())
ck('rust_forbid_unsafe','#![forbid(unsafe_code)]' in lib)
ck('no_process_command','process::Command' not in lib and 'std::process' not in lib)
ck('deny_short_circuit','if provenance_decision == HarvestDecision::Deny' in lib)
ck('quarantine_clears_candidates','candidates.clear()' in lib)
ck('content_addressed','source-vault/{bucket}/sha256' in lib)
ck('secret_scan','secret_markers' in lib and 'QuarantinedSecret' in lib)
ck('knowledge_raw_only','EpistemicState::RawObservation' in lib and 'NodeKind::Evidence' in lib)
ck('policy_fail_closed',pol.get('fail_closed') is True)
ck('policy_no_execution',pol.get('execute_external_code') is False)
ck('policy_deny_no_copy',pol.get('copy_denied_sources') is False)
ck('schema_id',sch.get('$id')=='phxclaw://schemas/source-harvest-receipt-v061')
ck('sql_candidate_allow_trigger','phx_source_candidate_must_be_allowed' in sql and "d IS DISTINCT FROM 'ALLOW'" in sql)
ck('sql_immutable_receipts','phx_source_harvest_immutable' in sql)
ck('sql_rls_force',sql.count('FORCE ROW LEVEL SECURITY')>=3)
ck('sql_current_setting_valid',"current_setting('phxclaw.tenant_uuid', true), '')::uuid" in sql)
# All JSON schema/config parses.
for p in [R/'config/source-harvest-policy.v061.json',R/'schemas/source-harvest-receipt-v061.schema.json',R/'docs/PROJECT_STATUS_V061_CONSOLIDATED.json']:
    try: json.loads(p.read_text()); ck('json:'+p.name,True)
    except Exception as e: ck('json:'+p.name,False,str(e))
fail=[x for x in checks if not x[1]]
report={'version':'0.61.0','pass':len(checks)-len(fail),'fail':len(fail),'checks':[{'name':n,'pass':c,'detail':d} for n,c,d in checks]}
(R/'reports/V061_STATIC_VERIFY.json').write_text(json.dumps(report,indent=2)+'\n')
(R/'reports/V061_STATIC_VERIFY.md').write_text('# PhxClaw v0.61 static verify\n\n'+f"PASS: {report['pass']}  FAIL: {report['fail']}\n\n"+'\n'.join(f"- {'PASS' if c else 'FAIL'} `{n}` {d}" for n,c,d in checks)+'\n')
print(f"PASS {report['pass']} / FAIL {report['fail']}")
for n,c,d in checks:
    if not c: print('FAIL',n,d)
sys.exit(1 if fail else 0)

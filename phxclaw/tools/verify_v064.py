#!/usr/bin/env python3
from pathlib import Path
import json, re, sys, tomllib
ROOT=Path(__file__).resolve().parents[1]
checks=[]
def ck(name,ok,detail=''): checks.append({'name':name,'pass':bool(ok),'detail':detail})
# root version/deps
cargo=(ROOT/'Cargo.toml').read_text()
ck('workspace_version_064','version = "0.64.0"' in cargo)
ck('reqwest_stream_feature','"stream"' in next((x for x in cargo.splitlines() if x.startswith('reqwest = ')),''))
# crate
crate=(ROOT/'crates/phxclaw-mcp-lsp-runtime/src/lib.rs').read_text()
for token in ['struct McpStdioSession','struct LspStdioSession','struct McpStreamableHttpClient','struct ProcessSecurityPolicy','struct CapabilityPolicy','struct Cancellation','struct ReconnectPolicy','notifications/cancelled','$/cancelRequest','MCP-Protocol-Version','Mcp-Method','bytes_stream','EvidenceLedger','EventEnvelope','env_clear()','kill_on_drop(true)']:
    ck('runtime_'+re.sub(r'\W+','_',token).strip('_'),token in crate,token)
# db/config/schema/docs
for rel in ['migrations/0064_mcp_lsp_managed_runtime.sql','config/mcp-lsp-runtime.v064.json','schemas/mcp-lsp-runtime-v064.schema.json','docs/ADR-0064-managed-mcp-lsp-runtime.md','docs/MCP_LSP_RUNTIME_V064.md','tests/v064/mcp_stdio_fixture.py','tests/v064/lsp_stdio_fixture.py','tools/v064_process_smoke.py']:
    ck('exists_'+rel.replace('/','_'),(ROOT/rel).is_file(),rel)
# migration hardening
sql=(ROOT/'migrations/0064_mcp_lsp_managed_runtime.sql').read_text()
for token in ['protocol_sessions','protocol_requests','protocol_session_events','FORCE ROW LEVEL SECURITY','protocol_append_only','current_tenant_uuid()']:
    ck('sql_'+token.replace(' ','_').replace('(','').replace(')',''),token in sql)
# canonical F18
state=json.loads((ROOT/'config/project-state-canonical.v064.json').read_text())
f18=next(x for x in state['sprints'] if x['id']=='F18')
ck('canonical_version',state['version']=='0.64.0')
ck('f18_source_ready',f18['implementation_state']=='source_ready')
ck('f18_static_verified',f18['verification_state']=='static_verified')
ck('f18_gaps_empty',f18['gaps']==[])
# sprint refs
sprints=list((ROOT/'sprints').glob('f??.json'))
ck('sprint_count_26',len(sprints)==26,str(len(sprints)))
ck('all_sprint_refs_v064',all(json.loads(p.read_text()).get('canonical_project_version')=='0.64.0' for p in sprints))
# process smoke
smoke=json.loads((ROOT/'reports/V064_PROCESS_SMOKE.json').read_text())
ck('real_process_smoke_7_7',smoke.get('pass')==7 and smoke.get('fail')==0,str(smoke))
# parsability
for p in [ROOT/'crates/phxclaw-mcp-lsp-runtime/Cargo.toml',ROOT/'Cargo.toml']:
    try: tomllib.loads(p.read_text()); ok=True; detail=''
    except Exception as e: ok=False; detail=str(e)
    ck('toml_'+p.name+'_'+p.parent.name,ok,detail)
for p in [ROOT/'config/mcp-lsp-runtime.v064.json',ROOT/'schemas/mcp-lsp-runtime-v064.schema.json',ROOT/'config/project-state-canonical.v064.json']:
    try: json.loads(p.read_text()); ok=True; detail=''
    except Exception as e: ok=False; detail=str(e)
    ck('json_'+p.name,ok,detail)
report={'version':'0.64.0','checks':checks,'pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'native_rust':False}
(ROOT/'reports/V064_STATIC_VERIFY.json').write_text(json.dumps(report,indent=2)+'\n')
(ROOT/'reports/V064_STATIC_VERIFY.md').write_text('# v0.64 Static Verify\n\n'+f"PASS: {report['pass']} / FAIL: {report['fail']}\n")
print(json.dumps(report,indent=2));sys.exit(1 if report['fail'] else 0)

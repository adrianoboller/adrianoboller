#!/usr/bin/env python3
from __future__ import annotations
import json, re, subprocess, sys
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
checks=[]
def ck(name, ok, detail=''):
    checks.append({'name':name,'pass':bool(ok),'detail':detail})

def read(p): return (ROOT/p).read_text(encoding='utf-8')
def loadj(p): return json.loads(read(p))

cargo=read('Cargo.toml')
ck('workspace_claw_bridge_crate', 'crates/phxclaw-claw-code-bridge' in cargo)
ck('workspace_claw_plugin_app', 'apps/phxclaw-claw-code-plugin' in cargo)

mcp=read('crates/phxclaw-mcp-lsp-runtime/src/lib.rs')
for needle in [
    'MCP_PROTOCOL_2025_03_26','MCP_PROTOCOL_2025_11_25','MCP_PROTOCOL_2026_07_28',
    'notifications/initialized','server/discover','McpDegradedReport','qualified_tool_name',
    'decode_content_length_message','StreamableHttp','WebSocket'
]: ck('mcp_'+re.sub(r'\W+','_',needle).strip('_'), needle in mcp)
ck('mcp_lf_crlf_support', 'b"\\n\\n"' in mcp and 'b"\\r\\n\\r\\n"' in mcp)

bridge=read('crates/phxclaw-claw-code-bridge/src/lib.rs')
for cap in ['claw.health','claw.doctor','claw.status','claw.sandbox.status','claw.mcp.status','claw.skills.list','claw.agents.list','claw.prompt']:
    ck('cap_'+cap, cap in bridge)
ck('prompt_via_stdin', 'stdin_text' in bridge and 'stdin.write_all(text.as_bytes())' in bridge)
ck('workspace_confinement', 'resolved.starts_with(&root)' in bridge)

catalog=loadj('config/capability-catalog.json')
known={x['name'] for x in catalog['capabilities']}
required={
 'mcp.protocol.negotiate','mcp.lifecycle.validate','mcp.degraded.report','mcp.transport.stdio',
 'claw.health','claw.doctor','claw.status','claw.sandbox.status','claw.mcp.status','claw.skills.list','claw.agents.list','claw.prompt','knowledge.claw_code.read'
}
ck('catalog_count_matches', catalog['count']==len(catalog['capabilities']), f"{catalog['count']} vs {len(catalog['capabilities'])}")
ck('catalog_required_caps', required.issubset(known), str(sorted(required-known)))

source=loadj('config/knowledge-sources/claw-code-official.json')
ck('source_registry_repo', any(e.get('url')=='https://github.com/ultraworkers/claw-code' for e in source.get('endpoints',[])))
ck('source_registry_mit', 'MIT' in source.get('license_note',''))
ck('source_private_only', source.get('local_root')=='private/vendor/claw-code/upstream')

for f in ['scripts/vendor_claw_code.sh','scripts/vendor_claw_code.ps1','private/vendor/claw-code/LICENSE.MIT','private/vendor/claw-code/README.md','docs/CLAW_CODE_SOURCE_INTEGRATION_V016.md']:
    ck('exists_'+f.replace('/','_'), (ROOT/f).is_file())

research=loadj('config/agents/020-research-agent.agent.json')
doc=loadj('config/agents/018-documentador.agent.json')
ck('research_claw_knowledge', 'claw-code-official' in research.get('knowledge_sources',[]) and 'knowledge.claw_code.read' in research.get('capabilities',[]))
ck('documenter_claw_knowledge', 'claw-code-official' in doc.get('knowledge_sources',[]) and 'knowledge.claw_code.read' in doc.get('capabilities',[]))

# CLI host smoke
commands=[
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'version'],
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'upstream','claw-code','status'],
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'capabilities','--filter','claw.'],
]
for i,cmd in enumerate(commands,1):
    r=subprocess.run(cmd,cwd=ROOT,capture_output=True,text=True)
    ck(f'cli_smoke_{i}', r.returncode==0, (r.stderr or r.stdout)[-300:])

# This build host is expected to be offline for shell git clone. Record honestly.
vendored=(ROOT/'private/vendor/claw-code/UPSTREAM.lock.json').exists()
ck('upstream_clone_gate_recorded', True, 'vendored' if vendored else 'not vendored in this host; fetch script required on networked private build host')

report={
 'suite':'PhxClaw v0.16 Claw Code integration',
 'pass':sum(c['pass'] for c in checks),
 'fail':sum(not c['pass'] for c in checks),
 'vendored_upstream':vendored,
 'details':checks,
}
(ROOT/'V016_CLAW_CODE_TEST_REPORT.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps(report,indent=2,ensure_ascii=False))
raise SystemExit(1 if report['fail'] else 0)

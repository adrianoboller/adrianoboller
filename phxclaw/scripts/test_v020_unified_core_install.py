#!/usr/bin/env python3
from __future__ import annotations
import hashlib, json, subprocess, sys, tomllib
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
REPORT=ROOT/'V020_UNIFIED_CORE_INSTALL_TEST_REPORT.json'
checks=[]
def ck(name,ok,detail=''): checks.append({'name':name,'pass':bool(ok),'detail':detail})
def text(p): return (ROOT/p).read_text(encoding='utf-8')
def loadj(p): return json.loads(text(p))

with (ROOT/'Cargo.toml').open('rb') as f: cargo=tomllib.load(f)
members=set(cargo['workspace']['members'])
ck('version_020',cargo['workspace']['package']['version']=='0.20.0')
for m in ['crates/phxclaw-core-runtime','crates/phxclaw-postgres-bootstrap','apps/phxclaw']:
    ck('workspace_'+m,m in members)

core=text('crates/phxclaw-core-runtime/src/lib.rs')
for needle in ['PhoenixCoreRuntime','CoreService','NativeSessionStore','ChannelAgentRouter','create_schedule','mcp_tool_name']:
    ck('core_'+needle,needle in core)
ck('core_product_surface','PRODUCT_NAME: &str = "PhxClaw"' in core and 'PRODUCT_CLI: &str = "phx"' in core)

pg=loadj('config/install/postgresql.json')
ck('postgres_prod_18',pg['production']['major']==18 and pg['production']['version']=='18.6')
ck('postgres_19_preview_disabled',pg['preview']['major']==19 and pg['preview']['allowed_for_production'] is False)
ck('postgres_loopback_only',pg['policy']['bind_loopback_only'] is True)
ck('postgres_no_password_argv',pg['policy']['passwords_in_argv'] is False)
ck('postgres_hash_required',pg['policy']['require_hash_verification_for_downloads'] is True)
ck('postgres_windows_sha256',len(pg['production']['windows_bundle_sha256'])==64)

installer=text('installer/bootstrap.py')
for needle in ['--pwfile=',"env['PGPASSWORD']",'safe_extract_zip','windows_bundle_sha256','apply_migrations','store_secret']:
    ck('installer_'+needle,needle in installer)
ck('installer_no_superpassword_argv','SuperPassword' not in installer and '--superpassword' not in installer.lower())
ck('installer_no_plaintext_runtime_password',"password_secret_uuid" in installer and "'password':apppass" not in installer)

catalog=loadj('config/capability-catalog.json')
known={x['name'] for x in catalog['capabilities']}
required={'core.runtime.start','core.runtime.status','database.postgresql.install','database.postgresql.configure','database.postgresql.migrate','database.postgresql.doctor'}
ck('capability_count',catalog['count']==len(catalog['capabilities']))
ck('new_capabilities',required.issubset(known),str(sorted(required-known)))

constitution=loadj('config/constitution.json')
ck('constitution_version',constitution['core_version']=='0.20.0' and constitution['version']=='0.20.0')
ck('constitution_services',{'core_runtime','postgres_bootstrap'}.issubset(set(constitution['platform_services'])))

# CLI live checks that do not mutate the host.
commands=[
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'version'],
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'status'],
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'core','status'],
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'db','plan'],
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'sprints','list','--no-color'],
]
outs=[]
for i,cmd in enumerate(commands,1):
    p=subprocess.run(cmd,cwd=ROOT,capture_output=True,text=True)
    ck(f'cli_{i}',p.returncode==0,(p.stderr or p.stdout)[-1000:]); outs.append(p.stdout)
ck('status_sprint_counts','sprints_green=0' in outs[1] and 'sprints_yellow=23' in outs[1] and 'sprints_red=3' in outs[1])
ck('sprint_labels','YELLOW' in outs[4] and 'RED' in outs[4] and 'GREEN=0 YELLOW=23 RED=3' in outs[4])

for p in ['docs/UNIFIED_CORE_RUNTIME_V020.md','docs/INSTALLATION_V020.md','migrations/0020_core_runtime_installation.sql']:
    ck('artifact_'+p,(ROOT/p).is_file() and (ROOT/p).stat().st_size>0)

report={'suite':'PhxClaw v0.20 unified product/runtime/installer','pass':sum(c['pass'] for c in checks),'fail':sum(not c['pass'] for c in checks),'checks':checks}
REPORT.write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps(report,indent=2,ensure_ascii=False))
raise SystemExit(1 if report['fail'] else 0)

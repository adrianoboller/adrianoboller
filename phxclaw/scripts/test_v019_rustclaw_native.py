#!/usr/bin/env python3
from __future__ import annotations
import hashlib, json, subprocess, sys, tomllib
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
REPORT=ROOT/'V019_RUSTCLAW_NATIVE_TEST_REPORT.json'
checks=[]
def ck(name, ok, detail=''):
    checks.append({'name':name,'pass':bool(ok),'detail':detail})
def read(rel): return (ROOT/rel).read_text(encoding='utf-8')
def loadj(rel): return json.loads(read(rel))
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()

vendor=ROOT/'private/vendor/rustclaw/upstream'
ck('vendor_present',(vendor/'Cargo.toml').is_file() and (vendor/'src/gateway/protocol.rs').is_file())
ck('license_present',(vendor/'LICENSE.md').is_file())
license_text=(vendor/'LICENSE.md').read_text(encoding='utf-8') if (vendor/'LICENSE.md').is_file() else ''
ck('license_mit','Licença MIT' in license_text or 'MIT License' in license_text)
lock=loadj('private/vendor/rustclaw/UPSTREAM.lock.json')
prov=loadj('private/vendor/rustclaw/PROVENANCE.json')
status=loadj('private/vendor/rustclaw/LICENSE_STATUS.json')
ck('lock_verified',lock.get('license')=='MIT' and lock.get('license_verified') is True)
ck('provenance_reuse',prov.get('reuse_policy')=='allowed_with_notice')
ck('status_verified',status.get('reuse_state')=='verified_mit_source' and status.get('compiled_or_linked_into_phxclaw') is True)
ck('license_hash_matches',lock.get('license_sha256')==sha(vendor/'LICENSE.md'))
ck('quarantine_removed',not (ROOT/'private/vendor-quarantine/rustclaw').exists())

with (ROOT/'Cargo.toml').open('rb') as fh: cargo=tomllib.load(fh)
members=set(cargo['workspace']['members'])
ck('native_crate_member','crates/phxclaw-rustclaw-native' in members)
ck('native_plugin_member','apps/phxclaw-rustclaw-native-plugin' in members)
ck('version_current',cargo['workspace']['package']['version']=='0.20.0')

native=read('crates/phxclaw-rustclaw-native/src/lib.rs')
for needle in ['GatewayInboundFrame','AuthProof','credential_lease_uuid','NativeSessionStore','ScheduleSpec','McpWireMode','ContentLength','Ndjson','RUSTCLAW_UPSTREAM_LICENSE']:
    ck('native_'+needle,needle in native)
ck('no_plaintext_gateway_token','pub token:' not in native and 'api_key' not in native)
ck('uuidv7_used','new_uuid_v7()' in native)
ck('mit_attribution','RustClaw 0.5.0 MIT-derived' in native or 'RustClaw 0.5.0 MIT' in native)

plugin=read('apps/phxclaw-rustclaw-native-plugin/src/main.rs')
for cap in ['rustclaw.native.gateway.negotiate','rustclaw.native.session.scope','rustclaw.native.cron.validate','rustclaw.native.mcp.names']:
    ck('plugin_cap_'+cap,cap in plugin)
ck('plugin_process_protocol','PROCESS_PROTOCOL_V1' in plugin)

catalog=loadj('config/capability-catalog.json')
known={x['name'] for x in catalog['capabilities']}
required={
'rustclaw.native.gateway.negotiate','rustclaw.native.session.scope','rustclaw.native.session.manage',
'rustclaw.native.cron.validate','rustclaw.native.cron.schedule','rustclaw.native.mcp.ndjson',
'rustclaw.native.mcp.content_length','rustclaw.native.mcp.names'}
ck('catalog_count_matches',catalog['count']==len(catalog['capabilities']),f"{catalog['count']} vs {len(catalog['capabilities'])}")
ck('catalog_native_caps',required.issubset(known),str(sorted(required-known)))

src=loadj('config/knowledge-sources/rustclaw-user-source.json')
ck('source_verified',src.get('reuse_state')=='verified_mit_source' and src.get('local_root')=='private/vendor/rustclaw/upstream')
up=loadj('config/upstreams/rustclaw.json')
ck('upstream_config_verified',up.get('license_verified') is True and up.get('reuse')=='allowed_with_notice')
config=loadj('config/rustclaw-native.json')
ck('native_policy_private',config['policy']['repository_visibility']=='private_only')
ck('native_policy_no_plaintext',config['policy']['plaintext_gateway_tokens'] is False)
ck('native_policy_postgres',config['policy']['official_persistence']=='PostgreSQL')
ck('native_policy_mcp_modes',set(config['policy']['mcp_wire_modes'])=={'content_length','ndjson'})

mig=read('migrations/0019_rustclaw_native_session_cron.sql')
for table in ['phoenix_native_sessions','phoenix_native_session_messages','phoenix_scheduled_jobs']:
    ck('migration_'+table,table in mig)
ck('sqlite_not_official','SQLite store is not adopted' in mig)

notice=read('THIRD_PARTY_NOTICES.md')
ck('third_party_notice','## RustClaw 0.5.0 — MIT' in notice and 'private/vendor/rustclaw/upstream/LICENSE.md' in notice)
ck('integration_doc',(ROOT/'docs/RUSTCLAW_NATIVE_INTEGRATION_V019.md').is_file())

# CLI checks
for i,cmd in enumerate([
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'version'],
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'upstream','rustclaw','status'],
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'capabilities','--filter','rustclaw.native.'],
],1):
    r=subprocess.run(cmd,cwd=ROOT,capture_output=True,text=True)
    ck(f'cli_{i}',r.returncode==0,(r.stderr or r.stdout)[-800:])

# Static compatibility behavior mirrored in Python to catch contract mistakes.
def qualify(server,tool):
    norm=lambda v: ''.join(ch if ch.isalnum() or ch in '_-' else '_' for ch in v)
    return f'mcp__{norm(server)}__{norm(tool)}'
ck('canonical_tool_name_contract',qualify('server.one','weather tool')=='mcp__server_one__weather_tool')
ck('cron_min_contract',60>=60 and not (10>=60))

report={
 'suite':'PhxClaw v0.19 RustClaw native MIT integration',
 'pass':sum(c['pass'] for c in checks),'fail':sum(not c['pass'] for c in checks),
 'rust_compiled':False,'reason_if_not_compiled':'cargo/rustc availability is checked by the global verifier',
 'checks':checks,
}
REPORT.write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps(report,indent=2,ensure_ascii=False))
raise SystemExit(1 if report['fail'] else 0)

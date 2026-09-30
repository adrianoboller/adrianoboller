#!/usr/bin/env python3
from __future__ import annotations
import hashlib, json, subprocess, sys, tomllib
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
checks=[]
def ck(name, ok, detail=''):
    checks.append({'name':name,'pass':bool(ok),'detail':detail})
def read(rel): return (ROOT/rel).read_text(encoding='utf-8')
def loadj(rel): return json.loads(read(rel))
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()

# openclaw-rs source is vendored and license verified from supplied archive
oc=ROOT/'private/vendor/openclaw-rs/upstream'
ck('openclaw_vendor_present', (oc/'Cargo.toml').is_file())
ck('openclaw_license_present', (oc/'LICENSE').is_file())
ck('openclaw_license_mit', 'MIT License' in (oc/'LICENSE').read_text(encoding='utf-8'))
with (oc/'Cargo.toml').open('rb') as fh: oc_cargo=tomllib.load(fh)
ck('openclaw_workspace_version', oc_cargo['workspace']['package']['version']=='0.1.0')
for rel in [
 'crates/openclaw-channels/src/traits.rs','crates/openclaw-channels/src/allowlist.rs',
 'crates/openclaw-providers/src/traits.rs','crates/openclaw-plugins/src/api.rs',
 'crates/openclaw-core/src/secrets/mod.rs','crates/openclaw-core/src/events/mod.rs',
 'crates/openclaw-ipc/src/transport.rs']:
    ck('openclaw_component_'+rel.replace('/','_'), (oc/rel).is_file())
lock=loadj('private/vendor/openclaw-rs/UPSTREAM.lock.json')
ck('openclaw_lock_license', lock.get('license')=='MIT')
ck('openclaw_lock_hash', len(lock.get('archive_sha256',''))==64)

# RustClaw source is now verified MIT and vendored from the supplied archive
rc=ROOT/'private/vendor/rustclaw/upstream'
ck('rustclaw_vendor_present', (rc/'Cargo.toml').is_file() and (rc/'README.md').is_file())
ck('rustclaw_license_present', (rc/'LICENSE.md').is_file())
ck('rustclaw_license_mit', 'Licença MIT' in (rc/'LICENSE.md').read_text(encoding='utf-8') or 'MIT License' in (rc/'LICENSE.md').read_text(encoding='utf-8'))
status=loadj('private/vendor/rustclaw/LICENSE_STATUS.json')
ck('rustclaw_verified_state', status.get('reuse_state')=='verified_mit_source')
ck('rustclaw_native_adopted', status.get('compiled_or_linked_into_phxclaw') is True)

cargo=read('Cargo.toml')
ck('openclaw_bridge_workspace', 'crates/phxclaw-openclaw-rs-bridge' in cargo and 'apps/phxclaw-openclaw-rs-plugin' in cargo)
ck('rustclaw_bridge_workspace', 'crates/phxclaw-rustclaw-bridge' in cargo and 'apps/phxclaw-rustclaw-plugin' in cargo)
ck('rustclaw_vendor_not_direct_workspace_member', 'private/vendor/rustclaw/upstream' not in cargo)
ck('rustclaw_native_workspace_member', 'crates/phxclaw-rustclaw-native' in cargo)
ck('openclaw_source_not_workspace_member', 'private/vendor/openclaw-rs/upstream' not in cargo)

oc_bridge=read('crates/phxclaw-openclaw-rs-bridge/src/lib.rs')
for cap in ['openclaw_rs.health','openclaw_rs.doctor','openclaw_rs.status','openclaw_rs.gateway.status','openclaw_rs.channels.list','openclaw_rs.channels.probe','openclaw_rs.config.show','openclaw_rs.config.validate']:
    ck('openclaw_cap_'+cap, cap in oc_bridge)
ck('openclaw_workspace_confinement', 'resolved.starts_with(&root)' in oc_bridge)

rc_bridge=read('crates/phxclaw-rustclaw-bridge/src/lib.rs')
for cap in ['rustclaw.health','rustclaw.status','rustclaw.github.scan','rustclaw.agent.prompt']:
    ck('rustclaw_cap_'+cap, cap in rc_bridge)
ck('rustclaw_prompt_argv_fail_closed', 'PromptArgvDenied' in rc_bridge and 'PHXCLAW_RUSTCLAW_ALLOW_PROMPT_ARGV' in rc_bridge)
ck('rustclaw_args_redacted', '[REDACTED]' in rc_bridge and 'args_redacted' in rc_bridge)

channel=read('crates/phxclaw-channel-gateway/src/lib.rs')
for needle in ['ChannelCapabilities','ChannelProbe','ChannelAllowlist','ChannelAllowlistEntry','ChannelRouteRule','ChannelAgentRouter','ChannelProviderV2']:
    ck('channel_'+needle, needle in channel)

sdk=read('crates/phxclaw-plugin-sdk/src/lib.rs')
for needle in ['ExtensionRuntimeKind','ExtensionHook','BeforeMessage','AfterMessage','BeforeToolCall','AfterToolCall','SessionStart','SessionEnd','AgentResponse','HookInvocation']:
    ck('sdk_'+needle, needle in sdk)
ck('sdk_version_current','PLUGIN_SDK_VERSION: &str = "0.20.0"' in sdk)

catalog=loadj('config/capability-catalog.json')
known={x['name'] for x in catalog['capabilities']}
required={
 'knowledge.openclaw_rs.read','knowledge.rustclaw.read','openclaw_rs.health','openclaw_rs.doctor','openclaw_rs.status',
 'openclaw_rs.gateway.status','openclaw_rs.channels.list','openclaw_rs.channels.probe','openclaw_rs.config.show','openclaw_rs.config.validate',
 'rustclaw.health','rustclaw.status','rustclaw.github.scan','rustclaw.agent.prompt','channel.provider.probe','channel.allowlist.evaluate','channel.agent.route','plugin.hook.invoke'
}
ck('catalog_count_matches', catalog['count']==len(catalog['capabilities']), f"{catalog['count']} vs {len(catalog['capabilities'])}")
ck('catalog_required_caps', required.issubset(known), str(sorted(required-known)))

oc_source=loadj('config/knowledge-sources/openclaw-rs-user-source.json')
rc_source=loadj('config/knowledge-sources/rustclaw-user-source.json')
ck('openclaw_source_authoritative', oc_source.get('authoritative') is True and 'MIT' in oc_source.get('license_note',''))
ck('rustclaw_source_verified_mit', rc_source.get('authoritative') is False and rc_source.get('reuse_state')=='verified_mit_source' and 'MIT' in rc_source.get('license_note',''))

research=loadj('config/agents/020-research-agent.agent.json')
doc=loadj('config/agents/018-documentador.agent.json')
for label,a in [('research',research),('documenter',doc)]:
    ck(label+'_openclaw_source', 'openclaw-rs-user-source' in a.get('knowledge_sources',[]) and 'knowledge.openclaw_rs.read' in a.get('capabilities',[]))
    ck(label+'_rustclaw_source', 'rustclaw-user-source' in a.get('knowledge_sources',[]) and 'knowledge.rustclaw.read' in a.get('capabilities',[]))

for rel in ['scripts/vendor_openclaw_rs.sh','scripts/vendor_openclaw_rs.ps1','scripts/vendor_rustclaw.sh','scripts/vendor_rustclaw.ps1','docs/UPLOADED_RUST_SOURCES_AUDIT_V017.md']:
    ck('exists_'+rel.replace('/','_'), (ROOT/rel).is_file())

# CLI smoke for source visibility
for i,cmd in enumerate([
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'version'],
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'upstream','openclaw-rs','status'],
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'upstream','rustclaw','status'],
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'capabilities','--filter','openclaw_rs.'],
    [sys.executable,str(ROOT/'scripts/phx_cli.py'),'capabilities','--filter','rustclaw.'],
],1):
    r=subprocess.run(cmd,cwd=ROOT,capture_output=True,text=True)
    ck(f'cli_smoke_{i}', r.returncode==0, (r.stderr or r.stdout)[-500:])

report={
 'suite':'PhxClaw v0.17 uploaded Rust source integration',
 'pass':sum(c['pass'] for c in checks),
 'fail':sum(not c['pass'] for c in checks),
 'openclaw_rs_vendored':True,
 'rustclaw_quarantined':False,
 'details':checks,
}
(ROOT/'V017_UPLOADED_SOURCES_TEST_REPORT.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps(report,indent=2,ensure_ascii=False))
raise SystemExit(1 if report['fail'] else 0)

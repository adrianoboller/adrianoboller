#!/usr/bin/env python3
from pathlib import Path
import base64, hashlib, json, sys, tomllib, uuid
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

HERE = Path(__file__).resolve()
PKG = HERE.parents[1]
ROOT = PKG / 'overlay' if (PKG / 'overlay/capabilities/V030_CAPABILITIES_DELTA.json').is_file() else PKG
REPORT_DIR = PKG / 'reports' if (PKG / 'overlay').exists() else ROOT / 'reports'
checks = []

def ck(name, ok, detail=''):
    checks.append({'name': name, 'pass': bool(ok), 'detail': detail})
    print(('PASS' if ok else 'FAIL'), name, detail)

cap = json.loads((ROOT / 'capabilities/V030_CAPABILITIES_DELTA.json').read_text())
ck('cap_delta', cap['delta_count'] == 37)
ck('cap_total', cap['projected_total'] == 367)
ck('cap_unique', len(cap['capabilities']) == len(set(cap['capabilities'])) == 37)

for c in ['phxclaw-ha-control-plane','phxclaw-obsidian-bridge','phxclaw-hermes-bridge','phxclaw-ollama-provider']:
    p = ROOT / f'crates/{c}/Cargo.toml'
    ck('crate_' + c, p.is_file())
    if p.is_file():
        try:
            tomllib.loads(p.read_text())
            ck('toml_' + c, True)
        except Exception as e:
            ck('toml_' + c, False, str(e))

ha = (ROOT / 'crates/phxclaw-ha-control-plane/src/lib.rs').read_text()
ck('ha_epoch', 'leader_epoch: u64' in ha and 'epoch: u64' in ha)
ck('ha_chrono_duration', 'renew_deadline(&self, renew_before: ChronoDuration)' in ha)
ck('ha_self_fence', 'must_self_fence' in ha and 'PartitionSelfFence' in ha)
ck('ha_rendezvous', 'assign_shard' in ha and 'score.as_slice() > b.as_slice()' in ha)
ck('ha_hash_fence', 'source_state_sha256' in ha and 'InvalidSha256' in ha)

sql = (ROOT / 'migrations/0030_distributed_control_plane.sql').read_text()
ck('sql_db_clock', 'clock_timestamp()' in sql)
ck('sql_advisory_lock', 'pg_advisory_xact_lock' in sql)
ck('sql_force_rls', 'FORCE ROW LEVEL SECURITY' in sql and 'phxclaw_controller_events' in sql)
ck('sql_fence_fn', 'phxclaw_assert_controller_fence' in sql)
ck('sql_commit_fenced', 'phxclaw_commit_reconcile_intent' in sql and 'IF NOT phxclaw_assert_controller_fence' in sql)
ck('sql_first_epoch', 'VALUES(p_tenant,p_lease,p_controller,1,n' in sql)
ck('sql_rls_setting', "current_setting(''phxclaw.tenant_uuid'', true)" in sql and "nullif(" in sql)
ck('sql_append_only', 'controller events are append-only' in sql)
ck('sql_active_owner_no_bump', 'active owner must use renew' in sql)

obs = (ROOT / 'crates/phxclaw-obsidian-bridge/src/lib.rs').read_text()
ck('obs_json_canvas', 'JsonCanvas' in obs)
ck('obs_import_unverified', 'stage_import' in obs and 'unverified_context' in obs)
ck('obs_path_guard', 'UnsafePath' in obs and 'safe_relative_path' in obs)

hermes = (ROOT / 'crates/phxclaw-hermes-bridge/src/lib.rs').read_text()
ck('hermes_f24', 'f24_candidate' in hermes)
ck('hermes_memory_unverified', 'observe_memory' in hermes and 'unverified_context' in hermes)
ck('hermes_readonly', 'ReadIssue' in hermes and 'ApprovalRequired' in hermes)
ck('hermes_no_secret', 'secret_import_allowed() -> bool { false }' in hermes)
ck('hermes_mcp_pinned', 'UnpinnedMcp' in hermes)
ck('hermes_cron', 'translate_cron' in hermes)
ck('hermes_subagent', 'translate_subagent' in hermes)

oll = (ROOT / 'crates/phxclaw-ollama-provider/src/lib.rs').read_text()
ck('ollama_loopback', '127.0.0.1:11434' in oll)
ck('ollama_scheme_guard', 'matches!(u.scheme(), "http" | "https")' in oll)
ck('ollama_userinfo_guard', 'u.username().is_empty()' in oll)
ck('ollama_chat', '/api/chat' in oll)
ck('ollama_embed', '/api/embed' in oll)
ck('ollama_openai', '/v1/chat/completions' in oll)
ck('ollama_anthropic', '/v1/messages' in oll)
ck('ollama_pull_approval', 'PullApprovalRequired' in oll)

plugin = ROOT / 'integrations/obsidian-phxclaw/src/main.ts'
ck('obsidian_plugin', plugin.is_file() and 'content_sha256' in plugin.read_text() and "'.phxclaw/inbox'" in plugin.read_text())
pkg = json.loads((ROOT / 'integrations/obsidian-phxclaw/package.json').read_text())
ck('obsidian_api_pin', pkg['devDependencies']['obsidian'] == '1.13.2')
ck('obsidian_plugin_license', (ROOT / 'integrations/obsidian-phxclaw/LICENSE').is_file())

for d in sorted((ROOT / 'plugins').iterdir()):
    if not d.is_dir():
        continue
    name = d.name
    m = json.loads((d / 'phxclaw.plugin.json').read_text())
    integ = json.loads((d / 'SOURCE_INTEGRITY.json').read_text())
    try:
        u = uuid.UUID(m['plugin_uuid'])
        uuid_ok = u.version == 7 and u.variant == uuid.RFC_4122
    except Exception:
        uuid_ok = False
    ck('plugin_uuidv7_' + name, uuid_ok)
    ck('plugin_semver_' + name, m.get('version') == '0.30.0')
    ck('plugin_governance_' + name, all(k in m for k in ['capabilities','permissions','lifecycle_hooks','provenance','rollback','release_requirements']))
    payload = json.dumps(m, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
    digest = hashlib.sha256(payload).hexdigest()
    ck('plugin_digest_' + name, digest == integ.get('canonical_manifest_sha256'))
    try:
        pub = base64.b64decode(integ['signature']['public_key_b64'])
        sig = base64.b64decode(integ['signature']['signature_b64'])
        Ed25519PublicKey.from_public_bytes(pub).verify(sig, payload)
        sig_ok = True
    except Exception:
        sig_ok = False
    ck('plugin_ed25519_' + name, sig_ok and integ.get('private_key_embedded') is False and integ.get('production_release_requires_resign') is True)

notice_path = PKG / 'THIRD_PARTY_NOTICES_DELTA.md' if (PKG / 'THIRD_PARTY_NOTICES_DELTA.md').exists() else ROOT / 'docs/THIRD_PARTY_NOTICES_V030.md'
notice = notice_path.read_text() if notice_path.exists() else ''
ck('notice_obs_sample_0bsd', 'obsidian-sample-plugin` — 0BSD' in notice)
ck('notice_no_vendor', 'No third-party source tree is vendored' in notice)

report = {'suite': 'PhxClaw v0.30 static verifier', 'pass': sum(x['pass'] for x in checks), 'fail': sum(not x['pass'] for x in checks), 'checks': checks}
REPORT_DIR.mkdir(parents=True, exist_ok=True)
(REPORT_DIR / 'V030_STATIC_VERIFY_REPORT.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'pass': report['pass'], 'fail': report['fail']}))
sys.exit(1 if report['fail'] else 0)

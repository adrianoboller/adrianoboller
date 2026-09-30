#!/usr/bin/env python3
from __future__ import annotations
import json, os, re, shutil, subprocess, sys
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
CLI=ROOT/'bin/phx'
REPORT=ROOT/'V018_SECRET_CHANNEL_TEST_REPORT.json'
checks=[]

def ck(name, ok, detail=''):
    checks.append({'name':name,'ok':bool(ok),'detail':detail})
    print(('PASS' if ok else 'FAIL'), name, detail)

def run(args, stdin=None, expect=0):
    p=subprocess.run([str(CLI),*args],cwd=ROOT,input=stdin,text=True,capture_output=True)
    ck('cmd_'+'_'.join(args[:2]).replace('/','_'),p.returncode==expect,f'rc={p.returncode}')
    if p.returncode!=expect:
        print(p.stdout); print(p.stderr,file=sys.stderr)
    return p

# isolate bootstrap state
sec=ROOT/'var/secrets'
backup=ROOT/'var/secrets.v018-test-backup'
if backup.exists(): shutil.rmtree(backup)
if sec.exists(): sec.rename(backup)
try:
    p=run(['secrets','init'])
    ck('secret_master_key_created',(sec/'master.key').is_file())
    token1='telegram-test-token-DO-NOT-LOG-123456'
    p=run(['secrets','store','telegram-bot','--namespace','channels','--scope','channel:telegram:send','--scope','channel:telegram:probe'],stdin=token1+'\n')
    ck('secret_not_in_store_stdout',token1 not in (p.stdout+p.stderr))
    m=re.search(r'STORED\s+([0-9a-f-]{36})',p.stdout)
    uid=m.group(1) if m else ''
    ck('secret_uuidv7',bool(uid) and uid.split('-')[2].startswith('7'),uid)
    enc=sec/f'{uid}.phxsecret'
    ck('encrypted_file_exists',enc.is_file())
    if enc.exists(): ck('plaintext_absent_at_rest',token1.encode() not in enc.read_bytes())
    p=run(['secrets','show',uid]); ck('show_metadata_only',token1 not in p.stdout and 'sha256' in p.stdout)
    p=run(['secrets','doctor',uid]); ck('doctor_redacted',token1 not in p.stdout and 'decrypt=PASS' in p.stdout)
    p=run(['secrets','lease',uid,'--consumer','channel.provider.telegram','--scope','channel:telegram:send','--ttl','30'])
    lease=json.loads(p.stdout) if p.returncode==0 else {}
    ck('lease_uuidv7',str(lease.get('uuid','')).split('-')[2].startswith('7') if lease else False)
    p=subprocess.run([str(CLI),'secrets','lease',uid,'--consumer','bad','--scope','channel:discord:send'],cwd=ROOT,text=True,capture_output=True)
    ck('scope_denied',p.returncode!=0 and 'scope denied' in (p.stderr+p.stdout).lower())
    token2='telegram-rotated-token-DO-NOT-LOG-987654'
    p=run(['secrets','rotate',uid],stdin=token2+'\n')
    ck('rotate_redacted',token2 not in (p.stdout+p.stderr))
    if enc.exists():
        b=enc.read_bytes(); ck('rotated_plaintext_absent',token1.encode() not in b and token2.encode() not in b)
    run(['secrets','revoke',uid])
    p=subprocess.run([str(CLI),'secrets','lease',uid,'--consumer','channel.provider.telegram','--scope','channel:telegram:send'],cwd=ROOT,text=True,capture_output=True)
    ck('revoked_secret_denies_new_lease',p.returncode!=0 and 'revoked' in (p.stderr+p.stdout).lower())

    # Static/contract checks for Rust implementation and F21 provider wiring.
    broker=(ROOT/'crates/phxclaw-secret-broker/src/lib.rs').read_text()
    providers=(ROOT/'crates/phxclaw-channel-providers/src/lib.rs').read_text()
    gateway=(ROOT/'crates/phxclaw-channel-gateway/src/lib.rs').read_text()
    cfg=json.loads((ROOT/'config/channel-providers.json').read_text())
    caps=json.loads((ROOT/'config/capability-catalog.json').read_text())
    names={x['name'] for x in caps['capabilities']}
    ck('rust_aes256_gcm','Aes256Gcm' in broker)
    ck('rust_secret_redacted_debug','SecretValue([REDACTED])' in broker)
    ck('rust_lease_expiry','expires_at' in broker and 'LeaseInactive' in broker)
    ck('rust_rotation_revocation','pub fn rotate' in broker and 'pub fn revoke_secret' in broker)
    ck('telegram_secret_lease','channel:telegram:send' in providers and 'issue_lease' in providers)
    ck('discord_secret_lease','channel:discord:send' in providers and 'issue_lease' in providers)
    ck('provider_origin_allowlist','allowed_origins' in providers and 'provider origin not allowed' in providers)
    ck('telegram_api_shape','sendMessage' in providers and 'getMe' in providers)
    ck('discord_api_shape','/users/@me' in providers and '/messages' in providers)
    ck('gateway_provider_v2','trait ChannelProviderV2' in gateway)
    ck('provider_config_no_tokens',all('token' not in k.lower() or k=='token_source' for item in cfg['providers'] for k in item.keys()))
    required={'secret.store','secret.rotate','secret.revoke','secret.lease.issue','secret.resolve','secret.redact','channel.telegram.send','channel.telegram.probe','channel.discord.send','channel.discord.probe'}
    ck('capability_catalog_f23_f21',required.issubset(names),str(required-names))
    f23=json.loads((ROOT/'sprints/f23.json').read_text()); ck('f23_review',f23['status']=='review')
    ck('migration_0018',(ROOT/'migrations/0018_secret_broker_and_channel_providers.sql').is_file())
finally:
    if sec.exists(): shutil.rmtree(sec)
    if backup.exists(): backup.rename(sec)

passed=sum(x['ok'] for x in checks); failed=len(checks)-passed
report={'suite':'PhxClaw v0.18 F23 Secret Broker + F21 providers','pass':passed,'fail':failed,'checks':checks}
REPORT.write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n')
print(f'PASS {passed} FAIL {failed}')
raise SystemExit(1 if failed else 0)

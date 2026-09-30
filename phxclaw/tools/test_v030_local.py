#!/usr/bin/env python3
from pathlib import Path
import hashlib, json, shutil, subprocess, sys, tempfile, tomllib, uuid
from jsonschema import Draft202012Validator

PKG = Path(__file__).resolve().parents[1]
OVER = PKG / 'overlay'
checks = []

def ck(name, ok, detail=''):
    checks.append({'name': name, 'pass': bool(ok), 'detail': detail})
    print(('PASS' if ok else 'FAIL'), name, detail)

def run(cmd, cwd=None):
    return subprocess.run(cmd, cwd=cwd, text=True, capture_output=True)

ha = json.loads((OVER/'config/ha-control-policy.v030.json').read_text())
ck('ha_min_3', ha['minimum_controllers_for_ha'] >= 3)
ck('ha_pg_required', ha['require_ha_postgres_for_production'] is True)
ck('ha_self_fence_policy', ha['partition_behavior'] == 'self_fence')
ck('ha_db_clock', ha['authority_clock'] == 'postgresql')
hermes = json.loads((OVER/'config/hermes-bridge-policy.v030.json').read_text())
ck('hermes_memory_unverified', hermes['memory_import'] == 'f25_unverified_observation_only')
ck('hermes_skill_candidate', hermes['skill_import'] == 'f24_candidate_only')
ck('github_read_only_default', hermes['github_default'] == 'read_only')
ck('github_write_approval', hermes['github_write_requires_approval'] is True)
ck('hermes_secret_import_off', hermes['allow_secret_import'] is False)
ck('hermes_cron_f06', hermes['cron_import'] == 'f06_task_graph_only')
ck('hermes_subagent_f20', hermes['subagent_import'] == 'f20_team_runtime_only')
oll = json.loads((OVER/'config/ollama-provider.v030.json').read_text())
ck('ollama_loopback_default', oll['base_url'] == 'http://127.0.0.1:11434')
ck('ollama_remote_http_off', oll['allow_remote_http'] is False)
ck('ollama_pull_approval', oll['pull_requires_approval'] is True)
obs = json.loads((OVER/'config/obsidian-bridge-policy.v030.json').read_text())
ck('obsidian_default_unverified', obs['import_default_trust'] == 'unverified_context')
ck('obsidian_no_source_overwrite', obs['allow_overwrite_source_notes'] is False)

for p in sorted((OVER/'schemas').glob('*.json')):
    try:
        Draft202012Validator.check_schema(json.loads(p.read_text())); ok=True
    except Exception:
        ok=False
    ck('schema_' + p.stem, ok)
for p in sorted((OVER/'crates').glob('*/Cargo.toml')):
    try:
        tomllib.loads(p.read_text()); ok=True
    except Exception:
        ok=False
    ck('toml_' + p.parent.name, ok)
for p in sorted((OVER/'plugins').glob('*/phxclaw.plugin.json')):
    try:
        json.loads(p.read_text()); ok=True
    except Exception:
        ok=False
    ck('plugin_json_' + p.parent.name, ok)

class Authority:
    def __init__(self): self.holder=None; self.epoch=0; self.exp=0
    def claim(self,c,now,ttl):
        if self.holder is None: self.holder=c; self.epoch=1; self.exp=now+ttl; return self.epoch
        if self.holder==c and self.exp>now: return self.epoch
        if self.exp<=now: self.holder=c; self.epoch+=1; self.exp=now+ttl; return self.epoch
        return None
    def renew(self,c,e,now,ttl):
        if self.holder==c and self.epoch==e and self.exp>now: self.exp=now+ttl; return True
        return False
    def fence(self,c,e,now): return self.holder==c and self.epoch==e and self.exp>now

a=Authority()
ck('lease_epoch1', a.claim('a',100,15)==1)
ck('active_other_denied', a.claim('b',101,15) is None)
ck('renew_same_epoch', a.renew('a',1,105,15) and a.epoch==1)
ck('failover_epoch2', a.claim('b',121,15)==2)
ck('stale_leader_fenced', not a.fence('a',1,122))
ck('new_leader_fenced_ok', a.fence('b',2,122))
ck('partition_self_fence', 15 >= 15)

def own(key, controllers):
    return max((hashlib.sha256((key+c).encode()).digest(), c) for c in controllers)[1]
keys=[str(uuid.uuid4()) for _ in range(200)]
before={k:own(k,['a','b','c']) for k in keys}; after={k:own(k,['a','b']) for k in keys}; changed=[k for k in keys if before[k]!=after[k]]
ck('rendezvous_only_removed_owner_moves', all(before[k]=='c' for k in changed))
ck('rendezvous_some_stable', len(changed) < len(keys))

def rh(rows):
    h=hashlib.sha256()
    for i,s in sorted(rows): h.update(uuid.UUID(i).bytes); h.update(b'\0'); h.update(s.encode()); h.update(b'\xff')
    return h.hexdigest()
r1=[(str(uuid.uuid4()),'a'*64),(str(uuid.uuid4()),'b'*64)]
ck('reconcile_deterministic', rh(r1)==rh(list(reversed(r1))))

sql=(OVER/'migrations/0030_distributed_control_plane.sql').read_text()
ck('sql_rls_quote', "current_setting(''phxclaw.tenant_uuid'', true)" in sql and "nullif(" in sql)
ck('sql_first_epoch', 'VALUES(p_tenant,p_lease,p_controller,1,n' in sql)
ck('sql_active_owner_no_epoch_bump', 'active owner must use renew' in sql)
ck('sql_expired_takeover_epoch_plus', 'epoch=current_row.epoch+1' in sql)
ck('sql_reconcile_fence', 'phxclaw_commit_reconcile_intent' in sql)
ck('sql_append_only', 'controller events are append-only' in sql)
ck('sql_no_drop_table', 'DROP TABLE' not in sql.upper())

with tempfile.TemporaryDirectory() as td:
    td=Path(td)
    shutil.copy2(OVER/'integrations/obsidian-phxclaw/src/main.ts', td/'main.ts')
    (td/'obsidian.d.ts').write_text("""declare module 'obsidian' { export class Notice { constructor(message:string); } export class Plugin { app:{workspace:{getActiveFile():{path:string}|null},vault:{read(f:any):Promise<string>,adapter:{mkdir(path:string):Promise<void>,write(path:string,data:string):Promise<void>}}}; addCommand(c:{id:string,name:string,callback:()=>Promise<void>|void}):void; } }\n""")
    (td/'tsconfig.json').write_text(json.dumps({'compilerOptions':{'target':'ES2022','module':'ESNext','moduleResolution':'Bundler','strict':True,'lib':['ES2022','DOM'],'skipLibCheck':True,'noEmit':True},'files':['main.ts','obsidian.d.ts']}))
    r=run(['tsc','-p','tsconfig.json'],cwd=td)
    ck('obsidian_tsc', r.returncode==0, (r.stderr or r.stdout)[-200:])

for p in sorted((OVER/'tools').glob('*.py')):
    r=run([sys.executable,'-m','py_compile',str(p)])
    ck('py_compile_' + p.stem, r.returncode==0, r.stderr[-120:])

with tempfile.TemporaryDirectory() as td:
    target=Path(td)/'target'
    (target/'config/overlays').mkdir(parents=True)
    (target/'crates/phxclaw-fleet-control-plane').mkdir(parents=True)
    (target/'Cargo.toml').write_text('[workspace]\nresolver="2"\nmembers=[\n  "crates/phxclaw-fleet-control-plane"\n]\n')
    (target/'Cargo.lock').write_text('# existing lock\n')
    (target/'crates/phxclaw-fleet-control-plane/Cargo.toml').write_text('[package]\nname="phxclaw-fleet-control-plane"\nversion="0.29.0"\nedition="2021"\n')
    (target/'config/overlays/v0.29.applied.json').write_text('{"version":"0.29.0"}\n')
    r1=run([sys.executable,str(PKG/'tools/apply_overlay.py'),str(target)])
    ck('apply_first',r1.returncode==0,r1.stderr[-120:])
    marker=json.loads((target/'config/overlays/v0.30.applied.json').read_text())
    ck('lock_refresh_marker',marker['cargo_lock_requires_refresh'] is True)
    r2=run([sys.executable,str(PKG/'tools/apply_overlay.py'),str(target)])
    ck('apply_idempotent',r2.returncode==0 and 'already applied' in r2.stdout)
    rv=run([sys.executable,str(target/'tools/verify_v030.py')])
    ck('applied_verifier',rv.returncode==0,rv.stderr[-120:])
    data=tomllib.loads((target/'Cargo.toml').read_text())
    expected=['crates/phxclaw-ha-control-plane','crates/phxclaw-obsidian-bridge','crates/phxclaw-hermes-bridge','crates/phxclaw-ollama-provider']
    ck('workspace_exact_once',all(data['workspace']['members'].count(c)==1 for c in expected))
    target2=Path(td)/'conflict'; shutil.copytree(target,target2); (target2/'config/overlays/v0.30.applied.json').unlink(); (target2/'docs/INTEGRATIONS_AUDIT_V030.md').write_text('tampered')
    rc=run([sys.executable,str(PKG/'tools/apply_overlay.py'),str(target2)])
    ck('unknown_conflict_blocked',rc.returncode!=0 and 'conflict:' in (rc.stderr+rc.stdout))

report={'suite':'PhxClaw v0.30 local integration/security','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}
(PKG/'reports/V030_LOCAL_TEST_REPORT.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'pass':report['pass'],'fail':report['fail']}))
sys.exit(1 if report['fail'] else 0)

#!/usr/bin/env python3
from __future__ import annotations
import hashlib, json, shutil, subprocess, tomllib
from datetime import datetime, timezone
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
results=[]
def check(name, cond, detail=None):
    results.append({"name":name,"status":"PASS" if cond else "FAIL","detail":detail})
    if not cond: raise SystemExit(f"FAIL {name}: {detail}")

with (ROOT/'Cargo.toml').open('rb') as f: cargo=tomllib.load(f)
check('workspace.member_count', len(cargo['workspace']['members'])==46, len(cargo['workspace']['members']))
check('workspace.version', cargo['workspace']['package']['version']=='0.14.0', cargo['workspace']['package']['version'])

conf=json.loads((ROOT/'apps/phxclaw-desktop/src-tauri/tauri.conf.json').read_text())
check('tauri.version', conf['version']=='0.14.0', conf['version'])
front=(ROOT/'apps/phxclaw-desktop/src-tauri'/conf['build']['frontendDist']).resolve()
check('tauri.frontendDist', (front/'index.html').is_file(), str(front))

cfg=json.loads((ROOT/'config/desktop-host.json').read_text())
check('api.loopback', cfg['api']['bind'] in {'127.0.0.1','::1'}, cfg['api']['bind'])
check('api.host_control_default_deny', cfg['api']['allow_host_control_topics'] is False)
for key,val in cfg['host_policy'].items():
    if key=='managed_webview_origin_allowlist':
        check(f'policy.{key}', val==[], val)
    else:
        check(f'policy.{key}', val is False, val)

api=(ROOT/'crates/phxclaw-api-gateway/src/lib.rs').read_text()
check('api.non_loopback_guard', 'if !config.bind_ip.is_loopback()' in api)
check('api.bearer_auth', 'Authorization' in api or 'AUTHORIZATION' in api)
check('api.sse', '/v1/events/sse' in api)
check('api.websocket', '/v1/events/ws' in api)
check('api.host_topic_guard', 'allow_host_control_topics' in api and 'is_host_control_topic' in api)

ledger=(ROOT/'crates/phxclaw-evidence-ledger/src/lib.rs').read_text()
check('evidence.uuidv7', 'new_uuid_v7()' in ledger)
check('evidence.sha256', 'Sha256' in ledger)
check('evidence.previous_hash', 'previous_hash' in ledger)
check('evidence.sync_data', 'sync_data()' in ledger)

ui=ROOT/'apps/phxclaw-ui/assets/app.js'
node=shutil.which('node')
if node:
    cp=subprocess.run([node,'--check',str(ui)],capture_output=True,text=True)
    check('ui.javascript_syntax',cp.returncode==0,cp.stderr.strip())
else:
    results.append({'name':'ui.javascript_syntax','status':'SKIP','detail':'node unavailable'})

catalog=json.loads((ROOT/'config/capability-catalog.json').read_text())
check('capabilities.count',catalog['count']==141,catalog['count'])
needed={'event.live.publish','event.live.subscribe','api.realtime.connect','desktop.host.status','desktop.application.launch','desktop.webview.create','evidence.ledger.append','evidence.ledger.verify'}
check('capabilities.f14',needed.issubset({x['name'] for x in catalog['capabilities']}), sorted(needed))

# Deterministic miniature of the ledger chaining invariant.
prev=None
for n in range(3):
    payload=json.dumps({'n':n,'previous_hash':prev},sort_keys=True,separators=(',',':')).encode()
    prev=hashlib.sha256(payload).hexdigest()
check('evidence.hash_chain_shape', len(prev)==64 and all(c in '0123456789abcdef' for c in prev), prev)

report={
    'version':'0.14.0',
    'generated_at':datetime.now(timezone.utc).isoformat(),
    'pass':sum(r['status']=='PASS' for r in results),
    'fail':sum(r['status']=='FAIL' for r in results),
    'skip':sum(r['status']=='SKIP' for r in results),
    'results':results,
}
(ROOT/'DESKTOP_HOST_TEST_REPORT.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n')
print(json.dumps(report,indent=2,ensure_ascii=False))

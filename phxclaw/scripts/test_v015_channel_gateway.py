#!/usr/bin/env python3
from pathlib import Path
import json, subprocess, sys
ROOT=Path(__file__).resolve().parents[1]
checks=[]
def check(name, ok, detail=''):
    checks.append({'name':name,'pass':bool(ok),'detail':str(detail)}); print(('PASS' if ok else 'FAIL'),name)
# files/source contracts
for rel in ['crates/phxclaw-channel-gateway/src/lib.rs','schemas/channel-message.schema.json','schemas/channel-binding.schema.json','migrations/0017_channel_gateway.sql']:
    check('exists.'+rel,(ROOT/rel).is_file())
text=(ROOT/'crates/phxclaw-channel-gateway/src/lib.rs').read_text()
for token in ['IdentityBinding','ChannelSession','DuplicateMessage','IdentityBlocked','ChannelProvider','channel.inbound','channel.outbound','EvidenceDraft']:
    check('source.'+token,token in text)
r=subprocess.run([sys.executable,str(ROOT/'scripts/channel_gateway_bootstrap.py'),'demo'],cwd=ROOT,text=True,capture_output=True,timeout=20)
check('channel.demo',r.returncode==0 and '"fail": 0' in r.stdout,r.stdout+r.stderr)
sprint=json.loads((ROOT/'sprints/f21.json').read_text()); check('sprint.review',sprint.get('status')=='review')
cat=json.loads((ROOT/'config/capability-catalog.json').read_text()); names={x['name'] for x in cat['capabilities']}
for n in ['channel.identity.bind','channel.inbound.accept','channel.session.resolve','channel.outbound.prepare','channel.outbound.send','channel.provider.invoke']:
    check('capability.'+n,n in names)
report={'suite':'PhxClaw v0.15 F21 Channel Gateway','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'details':checks}
(ROOT/'V015_CHANNEL_GATEWAY_TEST_REPORT.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'PASS':report['pass'],'FAIL':report['fail']})); raise SystemExit(1 if report['fail'] else 0)

#!/usr/bin/env python3
from __future__ import annotations
import datetime as dt, json, os, secrets, subprocess, sys, time
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
PROTOCOL='phxclaw-process-v1'
RESULT=[]

def uuid7():
    ms=int(time.time()*1000)&((1<<48)-1); a=secrets.randbits(12); b=secrets.randbits(62)
    v=(ms<<80)|(0x7<<76)|(a<<64)|(0b10<<62)|b; h=f'{v:032x}'
    return f'{h[:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:]}'

def env(kind,payload=None):
    return {'protocol':PROTOCOL,'message_uuid':uuid7(),'kind':kind,'emitted_at':dt.datetime.now(dt.timezone.utc).isoformat(),'payload':payload or {}}

def run(tool,kind='health',payload=None,extra_env=None,timeout=15):
    e=os.environ.copy(); e.update(extra_env or {})
    p=subprocess.run([str(tool)],cwd=ROOT,input=json.dumps(env(kind,payload))+'\n',text=True,capture_output=True,timeout=timeout,env=e)
    if p.returncode!=0: raise RuntimeError(f'exit={p.returncode} stderr={p.stderr[-600:]}')
    line=p.stdout.strip().splitlines()[-1]; return json.loads(line)

def check(name,fn):
    t=time.monotonic()
    try:
        detail=fn(); RESULT.append({'name':name,'status':'PASS','elapsed_ms':round((time.monotonic()-t)*1000),'detail':detail})
    except Exception as ex:
        RESULT.append({'name':name,'status':'FAIL','elapsed_ms':round((time.monotonic()-t)*1000),'detail':str(ex)})

ART=ROOT/'plugins/builtin/artifacts'
for name in ['morpheus-agent','oracle-agent','master-orchestrator-agent','local-model-provider','office-document-tool','media-intelligence-tool','system-automation-tool']:
    tool=ART/name
    check('health.'+name, lambda tool=tool: run(tool).get('status'))

# Deterministic real process execution, explicitly enabled for this isolated smoke test.
def direct_command():
    obj=run(ART/'system-automation-tool','execute',{'capability':'system.command.execute','payload':{'shell':'direct','program':sys.executable,'args':['-c',"print('PHOENIX_V09_OK')"],'timeout_ms':5000}}, {'PHXCLAW_SYSTEM_AUTOMATION':'1'})
    if obj.get('status')!='ok' or 'PHOENIX_V09_OK' not in obj.get('payload',{}).get('stdout',''):
        raise RuntimeError(obj)
    return {'exit_code':obj['payload'].get('exit_code'),'stdout':obj['payload'].get('stdout','').strip()}
check('system.command.real_process',direct_command)

# Same command must be denied without the explicit capability env gate.
def denied_command():
    e={k:v for k,v in os.environ.items() if k!='PHXCLAW_SYSTEM_AUTOMATION'}
    p=subprocess.run([str(ART/'system-automation-tool')],cwd=ROOT,input=json.dumps(env('execute',{'capability':'system.command.execute','payload':{'shell':'direct','program':sys.executable,'args':['-c','print(1)']}}))+'\n',text=True,capture_output=True,timeout=10,env=e)
    obj=json.loads(p.stdout.strip().splitlines()[-1])
    if obj.get('status')!='rejected': raise RuntimeError(obj)
    return {'status':obj.get('status'),'code':(obj.get('error') or {}).get('code')}
check('system.command.fail_closed',denied_command)

report={'version':'0.9.0','generated_at':dt.datetime.now(dt.timezone.utc).isoformat(),'pass':sum(x['status']=='PASS' for x in RESULT),'fail':sum(x['status']=='FAIL' for x in RESULT),'skip':0,'results':RESULT}
(ROOT/'V09_SMOKE_TEST_REPORT.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n')
print(json.dumps(report,indent=2,ensure_ascii=False))
raise SystemExit(1 if report['fail'] else 0)

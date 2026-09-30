#!/usr/bin/env python3
from __future__ import annotations
import datetime as dt
import json
import os
import secrets
import stat
import subprocess
import tempfile
import time
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
PLUGIN=ROOT/'community/examples/olmocr/phxclaw-olmocr-plugin'
RESULT=[]
PROTOCOL='phxclaw-process-v1'

def uuid7():
    ms=int(time.time()*1000)&((1<<48)-1); a=secrets.randbits(12); b=secrets.randbits(62)
    v=(ms<<80)|(0x7<<76)|(a<<64)|(0b10<<62)|b; h=f'{v:032x}'
    return f'{h[:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:]}'

def env(kind,payload=None):
    return {'protocol':PROTOCOL,'message_uuid':uuid7(),'correlation_uuid':None,'kind':kind,'sent_at':dt.datetime.now(dt.timezone.utc).isoformat(),'payload':payload or {}}

def run(frame, extra_env=None):
    e=os.environ.copy(); e.update(extra_env or {})
    p=subprocess.run([str(PLUGIN)],cwd=ROOT,input=json.dumps(frame)+'\n',text=True,capture_output=True,timeout=20,env=e)
    if p.returncode!=0: raise RuntimeError(p.stderr)
    return json.loads(p.stdout.strip().splitlines()[-1])

def check(name, fn):
    t=time.monotonic()
    try: RESULT.append({'name':name,'status':'PASS','elapsed_ms':round((time.monotonic()-t)*1000),'detail':fn()})
    except Exception as ex: RESULT.append({'name':name,'status':'FAIL','elapsed_ms':round((time.monotonic()-t)*1000),'detail':str(ex)})

check('health.cli', lambda: json.loads(subprocess.check_output([str(PLUGIN),'--health'],text=True)).get('status'))
check('health.protocol', lambda: run(env('health')).get('status'))

def unsupported():
    obj=run(env('execute',{'capability':'ocr.unknown','payload':{}}))
    if obj['status']!='rejected': raise RuntimeError(obj)
    return obj['error']['code']
check('capability.fail_closed',unsupported)

def traversal():
    obj=run(env('execute',{'capability':'ocr.olmocr.extract','payload':{'input_path':'../../etc/passwd'}}),{'PHXCLAW_OLMOCR_BIN':'/bin/true','PHXCLAW_OLMOCR_LOCAL_MODEL':'/tmp'})
    if obj['status']!='rejected': raise RuntimeError(obj)
    return obj['error']['code']
check('path.traversal.denied',traversal)

def fake_backend():
    with tempfile.TemporaryDirectory(prefix='phx-olmocr-test-',dir=str(ROOT/'var/olmocr')) as td:
        td=Path(td); inp=ROOT/'var/documents/olmocr-adapter-test.pdf'; inp.write_bytes(b'%PDF-1.4\n% fake adapter fixture\n')
        model=td/'model'; model.mkdir()
        fake=td/'olmocr'; fake.write_text('''#!/usr/bin/env python3\nimport pathlib,sys\nargs=sys.argv[1:]\nws=pathlib.Path(args[0]); pdfs=args[args.index("--pdfs")+1:]\nout=ws/"markdown"; out.mkdir(parents=True,exist_ok=True)\nfor p in pdfs:\n    src=pathlib.Path(p)\n    (out/(src.stem+".md")).write_text("# Invoice\\n\\nTotal: 123.45\\n",encoding="utf-8")\n''')
        fake.chmod(fake.stat().st_mode | stat.S_IEXEC)
        obj=run(env('execute',{'capability':'ocr.olmocr.extract','payload':{'input_path':inp.name,'mode':'local','output_format':'markdown'}}),{
            'PHXCLAW_OLMOCR_BIN':str(fake),
            'PHXCLAW_OLMOCR_LOCAL_MODEL':str(model),
        })
        inp.unlink(missing_ok=True)
        if obj['status']!='ok': raise RuntimeError(obj)
        out=obj['payload']['outputs'][0]
        if 'Total: 123.45' not in (out.get('content') or ''): raise RuntimeError(out)
        if len(out.get('source_sha256',''))!=64 or len(out.get('output_sha256',''))!=64: raise RuntimeError(out)
        return {'job_uuid':obj['payload']['job_uuid'],'output_sha256':out['output_sha256']}
check('adapter.subprocess.contract',fake_backend)

report={'version':'0.10.0','component':'olmocr-community-plugin','generated_at':dt.datetime.now(dt.timezone.utc).isoformat(),'pass':sum(x['status']=='PASS' for x in RESULT),'fail':sum(x['status']=='FAIL' for x in RESULT),'skip':0,'results':RESULT,'note':'The subprocess test uses a deterministic test double for the upstream CLI. It validates the adapter contract, not real olmOCR model inference.'}
(ROOT/'OLMOCR_PLUGIN_TEST_REPORT.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n')
print(json.dumps(report,indent=2,ensure_ascii=False))
raise SystemExit(1 if report['fail'] else 0)

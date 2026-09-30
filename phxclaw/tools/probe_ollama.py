#!/usr/bin/env python3
import argparse,json,urllib.request,urllib.error
p=argparse.ArgumentParser(); p.add_argument('--base-url',default='http://127.0.0.1:11434'); a=p.parse_args();
base=a.base_url.rstrip('/'); out={'base_url':base,'available':False,'checks':[]}
if not (base.startswith('http://127.0.0.1:') or base.startswith('http://localhost:') or base.startswith('http://[::1]:') or base.startswith('https://')): raise SystemExit('remote plaintext origin denied')
for path in ['/api/version','/api/tags']:
 try:
  with urllib.request.urlopen(base+path,timeout=3) as r: body=json.loads(r.read()); out['checks'].append({'path':path,'status':r.status,'ok':True,'body_type':type(body).__name__}); out['available']=True
 except Exception as e: out['checks'].append({'path':path,'ok':False,'error':type(e).__name__})
print(json.dumps(out,indent=2)); raise SystemExit(0 if out['available'] else 2)

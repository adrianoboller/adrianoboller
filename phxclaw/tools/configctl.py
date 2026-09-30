#!/usr/bin/env python3
from pathlib import Path
import argparse, copy, hashlib, json, os, tempfile
try:
    import jsonschema
except Exception:
    jsonschema=None

def canonical(v):
    if isinstance(v,dict): return {k:canonical(v[k]) for k in sorted(v)}
    if isinstance(v,list): return [canonical(x) for x in v]
    return v

def digest(v): return hashlib.sha256(json.dumps(canonical(v),separators=(',',':'),ensure_ascii=False).encode()).hexdigest()

def load(path): return json.loads(Path(path).read_text())
def _scan_secrets(v,path=''):
    import re
    secret=re.compile(r'password|api.?key|token|private.?key|client.?secret',re.I)
    ref=re.compile(r'_secret_uuid$|_secret_ref$|^password_secret_uuid$',re.I)
    if isinstance(v,dict):
        for k,x in v.items():
            p=f"{path}/{k}"
            if secret.search(k) and not ref.search(k) and isinstance(x,str) and x:
                raise SystemExit(f"plaintext secret-like value forbidden at {p}")
            _scan_secrets(x,p)
    elif isinstance(v,list):
        for i,x in enumerate(v): _scan_secrets(x,f"{path}/{i}")

def validate(cfg,schema):
    if jsonschema is None: raise SystemExit('jsonschema is required for configctl validate/set')
    jsonschema.Draft202012Validator(schema,format_checker=jsonschema.FormatChecker()).validate(cfg)
    _scan_secrets(cfg)
    if cfg.get('security',{}).get('deny_by_default') is not True: raise SystemExit('security.deny_by_default must remain true')
    if cfg.get('security',{}).get('secrets_plaintext_forbidden') is not True: raise SystemExit('security.secrets_plaintext_forbidden must remain true')

def parse_value(s):
    try:return json.loads(s)
    except Exception:return s

def get_path(cfg,path):
    cur=cfg
    for part in path.split('.'):
        cur=cur[part]
    return cur

def set_path(cfg,path,val):
    parts=path.split('.'); cur=cfg
    for p in parts[:-1]:
        if p not in cur or not isinstance(cur[p],dict): cur[p]={}
        cur=cur[p]
    cur[parts[-1]]=val

def atomic_save(path,cfg):
    path=Path(path); hist=path.parent/'history'; hist.mkdir(parents=True,exist_ok=True)
    old=load(path); old_hash=digest(old); (hist/f"revision-{old['revision']:08}-{old_hash}.json").write_text(json.dumps(old,indent=2)+'\n')
    fd,tmp=tempfile.mkstemp(prefix=path.name+'.',suffix='.tmp',dir=path.parent); os.close(fd)
    Path(tmp).write_text(json.dumps(cfg,indent=2)+'\n'); os.replace(tmp,path)

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('--config',default='config/phxclaw.config.json'); ap.add_argument('--schema',default='schemas/phxclaw-config-v040.schema.json')
    sp=ap.add_subparsers(dest='cmd',required=True)
    sp.add_parser('validate'); sp.add_parser('hash')
    g=sp.add_parser('get'); g.add_argument('path')
    s=sp.add_parser('set'); s.add_argument('path'); s.add_argument('value'); s.add_argument('--expected-revision',type=int,required=True)
    args=ap.parse_args(); cfg=load(args.config); schema=load(args.schema)
    if args.cmd=='validate': validate(cfg,schema); print('VALID'); return
    if args.cmd=='hash': validate(cfg,schema); print(digest(cfg)); return
    if args.cmd=='get': print(json.dumps(get_path(cfg,args.path),indent=2,ensure_ascii=False)); return
    if args.cmd=='set':
        validate(cfg,schema)
        if cfg['revision']!=args.expected_revision: raise SystemExit(f"revision conflict: expected {args.expected_revision}, actual {cfg['revision']}")
        nxt=copy.deepcopy(cfg); set_path(nxt,args.path,parse_value(args.value)); nxt['revision']=cfg['revision']+1; validate(nxt,schema); atomic_save(args.config,nxt); print(json.dumps({'revision':nxt['revision'],'sha256':digest(nxt)})); return
if __name__=='__main__': main()

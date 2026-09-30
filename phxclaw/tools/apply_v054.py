#!/usr/bin/env python3
from __future__ import annotations
import argparse,json,shutil,sys
from pathlib import Path
PACKAGE=Path(__file__).resolve().parents[1]; OVERLAY=PACKAGE/'overlay'; MEMBER='"crates/phxclaw-performance-fabric"'
class ApplyError(RuntimeError):pass

def merge(dst,src):
 for k,v in src.items():
  if isinstance(v,dict) and isinstance(dst.get(k),dict):merge(dst[k],v)
  else:dst[k]=v

def safe_copy(src,dst):
 dst.parent.mkdir(parents=True,exist_ok=True)
 if dst.exists():
  if dst.read_bytes()==src.read_bytes():return False
  raise ApplyError(f'refusing to overwrite different existing file: {dst}')
 shutil.copy2(src,dst);return True

def patch_cargo(p):
 if not p.exists(): raise ApplyError('Cargo.toml missing')
 t=p.read_text()
 if MEMBER in t:return False
 i=t.find('members = [')
 if i<0:raise ApplyError('Cargo workspace members array not found')
 i+=len('members = [');p.write_text(t[:i]+'\n  '+MEMBER+','+t[i:]);return True

def patch_config(root):
 p=root/'config/phxclaw.config.json'
 if not p.exists():raise ApplyError('config/phxclaw.config.json missing')
 cfg=json.loads(p.read_text()); before=json.dumps(cfg,sort_keys=True)
 merge(cfg,json.loads((OVERLAY/'config/consolidation-performance.v054.json').read_text()))
 if before==json.dumps(cfg,sort_keys=True):return False
 p.write_text(json.dumps(cfg,indent=2,ensure_ascii=False)+'\n');return True

def apply(root):
 prereqs=['crates/phxclaw-adaptive-portfolio-execution-controller','crates/phxclaw-active-project-scheduler']
 for q in prereqs:
  if not (root/q).exists():raise ApplyError(f'missing prerequisite: {q}')
 changed=[]
 if patch_cargo(root/'Cargo.toml'):changed.append('Cargo.toml')
 if patch_config(root):changed.append('config/phxclaw.config.json')
 skip={Path('config/consolidation-performance.v054.json')}
 for src in sorted(OVERLAY.rglob('*')):
  if not src.is_file():continue
  rel=src.relative_to(OVERLAY)
  if rel in skip:continue
  if safe_copy(src,root/rel):changed.append(str(rel))
 for n in ['apply_v054.py','assemble_checkout_v054.py','verify_v054.py','package_check_v054.py','ollama_probe_v054.py','native_qualify_v054.py']:
  if safe_copy(PACKAGE/'tools'/n,root/'tools'/n):changed.append('tools/'+n)
 return changed
if __name__=='__main__':
 ap=argparse.ArgumentParser();ap.add_argument('root',type=Path);a=ap.parse_args()
 try:print(json.dumps({'status':'ok','changed':apply(a.root.resolve())},indent=2))
 except Exception as e:print(json.dumps({'status':'error','error':str(e)},indent=2),file=sys.stderr);raise SystemExit(2)

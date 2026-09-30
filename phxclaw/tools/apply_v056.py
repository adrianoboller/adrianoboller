#!/usr/bin/env python3
from pathlib import Path
import argparse,json,shutil,hashlib
P=Path(__file__).resolve().parents[1]
def same(a,b):return a.exists() and hashlib.sha256(a.read_bytes()).digest()==hashlib.sha256(b.read_bytes()).digest()
def main():
 ap=argparse.ArgumentParser();ap.add_argument('root',type=Path);a=ap.parse_args();root=a.root.resolve();changed=[]
 for src in sorted((P/'overlay').rglob('*')):
  if not src.is_file():continue
  rel=src.relative_to(P/'overlay');dst=root/rel;dst.parent.mkdir(parents=True,exist_ok=True)
  if dst.exists():
   if same(dst,src):continue
   if rel.as_posix()=='config/native-qualification.v056.json': pass
   else: raise SystemExit(f'conflict: {rel}')
  shutil.copy2(src,dst);changed.append(rel.as_posix())
 # tools are versioned and safe to copy if absent/same
 for src in sorted((P/'tools').glob('*.py')):
  dst=root/'tools'/src.name;dst.parent.mkdir(parents=True,exist_ok=True)
  if dst.exists() and not same(dst,src):raise SystemExit(f'conflict: tools/{src.name}')
  if not dst.exists():shutil.copy2(src,dst);changed.append('tools/'+src.name)
 print(json.dumps({'changed':changed,'count':len(changed)},indent=2))
if __name__=='__main__':main()

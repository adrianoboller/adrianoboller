#!/usr/bin/env python3
from pathlib import Path
import argparse,hashlib,json,os
EX={'.git','target','reports','var','logs','backups','node_modules','.cache'}
def digest(root:Path):
 h=hashlib.sha256(); files=[]
 for p in sorted(root.rglob('*')):
  if not p.is_file() or p.is_symlink(): continue
  rel=p.relative_to(root).as_posix(); parts=set(p.relative_to(root).parts)
  if parts & EX: continue
  files.append(rel); h.update(rel.encode()+b'\0'); h.update(hashlib.sha256(p.read_bytes()).digest())
 return h.hexdigest(),files
if __name__=='__main__':
 ap=argparse.ArgumentParser();ap.add_argument('root',type=Path);ap.add_argument('--json',action='store_true');a=ap.parse_args();d,f=digest(a.root.resolve());print(json.dumps({'source_state_sha256':d,'files':len(f)},indent=2) if a.json else d)

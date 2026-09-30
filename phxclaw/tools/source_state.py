#!/usr/bin/env python3
from __future__ import annotations
from pathlib import Path
import argparse, hashlib, json

EXCLUDE_PREFIXES=(".git/","target/","var/","dist/","reports/release-qualification/")
EXCLUDE_SUFFIXES=(".log",".tmp",".pyc")
EXCLUDE_NAMES={".DS_Store"}

def normalized_rel(path:Path, root:Path)->str:
    return path.relative_to(root).as_posix()

def included(rel:str)->bool:
    if rel in EXCLUDE_NAMES or rel.rsplit('/',1)[-1] in EXCLUDE_NAMES: return False
    if any(rel.startswith(p) for p in EXCLUDE_PREFIXES): return False
    if any(rel.endswith(s) for s in EXCLUDE_SUFFIXES): return False
    return True

def file_sha256(path:Path)->str:
    h=hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda:f.read(1024*1024),b''): h.update(chunk)
    return h.hexdigest()

def manifest(root:Path):
    rows=[]
    for p in sorted((p for p in root.rglob('*') if p.is_file()), key=lambda p:p.relative_to(root).as_posix()):
        rel=normalized_rel(p,root)
        if included(rel): rows.append({'path':rel,'sha256':file_sha256(p),'size':p.stat().st_size})
    return rows

def tree_hash(rows)->str:
    h=hashlib.sha256()
    for row in rows:
        h.update(row['path'].encode()); h.update(b'\0'); h.update(row['sha256'].encode()); h.update(b'\0'); h.update(str(row['size']).encode()); h.update(b'\n')
    return h.hexdigest()

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--manifest',type=Path); args=ap.parse_args()
    root=args.root.resolve(); rows=manifest(root); digest=tree_hash(rows)
    out={'algorithm':'sha256-tree-v1','workspace_sha256':digest,'file_count':len(rows),'files':rows}
    if args.manifest:
        args.manifest.parent.mkdir(parents=True,exist_ok=True); args.manifest.write_text(json.dumps(out,indent=2)+'\n',encoding='utf-8')
    print(digest)
if __name__=='__main__': main()

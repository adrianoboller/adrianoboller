#!/usr/bin/env python3
from __future__ import annotations
import argparse, hashlib, json, os, shutil, stat, subprocess, sys, tempfile, zipfile
from pathlib import Path

class AssemblyError(RuntimeError): pass

def sha256(p:Path)->str:
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
 return h.hexdigest()

def safe_extract(zp:Path,dst:Path,max_files=20000,max_uncompressed=2_000_000_000):
 with zipfile.ZipFile(zp) as z:
  infos=z.infolist();
  if len(infos)>max_files: raise AssemblyError('zip file-count limit exceeded')
  if sum(i.file_size for i in infos)>max_uncompressed: raise AssemblyError('zip uncompressed-size limit exceeded')
  seen=set()
  for i in infos:
   name=i.filename.replace('\\','/')
   if name in seen: raise AssemblyError(f'duplicate zip entry: {name}')
   seen.add(name)
   p=Path(name)
   if p.is_absolute() or '..' in p.parts: raise AssemblyError(f'unsafe zip path: {name}')
   mode=(i.external_attr>>16)&0o170000
   if mode==stat.S_IFLNK: raise AssemblyError(f'symlink zip entry rejected: {name}')
  z.extractall(dst)

def locate_root(d:Path)->Path:
 if (d/'Cargo.toml').exists(): return d
 children=[p for p in d.iterdir() if p.is_dir()]
 if len(children)==1 and (children[0]/'Cargo.toml').exists(): return children[0]
 raise AssemblyError('unable to locate workspace root after base extraction')

def run_apply(package:Path,root:Path):
 tools=package/'tools'
 scripts=sorted(tools.glob('apply_v*.py'))
 if not scripts: raise AssemblyError(f'no apply script in {package}')
 subprocess.run([sys.executable,str(scripts[-1]),str(root)],check=True)

def main():
 ap=argparse.ArgumentParser()
 ap.add_argument('--base',type=Path,required=True)
 ap.add_argument('--overlays-dir',type=Path,required=True)
 ap.add_argument('--v054-package',type=Path,default=Path(__file__).resolve().parents[1])
 ap.add_argument('--out',type=Path,required=True)
 ap.add_argument('--base-sha256')
 ap.add_argument('--allow-unpinned-base',action='store_true')
 a=ap.parse_args()
 manifest=json.loads((a.v054_package/'overlay/config/consolidation-manifest.v054.json').read_text())
 if not a.base.exists(): raise AssemblyError('base zip missing')
 if a.base.stat().st_size != manifest['base']['size_bytes']: raise AssemblyError('base size mismatch')
 actual=sha256(a.base)
 expected=a.base_sha256
 if not expected and not a.allow_unpinned_base: raise AssemblyError('release assembly requires --base-sha256; use --allow-unpinned-base only for development')
 if expected and actual.lower()!=expected.lower(): raise AssemblyError('base sha256 mismatch')
 with tempfile.TemporaryDirectory(prefix='phxclaw-v054-') as td:
  td=Path(td); base_dir=td/'base'; base_dir.mkdir(); safe_extract(a.base,base_dir); root=locate_root(base_dir)
  for item in manifest['overlays']:
   zp=a.overlays_dir/item['filename']
   if not zp.exists(): raise AssemblyError(f'missing overlay: {zp.name}')
   if sha256(zp)!=item['sha256']: raise AssemblyError(f'overlay sha256 mismatch: {zp.name}')
   pkg=td/f"ov-{item['version']}"; pkg.mkdir(); safe_extract(zp,pkg)
   dirs=[p for p in pkg.iterdir() if p.is_dir()]
   package=dirs[0] if len(dirs)==1 and (dirs[0]/'tools').exists() else pkg
   run_apply(package,root)
  run_apply(a.v054_package,root)
  legacy=[]
  allow_parts={'private','vendor','vendor-quarantine','third_party','third-party','.git','reports','docs/history'}
  for p in root.rglob('*'):
   if not p.is_file() or p.suffix.lower() in {'.png','.jpg','.jpeg','.gif','.ico','.zip','.pdf'}: continue
   rel=str(p.relative_to(root)).replace('\\','/')
   if any(part in rel for part in allow_parts): continue
   try: txt=p.read_text(errors='ignore')
   except Exception: continue
   if 'PhoenixClaw' in txt or 'phoenixclaw' in txt or 'PHOENIXCLAW' in txt: legacy.append(rel)
  if legacy: raise AssemblyError('legacy product brand remains in live source: '+', '.join(legacy[:20]))
  if a.out.exists(): shutil.rmtree(a.out)
  shutil.copytree(root,a.out,symlinks=False)
  print(json.dumps({'status':'ok','root':str(a.out),'source_sha256':tree_hash(a.out)},indent=2))

def tree_hash(root:Path)->str:
 h=hashlib.sha256()
 for p in sorted(x for x in root.rglob('*') if x.is_file() and '.git' not in x.parts):
  rel=p.relative_to(root).as_posix().encode(); h.update(len(rel).to_bytes(4,'big')); h.update(rel); h.update(bytes.fromhex(sha256(p)))
 return h.hexdigest()

if __name__=='__main__':
 try: main()
 except Exception as e:
  print(json.dumps({'status':'error','error':str(e)},indent=2),file=sys.stderr); raise SystemExit(2)

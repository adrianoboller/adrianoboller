#!/usr/bin/env python3
from pathlib import Path
import argparse, hashlib, json, re, shutil, tomllib
HERE=Path(__file__).resolve().parents[1]; OVER=HERE/'overlay'; CRATES=['crates/phxclaw-config-runtime','crates/phxclaw-software-factory']
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def integrate(cargo):
 text=cargo.read_text(); tomllib.loads(text)
 if '"crates/*"' in text or "'crates/*'" in text: return 'workspace wildcard covers v0.40 crates'
 m=re.search(r'(?ms)^members\s*=\s*\[(.*?)\]',text)
 if not m: raise SystemExit('unsupported Cargo.toml workspace shape')
 block=m.group(1); lines=[]
 for line in block.splitlines():
  s=line.strip()
  if s and not s.startswith('#') and re.fullmatch(r'["\'][^"\']+["\']',s): line=line.rstrip()+','
  lines.append(line)
 block='\n'.join(lines)
 for c in CRATES:
  if c not in block: block=block.rstrip()+f'\n  "{c}",\n'
 updated=text[:m.start(1)]+block+text[m.end(1):]; members=tomllib.loads(updated).get('workspace',{}).get('members',[])
 for c in CRATES:
  if members.count(c)!=1: raise SystemExit('workspace member missing/duplicated: '+c)
 cargo.write_text(updated); return 'v0.40 crates present exactly once'
def validate_base(t):
 if not (t/'config/overlays/v0.39.applied.json').is_file(): raise SystemExit('apply v0.39 before v0.40')
 if not (t/'crates/phxclaw-swarm-merge-intelligence/Cargo.toml').is_file(): raise SystemExit('v0.39 prerequisite missing')
def main():
 ap=argparse.ArgumentParser();ap.add_argument('target',type=Path);a=ap.parse_args();t=a.target.resolve();validate_base(t);marker=t/'config/overlays/v0.40.applied.json'
 if marker.exists(): print('PhxClaw v0.40 overlay already applied'); print(integrate(t/'Cargo.toml')); return
 copied=[]
 for src in OVER.rglob('*'):
  if not src.is_file(): continue
  rel=src.relative_to(OVER); dst=t/rel
  if dst.exists():
   if sha(src)==sha(dst): continue
   raise SystemExit(f'conflict: {dst}; v0.40 never force-overwrites unknown content')
  dst.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(src,dst);copied.append({'path':str(rel),'sha256':sha(dst)})
 msg=integrate(t/'Cargo.toml'); marker.parent.mkdir(parents=True,exist_ok=True); marker.write_text(json.dumps({'version':'0.40.0','base':'0.39.0','copied':copied,'workspace':msg,'canonical_config':'config/phxclaw.config.json','cargo_lock_requires_refresh':(t/'Cargo.lock').exists()},indent=2)+'\n'); print(f'PhxClaw v0.40 applied: {len(copied)} files');print(msg)
if __name__=='__main__':main()

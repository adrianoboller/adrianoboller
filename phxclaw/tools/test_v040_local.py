#!/usr/bin/env python3
from pathlib import Path
import json, shutil, subprocess, tempfile, tomllib, sys
pkg=Path(__file__).resolve().parents[1]; checks=[]
def ck(n,c,d=''): checks.append((n,bool(c),d))
# config semantic guards in Python equivalent
cfg=json.loads((pkg/'overlay/config/phxclaw.config.json').read_text()); ck('revision',cfg['revision']==1); ck('secret_refs',cfg['database']['password_secret_uuid'] is None and cfg['models']['providers']['openai']['api_key_secret_uuid'] is None); ck('stages',cfg['software_factory']['stages'][0]=='intake' and cfg['software_factory']['stages'][-1]=='delivery')
# apply and idempotency on synthetic v0.39 tree
with tempfile.TemporaryDirectory() as td:
 t=Path(td); (t/'config/overlays').mkdir(parents=True); (t/'config/overlays/v0.39.applied.json').write_text('{}'); (t/'crates/phxclaw-swarm-merge-intelligence').mkdir(parents=True); (t/'crates/phxclaw-swarm-merge-intelligence/Cargo.toml').write_text('[package]\nname="x"\nversion="0.1.0"\n'); (t/'Cargo.toml').write_text('[workspace]\nresolver="2"\nmembers=[\n "crates/base"\n]\n'); (t/'crates/base').mkdir(parents=True); (t/'crates/base/Cargo.toml').write_text('[package]\nname="base"\nversion="0.1.0"\n')
 a=subprocess.run([sys.executable,str(pkg/'tools/apply_overlay.py'),str(t)],capture_output=True,text=True); ck('apply',a.returncode==0,a.stderr); members=tomllib.loads((t/'Cargo.toml').read_text())['workspace']['members']; ck('members_once',members.count('crates/phxclaw-config-runtime')==1 and members.count('crates/phxclaw-software-factory')==1); b=subprocess.run([sys.executable,str(pkg/'tools/apply_overlay.py'),str(t)],capture_output=True,text=True); ck('idempotent',b.returncode==0); v=subprocess.run([sys.executable,str(t/'tools/verify_v040.py')],capture_output=True,text=True); ck('installed_verify',v.returncode==0,v.stdout[-500:])
for n,c,d in checks: print(('PASS' if c else 'FAIL'),n,d)
print(f'TOTAL {sum(c for _,c,_ in checks)} PASS / {sum(not c for _,c,_ in checks)} FAIL'); sys.exit(1 if any(not c for _,c,_ in checks) else 0)

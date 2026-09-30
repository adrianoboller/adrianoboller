#!/usr/bin/env python3
import json, tempfile, shutil, subprocess, sys, hashlib, os
from pathlib import Path
pkg=Path(__file__).resolve().parents[1]
checks=[]
def ck(n,c,d=''): checks.append((n,bool(c),d))
# package verify
r=subprocess.run([sys.executable,str(pkg/'tools/verify_v042.py'),str(pkg)],capture_output=True,text=True); ck('package_verify',r.returncode==0,r.stdout+r.stderr)
# synthetic post-v041 root
with tempfile.TemporaryDirectory() as td:
    root=Path(td)
    (root/'config').mkdir(); (root/'config/phxclaw.config.json').write_text(json.dumps({'schema_version':'0.41.0','product':{'name':'PhxClaw','version':'0.41.0'},'skills':{'catalog':[]},'config_management':{'canonical_path':'config/phxclaw.config.json'}},indent=2))
    (root/'Cargo.toml').write_text('[workspace]\nmembers = []\n')
    a=subprocess.run([sys.executable,str(pkg/'tools/apply_overlay.py'),str(root)],capture_output=True,text=True); ck('apply',a.returncode==0,a.stdout+a.stderr)
    b=subprocess.run([sys.executable,str(root/'tools/verify_v042.py'),str(root)],capture_output=True,text=True); ck('installed_verify',b.returncode==0,b.stdout+b.stderr)
    cfg=json.loads((root/'config/phxclaw.config.json').read_text()); ck('canonical_pm_section','project_management' in cfg); ck('catalog_79',len(cfg['skills']['catalog'])==79)
    before=hashlib.sha256((root/'config/phxclaw.config.json').read_bytes()).hexdigest(); a2=subprocess.run([sys.executable,str(pkg/'tools/apply_overlay.py'),str(root)],capture_output=True,text=True); after=hashlib.sha256((root/'config/phxclaw.config.json').read_bytes()).hexdigest(); ck('idempotent',a2.returncode==0 and before==after,a2.stdout+a2.stderr)
# secret pattern absent from canonical module data
raw=(pkg/'overlay/config/project-management-skill-catalog.v042.json').read_text(); ck('no_plain_secret', not __import__('re').search(r'(?i)(api[_-]?key|password|bearer|secret)\s*[=:]\s*[\"\'][A-Za-z0-9_\-]{16,}', raw))
fail=[x for x in checks if not x[1]]
report={'suite':'PhxClaw v0.42 local overlay','pass':len(checks)-len(fail),'fail':len(fail),'checks':[{'name':n,'pass':c,'detail':d} for n,c,d in checks]}
(pkg/'reports').mkdir(exist_ok=True); (pkg/'reports/V042_LOCAL_TEST_REPORT.json').write_text(json.dumps(report,indent=2)+'\n')
print(f"{report['pass']}/{len(checks)} PASS")
for f in fail: print('FAIL',f[0],f[2])
raise SystemExit(1 if fail else 0)

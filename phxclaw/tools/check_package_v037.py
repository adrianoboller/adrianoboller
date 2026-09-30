#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,sys,zipfile,stat
root=Path(sys.argv[1] if len(sys.argv)>1 else '.').resolve(); checks=[]
def ck(n,c,d=''): checks.append({'name':n,'pass':bool(c),'detail':d}); print(('PASS ' if c else 'FAIL ')+n+((' '+d) if d else ''))
for n in ['PACKAGE.json','README.md','THIRD_PARTY_NOTICES_DELTA.md','tools/apply_overlay.py','tools/verify_v037.py','tools/test_v037_local.py'] : ck('package/'+n,(root/n).is_file())
# no symlinks
syms=[str(p.relative_to(root)) for p in root.rglob('*') if p.is_symlink()]; ck('no_symlinks',not syms,str(syms))
# no obvious secrets
bad=[]
for p in root.rglob('*'):
 if p.name=='check_package_v037.py': continue
 if p.is_file() and p.suffix.lower() in {'.py','.rs','.json','.md','.sql','.sh','.ps1','.toml'}:
  s=p.read_text(errors='ignore')
  if 'sk-' in s or 'AKIA' in s or 'BEGIN PRIVATE KEY' in s: bad.append(str(p.relative_to(root)))
ck('no_obvious_secrets',not bad,str(bad))
pkg=json.load(open(root/'PACKAGE.json')); ck('pkg_version',pkg['version']=='0.37.0'); ck('skills_reused',pkg['external_skills_reused']==20); ck('no_vendor',pkg['third_party_source_vendored'] is False)
# Require v0.36 notices statement
notice=(root/'THIRD_PARTY_NOTICES_DELTA.md').read_text(); ck('notice_no_new_vendor','No new third-party source is vendored' in notice)
passed=sum(x['pass'] for x in checks); failed=len(checks)-passed
out={'suite':'PhxClaw v0.37 package/security checks','pass':passed,'fail':failed,'checks':checks}; p=root/'reports/V037_PACKAGE_CHECKS.json'; p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(out,indent=2)+'\n')
print(f'SUMMARY {passed} PASS / {failed} FAIL'); raise SystemExit(1 if failed else 0)

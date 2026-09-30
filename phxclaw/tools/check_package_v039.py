#!/usr/bin/env python3
from pathlib import Path
import json, tomllib, hashlib, os, sys
ROOT=Path(__file__).resolve().parents[1]; checks=[]
def ck(n,c,d=''): checks.append({'name':n,'pass':bool(c),'detail':d})
for p in sorted(ROOT.rglob('*.json')):
 if p.name=='V039_PACKAGE_CHECKS.json': continue
 try: json.loads(p.read_text()); ck('parse_json_'+str(p.relative_to(ROOT)),True)
 except Exception as e: ck('parse_json_'+str(p.relative_to(ROOT)),False,str(e))
for p in sorted(ROOT.rglob('Cargo.toml')):
 try: tomllib.loads(p.read_text()); ck('parse_toml_'+str(p.relative_to(ROOT)),True)
 except Exception as e: ck('parse_toml_'+str(p.relative_to(ROOT)),False,str(e))
# avoid self-reference scanner false positives
secret_markers=['sk-'+'ant-','sk-'+'proj-','BEGIN PRIVATE'+' KEY','ghp'+'_']
text='\n'.join(p.read_text(errors='ignore') for p in ROOT.rglob('*') if p.is_file() and p.name not in {'check_package_v039.py','V039_PACKAGE_CHECKS.json','SHA256SUMS'})
for s in secret_markers: ck('no_secret_'+s,s not in text)
ck('no_vendor_source',not any('vendor/' in str(p.relative_to(ROOT)).replace('\\','/') for p in ROOT.rglob('*') if p.is_file()))
ck('no_symlinks',not any(p.is_symlink() for p in ROOT.rglob('*')))
ck('repair_manifest_exists',(ROOT/'repairs/V038_RLS_REPAIR.json').is_file())
ck('native_separate_rls_url','PHXCLAW_RLS_DATABASE_URL' in (ROOT/'overlay/ci/run-v039-native.sh').read_text())
ck('no_direct_main_push','git push' not in text and 'push origin main' not in text)
report={'suite':'PhxClaw v0.39 package/security','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}
(ROOT/'reports').mkdir(exist_ok=True); (ROOT/'reports/V039_PACKAGE_CHECKS.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'pass':report['pass'],'fail':report['fail']})); sys.exit(1 if report['fail'] else 0)

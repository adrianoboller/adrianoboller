#!/usr/bin/env python3
from pathlib import Path
import json, hashlib, sys, tomllib
P=Path(__file__).resolve().parents[1]; checks=[]
def ck(n,c,d=''): checks.append({'name':n,'pass':bool(c),'detail':d})
for p in P.rglob('*'):
 if p.is_symlink(): ck('no_symlink_'+str(p.relative_to(P)),False)
for p in P.rglob('*.json'):
 if p.name=='V038_PACKAGE_CHECKS.json': continue
 try: json.loads(p.read_text()); ck('parse_json_'+str(p.relative_to(P)),True)
 except Exception as e: ck('parse_json_'+str(p.relative_to(P)),False,str(e))
for p in P.rglob('*.toml'):
 try: tomllib.loads(p.read_text()); ck('parse_toml_'+str(p.relative_to(P)),True)
 except Exception as e: ck('parse_toml_'+str(p.relative_to(P)),False,str(e))
text='\n'.join(p.read_text(errors='ignore') for p in P.rglob('*') if p.is_file() and p.name not in {'check_package_v038.py','V038_PACKAGE_CHECKS.json'} and p.stat().st_size<2_000_000)
for needle in ['sk-ant-','sk-proj-','BEGIN PRIVATE KEY','ghp_']:
 ck('no_secret_'+needle,needle not in text)
ck('no_vendor_source',not any('vendor/' in str(p.relative_to(P)).replace('\\','/') for p in P.rglob('*')))
ck('native_separate_rls_url','PHXCLAW_RLS_DATABASE_URL' in (P/'overlay/ci/run-v038-native.sh').read_text())
ck('no_direct_main_push',json.loads((P/'overlay/config/engineering-swarm-policy.v038.json').read_text())['workspace']['direct_main_push'] is False)
report={'suite':'PhxClaw v0.38 package/security','pass':sum(c['pass'] for c in checks),'fail':sum(not c['pass'] for c in checks),'checks':checks}
(P/'reports/V038_PACKAGE_CHECKS.json').write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps({'pass':report['pass'],'fail':report['fail']})); sys.exit(1 if report['fail'] else 0)

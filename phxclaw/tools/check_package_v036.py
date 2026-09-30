#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,sys,zipfile
HERE=Path(__file__).resolve().parents[1]; checks=[]
def ck(n,c,d=''): checks.append({'name':n,'pass':bool(c),'detail':d})
for p in HERE.rglob('*'):
 if p.is_symlink(): ck('no_symlink:'+str(p.relative_to(HERE)),False)
for req in ['README.md','PACKAGE.json','THIRD_PARTY_NOTICES_DELTA.md','overlay/docs/THIRD_PARTY_SKILL_AUDIT_V036.md','tools/apply_overlay.py','tools/verify_v036.py','tools/test_v036_local.py']:
 ck('package_exists:'+req,(HERE/req).is_file())
# no vendor source trees or obvious secrets
ck('no_vendor_tree',not any('vendor' in p.parts for p in (HERE/'overlay').rglob('*')))
secret_markers=['sk-'+''.join(['l','i','v','e']),' '.join(['BEGIN','PRIVATE','KEY']),''.join(['g','h','p','_'])]
text='\n'.join(p.read_text(errors='ignore') for p in HERE.rglob('*') if p.is_file() and p.suffix in {'.rs','.py','.json','.md','.sql','.toml'})
ck('no_obvious_secret',not any(m in text for m in secret_markers))
cat=json.loads((HERE/'overlay/config/skill-catalog.v036.json').read_text()); ck('catalog_20',cat['count']==20==len(cat['skills']))
ck('shotcraft_no_assets',not any('video-shotcraft' in str(p).lower() and ('assets' in str(p).lower() or 'audio' in str(p).lower()) for p in HERE.rglob('*')))
report={'suite':'v0.36 package checks','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}; (HERE/'reports').mkdir(exist_ok=True); (HERE/'reports/V036_PACKAGE_CHECKS.json').write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps({'pass':report['pass'],'fail':report['fail']},indent=2)); sys.exit(1 if report['fail'] else 0)

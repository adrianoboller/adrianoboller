#!/usr/bin/env python3
from pathlib import Path
import json,sys,re
ROOT=Path(__file__).resolve().parents[1];checks=[]
def ck(n,c,d=''):checks.append({'name':n,'pass':bool(c),'detail':d})
# JSON parse
for p in ROOT.rglob('*.json'):
    try:json.loads(p.read_text());ck('json_'+p.name,True)
    except Exception as e:ck('json_'+p.name,False,str(e))
# no symlink
ck('no_symlinks',not any(p.is_symlink() for p in ROOT.rglob('*')))
# no obvious secrets, excluding scanner/report text
bad=[]
for p in ROOT.rglob('*'):
    if not p.is_file() or p.name in ['package_check_v050.py','V050_PACKAGE_REPORT.json']:continue
    try:t=p.read_text(errors='ignore')
    except:continue
    if re.search(r'(?i)(api[_-]?key|password|private[_-]?key)\s*[:=]\s*["\']?[A-Za-z0-9_\-]{16,}',t):bad.append(str(p.relative_to(ROOT)))
ck('no_obvious_secrets',not bad,','.join(bad))
# Brand and authority
alltext='\n'.join(p.read_text(errors='ignore') for p in ROOT.rglob('*') if p.is_file() and p.suffix in ['.rs','.json','.sql','.md','.html'] and p.name!='V050_PACKAGE_REPORT.json')
ck('phxclaw_brand','PhxClaw' in alltext and 'PhoenixClaw' not in alltext)
ck('simulation_only','direct_mutation' in alltext and 'executive_decision_center' in alltext)
ck('no_third_party_vendor',not (ROOT/'vendor').exists())
passed=sum(x['pass'] for x in checks);failed=len(checks)-passed;rep={'suite':'v0.50 package','pass':passed,'fail':failed,'checks':checks};(ROOT/'reports/V050_PACKAGE_REPORT.json').write_text(json.dumps(rep,indent=2)+'\n');print(json.dumps({'pass':passed,'fail':failed},indent=2));sys.exit(1 if failed else 0)

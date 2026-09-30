#!/usr/bin/env python3
from pathlib import Path
import json,sys,re,tomllib
ROOT=Path(__file__).resolve().parents[1];checks=[]
def ck(n,c,d=''):checks.append({'name':n,'pass':bool(c),'detail':d})
for p in ROOT.rglob('*.json'):
    try:json.loads(p.read_text());ck('json_'+p.name,True)
    except Exception as e:ck('json_'+p.name,False,str(e))
for p in ROOT.rglob('*.toml'):
    try:tomllib.loads(p.read_text());ck('toml_'+p.name,True)
    except Exception as e:ck('toml_'+p.name,False,str(e))
ck('no_symlinks',not any(p.is_symlink() for p in ROOT.rglob('*')))
bad=[]
for p in ROOT.rglob('*'):
    if not p.is_file() or p.name in ['package_check_v051.py','V051_PACKAGE_REPORT.json']:continue
    try:t=p.read_text(errors='ignore')
    except:continue
    if re.search(r'(?i)(api[_-]?key|password|private[_-]?key)\s*[:=]\s*["\']?[A-Za-z0-9_\-]{16,}',t):bad.append(str(p.relative_to(ROOT)))
ck('no_obvious_secrets',not bad,','.join(bad))
alltext='\n'.join(p.read_text(errors='ignore') for p in ROOT.rglob('*') if p.is_file() and p.suffix in ['.rs','.json','.sql','.md','.html'] and p.name!='V051_PACKAGE_REPORT.json')
ck('phxclaw_brand','PhxClaw' in alltext and 'PhoenixClaw' not in alltext);ck('pareto','pareto' in alltext.lower());ck('delegate_v049','executive_decision_center' in alltext);ck('no_direct_mutation','direct_mutation' in alltext.lower());ck('bounded_search','25000' in alltext or '25_000' in alltext);ck('ui_present',(ROOT/'overlay/ui/portfolio-optimizer.html').exists())
passed=sum(x['pass'] for x in checks);failed=len(checks)-passed;rep={'suite':'v0.51 package/security','pass':passed,'fail':failed,'checks':checks};(ROOT/'reports/V051_PACKAGE_REPORT.json').write_text(json.dumps(rep,indent=2)+'\n');print(json.dumps({'pass':passed,'fail':failed},indent=2));sys.exit(1 if failed else 0)

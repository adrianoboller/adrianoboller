#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,re,sys
P=Path(__file__).resolve().parents[1];checks=[]
def c(n,v,d=''):checks.append({'name':n,'pass':bool(v),'detail':d})
for p in P.rglob('*.json'):
 try:json.loads(p.read_text());c('json_'+p.name,True)
 except Exception as e:c('json_'+p.name,False,str(e))
c('no_symlinks',not any(p.is_symlink() for p in P.rglob('*')))
# no literal secret values; exclude scanner source itself
pat=re.compile(r'(?i)(api[_-]?key|password|private[_-]?key)\s*[:=]\s*["\'][^"\']{8,}["\']')
for p in P.rglob('*'):
 if p.is_file() and p.name!='package_check_v056.py' and p.suffix.lower() in {'.py','.json','.md','.html','.toml','.sql'}:
  c('no_secret_'+p.name,not bool(pat.search(p.read_text(errors='ignore'))))
report={'suite':'v0.56 package','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks};print(json.dumps(report,indent=2));sys.exit(1 if report['fail'] else 0)

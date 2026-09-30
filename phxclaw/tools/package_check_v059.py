#!/usr/bin/env python3
from pathlib import Path
import json,re,sys
here=Path(__file__).resolve(); raw=here.parents[1]; base=raw.parent if raw.name=='overlay' else raw
checks=[]
def c(n,o):checks.append({'name':n,'pass':bool(o)})
required=['README.md','THIRD_PARTY_NOTICES_DELTA.md','overlay/config/historical-table-inventory.v059.json','overlay/config/historical-backfill-registry.v059.json','overlay/migrations/0059_historical_backfill.sql','overlay/tools/backfill_v059.py','overlay/tools/verify_v059.py','overlay/ui/historical-backfill.html','tools/apply_v059.py']
for x in required:c('exists:'+x,(base/x).exists())
for p in base.rglob('*.json'):
 if p.name=='V059_PACKAGE_REPORT.json': continue
 try:json.loads(p.read_text());c('json:'+str(p.relative_to(base)),True)
 except:c('json:'+str(p.relative_to(base)),False)
for p in list((base/'overlay/tools').glob('*.py'))+list((base/'tools').glob('*.py')):
 try:compile(p.read_text(),str(p),'exec');c('py:'+str(p.relative_to(base)),True)
 except:c('py:'+str(p.relative_to(base)),False)
legacy=[];secrets=[]
for p in base.rglob('*'):
 if not p.is_file() or '__pycache__' in p.parts or p.name in {'SHA256SUMS','package_check_v059.py','V059_PACKAGE_REPORT.json'}:continue
 try:t=p.read_text(errors='ignore')
 except:continue
 if 'PhoenixClaw' in t:legacy.append(str(p.relative_to(base)))
 if re.search(r'(?i)(password|api[_-]?key|private[_-]?key|access[_-]?token|refresh[_-]?token)\s*[:=]\s*["\'][^"\']{8,}',t):secrets.append(str(p.relative_to(base)))
c('no_legacy_brand',not legacy);c('no_obvious_secrets',not secrets)
fail=[x for x in checks if not x['pass']];print(json.dumps({'suite':'v0.59 package','pass':len(checks)-len(fail),'fail':len(fail),'legacy':legacy,'secrets':secrets,'checks':checks},indent=2));sys.exit(1 if fail else 0)

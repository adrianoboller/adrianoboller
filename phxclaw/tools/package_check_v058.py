#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,sys,re
base=Path(__file__).resolve().parents[1]
checks=[]
def c(n,o): checks.append({'name':n,'pass':bool(o)})
c('readme',(base/'README.md').exists()); c('sql',(base/'overlay/migrations/0058_project_trace_tree.sql').exists()); c('ui',(base/'overlay/ui/project-trace-tree.html').exists())
legacy=[]; secrets=[]
for p in base.rglob('*'):
 if not p.is_file() or p.name in {'SHA256SUMS','package_check_v058.py'}: continue
 try:t=p.read_text(errors='ignore')
 except: continue
 if 'PhoenixClaw' in t: legacy.append(str(p.relative_to(base)))
 if re.search(r'(?i)(password|api[_-]?key|private[_-]?key)\s*[:=]\s*["\'][^"\']{8,}',t): secrets.append(str(p.relative_to(base)))
c('no_legacy_brand',not legacy); c('no_obvious_secrets',not secrets)
fail=[x for x in checks if not x['pass']]; print(json.dumps({'pass':len(checks)-len(fail),'fail':len(fail),'checks':checks,'legacy':legacy,'secrets':secrets},indent=2)); sys.exit(1 if fail else 0)

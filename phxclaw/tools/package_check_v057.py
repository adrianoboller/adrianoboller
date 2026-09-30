#!/usr/bin/env python3
from pathlib import Path
import json,re,sys
P=Path(__file__).resolve().parents[1]; checks=[]
def c(n,v,d=''): checks.append({'name':n,'pass':bool(v),'detail':d})
for p in P.rglob('*.json'):
    try: json.loads(p.read_text()); c('json_'+str(p.relative_to(P)),True)
    except Exception as e: c('json_'+str(p.relative_to(P)),False,str(e))
c('no_symlinks',not any(p.is_symlink() for p in P.rglob('*')))
pat=re.compile(r"(?i)(api[_-]?key|password|private[_-]?key)\\s*[:=]\\s*[\"'][^\"']{8,}[\"']")
for p in P.rglob('*'):
    if p.is_file() and p.name!='package_check_v057.py' and p.suffix.lower() in {'.py','.json','.md','.html','.toml','.sql','.rs'}:
        txt=p.read_text(errors='ignore'); c('no_secret_'+str(p.relative_to(P)),not bool(pat.search(txt)))
# No legacy brand in live implementation; notices may contain historical text but this delta does not.
legacy=[]
for p in (P/'overlay').rglob('*'):
    if p.is_file() and p.suffix.lower() in {'.py','.json','.md','.html','.toml','.sql','.rs'}:
        t=p.read_text(errors='ignore')
        if any(x in t for x in ['PhoenixClaw','phoenixclaw','PHOENIXCLAW']): legacy.append(str(p.relative_to(P)))
c('no_legacy_brand',not legacy,','.join(legacy))
report={'suite':'PhxClaw v0.57 package','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}; print(json.dumps(report,indent=2)); sys.exit(1 if report['fail'] else 0)

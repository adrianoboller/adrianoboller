#!/usr/bin/env python3
import json, hashlib, re, sys, zipfile
from pathlib import Path
pkg=Path(sys.argv[1] if len(sys.argv)>1 else Path(__file__).resolve().parents[1])
checks=[]
def ck(n,c,d=''): checks.append((n,bool(c),d))
# JSON parse
for p in pkg.rglob('*.json'):
    try: json.loads(p.read_text())
    except Exception as e: ck('json_'+str(p.relative_to(pkg)),False,str(e))
ck('json_parse',not any(not c for _,c,_ in checks))
# no legacy product name in live source (historical notice names excluded)
legacy=[]
for p in pkg.rglob('*'):
    rel=str(p.relative_to(pkg)) if p.is_file() else ''
    if p.is_file() and p.suffix.lower() in {'.rs','.json','.html','.py','.sql','.md','.toml'} and 'THIRD_PARTY' not in p.name and rel not in {'tools/check_package_v042.py','overlay/tools/check_package_v042.py','tools/apply_overlay.py','overlay/tools/apply_overlay_v042.py'}:
        t=p.read_text(errors='ignore')
        if 'PhoenixClaw' in t or 'phoenixclaw' in t or 'PHOENIXCLAW' in t: legacy.append(rel)
ck('no_legacy_brand',not legacy,','.join(legacy[:10]))
ck('skill_manifests_79',len(list((pkg/'overlay/plugins/project-management/skills').glob('*/skill.json')))==79)
ck('schemas_158',len(list((pkg/'overlay/schemas/skills').glob('*.schema.json')))+len(list((pkg/'overlay/schemas/integrations').glob('*.schema.json')))==158)
ck('ui_exists',(pkg/'overlay/ui/project-management.html').exists())
ck('migration_exists',(pkg/'overlay/migrations/0042_project_management_suite.sql').exists())
# secrets heuristic
bad=[]
patterns=[re.compile(r'(?i)(api[_-]?key|password|token)\s*[=:]\s*["\']?[A-Za-z0-9_\-]{16,}')]
for p in pkg.rglob('*'):
    if p.is_file() and p.name not in {'check_package_v042.py'} and p.suffix.lower() in {'.json','.md','.rs','.py','.html','.sql'}:
        t=p.read_text(errors='ignore')
        if any(x.search(t) for x in patterns): bad.append(str(p.relative_to(pkg)))
ck('no_obvious_secrets',not bad,','.join(bad[:10]))
fail=[x for x in checks if not x[1]]
report={'suite':'PhxClaw v0.42 package checks','pass':len(checks)-len(fail),'fail':len(fail),'checks':[{'name':n,'pass':c,'detail':d} for n,c,d in checks]}
(pkg/'reports').mkdir(exist_ok=True); (pkg/'reports/V042_PACKAGE_CHECKS.json').write_text(json.dumps(report,indent=2)+'\n')
print(f"{report['pass']}/{len(checks)} PASS")
for f in fail: print('FAIL',f[0],f[2])
raise SystemExit(1 if fail else 0)

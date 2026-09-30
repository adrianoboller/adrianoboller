#!/usr/bin/env python3
from pathlib import Path
import json, sys
BASE=Path(__file__).resolve().parents[1]; SRC=BASE/'overlay' if (BASE/'overlay').is_dir() else BASE
paths=sorted((SRC/'migrations').glob('*.sql'))
checks=[]
for p in paths:
 text=p.read_text()
 if 'current_setting' in text:
  bad=', )::uuid' in text or 'current_setting(phxclaw.tenant_uuid' in text
  if p.name=='0035_ai_sre_autopilot.sql':
   good="current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid" in text
  else: good=True
  checks.append({'name':p.name,'pass':good and not bad})
report={'suite':'v0.35 migration quoting','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}
out=(BASE/'reports'); out.mkdir(parents=True,exist_ok=True); (out/'V035_MIGRATION_QUOTING_REPORT.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2)); sys.exit(1 if report['fail'] else 0)

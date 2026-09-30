#!/usr/bin/env python3
import argparse,json,hashlib,sys
from pathlib import Path
REQ_DEFAULT=['cargo_fmt','cargo_check','cargo_test','cargo_clippy','postgresql_e2e','rls_e2e','runtime_e2e']
def h(o):return hashlib.sha256(json.dumps(o,sort_keys=True,separators=(',',':')).encode()).hexdigest()
ap=argparse.ArgumentParser();ap.add_argument('evidence',type=Path);ap.add_argument('--source-state',required=True);ap.add_argument('--out',type=Path);a=ap.parse_args()
evs=json.loads(a.evidence.read_text());out=[]
for i in range(26):
 s=f'F{i:02d}';req=REQ_DEFAULT;seen={}
 for e in evs:
  if e.get('sprint_id')==s and e.get('source_state_sha256')==a.source_state:seen[e['gate_id']]=e['result']
 missing=[g for g in req if g not in seen];fail=[g for g,v in seen.items() if v=='fail' and g in req];un=[g for g,v in seen.items() if v=='unavailable' and g in req]
 green=not missing and not fail and not un and all(seen.get(g)=='pass' for g in req)
 st={'sprint_id':s,'color':'green' if green else 'yellow','missing_gates':missing,'failing_gates':fail,'unavailable_gates':un};st['evidence_sha256']=h(st);out.append(st)
summary={'source_state_sha256':a.source_state,'green':sum(x['color']=='green' for x in out),'yellow':sum(x['color']=='yellow' for x in out),'red':0,'sprints':out}
text=json.dumps(summary,indent=2);print(text)
if a.out:a.out.write_text(text+'\n')

#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,sys
ROOT=Path(__file__).resolve().parents[1]
CANON=ROOT/'config/project-state-canonical.v062.json'
def digest(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    d=json.loads(CANON.read_text())
    assert d['project']=='PhxClaw' and d['version']=='0.62.0'
    assert len(d['sprints'])==26 and {x['id'] for x in d['sprints']}=={f'F{i:02d}' for i in range(26)}
    errors=[]
    for s in d['sprints']:
        p=ROOT/'sprints'/f"{s['id'].lower()}.json"; x=json.loads(p.read_text())
        for k in ['canonical_project_version','canonical_state_ref','implementation_state','verification_state','open_gaps']:
            if k not in x: errors.append(f"{s['id']} missing {k}")
        if x.get('implementation_state')!=s['implementation_state'] or x.get('verification_state')!=s['verification_state'] or x.get('open_gaps')!=s['gaps']:
            errors.append(f"{s['id']} diverges from canonical state")
    print(json.dumps({'canonical':str(CANON.relative_to(ROOT)),'sha256':digest(CANON),'sprints':26,'errors':errors},indent=2))
    return 1 if errors else 0
if __name__=='__main__': raise SystemExit(main())

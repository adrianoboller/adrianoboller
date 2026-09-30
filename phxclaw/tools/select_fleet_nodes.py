#!/usr/bin/env python3
from pathlib import Path
import argparse,json,sys
sys.path.insert(0,str(Path(__file__).resolve().parent)); from v028_common import *
def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--rollout-plan',type=Path,required=True); ap.add_argument('--stage-index',type=int,required=True); ap.add_argument('--nodes',type=Path,required=True); ap.add_argument('--out',type=Path,required=True); a=ap.parse_args(); root=a.root.resolve(); p=load_json(a.rollout_plan); verify_fleet(root,p['signer_key_id'],'fleet.rollout',p['signature_b64'],rollout_payload(p),'rollout plan'); s=next((x for x in p['stages'] if int(x['index'])==a.stage_index),None)
    if s is None: raise SystemExit('unknown rollout stage')
    rows=load_json(a.nodes); out=[]; seen=set()
    for n in rows:
        uid=n['node_uuid'];
        if uid in seen: raise SystemExit('duplicate node UUID')
        seen.add(uid)
        b=cohort_bucket(uid,p['rollout_uuid']); out.append({'node_uuid':uid,'cohort_bucket':b,'selected':b<int(s['percent'])*100})
    write_json(a.out,out); print(json.dumps({'status':'selected','percent':s['percent'],'selected':sum(x['selected'] for x in out),'total':len(out),'out':str(a.out)},indent=2))
if __name__=='__main__': main()

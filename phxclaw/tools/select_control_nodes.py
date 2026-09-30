#!/usr/bin/env python3
import argparse,json
from v029_common import *
ap=argparse.ArgumentParser();ap.add_argument('--snapshot',required=True);ap.add_argument('--inventory',required=True);ap.add_argument('--max-fanout',type=int,required=True);a=ap.parse_args()
s=load(a.snapshot); nodes=load(a.inventory)
if sha_obj(s['selector'])!=s['selector_sha256']: raise SystemExit('selector hash mismatch')
ids=membership(nodes,s['selector'])
if len(ids)!=s['member_count'] or sha_members(ids)!=s['membership_sha256']: raise SystemExit('membership drift detected')
if len(ids)>a.max_fanout: raise SystemExit('fanout exceeds policy')
print(json.dumps(ids,indent=2))

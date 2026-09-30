#!/usr/bin/env python3
import argparse,json
from v029_common import *
ap=argparse.ArgumentParser();ap.add_argument('--snapshot',required=True);ap.add_argument('--inventory',required=True);ap.add_argument('--rollout-uuid',required=True);ap.add_argument('--percent',type=int,required=True);a=ap.parse_args()
if not 1<=a.percent<=100: raise SystemExit('percent out of range')
s=load(a.snapshot); nodes=load(a.inventory); ids=membership(nodes,s['selector'])
if sha_members(ids)!=s['membership_sha256'] or len(ids)!=s['member_count']: raise SystemExit('membership drift detected')
sel=[x for x in ids if bucket(x,a.rollout_uuid)<a.percent*100]
print(json.dumps({'rollout_uuid':a.rollout_uuid,'percent':a.percent,'selected':sel,'selected_count':len(sel),'membership_sha256':s['membership_sha256']},indent=2))

#!/usr/bin/env python3
import argparse,json
from datetime import datetime,timezone
from v029_common import *
ap=argparse.ArgumentParser();ap.add_argument('--inventory',required=True);ap.add_argument('--policy',required=True);ap.add_argument('--now');a=ap.parse_args()
p=load(a.policy); nodes=load(a.inventory); now=datetime.fromisoformat(a.now.replace('Z','+00:00')) if a.now else datetime.now(timezone.utc)
out=[]
for n in nodes:
 st=n.get('device_state','active')
 if st=='revoked': c='revoked'
 elif st=='quarantined': c='quarantined'
 elif n.get('maintenance',False): c='maintenance'
 elif not n.get('last_heartbeat_at'): c='offline'
 else:
  age=(now-datetime.fromisoformat(n['last_heartbeat_at'].replace('Z','+00:00'))).total_seconds()
  c='offline' if age>p['offline_after_seconds'] else ('stale' if age>p['stale_after_seconds'] else 'online')
 m=dict(n);m['control_state']=c;out.append(m)
print(json.dumps(out,indent=2,sort_keys=True))

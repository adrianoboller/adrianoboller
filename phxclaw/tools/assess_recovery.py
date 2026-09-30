#!/usr/bin/env python3
import argparse,json
from datetime import datetime,timezone
from v029_common import *
ap=argparse.ArgumentParser();ap.add_argument('--observation',required=True);ap.add_argument('--policy',required=True);ap.add_argument('--now');a=ap.parse_args();o=load(a.observation);p=load(a.policy)
now=datetime.fromisoformat(a.now.replace('Z','+00:00')) if a.now else datetime.now(timezone.utc); hb=datetime.fromisoformat(o['heartbeat_at'].replace('Z','+00:00'))
if o['claimed_sequence']<o['previous_sequence']: raise SystemExit('sequence regression: quarantine required')
if o['received_fencing_token']!=o['expected_fencing_token']: raise SystemExit('fencing mismatch')
if (now-hb).total_seconds()>p['recovery_grace_seconds']: raise SystemExit('recovery evidence too old')
print(json.dumps({'node_uuid':o['node_uuid'],'state':'recovering','reconcile_required':True}))

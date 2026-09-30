#!/usr/bin/env python3
from pathlib import Path
from datetime import datetime,timezone,timedelta
import argparse,json,sys
from packaging.version import Version
sys.path.insert(0,str(Path(__file__).resolve().parent)); from v028_common import *
def main():
 ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--authorization',type=Path,required=True); ap.add_argument('--rollout-uuid',required=True); ap.add_argument('--from-version',required=True); ap.add_argument('--to-version',required=True); ap.add_argument('--from-sequence',type=int,required=True); ap.add_argument('--to-sequence',type=int,required=True); a=ap.parse_args(); root=a.root.resolve(); o=load_json(a.authorization); verify_fleet(root,o['signer_key_id'],'fleet.rollback',o['signature_b64'],rollback_payload(o),'fleet rollback'); now=datetime.now(timezone.utc)
 if o['rollout_uuid']!=a.rollout_uuid or o['from_version']!=a.from_version or o['to_version']!=a.to_version or int(o['from_sequence'])!=a.from_sequence or int(o['to_sequence'])!=a.to_sequence: raise SystemExit('fleet rollback authorization scope mismatch')
 if parse_time(o['expires_at'])<=now or parse_time(o['created_at'])>now+timedelta(minutes=5) or parse_time(o['expires_at'])-parse_time(o['created_at'])>timedelta(hours=24): raise SystemExit('fleet rollback authorization expired/invalid')
 if int(o['to_sequence'])>=int(o['from_sequence']) or Version(o['to_version'])>=Version(o['from_version']): raise SystemExit('fleet rollback target is not older')
 print(json.dumps({'status':'verified','authorization_uuid':o['authorization_uuid'],'max_nodes':o['max_nodes']},indent=2))
if __name__=='__main__': main()

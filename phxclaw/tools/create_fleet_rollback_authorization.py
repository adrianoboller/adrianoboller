#!/usr/bin/env python3
from pathlib import Path
from datetime import datetime,timezone,timedelta
import argparse,json,sys
from packaging.version import Version
sys.path.insert(0,str(Path(__file__).resolve().parent)); from v028_common import *
def main():
 ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--rollout-plan',type=Path,required=True); ap.add_argument('--from-version',required=True); ap.add_argument('--to-version',required=True); ap.add_argument('--from-sequence',type=int,required=True); ap.add_argument('--to-sequence',type=int,required=True); ap.add_argument('--max-nodes',type=int,required=True); ap.add_argument('--reason',required=True); ap.add_argument('--out',type=Path,required=True); a=ap.parse_args(); root=a.root.resolve(); plan=load_json(a.rollout_plan); verify_fleet(root,plan['signer_key_id'],'fleet.rollout',plan['signature_b64'],rollout_payload(plan),'rollout plan')
 if a.from_version!=plan['version'] or a.from_sequence!=int(plan['sequence']): raise SystemExit('rollback authorization must start from rollout target')
 if a.to_sequence>=a.from_sequence or Version(a.to_version)>=Version(a.from_version): raise SystemExit('fleet rollback authorization must lower version and sequence')
 now=datetime.now(timezone.utc); o={'schema_version':'0.28.0','authorization_uuid':uuid7(),'rollout_uuid':plan['rollout_uuid'],'from_version':a.from_version,'to_version':a.to_version,'from_sequence':a.from_sequence,'to_sequence':a.to_sequence,'max_nodes':a.max_nodes,'reason':a.reason,'created_at':now.isoformat().replace('+00:00','Z'),'expires_at':(now+timedelta(hours=24)).isoformat().replace('+00:00','Z'),'signer_key_id':'','signature_algorithm':'ed25519','signature_b64':''}; sig=sign_fleet(root,rollback_payload(o),'fleet.rollback','fleet-rollback'); o.update(sig); write_json(a.out,o); print(json.dumps({'status':'authorized','authorization_uuid':o['authorization_uuid'],'out':str(a.out)},indent=2))
if __name__=='__main__': main()

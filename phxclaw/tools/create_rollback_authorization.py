#!/usr/bin/env python3
from pathlib import Path
from datetime import datetime, timezone, timedelta
import argparse, json
from packaging.version import Version
from v027_common import *
def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--from-version',required=True); ap.add_argument('--to-version',required=True); ap.add_argument('--from-sequence',type=int,required=True); ap.add_argument('--to-sequence',type=int,required=True); ap.add_argument('--reason',required=True); ap.add_argument('--expires-hours',type=int,default=24); ap.add_argument('--output',type=Path,required=True); args=ap.parse_args()
    if Version(args.to_version)>=Version(args.from_version): raise SystemExit('rollback authorization requires a lower semantic version')
    if args.to_sequence<=args.from_sequence: raise SystemExit('rollback metadata sequence must still increase')
    if not (1<=args.expires_hours<=72): raise SystemExit('rollback authorization lifetime must be 1..72 hours')
    now=utcnow(); exp=(datetime.now(timezone.utc)+timedelta(hours=args.expires_hours)).isoformat().replace('+00:00','Z')
    o={'schema_version':'0.27.0','authorization_uuid':uuid7(),'from_version':args.from_version,'to_version':args.to_version,'from_sequence':args.from_sequence,'to_sequence':args.to_sequence,'reason':args.reason,'created_at':now,'expires_at':exp,'signer_key_id':None,'signature_algorithm':None,'signature_b64':None}
    sig=sign_payload(args.root.resolve(),rollback_payload(o),'rollback-authorization'); o.update({'signer_key_id':sig['signer_key_id'],'signature_algorithm':sig.get('signature_algorithm','ed25519'),'signature_b64':sig['signature_b64']}); write_json(args.output,o); print(json.dumps({'status':'signed','authorization_uuid':o['authorization_uuid'],'from_version':o['from_version'],'to_version':o['to_version']},indent=2))
if __name__=='__main__': main()

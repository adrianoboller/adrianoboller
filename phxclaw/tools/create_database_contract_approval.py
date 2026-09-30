#!/usr/bin/env python3
from pathlib import Path
from datetime import datetime,timezone,timedelta
import argparse,json,sys
sys.path.insert(0,str(Path(__file__).resolve().parent)); from v028_common import *
def main():
 ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--component-plan',type=Path,required=True); ap.add_argument('--fleet-state',type=Path,required=True); ap.add_argument('--out',type=Path,required=True); a=ap.parse_args(); root=a.root.resolve(); plan=load_json(a.component_plan); state=load_json(a.fleet_state); verify_fleet(root,state['signer_key_id'],'fleet.state',state['signature_b64'],state_payload(state),'fleet state')
 if state['state']!='completed': raise SystemExit('database contract approval requires completed fleet rollout')
 now=datetime.now(timezone.utc); o={'schema_version':'0.28.0','approval_uuid':uuid7(),'component_plan_sha256':sha_file(a.component_plan),'fleet_state_sha256':sha_file(a.fleet_state),'target_core_version':plan['target_core_version'],'fleet_compatible_percent':100,'created_at':now.isoformat().replace('+00:00','Z'),'expires_at':(now+timedelta(hours=8)).isoformat().replace('+00:00','Z'),'signer_key_id':'','signature_algorithm':'ed25519','signature_b64':''}; sig=sign_fleet(root,db_contract_payload(o),'fleet.database_contract','database-contract'); o.update(sig); write_json(a.out,o); print(json.dumps({'status':'approved','approval_uuid':o['approval_uuid'],'out':str(a.out)},indent=2))
if __name__=='__main__': main()

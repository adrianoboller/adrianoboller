#!/usr/bin/env python3
from pathlib import Path
from datetime import datetime, timezone
import argparse, json, sys
sys.path.insert(0,str(Path(__file__).resolve().parent)); from v028_common import *
def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--rollout-plan',type=Path,required=True); ap.add_argument('--stage-index',type=int,required=True); ap.add_argument('--metrics',type=Path,required=True); ap.add_argument('--out',type=Path,required=True); args=ap.parse_args(); root=args.root.resolve(); plan=load_json(args.rollout_plan); verify_fleet(root,plan['signer_key_id'],'fleet.rollout',plan['signature_b64'],rollout_payload(plan),'rollout plan'); m=load_json(args.metrics)
    required=['samples','success_rate','crash_rate','rollback_rate','stale_heartbeat_rate','install_error_rate','window_started_at'];
    if any(k not in m for k in required): raise SystemExit('metrics missing required fields')
    now=datetime.now(timezone.utc); o={'schema_version':'0.28.0','evidence_uuid':uuid7(),'rollout_uuid':plan['rollout_uuid'],'stage_index':args.stage_index,**{k:m[k] for k in required},'created_at':now.isoformat().replace('+00:00','Z'),'signer_key_id':'','signature_algorithm':'ed25519','signature_b64':''}
    sig=sign_fleet(root,health_payload(o),'fleet.health','health-evidence'); o.update(sig); write_json(args.out,o); print(json.dumps({'status':'created','evidence_uuid':o['evidence_uuid'],'out':str(args.out)},indent=2))
if __name__=='__main__': main()

#!/usr/bin/env python3
from pathlib import Path
from datetime import datetime, timezone, timedelta
import argparse, json, sys
sys.path.insert(0,str(Path(__file__).resolve().parent))
from v028_common import *
def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--update-manifest',type=Path,required=True); ap.add_argument('--tenant-uuid',required=True); ap.add_argument('--channel',choices=['canary','beta','stable'],required=True); ap.add_argument('--out',type=Path,required=True); args=ap.parse_args(); root=args.root.resolve()
    u=load_json(args.update_manifest); policy=load_json(root/'config/fleet-rollout-policy.v028.json')
    if u.get('schema_version')!='0.27.0' or u.get('channel')!='stable': raise SystemExit('fleet rollout requires a signed stable v0.27 GA update manifest')
    sys.path.insert(0,str(root/'tools')); from v027_common import verify_ed25519, update_payload
    verify_ed25519(root,u['signer_key_id'],u['signature_b64'],update_payload(u),'release update manifest')
    now=datetime.now(timezone.utc); update_exp=parse_time(u['expires_at'])
    if update_exp <= now: raise SystemExit('update manifest is expired')
    stages=[]
    for i,s in enumerate(policy['channel_stages'][args.channel]): stages.append({'index':i,**s})
    o={'schema_version':'0.28.0','rollout_uuid':uuid7(),'tenant_uuid':args.tenant_uuid,'channel':args.channel,'version':u['version'],'sequence':int(u['sequence']),'source_state_sha256':u['source_state_sha256'].lower(),'update_manifest_sha256':sha_file(args.update_manifest),'policy_sha256':sha_file(root/'config/fleet-rollout-policy.v028.json'),'health_thresholds':policy['health_thresholds'],'critical_thresholds':policy['critical_failure'],'health_evidence_max_age_seconds':policy['health_evidence_max_age_seconds'],'stages':stages,'created_at':now.isoformat().replace('+00:00','Z'),'expires_at':min(now+timedelta(days=14),update_exp).isoformat().replace('+00:00','Z'),'signer_key_id':'','signature_algorithm':'ed25519','signature_b64':''}
    sig=sign_fleet(root,rollout_payload(o),'fleet.rollout','rollout-plan'); o.update(sig); write_json(args.out,o); print(json.dumps({'status':'created','rollout_uuid':o['rollout_uuid'],'channel':o['channel'],'stages':len(stages),'out':str(args.out)},indent=2))
if __name__=='__main__': main()

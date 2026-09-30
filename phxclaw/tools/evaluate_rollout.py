#!/usr/bin/env python3
from pathlib import Path
from datetime import datetime, timezone, timedelta
import argparse, json, sys
sys.path.insert(0,str(Path(__file__).resolve().parent)); from v028_common import *
def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--rollout-plan',type=Path,required=True); ap.add_argument('--health-evidence',type=Path,required=True); ap.add_argument('--state',type=Path); ap.add_argument('--out',type=Path,required=True); args=ap.parse_args(); root=args.root.resolve(); policy_path=root/'config/fleet-rollout-policy.v028.json'; policy=load_json(policy_path); plan=load_json(args.rollout_plan); health=load_json(args.health_evidence)
    if plan.get('policy_sha256')!=sha_file(policy_path): raise SystemExit('rollout policy changed after plan signing')
    verify_fleet(root,plan['signer_key_id'],'fleet.rollout',plan['signature_b64'],rollout_payload(plan),'rollout plan'); verify_fleet(root,health['signer_key_id'],'fleet.health',health['signature_b64'],health_payload(health),'health evidence')
    now=datetime.now(timezone.utc); stage=next((s for s in plan['stages'] if int(s['index'])==int(health['stage_index'])),None)
    if stage is None or health['rollout_uuid']!=plan['rollout_uuid']: raise SystemExit('health evidence rollout/stage mismatch')
    if parse_time(health['created_at']) < parse_time(health['window_started_at']) or now-parse_time(health['created_at'])>timedelta(seconds=int(plan['health_evidence_max_age_seconds'])): raise SystemExit('health evidence stale/invalid')
    if parse_time(health['created_at'])-parse_time(health['window_started_at'])<timedelta(seconds=int(stage['soak_seconds'])): raise SystemExit('soak period incomplete')
    if int(health['samples'])<int(stage['min_samples']): raise SystemExit('insufficient health samples')
    t=plan['health_thresholds']; vals={k:float(health[k]) for k in ['success_rate','crash_rate','rollback_rate','stale_heartbeat_rate','install_error_rate']}
    if any(v<0 or v>1 for v in vals.values()): raise SystemExit('invalid health rate')
    critical=vals['crash_rate']>=plan['critical_thresholds']['crash_rate_gte'] or vals['install_error_rate']>=plan['critical_thresholds']['install_error_rate_gte'] or vals['rollback_rate']>=plan['critical_thresholds']['rollback_rate_gte']
    healthy=vals['success_rate']>=t['min_success_rate'] and vals['crash_rate']<=t['max_crash_rate'] and vals['rollback_rate']<=t['max_rollback_rate'] and vals['stale_heartbeat_rate']<=t['max_stale_heartbeat_rate'] and vals['install_error_rate']<=t['max_install_error_rate']
    old=load_json(args.state) if args.state else None
    if old:
        verify_fleet(root,old['signer_key_id'],'fleet.state',old['signature_b64'],state_payload(old),'fleet state');
        if old['rollout_uuid']!=plan['rollout_uuid'] or int(old['stage_index'])!=int(stage['index']): raise SystemExit('state rollout/stage mismatch')
        gen=int(old['generation'])+1; fence=int(old['fencing_token'])+1
    else: gen=1; fence=1
    if critical: new_state='rollback_required'; reason='critical_health_threshold'
    elif not healthy: new_state='paused'; reason='health_threshold_failed'
    elif int(stage['index'])+1 < len(plan['stages']): new_state='active'; reason='advance_stage_verified'; stage={'index':int(stage['index'])+1}
    else: new_state='completed'; reason='all_stages_verified'; stage={'index':int(stage['index'])}
    o={'schema_version':'0.28.0','rollout_uuid':plan['rollout_uuid'],'generation':gen,'fencing_token':fence,'state':new_state,'stage_index':stage['index'],'updated_at':now.isoformat().replace('+00:00','Z'),'reason':reason,'signer_key_id':'','signature_algorithm':'ed25519','signature_b64':''}; sig=sign_fleet(root,state_payload(o),'fleet.state','fleet-state'); o.update(sig); write_json(args.out,o); print(json.dumps({'status':new_state,'stage_index':o['stage_index'],'generation':gen,'out':str(args.out)},indent=2))
if __name__=='__main__': main()

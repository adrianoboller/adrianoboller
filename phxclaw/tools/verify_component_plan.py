#!/usr/bin/env python3
from pathlib import Path
import argparse,json,sys
from packaging.version import Version
sys.path.insert(0,str(Path(__file__).resolve().parent)); from v028_common import *
ORDER=['preflight','backup','database_expand','core','plugin','health_check','database_contract','cleanup']
def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--plan',type=Path,required=True); ap.add_argument('--fleet-compatible-percent',type=int,default=0); ap.add_argument('--fleet-state',type=Path); ap.add_argument('--database-contract-approval',type=Path); args=ap.parse_args(); root=args.root.resolve(); o=load_json(args.plan);
    if o.get('policy_sha256')!=sha_file(root/'config/component-update-policy.v028.json'): raise SystemExit('component policy changed after plan signing')
    verify_fleet(root,o['signer_key_id'],'fleet.component_plan',o['signature_b64'],component_plan_payload(o),'component plan')
    comps=o.get('components',[]); ids={x['component_uuid'] for x in comps}
    if len(ids)!=len(comps): raise SystemExit('duplicate component UUID')
    target=Version(o['target_core_version'])
    by={x['component_uuid']:x for x in comps}; indeg={i:0 for i in ids}; rev={i:[] for i in ids}
    for c in comps:
        if not (len(c['sha256'])==64 and all(ch in '0123456789abcdefABCDEF' for ch in c['sha256'])): raise SystemExit('invalid component digest')
        if c['kind']=='plugin':
            if c.get('core_min') and target<Version(c['core_min']): raise SystemExit('plugin requires newer core')
            if c.get('core_max_exclusive') and target>=Version(c['core_max_exclusive']): raise SystemExit('plugin excludes target core')
        for d in c.get('dependencies',[]):
            if d not in ids: raise SystemExit('unknown component dependency')
            indeg[c['component_uuid']]+=1; rev[d].append(c['component_uuid'])
    q=sorted([i for i,n in indeg.items() if n==0]); result=[]
    while q:
        i=q.pop(0); result.append(i)
        for x in rev[i]:
            indeg[x]-=1
            if indeg[x]==0:q.append(x);q.sort()
    if len(result)!=len(comps): raise SystemExit('component dependency cycle')
    kinds=[by[i]['kind'] for i in result]
    rank=[ORDER.index(k) for k in kinds]
    if rank!=sorted(rank): raise SystemExit('component dependency order violates expand/core/plugin/contract policy')
    if 'database_contract' in kinds:
        if args.fleet_compatible_percent!=100 or not args.fleet_state or not args.database_contract_approval: raise SystemExit('database_contract blocked until 100% compatible + signed approval')
        state=load_json(args.fleet_state); verify_fleet(root,state['signer_key_id'],'fleet.state',state['signature_b64'],state_payload(state),'fleet state')
        if state['state']!='completed': raise SystemExit('database_contract requires completed fleet state')
        approval=load_json(args.database_contract_approval); verify_fleet(root,approval['signer_key_id'],'fleet.database_contract',approval['signature_b64'],db_contract_payload(approval),'database contract approval')
        from datetime import datetime,timezone,timedelta
        now=datetime.now(timezone.utc)
        if parse_time(approval['expires_at'])<=now or parse_time(approval['created_at'])>now+timedelta(minutes=5) or parse_time(approval['expires_at'])-parse_time(approval['created_at'])>timedelta(hours=8): raise SystemExit('database contract approval expired/invalid')
        if approval['component_plan_sha256']!=sha_file(args.plan) or approval['fleet_state_sha256']!=sha_file(args.fleet_state) or approval['target_core_version']!=o['target_core_version'] or int(approval['fleet_compatible_percent'])!=100: raise SystemExit('database contract approval scope mismatch')
    print(json.dumps({'status':'verified','ordered_components':result,'kinds':kinds},indent=2))
if __name__=='__main__': main()

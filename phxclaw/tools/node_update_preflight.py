#!/usr/bin/env python3
from pathlib import Path
from packaging.version import Version
import argparse,json,subprocess,sys
sys.path.insert(0,str(Path(__file__).resolve().parent)); from v028_common import *
def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--rollout-plan',type=Path,required=True); ap.add_argument('--update-manifest',type=Path,required=True); ap.add_argument('--node-state',type=Path,required=True); ap.add_argument('--node-uuid',required=True); ap.add_argument('--stage-index',type=int,required=True); ap.add_argument('--platform',required=True); ap.add_argument('--architecture',required=True); ap.add_argument('--artifact',type=Path); args=ap.parse_args(); root=args.root.resolve(); p=load_json(args.rollout_plan); u=load_json(args.update_manifest); n=load_json(args.node_state)
    verify_fleet(root,p['signer_key_id'],'fleet.rollout',p['signature_b64'],rollout_payload(p),'rollout plan')
    if sha_file(args.update_manifest)!=p['update_manifest_sha256'] or u['version']!=p['version'] or int(u['sequence'])!=int(p['sequence']) or u['source_state_sha256'].lower()!=p['source_state_sha256'].lower(): raise SystemExit('rollout plan/update manifest identity mismatch')
    stage=next((x for x in p['stages'] if int(x['index'])==args.stage_index),None)
    if stage is None or cohort_bucket(args.node_uuid,p['rollout_uuid'])>=int(stage['percent'])*100: raise SystemExit('node not selected for rollout stage')
    if int(p['sequence'])<=int(n['current_sequence']): raise SystemExit('anti-rollback sequence rejected')
    cmd=[sys.executable,str(root/'tools/verify_update_manifest.py'),str(root),'--manifest',str(args.update_manifest),'--current-version',n['current_version'],'--current-sequence',str(n['current_sequence']),'--platform',args.platform,'--architecture',args.architecture]
    if args.artifact: cmd += ['--artifact',str(args.artifact)]
    r=subprocess.run(cmd,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True); 
    if r.returncode: raise SystemExit('underlying v0.27 update verification failed: '+r.stderr[:800])
    print(json.dumps({'status':'eligible','node_uuid':args.node_uuid,'rollout_uuid':p['rollout_uuid'],'stage_index':args.stage_index,'cohort_bucket':cohort_bucket(args.node_uuid,p['rollout_uuid']),'target_version':p['version'],'target_sequence':p['sequence']},indent=2))
if __name__=='__main__': main()

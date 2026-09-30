#!/usr/bin/env python3
from __future__ import annotations
from pathlib import Path
from datetime import datetime, timezone, timedelta
import argparse, json
from packaging.version import Version
from v027_common import *
def parse_time(s): return datetime.fromisoformat(s.replace('Z','+00:00'))
def verify_rollback(root,token,current_version,target_version,current_seq,target_seq,now):
    if token is None: return False
    o=load_json(token)
    if o.get('schema_version')!='0.27.0' or o.get('signature_algorithm')!='ed25519': raise SystemExit('invalid rollback authorization')
    verify_ed25519(root,o['signer_key_id'],o['signature_b64'],rollback_payload(o),'rollback authorization')
    created=parse_time(o['created_at']); exp=parse_time(o['expires_at'])
    if exp<=now or created>now+timedelta(minutes=5) or exp-created>timedelta(hours=72): raise SystemExit('rollback authorization lifetime invalid')
    if o['from_version']!=current_version or o['to_version']!=target_version or int(o['from_sequence'])!=current_seq or int(o['to_sequence'])!=target_seq: raise SystemExit('rollback authorization scope mismatch')
    if Version(o['to_version'])>=Version(o['from_version']): raise SystemExit('rollback authorization is not a downgrade')
    return True
def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--manifest',type=Path,required=True); ap.add_argument('--current-version',required=True); ap.add_argument('--current-sequence',type=int,required=True); ap.add_argument('--platform',required=True); ap.add_argument('--architecture',required=True); ap.add_argument('--artifact',type=Path); ap.add_argument('--rollback-authorization',type=Path); args=ap.parse_args()
    root=args.root.resolve(); o=load_json(args.manifest)
    if o.get('schema_version')!='0.27.0' or o.get('signature_algorithm')!='ed25519' or o.get('channel') not in {'stable','rc'}: raise SystemExit('invalid update manifest')
    verify_ed25519(root,o['signer_key_id'],o['signature_b64'],update_payload(o),'update manifest')
    now=datetime.now(timezone.utc); created=parse_time(o['created_at']); exp=parse_time(o['expires_at'])
    if exp<=now or created>now+timedelta(minutes=5) or exp-created>timedelta(hours=168): raise SystemExit('update manifest lifetime invalid')
    if int(o['sequence'])<=args.current_sequence: raise SystemExit('update sequence is not monotonic')
    keys=[(x['platform'],x['architecture']) for x in o.get('targets',[])]
    if len(keys)!=len(set(keys)) or not (1<=len(keys)<=16): raise SystemExit('duplicate/invalid update targets')
    target=next((x for x in o['targets'] if x['platform']==args.platform and x['architecture']==args.architecture),None)
    if target is None: raise SystemExit('no update target for platform/architecture')
    if not target['url'].startswith('https://') or not valid_sha256(target.get('sha256','')) or int(target.get('size',0))<1: raise SystemExit('invalid update target metadata')
    downgrade=Version(o['version'])<Version(args.current_version); authorized=verify_rollback(root,args.rollback_authorization,args.current_version,o['version'],args.current_sequence,int(o['sequence']),now) if downgrade else False
    if downgrade and not authorized: raise SystemExit('downgrade denied without trusted rollback authorization')
    if args.artifact:
        p=args.artifact.resolve()
        if p.is_symlink() or not p.is_file(): raise SystemExit('artifact must be a regular non-symlink file')
        if sha_file(p)!=target['sha256'] or p.stat().st_size!=target['size']: raise SystemExit('downloaded artifact digest/size mismatch')
    print(json.dumps({'status':'verified','version':o['version'],'sequence':o['sequence'],'platform':args.platform,'architecture':args.architecture,'downgrade':downgrade,'rollback_authorized':authorized,'target_sha256':target['sha256']},indent=2))
if __name__=='__main__': main()

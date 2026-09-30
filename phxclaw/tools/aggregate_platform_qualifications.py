#!/usr/bin/env python3
from __future__ import annotations
from pathlib import Path
import argparse, json
from v027_common import *
def validate(o,root,first_release):
    if o.get('schema_version')!='0.27.0' or o.get('signature_algorithm')!='ed25519': raise SystemExit('invalid platform evidence schema/signature algorithm')
    if bool(o.get('first_release'))!=bool(first_release): raise SystemExit('platform evidence first_release mismatch')
    if o.get('evidence_bundle_sha256')!=evidence_bundle_sha256(o.get('evidence_files',[])): raise SystemExit('platform evidence bundle digest mismatch')
    for k in ('source_state_sha256','rc_archive_sha256','artifact_sha256','evidence_bundle_sha256'):
        if not valid_sha256(o.get(k,'')): raise SystemExit('invalid platform digest: '+k)
    verify_ed25519(root,o['signer_key_id'],o['signature_b64'],platform_payload(o),f"{o.get('platform')} evidence")
    if o.get('native_tests')!='verified' or o.get('fresh_install')!='verified': raise SystemExit('platform native/fresh install evidence incomplete')
    if not first_release and (o.get('upgrade_n_minus_1')!='verified' or o.get('rollback_or_restore')!='verified' or not o.get('previous_version')): raise SystemExit('N/N-1 evidence incomplete')
    if o.get('platform')=='windows' and o.get('code_signing')!='verified': raise SystemExit('Windows code signing not verified')
    if o.get('platform')=='macos' and (o.get('code_signing')!='verified' or o.get('notarization')!='verified'): raise SystemExit('macOS signing/notarization not verified')
def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--evidence',type=Path,action='append',required=True); ap.add_argument('--output-dir',type=Path,required=True); ap.add_argument('--first-release',action='store_true'); args=ap.parse_args()
    root=args.root.resolve(); out=args.output_dir.resolve();
    if out.exists() and any(out.iterdir()): raise SystemExit('output-dir must be empty')
    out.mkdir(parents=True,exist_ok=True); cfg=load_json(root/'config/multi-platform-qualification.v027.json'); objs=[]
    for p in args.evidence:
        p=p.resolve(); o=load_json(p); validate(o,root,args.first_release); objs.append((p,o))
    required={(x['platform'],x['architecture']) for x in cfg['required_platforms']}; keys=[(o['platform'],o['architecture']) for _,o in objs]
    if len(keys)!=len(set(keys)): raise SystemExit('duplicate platform evidence target')
    if set(keys)!=required or len(keys)!=len(required): raise SystemExit(f'platform set mismatch: required={sorted(required)} got={sorted(keys)}')
    sources={o['source_state_sha256'] for _,o in objs}; candidates={o['candidate_version'] for _,o in objs}; rcs={o['rc_archive_sha256'] for _,o in objs}; prev={o.get('previous_version') for _,o in objs}
    if len(sources)!=1 or len(candidates)!=1 or len(rcs)!=1 or len(prev)!=1: raise SystemExit('platform identity/previous-version mismatch')
    rows=[]
    for p,o in sorted(objs,key=lambda x:(x[1]['platform'],x[1]['architecture'])):
        rows.append({'platform':o['platform'],'architecture':o['architecture'],'candidate_version':o['candidate_version'],'source_state_sha256':o['source_state_sha256'],'rc_archive_sha256':o['rc_archive_sha256'],'artifact_name':o['artifact_name'],'artifact_sha256':o['artifact_sha256'],'artifact_size':o['artifact_size'],'fresh_install':o['fresh_install'],'upgrade_n_minus_1':o['upgrade_n_minus_1'],'rollback_or_restore':o['rollback_or_restore'],'native_tests':o['native_tests'],'code_signing':o['code_signing'],'notarization':o['notarization'],'evidence_sha256':sha_file(p),'evidence_uuid':o['evidence_uuid'],'signer_key_id':o['signer_key_id']})
    previous=next(iter(prev)); matrix={'schema_version':'0.27.0','candidate_version':next(iter(candidates)),'source_state_sha256':next(iter(sources)),'rc_archive_sha256':next(iter(rcs)),'first_release':bool(args.first_release),'previous_version':previous,'platforms':rows,'status':'qualified','created_at':utcnow()}; mp=out/'compatibility-n-n1.json'; write_json(mp,matrix)
    platform_digest=sha_bytes(json.dumps([{'platform':r['platform'],'architecture':r['architecture'],'evidence_sha256':r['evidence_sha256']} for r in rows],separators=(',',':'),ensure_ascii=False).encode())
    agg={'schema_version':'0.27.0','aggregate_uuid':uuid7(),'candidate_version':next(iter(candidates)),'source_state_sha256':next(iter(sources)),'rc_archive_sha256':next(iter(rcs)),'first_release':bool(args.first_release),'previous_version':previous,'platform_bundle_sha256':platform_digest,'compatibility_matrix_sha256':sha_file(mp),'platforms':rows,'created_at':utcnow(),'signer_key_id':None,'signature_algorithm':None,'signature_b64':None}
    sig=sign_payload(root,aggregate_payload(agg),'multiplatform-qualification'); agg.update({'signer_key_id':sig['signer_key_id'],'signature_algorithm':sig.get('signature_algorithm','ed25519'),'signature_b64':sig['signature_b64']}); write_json(out/'multi-platform-qualification.json',agg)
    print(json.dumps({'status':'qualified','platforms':len(rows),'source_state_sha256':agg['source_state_sha256'],'candidate_version':agg['candidate_version'],'rc_archive_sha256':agg['rc_archive_sha256'],'previous_version':previous},indent=2))
if __name__=='__main__': main()

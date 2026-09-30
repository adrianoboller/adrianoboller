#!/usr/bin/env python3
from __future__ import annotations
from pathlib import Path
import argparse, json, subprocess, sys
from v027_common import *
REQ={'native_tests':'native-tests.log','fresh_install':'fresh-install.log','upgrade_n_minus_1':'upgrade-n-minus-1.log','rollback_or_restore':'rollback-or-restore.log','code_signing':'code-signing.log','notarization':'notarization.log'}
def current_source(root):
    r=subprocess.run([sys.executable,str(root/'tools/source_state.py'),str(root)],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=300)
    if r.returncode: raise SystemExit('source-state calculation failed: '+r.stderr[:800])
    return r.stdout.strip().splitlines()[-1].lower()
def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--artifact',type=Path,required=True); ap.add_argument('--evidence-dir',type=Path,required=True); ap.add_argument('--output',type=Path,required=True); args=ap.parse_args()
    root=args.root.resolve(); art=args.artifact.resolve(); ed=args.evidence_dir.resolve(); out=args.output.resolve()
    if art.is_symlink() or not art.is_file(): raise SystemExit('artifact must be a regular non-symlink file')
    status=load_json(ed/'status.json')
    if status.get('origin')!='phxclaw-native-platform-runner-v027': raise SystemExit('status.json was not produced by native platform runner')
    if current_source(root)!=status.get('source_state_sha256','').lower(): raise SystemExit('current source-state differs from native runner status')
    if art.name!=status.get('artifact_name') or sha_file(art)!=status.get('artifact_sha256') or art.stat().st_size!=status.get('artifact_size'): raise SystemExit('artifact differs from native runner status')
    rows=[]; checks=status.get('checks',{}); declared_logs=status.get('logs',{})
    for field,name in REQ.items():
        value=checks.get(field,'missing'); p=ed/name
        if not p.is_file() or p.is_symlink() or p.stat().st_size==0: raise SystemExit(f'missing/non-regular evidence log: {name}')
        row={'name':name,'sha256':sha_file(p),'size':p.stat().st_size}; rows.append(row)
        d=declared_logs.get(name,{})
        if d.get('sha256')!=row['sha256'] or d.get('size')!=row['size']: raise SystemExit('evidence log differs from runner status: '+name)
    if checks.get('native_tests')!='verified' or checks.get('fresh_install')!='verified': raise SystemExit('native_tests/fresh_install not verified')
    if not status.get('first_release') and (checks.get('upgrade_n_minus_1')!='verified' or checks.get('rollback_or_restore')!='verified' or not status.get('previous_version')): raise SystemExit('N/N-1 evidence incomplete')
    if status.get('platform')=='windows' and checks.get('code_signing')!='verified': raise SystemExit('Windows Authenticode evidence required')
    if status.get('platform')=='macos' and (checks.get('code_signing')!='verified' or checks.get('notarization')!='verified'): raise SystemExit('macOS signing/notarization evidence required')
    for k in ('source_state_sha256','rc_archive_sha256','artifact_sha256'):
        if not valid_sha256(status.get(k,'')): raise SystemExit('invalid '+k)
    obj={'schema_version':'0.27.0','evidence_uuid':uuid7(),'platform':status['platform'],'architecture':status['architecture'],'candidate_version':status['candidate_version'],'source_state_sha256':status['source_state_sha256'].lower(),'rc_archive_sha256':status['rc_archive_sha256'].lower(),'artifact_name':art.name,'artifact_sha256':status['artifact_sha256'].lower(),'artifact_size':status['artifact_size'],'evidence_files':rows,'evidence_bundle_sha256':evidence_bundle_sha256(rows),'fresh_install':checks.get('fresh_install','missing'),'upgrade_n_minus_1':checks.get('upgrade_n_minus_1','missing'),'rollback_or_restore':checks.get('rollback_or_restore','missing'),'native_tests':checks.get('native_tests','missing'),'code_signing':checks.get('code_signing','missing'),'notarization':checks.get('notarization','missing'),'first_release':bool(status.get('first_release')),'previous_version':status.get('previous_version'),'created_at':utcnow(),'signer_key_id':None,'signature_algorithm':None,'signature_b64':None}
    sig=sign_payload(root,platform_payload(obj),f"platform-{obj['platform']}-{obj['architecture']}"); obj.update({'signer_key_id':sig['signer_key_id'],'signature_algorithm':sig.get('signature_algorithm','ed25519'),'signature_b64':sig['signature_b64']}); write_json(out,obj)
    print(json.dumps({'status':'signed','output':str(out),'artifact_sha256':obj['artifact_sha256'],'evidence_bundle_sha256':obj['evidence_bundle_sha256'],'signer_key_id':obj['signer_key_id']},indent=2))
if __name__=='__main__': main()

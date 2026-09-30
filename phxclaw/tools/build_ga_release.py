#!/usr/bin/env python3
from __future__ import annotations
from pathlib import Path
from datetime import datetime, timezone, timedelta
from urllib.parse import quote
import argparse, json, mimetypes, re, shutil, subprocess, sys
from packaging.version import Version
from v027_common import *
def safe_https_base(v):
    if not v.startswith('https://'): raise SystemExit('--base-url must use HTTPS')
    return v.rstrip('/')
def verify_rc(root,rc):
    r=subprocess.run([sys.executable,str(root/'tools/verify_release_candidate.py'),str(rc),'--trust-store',str(root/'config/trusted-release-signers.v025.json')],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=300)
    if r.returncode: raise SystemExit('RC verification failed: '+(r.stderr or r.stdout)[:1200])
    return json.loads(r.stdout)
def validate_aggregate(root,p):
    o=load_json(p)
    if o.get('schema_version')!='0.27.0' or o.get('signature_algorithm')!='ed25519': raise SystemExit('invalid aggregate schema/signature')
    verify_ed25519(root,o['signer_key_id'],o['signature_b64'],aggregate_payload(o),'multi-platform qualification'); return o
def parse_artifacts(items):
    out={}
    for raw in items:
        if '=' not in raw or ':' not in raw.split('=',1)[0]: raise SystemExit('--artifact requires platform:arch=/path')
        key,path=raw.split('=',1); platform,arch=key.split(':',1); k=(platform,arch)
        if k in out: raise SystemExit('duplicate platform artifact: '+key)
        p=Path(path).resolve()
        if p.is_symlink() or not p.is_file(): raise SystemExit('artifact must be regular non-symlink: '+str(p))
        out[k]=p
    return out
def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--rc',type=Path,required=True); ap.add_argument('--multi-platform',type=Path,required=True); ap.add_argument('--compatibility-matrix',type=Path,required=True); ap.add_argument('--version',required=True); ap.add_argument('--artifact',action='append',default=[],required=True); ap.add_argument('--sequence',type=int,required=True); ap.add_argument('--previous-sequence',type=int,required=True); ap.add_argument('--base-url',required=True); ap.add_argument('--expires-hours',type=int,default=72); ap.add_argument('--output',type=Path,required=True); args=ap.parse_args()
    root=args.root.resolve(); out=args.output.resolve(); rc=args.rc.resolve(); mp=args.multi_platform.resolve(); cm=args.compatibility_matrix.resolve()
    if out.exists() and any(out.iterdir()): raise SystemExit('GA output directory must be empty')
    out.mkdir(parents=True,exist_ok=True)
    if rc.is_symlink() or not rc.is_file(): raise SystemExit('RC must be a regular file')
    if args.sequence<=args.previous_sequence or args.previous_sequence<0: raise SystemExit('GA sequence must be greater than previous-sequence')
    if not (1<=args.expires_hours<=168): raise SystemExit('expires-hours must be 1..168')
    base=safe_https_base(args.base_url); report=verify_rc(root,rc); agg=validate_aggregate(root,mp); matrix=load_json(cm)
    if sha_file(rc)!=agg.get('rc_archive_sha256'): raise SystemExit('aggregate is bound to a different RC archive')
    if sha_file(cm)!=agg.get('compatibility_matrix_sha256') or matrix.get('status')!='qualified': raise SystemExit('compatibility matrix invalid')
    if report['source_state_sha256']!=agg['source_state_sha256'] or matrix.get('source_state_sha256')!=agg['source_state_sha256']: raise SystemExit('RC/platform source-state mismatch')
    if report['candidate_version']!=agg['candidate_version'] or matrix.get('candidate_version')!=agg['candidate_version']: raise SystemExit('RC/platform candidate mismatch')
    cv=Version(report['candidate_version']); gv=Version(args.version)
    if not cv.is_prerelease or gv.is_prerelease or cv.release!=gv.release or not re.fullmatch(r'\d+\.\d+\.\d+',args.version): raise SystemExit('GA version must be stable and match RC release tuple')
    if agg.get('first_release'):
        if agg.get('previous_version') is not None or args.previous_sequence!=0: raise SystemExit('first release must not claim previous version/sequence')
    else:
        pv=Version(agg.get('previous_version') or '')
        if pv >= gv: raise SystemExit('previous version must be lower than GA version')
    artifacts=parse_artifacts(args.artifact); required={(x['platform'],x['architecture']) for x in agg['platforms']}
    if set(artifacts)!=required: raise SystemExit(f'artifact set mismatch: required={sorted(required)} got={sorted(artifacts)}')
    targets=[]; artifact_rows=[]; artdir=out/'artifacts'
    for row in sorted(agg['platforms'],key=lambda x:(x['platform'],x['architecture'])):
        k=(row['platform'],row['architecture']); p=artifacts[k]
        if sha_file(p)!=row['artifact_sha256'] or p.stat().st_size!=row['artifact_size']: raise SystemExit('artifact differs from signed platform evidence: '+str(k))
        dest=artdir/f"{row['platform']}-{row['architecture']}"/p.name; dest.parent.mkdir(parents=True,exist_ok=True); shutil.copy2(p,dest)
        rel=dest.relative_to(out).as_posix(); url=f"{base}/{args.version}/{quote(rel,safe='/')}"
        target={'platform':row['platform'],'architecture':row['architecture'],'name':p.name,'url':url,'sha256':row['artifact_sha256'],'size':row['artifact_size'],'media_type':mimetypes.guess_type(p.name)[0] or 'application/octet-stream'}
        targets.append(target); artifact_rows.append({**target,'path':rel})
    release_uuid=uuid7(); now=utcnow(); expires=(datetime.now(timezone.utc)+timedelta(hours=args.expires_hours)).isoformat().replace('+00:00','Z')
    update={'schema_version':'0.27.0','channel':'stable','sequence':args.sequence,'version':args.version,'source_state_sha256':agg['source_state_sha256'],'ga_release_uuid':release_uuid,'targets':targets,'created_at':now,'expires_at':expires,'signer_key_id':None,'signature_algorithm':None,'signature_b64':None}
    usig=sign_payload(root,update_payload(update),'update-manifest'); update.update({'signer_key_id':usig['signer_key_id'],'signature_algorithm':usig.get('signature_algorithm','ed25519'),'signature_b64':usig['signature_b64']}); write_json(out/'update-manifest.json',update)
    shutil.copy2(rc,out/'release-candidate.zip'); shutil.copy2(mp,out/'multi-platform-qualification.json'); shutil.copy2(cm,out/'compatibility-n-n1.json')
    write_json(out/'ga-artifact-manifest.json',{'schema_version':'0.27.0','version':args.version,'source_state_sha256':agg['source_state_sha256'],'artifacts':artifact_rows,'created_at':now})
    ga={'schema_version':'0.27.0','release_uuid':release_uuid,'version':args.version,'source_state_sha256':agg['source_state_sha256'],'rc_candidate_version':report['candidate_version'],'rc_archive_sha256':sha_file(out/'release-candidate.zip'),'platform_qualification_sha256':sha_file(out/'multi-platform-qualification.json'),'compatibility_matrix_sha256':sha_file(out/'compatibility-n-n1.json'),'artifact_manifest_sha256':sha_file(out/'ga-artifact-manifest.json'),'update_manifest_sha256':sha_file(out/'update-manifest.json'),'created_at':now,'signer_key_id':None,'signature_algorithm':None,'signature_b64':None}
    gsig=sign_payload(root,ga_payload(ga),'ga-release'); ga.update({'signer_key_id':gsig['signer_key_id'],'signature_algorithm':gsig.get('signature_algorithm','ed25519'),'signature_b64':gsig['signature_b64']}); write_json(out/'ga-release.json',ga)
    archive=out/f"PhxClaw-{args.version}-GA.zip"; entries=[]
    for p in out.rglob('*'):
        if p.is_file() and p!=archive and p.name!='ga-build-report.json': entries.append((p.relative_to(out).as_posix(),p))
    deterministic_zip(archive,entries)
    result={'version':'0.27.0','status':'ga_built','release_version':args.version,'rc_candidate_version':report['candidate_version'],'release_uuid':release_uuid,'platforms':len(targets),'source_state_sha256':agg['source_state_sha256'],'ga_signed':True,'update_manifest_signed':True,'sequence':args.sequence,'previous_sequence':args.previous_sequence,'ga_archive':archive.name,'ga_archive_sha256':sha_file(archive)}
    write_json(out/'ga-build-report.json',result); print(json.dumps(result,indent=2))
if __name__=='__main__': main()

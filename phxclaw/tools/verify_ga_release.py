#!/usr/bin/env python3
from __future__ import annotations
from pathlib import Path, PurePosixPath
import argparse, json, subprocess, sys, tempfile, zipfile
from v027_common import *
def safe_unzip(src,dst):
    max_entries=5000; max_total=20*1024*1024*1024; max_file=8*1024*1024*1024
    with zipfile.ZipFile(src) as z:
        infos=z.infolist(); names=set(); total=0
        if len(infos)>max_entries: raise SystemExit('GA archive has too many entries')
        for i in infos:
            if i.filename in names: raise SystemExit('duplicate GA zip entry: '+i.filename)
            names.add(i.filename); total+=i.file_size
            if i.file_size>max_file or total>max_total: raise SystemExit('GA archive exceeds extraction limits')
            q=PurePosixPath(i.filename)
            if q.is_absolute() or '..' in q.parts or ((i.external_attr>>16)&0o170000)==0o120000: raise SystemExit('unsafe GA zip entry: '+i.filename)
            out=dst.joinpath(*q.parts)
            if i.is_dir(): out.mkdir(parents=True,exist_ok=True); continue
            out.parent.mkdir(parents=True,exist_ok=True); out.write_bytes(z.read(i))
def validate_aggregate(root,p,cm):
    a=load_json(p); verify_ed25519(root,a['signer_key_id'],a['signature_b64'],aggregate_payload(a),'multi-platform qualification')
    if sha_file(cm)!=a['compatibility_matrix_sha256']: raise SystemExit('compatibility matrix digest mismatch')
    matrix=load_json(cm); keys=[(x['platform'],x['architecture']) for x in a.get('platforms',[])]
    if matrix.get('status')!='qualified' or len(keys)!=3 or len(keys)!=len(set(keys)): raise SystemExit('multi-platform qualification incomplete/duplicate')
    if matrix.get('rc_archive_sha256')!=a.get('rc_archive_sha256'): raise SystemExit('matrix/aggregate RC mismatch')
    return a,matrix
def verify_dir(root,d):
    ga=load_json(d/'ga-release.json'); update=load_json(d/'update-manifest.json')
    if ga.get('schema_version')!='0.27.0' or ga.get('signature_algorithm')!='ed25519': raise SystemExit('invalid GA manifest')
    verify_ed25519(root,ga['signer_key_id'],ga['signature_b64'],ga_payload(ga),'GA manifest')
    files={'release-candidate.zip':'rc_archive_sha256','multi-platform-qualification.json':'platform_qualification_sha256','compatibility-n-n1.json':'compatibility_matrix_sha256','ga-artifact-manifest.json':'artifact_manifest_sha256','update-manifest.json':'update_manifest_sha256'}
    for name,key in files.items():
        p=d/name
        if p.is_symlink() or not p.is_file() or sha_file(p)!=ga[key]: raise SystemExit(name+' digest mismatch')
    if update.get('ga_release_uuid')!=ga['release_uuid'] or update.get('version')!=ga['version'] or update.get('source_state_sha256')!=ga['source_state_sha256'] or update.get('channel')!='stable': raise SystemExit('update manifest not bound to stable GA')
    verify_ed25519(root,update['signer_key_id'],update['signature_b64'],update_payload(update),'update manifest')
    agg,matrix=validate_aggregate(root,d/'multi-platform-qualification.json',d/'compatibility-n-n1.json')
    if agg['source_state_sha256']!=ga['source_state_sha256'] or matrix['source_state_sha256']!=ga['source_state_sha256'] or agg['rc_archive_sha256']!=ga['rc_archive_sha256']: raise SystemExit('GA identity mismatch')
    r=subprocess.run([sys.executable,str(root/'tools/verify_release_candidate.py'),str(d/'release-candidate.zip'),'--trust-store',str(root/'config/trusted-release-signers.v025.json')],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=300)
    if r.returncode: raise SystemExit('embedded RC verification failed: '+(r.stderr or r.stdout)[:1000])
    rc=json.loads(r.stdout)
    if rc['source_state_sha256']!=ga['source_state_sha256'] or rc['candidate_version']!=ga['rc_candidate_version']: raise SystemExit('embedded RC identity mismatch')
    manifest=load_json(d/'ga-artifact-manifest.json'); targets={(x['platform'],x['architecture']):x for x in update['targets']}
    if len(targets)!=len(update['targets']) or len(manifest.get('artifacts',[]))!=len(targets): raise SystemExit('GA artifact/target count or uniqueness mismatch')
    seen=set()
    for row in manifest['artifacts']:
        k=(row['platform'],row['architecture'])
        if k in seen:
            raise SystemExit('duplicate GA artifact target')
        seen.add(k)
        target=targets.get(k)
        if target is None: raise SystemExit('target missing for artifact '+str(k))
        rel=PurePosixPath(row['path'])
        if rel.is_absolute() or '..' in rel.parts: raise SystemExit('unsafe artifact manifest path')
        p=d.joinpath(*rel.parts)
        if not p.is_file() or p.is_symlink() or sha_file(p)!=row['sha256'] or p.stat().st_size!=row['size']: raise SystemExit('GA artifact digest/size mismatch')
        if target['sha256']!=row['sha256'] or target['size']!=row['size'] or not target['url'].startswith('https://'): raise SystemExit('update target differs from GA artifact manifest')
    return {'status':'verified','version':ga['version'],'release_uuid':ga['release_uuid'],'source_state_sha256':ga['source_state_sha256'],'rc_candidate_version':ga['rc_candidate_version'],'platforms':len(targets),'ga_signer':ga['signer_key_id'],'update_signer':update['signer_key_id']}
def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('ga',type=Path); args=ap.parse_args(); root=args.root.resolve(); src=args.ga.resolve()
    if src.is_dir(): result=verify_dir(root,src)
    elif src.is_file() and src.suffix.lower()=='.zip':
        with tempfile.TemporaryDirectory(prefix='phxclaw-ga-verify-') as td:
            d=Path(td); safe_unzip(src,d); result=verify_dir(root,d); result['ga_archive_sha256']=sha_file(src)
    else: raise SystemExit('GA input must be directory or zip')
    print(json.dumps(result,indent=2))
if __name__=='__main__': main()

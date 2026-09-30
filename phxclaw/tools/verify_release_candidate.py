#!/usr/bin/env python3
from pathlib import Path, PurePosixPath
import argparse, base64, hashlib, json, tempfile, zipfile

REQUIRED_GATES=["workspace_static","json_schema_static","migration_static","license_sbom","supply_chain","cargo_fmt","cargo_check","cargo_test","cargo_clippy","postgres_migration","rls_cross_tenant","skill_knowledge_lineage","mission_e2e","tauri_native_e2e","provider_model_e2e"]

def sha(p:Path):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for c in iter(lambda:f.read(1024*1024),b''): h.update(c)
    return h.hexdigest()

def sha_bytes(b:bytes): return hashlib.sha256(b).hexdigest()

def safe_unzip(zp:Path,dst:Path):
    max_entries=200000; max_total=20*1024*1024*1024; max_file=4*1024*1024*1024
    with zipfile.ZipFile(zp) as z:
        infos=z.infolist()
        if len(infos)>max_entries: raise SystemExit('RC archive has too many entries')
        names=set(); total=0
        for i in infos:
            if i.filename in names: raise SystemExit('duplicate zip entry rejected: '+i.filename)
            names.add(i.filename); total += i.file_size
            if i.file_size>max_file or total>max_total: raise SystemExit('RC archive exceeds extraction limits')
            q=PurePosixPath(i.filename)
            if q.is_absolute() or '..' in q.parts: raise SystemExit('unsafe zip path: '+i.filename)
            mode=(i.external_attr>>16)&0o170000
            if mode==0o120000: raise SystemExit('symlink entry rejected: '+i.filename)
            out=dst.joinpath(*q.parts)
            if i.is_dir(): out.mkdir(parents=True,exist_ok=True); continue
            out.parent.mkdir(parents=True,exist_ok=True)
            out.write_bytes(z.read(i))

def canonical_proof_bundle_sha256(proofs):
    rows=[]
    for p in sorted(proofs,key=lambda x:x['gate']):
        rows.append({'gate':p['gate'],'status':p['status'],'source_state_sha256':p['source_state_sha256_hex'].lower(),'evidence_sha256':p.get('evidence_sha256_hex'),'tool_name':p['tool_name'],'tool_version':p.get('tool_version'),'evidence_ref':p.get('evidence_ref'),'verified_at':p['verified_at']})
    return sha_bytes(json.dumps(rows,separators=(',',':'),ensure_ascii=False).encode())

def attestation_payload(a):
    ass=a['assessment']
    return (f"phxclaw-release-attestation-v025\nattestation_uuid={a['attestation_uuid']}\nrun_uuid={a['run_uuid']}\nrelease_uuid={a['release_uuid']}\nversion={a['version']}\nworkspace_sha256={a['workspace_sha256'].lower()}\nproof_bundle_sha256={a['proof_bundle_sha256'].lower()}\nsource_ready={1 if ass['source_ready'] else 0}\nstatic_verified={1 if ass['static_verified'] else 0}\nruntime_verified={1 if ass['runtime_verified'] else 0}\ne2e_verified={1 if ass['e2e_verified'] else 0}\nrelease_ready={1 if ass['release_ready'] else 0}\ncreated_at={a['created_at']}\n").encode()

def rc_payload(m):
    arts=sha_bytes(json.dumps(m['artifacts'],separators=(',',':'),ensure_ascii=False).encode())
    return (f"phxclaw-release-candidate-v026\ncandidate_uuid={m['candidate_uuid']}\ncandidate_version={m['candidate_version']}\nsource_state_sha256={m['source_state_sha256'].lower()}\nsource_bundle_sha256={m['source_bundle_sha256'].lower()}\nqualification_run_sha256={m['qualification_run_sha256'].lower()}\nqualification_attestation_sha256={m['qualification_attestation_sha256'].lower()}\nartifact_manifest_sha256={m['artifact_manifest_sha256'].lower()}\nsbom_sha256={m['sbom_sha256'].lower()}\nprovenance_sha256={m['provenance_sha256'].lower()}\ncompatibility_matrix_sha256={m['compatibility_matrix_sha256'].lower()}\nsprint_promotion_plan_sha256={m['sprint_promotion_plan_sha256'].lower()}\nrollback_bundle_sha256={m['rollback_bundle_sha256'].lower()}\nartifacts_sha256={arts}\ncreated_at={m['created_at']}\n").encode()

def signer(trust,key_id):
    return next((x for x in trust.get('signers',[]) if x.get('key_id')==key_id),None)

def verify_ed25519(trust,key_id,signature_b64,payload,label):
    s=signer(trust,key_id)
    if s is None: raise SystemExit(label+' signer is not trusted')
    try:
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
        pub=base64.b64decode(s['public_key_b64'],validate=True); sig=base64.b64decode(signature_b64,validate=True)
        if len(pub)!=32 or len(sig)!=64: raise ValueError('invalid Ed25519 length')
        Ed25519PublicKey.from_public_bytes(pub).verify(sig,payload)
    except Exception as e: raise SystemExit(label+' signature invalid: '+str(e))

def source_tree_hash(source_zip:Path):
    rows=[]; total=0; names=set(); max_total=20*1024*1024*1024; max_entries=200000
    with zipfile.ZipFile(source_zip) as z:
        infos=[x for x in z.infolist() if not x.is_dir()]
        if len(infos)>max_entries: raise SystemExit('source bundle has too many entries')
        for i in sorted(infos,key=lambda x:x.filename):
            if i.filename in names: raise SystemExit('duplicate source bundle entry: '+i.filename)
            names.add(i.filename); total += i.file_size
            if total>max_total: raise SystemExit('source bundle exceeds verification limit')
            q=PurePosixPath(i.filename)
            if q.is_absolute() or '..' in q.parts: raise SystemExit('unsafe source bundle path: '+i.filename)
            b=z.read(i); rows.append({'path':i.filename,'sha256':sha_bytes(b),'size':len(b)})
    h=hashlib.sha256()
    for row in rows:
        h.update(row['path'].encode()); h.update(b'\0'); h.update(row['sha256'].encode()); h.update(b'\0'); h.update(str(row['size']).encode()); h.update(b'\n')
    return h.hexdigest(),rows

def verify_dir(d:Path,trust:dict):
    m=json.loads((d/'release-candidate.json').read_text())
    srcs=list(d.glob('PhxClaw-*-source.zip'))
    if len(srcs)!=1: raise SystemExit('exactly one source bundle is required')
    expected={'source_bundle_sha256':srcs[0],'qualification_run_sha256':d/'evidence/qualification-run.json','qualification_attestation_sha256':d/'evidence/release-attestation.json','artifact_manifest_sha256':d/'artifact-manifest.json','sbom_sha256':d/'SBOM.spdx.json','provenance_sha256':d/'provenance.slsa.json','compatibility_matrix_sha256':d/'compatibility-matrix.json','sprint_promotion_plan_sha256':d/'sprint-promotion-plan.json','rollback_bundle_sha256':d/'rollback-bundle.zip'}
    for field,p in expected.items():
        if not p.is_file(): raise SystemExit('missing RC component: '+str(p))
        if sha(p)!=m[field]: raise SystemExit('digest mismatch: '+field)
    tree,_=source_tree_hash(srcs[0])
    if tree!=m['source_state_sha256']: raise SystemExit('source bundle does not reproduce signed source-state hash')
    for a in m['artifacts']:
        p=d/'artifacts'/a['name']
        if not p.is_file() or sha(p)!=a['sha256'] or p.stat().st_size!=a['size']: raise SystemExit('artifact mismatch: '+a['name'])
    run=json.loads(expected['qualification_run_sha256'].read_text()); att=json.loads(expected['qualification_attestation_sha256'].read_text())
    if run.get('workspace_sha256')!=run.get('final_workspace_sha256') or not run.get('source_state_stable'): raise SystemExit('embedded qualification source-state is not stable')
    if run.get('workspace_sha256','').lower()!=m['source_state_sha256'] or att.get('workspace_sha256','').lower()!=m['source_state_sha256']: raise SystemExit('embedded qualification source-state mismatch')
    if run.get('run_uuid')!=att.get('run_uuid') or run.get('release_uuid')!=att.get('release_uuid') or run.get('assessment')!=att.get('assessment'): raise SystemExit('embedded qualification identity/assessment mismatch')
    if not att.get('assessment',{}).get('release_ready'): raise SystemExit('embedded qualification is not release-ready')
    proofs=run.get('proofs',[]); by={p['gate']:p for p in proofs}
    if canonical_proof_bundle_sha256(proofs)!=att.get('proof_bundle_sha256'): raise SystemExit('embedded qualification proof bundle mismatch')
    missing=[g for g in REQUIRED_GATES if by.get(g,{}).get('status')!='verified' or by.get(g,{}).get('source_state_sha256_hex','').lower()!=m['source_state_sha256']]
    if missing: raise SystemExit('embedded qualification missing verified gates: '+','.join(missing))
    ap=attestation_payload(att)
    if sha_bytes(ap)!=att.get('signing_payload_sha256'): raise SystemExit('embedded qualification signing-payload hash mismatch')
    if att.get('signature_algorithm')!='ed25519': raise SystemExit('embedded qualification is not Ed25519 signed')
    verify_ed25519(trust,att.get('signer_key_id'),att.get('signature_b64'),ap,'qualification')
    plan=json.loads(expected['sprint_promotion_plan_sha256'].read_text())
    if plan.get('source_state_sha256')!=m['source_state_sha256'] or plan.get('qualification_run_uuid')!=run['run_uuid']: raise SystemExit('promotion plan is not bound to qualification/source-state')
    if plan.get('summary',{}).get('green')!=26 or plan.get('summary',{}).get('yellow')!=0 or len(plan.get('sprints',[]))!=26: raise SystemExit('RC does not contain 26 green sprints')
    verify_ed25519(trust,m.get('signer_key_id'),m.get('signature_b64'),rc_payload(m),'RC')
    return {'status':'verified','candidate_version':m['candidate_version'],'candidate_uuid':m['candidate_uuid'],'components':len(expected),'artifacts':len(m['artifacts']),'sprints_green':26,'source_state_sha256':m['source_state_sha256'],'qualification_signer':att['signer_key_id'],'rc_signer':m['signer_key_id']}

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('rc',type=Path,help='RC directory or PhxClaw-*-RC.zip'); ap.add_argument('--trust-store',type=Path,required=True); args=ap.parse_args(); trust=json.loads(args.trust_store.read_text()); src=args.rc.resolve()
    if src.is_dir(): result=verify_dir(src,trust)
    elif src.is_file() and src.suffix.lower()=='.zip':
        with tempfile.TemporaryDirectory(prefix='phxclaw-rc-verify-') as td:
            d=Path(td); safe_unzip(src,d); result=verify_dir(d,trust); result['rc_archive_sha256']=sha(src)
    else: raise SystemExit('RC input must be a directory or .zip archive')
    print(json.dumps(result,indent=2))
if __name__=='__main__': main()

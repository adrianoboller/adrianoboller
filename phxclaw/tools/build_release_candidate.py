#!/usr/bin/env python3
from __future__ import annotations
from pathlib import Path
import argparse, base64, hashlib, json, mimetypes, os, platform, shlex, shutil, subprocess, sys, tomllib, uuid, zipfile
from datetime import datetime, timezone
try:
    from packaging.version import Version
except ImportError:
    Version=None

HERE=Path(__file__).resolve().parent
if str(HERE) not in sys.path: sys.path.insert(0,str(HERE))
import source_state

REQUIRED_GATES=["workspace_static","json_schema_static","migration_static","license_sbom","supply_chain","cargo_fmt","cargo_check","cargo_test","cargo_clippy","postgres_migration","rls_cross_tenant","skill_knowledge_lineage","mission_e2e","tauri_native_e2e","provider_model_e2e"]
FIXED_ZIP_DT=(1980,1,1,0,0,0)

def utcnow(): return datetime.now(timezone.utc).isoformat().replace('+00:00','Z')
def sha256_bytes(b): return hashlib.sha256(b).hexdigest()
def sha256_file(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for c in iter(lambda:f.read(1024*1024),b''): h.update(c)
    return h.hexdigest()
def write_json(p,obj): p.parent.mkdir(parents=True,exist_ok=True); p.write_text(json.dumps(obj,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
def uuid7():
    # timestamp-first UUIDv7 generator without third-party runtime dependency
    import time, secrets
    ms=int(time.time()*1000) & ((1<<48)-1); rnd=secrets.randbits(74)
    value=(ms<<80) | (0x7<<76) | (((rnd>>62)&0xfff)<<64) | (0b10<<62) | (rnd & ((1<<62)-1))
    return uuid.UUID(int=value)
def parse_candidate(v):
    import re
    if '/' in v or '\\' in v or not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+-[0-9A-Za-z.-]+',v):
        raise ValueError('candidate version must be a safe SemVer prerelease')
    if Version is not None:
        x=Version(v)
        if not x.is_prerelease: raise ValueError('candidate version must be a prerelease')
def deterministic_zip(out:Path, entries:list[tuple[str,Path]]):
    out.parent.mkdir(parents=True,exist_ok=True)
    with zipfile.ZipFile(out,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as z:
        for arc,p in sorted(entries,key=lambda x:x[0]):
            info=zipfile.ZipInfo(arc,FIXED_ZIP_DT); info.compress_type=zipfile.ZIP_DEFLATED; info.external_attr=(0o644 & 0xffff)<<16
            z.writestr(info,p.read_bytes())
def load_json(p): return json.loads(p.read_text(encoding='utf-8'))
def signing_payload_att(a):
    ass=a['assessment']
    return (f"phxclaw-release-attestation-v025\nattestation_uuid={a['attestation_uuid']}\nrun_uuid={a['run_uuid']}\nrelease_uuid={a['release_uuid']}\nversion={a['version']}\nworkspace_sha256={a['workspace_sha256'].lower()}\nproof_bundle_sha256={a['proof_bundle_sha256'].lower()}\nsource_ready={1 if ass['source_ready'] else 0}\nstatic_verified={1 if ass['static_verified'] else 0}\nruntime_verified={1 if ass['runtime_verified'] else 0}\ne2e_verified={1 if ass['e2e_verified'] else 0}\nrelease_ready={1 if ass['release_ready'] else 0}\ncreated_at={a['created_at']}\n").encode()
def trusted_signers(root): return load_json(root/'config/trusted-release-signers.v025.json').get('signers',[])
def verify_ed25519(root,key_id,sig_b64,payload):
    signer=next((x for x in trusted_signers(root) if x.get('key_id')==key_id),None)
    if not signer: return False,'signer is not trusted'
    try:
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
        pub=base64.b64decode(signer['public_key_b64'],validate=True); sig=base64.b64decode(sig_b64,validate=True)
        if len(pub)!=32 or len(sig)!=64: return False,'invalid Ed25519 length'
        Ed25519PublicKey.from_public_bytes(pub).verify(sig,payload); return True,None
    except Exception as e: return False,f'Ed25519 verification failed: {e}'
def canonical_proof_bundle_sha256(proofs):
    rows=[]
    for p in sorted(proofs,key=lambda x:x['gate']):
        rows.append({
          'gate':p['gate'],'status':p['status'],'source_state_sha256':p['source_state_sha256_hex'].lower(),
          'evidence_sha256':p.get('evidence_sha256_hex'),'tool_name':p['tool_name'],'tool_version':p.get('tool_version'),
          'evidence_ref':p.get('evidence_ref'),'verified_at':p['verified_at']})
    return sha256_bytes(json.dumps(rows,separators=(',',':'),ensure_ascii=False).encode())

def verify_qualification(root,qd,current_hash,require_release_ready=True):
    run_path=qd/'qualification-run.json'; att_path=qd/'release-attestation.json'
    if run_path.is_symlink() or att_path.is_symlink(): raise SystemExit('qualification evidence symlink rejected')
    run=load_json(run_path); att=load_json(att_path)
    if run.get('workspace_sha256')!=run.get('final_workspace_sha256') or not run.get('source_state_stable'): raise SystemExit('qualification source-state was not stable')
    if run.get('workspace_sha256','').lower()!=current_hash.lower() or att.get('workspace_sha256','').lower()!=current_hash.lower(): raise SystemExit('current source-state differs from qualified source-state')
    if run.get('run_uuid')!=att.get('run_uuid') or run.get('release_uuid')!=att.get('release_uuid') or run.get('version')!=att.get('version'): raise SystemExit('qualification run/attestation identity mismatch')
    if run.get('assessment')!=att.get('assessment'): raise SystemExit('qualification assessment differs from signed attestation')
    if require_release_ready and not att.get('assessment',{}).get('release_ready'): raise SystemExit('qualification attestation is not release-ready')
    proofs=run.get('proofs',[])
    if canonical_proof_bundle_sha256(proofs)!=att.get('proof_bundle_sha256','').lower(): raise SystemExit('qualification proof bundle differs from signed attestation')
    payload=signing_payload_att(att)
    if sha256_bytes(payload)!=att.get('signing_payload_sha256','').lower(): raise SystemExit('qualification signing payload hash mismatch')
    by={p['gate']:p for p in proofs}
    missing=[g for g in REQUIRED_GATES if by.get(g,{}).get('status')!='verified' or by.get(g,{}).get('source_state_sha256_hex','').lower()!=current_hash.lower()]
    if require_release_ready and missing: raise SystemExit('required qualification gates are not verified for this source-state: '+','.join(missing))
    if att.get('signature_algorithm')!='ed25519' or not att.get('signer_key_id') or not att.get('signature_b64'): raise SystemExit('qualification attestation is not signed Ed25519')
    ok,reason=verify_ed25519(root,att['signer_key_id'],att['signature_b64'],payload)
    if not ok: raise SystemExit('qualification signature rejected: '+reason)
    return run,att,by

def spdx_id(name,version): return 'SPDXRef-Package-'+hashlib.sha256(f'{name}@{version}'.encode()).hexdigest()[:20]
def make_sbom(root,source_bundle,artifacts):
    lock=root/'Cargo.lock'
    if not lock.exists(): raise SystemExit('Cargo.lock is required for final SBOM')
    data=tomllib.loads(lock.read_text(encoding='utf-8')); packages=[]
    for p in sorted(data.get('package',[]),key=lambda x:(x.get('name',''),x.get('version',''))):
        obj={"SPDXID":spdx_id(p['name'],p['version']),"name":p['name'],"versionInfo":p['version'],"downloadLocation":p.get('source','NOASSERTION'),"filesAnalyzed":False,"licenseConcluded":"NOASSERTION","licenseDeclared":"NOASSERTION"}
        if p.get('checksum'): obj['checksums']=[{"algorithm":"SHA256","checksumValue":p['checksum']}]
        packages.append(obj)
    packages.insert(0,{"SPDXID":"SPDXRef-Package-PhxClaw-Source","name":"PhxClaw-source","versionInfo":"qualified-source-state","downloadLocation":"NOASSERTION","filesAnalyzed":False,"licenseConcluded":"NOASSERTION","licenseDeclared":"Apache-2.0","checksums":[{"algorithm":"SHA256","checksumValue":sha256_file(source_bundle)}]})
    for a in artifacts:
        packages.append({"SPDXID":"SPDXRef-Artifact-"+hashlib.sha256(a['name'].encode()).hexdigest()[:20],"name":a['name'],"versionInfo":"release-candidate-artifact","downloadLocation":"NOASSERTION","filesAnalyzed":False,"licenseConcluded":"NOASSERTION","licenseDeclared":"NOASSERTION","checksums":[{"algorithm":"SHA256","checksumValue":a['sha256']}]})
    return {"spdxVersion":"SPDX-2.3","dataLicense":"CC0-1.0","SPDXID":"SPDXRef-DOCUMENT","name":"PhxClaw Release Candidate SBOM","documentNamespace":"urn:uuid:"+str(uuid7()),"creationInfo":{"created":utcnow(),"creators":["Tool: PhxClaw RC Factory 0.26.0"]},"packages":packages,"relationships":[{"spdxElementId":"SPDXRef-DOCUMENT","relationshipType":"DESCRIBES","relatedSpdxElement":p['SPDXID']} for p in packages]}

def promotion_plan(root,run,by,current_hash):
    mapping=load_json(root/'config/sprint-gate-map.v026.json')['sprints']; rows=[]
    for sprint in sorted(mapping):
        req=mapping[sprint]['required_gates']; verified=[g for g in req if by.get(g,{}).get('status')=='verified' and by.get(g,{}).get('source_state_sha256_hex','').lower()==current_hash.lower()]
        missing=[g for g in req if g not in verified]
        rows.append({"sprint":sprint,"status":"green" if not missing else "yellow","required_gates":req,"verified_gates":verified,"missing_gates":missing})
    return {"version":"0.26.0","source_state_sha256":current_hash.lower(),"qualification_run_uuid":run['run_uuid'],"sprints":rows,"summary":{"green":sum(r['status']=='green' for r in rows),"yellow":sum(r['status']=='yellow' for r in rows),"red":0,"total":26}}

def rc_payload(m):
    arts=sha256_bytes(json.dumps(m['artifacts'],separators=(',',':'),ensure_ascii=False).encode())
    return (f"phxclaw-release-candidate-v026\ncandidate_uuid={m['candidate_uuid']}\ncandidate_version={m['candidate_version']}\nsource_state_sha256={m['source_state_sha256'].lower()}\nsource_bundle_sha256={m['source_bundle_sha256'].lower()}\nqualification_run_sha256={m['qualification_run_sha256'].lower()}\nqualification_attestation_sha256={m['qualification_attestation_sha256'].lower()}\nartifact_manifest_sha256={m['artifact_manifest_sha256'].lower()}\nsbom_sha256={m['sbom_sha256'].lower()}\nprovenance_sha256={m['provenance_sha256'].lower()}\ncompatibility_matrix_sha256={m['compatibility_matrix_sha256'].lower()}\nsprint_promotion_plan_sha256={m['sprint_promotion_plan_sha256'].lower()}\nrollback_bundle_sha256={m['rollback_bundle_sha256'].lower()}\nartifacts_sha256={arts}\ncreated_at={m['created_at']}\n").encode()
def sign_manifest(root,out,m):
    raw=os.environ.get('PHXCLAW_RELEASE_SIGN_CMD','').strip()
    if not raw: raise SystemExit('PHXCLAW_RELEASE_SIGN_CMD is required for public RC')
    payload=rc_payload(m); pp=out/'rc-signing-payload.bin'; pp.write_bytes(payload)
    r=subprocess.run(shlex.split(raw)+[str(pp)],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=300)
    if r.returncode: raise SystemExit('RC signer failed: '+r.stderr[:500])
    sig=json.loads(r.stdout); m['signer_key_id']=sig['signer_key_id']; m['signature_algorithm']=sig.get('signature_algorithm','ed25519'); m['signature_b64']=sig['signature_b64']
    if m['signature_algorithm']!='ed25519': raise SystemExit('RC signature algorithm must be ed25519')
    ok,reason=verify_ed25519(root,m['signer_key_id'],m['signature_b64'],payload)
    if not ok: raise SystemExit('RC signature verification failed: '+reason)

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--qualification-dir',type=Path,required=True); ap.add_argument('--candidate-version',required=True); ap.add_argument('--artifact',type=Path,action='append',default=[]); ap.add_argument('--previous-release',type=Path); ap.add_argument('--first-release',action='store_true'); ap.add_argument('--output',type=Path,required=True); ap.add_argument('--public-rc',action='store_true'); ap.add_argument('--plan-only',action='store_true'); args=ap.parse_args()
    root=args.root.resolve(); qd=(root/args.qualification_dir).resolve() if not args.qualification_dir.is_absolute() else args.qualification_dir.resolve(); out=args.output.resolve(); out.mkdir(parents=True,exist_ok=True)
    try: parse_candidate(args.candidate_version)
    except Exception as e: raise SystemExit(str(e))
    rows=source_state.manifest(root); current=source_state.tree_hash(rows)
    run,att,by=verify_qualification(root,qd,current,require_release_ready=not args.plan_only)
    evidence_dir=out/'evidence'; evidence_dir.mkdir(exist_ok=True)
    shutil.copy2(qd/'qualification-run.json',evidence_dir/'qualification-run.json')
    shutil.copy2(qd/'release-attestation.json',evidence_dir/'release-attestation.json')
    if args.plan_only:
        plan=promotion_plan(root,run,by,current); write_json(out/'sprint-promotion-plan.json',plan)
        report={'version':'0.26.0','mode':'plan_only','candidate_version':args.candidate_version,'source_state_sha256':current,'qualification_release_ready':bool(att.get('assessment',{}).get('release_ready')),'qualification_signature_verified':True,'sprints':plan['summary']}
        write_json(out/'promotion-plan-report.json',report); print(json.dumps(report,indent=2)); return
    if not args.artifact: raise SystemExit('at least one native --artifact is required')
    if not args.first_release and args.previous_release is None: raise SystemExit('--previous-release required unless --first-release')
    if args.first_release and args.previous_release is not None: raise SystemExit('--first-release and --previous-release are mutually exclusive')

    # deterministic source bundle from exactly the qualified source-state file list
    srczip=out/f'PhxClaw-{args.candidate_version}-source.zip'; entries=[]
    for row in rows:
        p=root/row['path']
        if p.is_symlink(): raise SystemExit('source-state symlink rejected: '+row['path'])
        resolved=p.resolve()
        if root != resolved and root not in resolved.parents: raise SystemExit('source-state path escapes workspace: '+row['path'])
        if not resolved.is_file(): raise SystemExit('source-state entry is not a regular file: '+row['path'])
        entries.append((row['path'],resolved))
    deterministic_zip(srczip,entries)

    adir=out/'artifacts'; adir.mkdir(exist_ok=True); artifacts=[]
    for apath in args.artifact:
        raw=(root/apath) if not apath.is_absolute() else apath
        if raw.is_symlink(): raise SystemExit(f'artifact symlink rejected: {raw}')
        p=raw.resolve()
        if not p.is_file(): raise SystemExit(f'artifact not found: {p}')
        dst=adir/p.name
        if dst.exists() and sha256_file(dst)!=sha256_file(p): raise SystemExit(f'artifact basename collision: {p.name}')
        shutil.copy2(p,dst); artifacts.append({'name':p.name,'sha256':sha256_file(dst),'size':dst.stat().st_size,'media_type':mimetypes.guess_type(dst.name)[0] or 'application/octet-stream'})
    write_json(out/'artifact-manifest.json',{'version':'0.26.0','candidate_version':args.candidate_version,'artifacts':artifacts})

    sbom=make_sbom(root,srczip,artifacts); write_json(out/'SBOM.spdx.json',sbom)
    plan=promotion_plan(root,run,by,current); write_json(out/'sprint-promotion-plan.json',plan)
    if args.public_rc and plan['summary']['green']!=26: raise SystemExit('public RC requires all 26 sprints green from evidence')

    os_name=platform.system().lower(); arch=platform.machine().lower()
    current_target=("linux-x86_64" if os_name=='linux' and arch in ('x86_64','amd64') else "windows-x86_64" if os_name=='windows' and arch in ('x86_64','amd64') else "macos-arm64" if os_name=='darwin' and ('arm' in arch or 'aarch64' in arch) else f"{os_name}-{arch}")
    targets=[]
    for target in ['linux-x86_64','windows-x86_64','macos-arm64']:
        targets.append({'target':target,'status':'rc_builder_attested' if target==current_target else 'not_qualified'})
    compat={"version":"0.26.0","candidate_version":args.candidate_version,"qualified_source_state_sha256":current,"qualification":{"profile":qd.name,"qualification_run_uuid":run['run_uuid'],"gates_release_ready":True,"platform_identity_recorded_in_v025_attestation":False},"rc_builder_observed":{"os":os_name,"architecture":arch,"target":current_target,"attested_by_signed_rc_manifest":args.public_rc},"policy_targets":targets,"note":"rc_builder_attested means the signed RC records where this artifact bundle was assembled; it does not silently claim that v0.25 cryptographically recorded the qualification host. not_qualified is not equivalent to unsupported."}
    write_json(out/'compatibility-matrix.json',compat)

    rollback_dir=out/'rollback-staging'; rollback_dir.mkdir(exist_ok=True)
    rbmanifest={"version":"0.26.0","candidate_version":args.candidate_version,"first_release":args.first_release,"previous_release":None,"instructions":["stop PhxClaw services","verify previous release SHA-256","restore previous signed release package","run database compatibility/downgrade plan before any destructive change","restart and verify health/evidence gates"]}
    if args.previous_release:
        raw_prev=args.previous_release
        if raw_prev.is_symlink(): raise SystemExit('previous release symlink rejected')
        prev=raw_prev.resolve()
        if not prev.is_file(): raise SystemExit('previous release file missing')
        shutil.copy2(prev,rollback_dir/prev.name); rbmanifest['previous_release']={'name':prev.name,'sha256':sha256_file(prev),'size':prev.stat().st_size}
    write_json(rollback_dir/'rollback-manifest.json',rbmanifest)
    rbzip=out/'rollback-bundle.zip'; deterministic_zip(rbzip,[(p.relative_to(rollback_dir).as_posix(),p) for p in rollback_dir.rglob('*') if p.is_file()]); shutil.rmtree(rollback_dir)

    subjects=[{'name':a['name'],'digest':{'sha256':a['sha256']}} for a in artifacts]+[{'name':srczip.name,'digest':{'sha256':sha256_file(srczip)}}]
    prov={"_type":"https://in-toto.io/Statement/v1","subject":subjects,"predicateType":"https://slsa.dev/provenance/v1","predicate":{"buildDefinition":{"buildType":"https://phxclaw.dev/build-types/native-release-candidate/v1","externalParameters":{"candidate_version":args.candidate_version},"internalParameters":{"factory_version":"0.26.0"},"resolvedDependencies":[{"uri":"urn:phxclaw:source-state","digest":{"sha256":current}},{"uri":"urn:phxclaw:qualification-attestation","digest":{"sha256":sha256_file(qd/'release-attestation.json')}}]},"runDetails":{"builder":{"id":"phxclaw-release-candidate-factory/0.26.0"},"metadata":{"invocationId":str(uuid7()),"startedOn":run['started_at'],"finishedOn":utcnow()}}}}
    write_json(out/'provenance.slsa.json',prov)

    manifest={"schema_version":"0.26.0","candidate_uuid":str(uuid7()),"candidate_version":args.candidate_version,"source_state_sha256":current.lower(),"source_bundle_sha256":sha256_file(srczip),"qualification_run_sha256":sha256_file(evidence_dir/'qualification-run.json'),"qualification_attestation_sha256":sha256_file(evidence_dir/'release-attestation.json'),"artifact_manifest_sha256":sha256_file(out/'artifact-manifest.json'),"sbom_sha256":sha256_file(out/'SBOM.spdx.json'),"provenance_sha256":sha256_file(out/'provenance.slsa.json'),"compatibility_matrix_sha256":sha256_file(out/'compatibility-matrix.json'),"sprint_promotion_plan_sha256":sha256_file(out/'sprint-promotion-plan.json'),"rollback_bundle_sha256":sha256_file(rbzip),"artifacts":artifacts,"created_at":utcnow(),"signer_key_id":None,"signature_algorithm":None,"signature_b64":None}
    if args.public_rc: sign_manifest(root,out,manifest)
    write_json(out/'release-candidate.json',manifest)

    if args.public_rc and not manifest['signature_b64']: raise SystemExit('public RC must be signed')
    before=current; after=source_state.tree_hash(source_state.manifest(root))
    if before!=after: raise SystemExit('RC generation mutated canonical source-state')
    # Final RC archive contains every signed/hashed component plus evidence and artifacts.
    rc_archive=out/f'PhxClaw-{args.candidate_version}-RC.zip'
    rc_entries=[]
    for p in out.rglob('*'):
        if p.is_file() and p != rc_archive and p.name != 'rc-factory-report.json':
            rc_entries.append((p.relative_to(out).as_posix(),p))
    deterministic_zip(rc_archive,rc_entries)
    report={'version':'0.26.0','candidate_version':args.candidate_version,'public_rc':args.public_rc,'source_state_stable':True,'source_state_sha256':before,'qualification_release_ready':True,'qualification_signature_verified':True,'sprints':plan['summary'],'rc_signed':bool(manifest['signature_b64']),'rc_archive':rc_archive.name,'rc_archive_sha256':sha256_file(rc_archive),'outputs':sorted(p.name for p in out.iterdir() if p.is_file())}
    write_json(out/'rc-factory-report.json',report)
    print(json.dumps(report,indent=2))
if __name__=='__main__': main()

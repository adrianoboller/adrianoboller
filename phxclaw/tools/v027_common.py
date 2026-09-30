from __future__ import annotations
from pathlib import Path, PurePosixPath
from datetime import datetime, timezone
import base64, hashlib, json, os, shlex, subprocess, uuid, zipfile

FIXED_ZIP_DT=(1980,1,1,0,0,0)
def utcnow(): return datetime.now(timezone.utc).isoformat().replace('+00:00','Z')
def sha_bytes(b:bytes)->str: return hashlib.sha256(b).hexdigest()
def sha_file(p:Path)->str:
    h=hashlib.sha256()
    with p.open('rb') as f:
        for c in iter(lambda:f.read(1024*1024),b''): h.update(c)
    return h.hexdigest()
def valid_sha256(v): return isinstance(v,str) and len(v)==64 and all(c in '0123456789abcdefABCDEF' for c in v)
def load_json(p:Path):
    if p.is_symlink() or not p.is_file(): raise SystemExit('JSON input must be a regular non-symlink file: '+str(p))
    return json.loads(p.read_text(encoding='utf-8'))
def write_json(p:Path,obj): p.parent.mkdir(parents=True,exist_ok=True); p.write_text(json.dumps(obj,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
def uuid7():
    import time, secrets
    ms=int(time.time()*1000)&((1<<48)-1); rnd=secrets.randbits(74)
    value=(ms<<80)|(0x7<<76)|(((rnd>>62)&0xfff)<<64)|(0b10<<62)|(rnd&((1<<62)-1))
    return str(uuid.UUID(int=value))
def trust_store(root:Path):
    rows=load_json(root/'config/trusted-release-signers.v025.json').get('signers',[])
    return [x for x in rows if x.get('enabled',True) is True]
def verify_ed25519(root:Path,key_id:str,sig_b64:str,payload:bytes,label='signature'):
    signer=next((x for x in trust_store(root) if x.get('key_id')==key_id),None)
    if signer is None: raise SystemExit(f'{label} signer is not trusted/enabled')
    try:
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
        pub=base64.b64decode(signer['public_key_b64'],validate=True); sig=base64.b64decode(sig_b64,validate=True)
        if len(pub)!=32 or len(sig)!=64: raise ValueError('invalid Ed25519 length')
        Ed25519PublicKey.from_public_bytes(pub).verify(sig,payload)
    except Exception as e: raise SystemExit(f'{label} signature invalid: {e}')
def sign_payload(root:Path,payload:bytes,label:str):
    raw=os.environ.get('PHXCLAW_RELEASE_SIGN_CMD','').strip()
    if not raw: raise SystemExit('PHXCLAW_RELEASE_SIGN_CMD is required')
    tmp=root/'var'/'release-signing'; tmp.mkdir(parents=True,exist_ok=True)
    p=tmp/f'{label}.payload.bin'; p.write_bytes(payload)
    r=subprocess.run(shlex.split(raw)+[str(p)],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=300)
    if r.returncode: raise SystemExit(f'{label} signer failed: '+r.stderr[:800])
    try: obj=json.loads(r.stdout)
    except Exception as e: raise SystemExit(f'{label} signer output is not JSON: {e}')
    if obj.get('signature_algorithm','ed25519')!='ed25519': raise SystemExit(f'{label} signature algorithm must be ed25519')
    verify_ed25519(root,obj['signer_key_id'],obj['signature_b64'],payload,label)
    return obj
def evidence_bundle_sha256(rows):
    canon=[{'name':x['name'],'sha256':x['sha256'].lower(),'size':int(x['size'])} for x in sorted(rows,key=lambda x:x['name'])]
    return sha_bytes(json.dumps(canon,separators=(',',':'),ensure_ascii=False).encode())
def platform_payload(o):
    return (f"phxclaw-platform-qualification-v027\nplatform={o['platform']}\narchitecture={o['architecture']}\ncandidate_version={o['candidate_version']}\nsource_state_sha256={o['source_state_sha256'].lower()}\nrc_archive_sha256={o['rc_archive_sha256'].lower()}\nartifact_sha256={o['artifact_sha256'].lower()}\nartifact_size={o['artifact_size']}\nevidence_bundle_sha256={o['evidence_bundle_sha256'].lower()}\nfresh_install={o['fresh_install']}\nupgrade_n_minus_1={o['upgrade_n_minus_1']}\nrollback_or_restore={o['rollback_or_restore']}\nnative_tests={o['native_tests']}\ncode_signing={o['code_signing']}\nnotarization={o['notarization']}\nfirst_release={1 if o['first_release'] else 0}\nprevious_version={o.get('previous_version') or ''}\ncreated_at={o['created_at']}\n").encode()
def aggregate_payload(o):
    return (f"phxclaw-multiplatform-qualification-v027\naggregate_uuid={o['aggregate_uuid']}\ncandidate_version={o['candidate_version']}\nsource_state_sha256={o['source_state_sha256'].lower()}\nrc_archive_sha256={o['rc_archive_sha256'].lower()}\ncompatibility_matrix_sha256={o['compatibility_matrix_sha256'].lower()}\nplatform_bundle_sha256={o['platform_bundle_sha256'].lower()}\nfirst_release={1 if o['first_release'] else 0}\nprevious_version={o.get('previous_version') or ''}\ncreated_at={o['created_at']}\n").encode()
def ga_payload(o):
    return (f"phxclaw-ga-release-v027\nrelease_uuid={o['release_uuid']}\nversion={o['version']}\nsource_state_sha256={o['source_state_sha256'].lower()}\nrc_candidate_version={o['rc_candidate_version']}\nrc_archive_sha256={o['rc_archive_sha256'].lower()}\nplatform_qualification_sha256={o['platform_qualification_sha256'].lower()}\ncompatibility_matrix_sha256={o['compatibility_matrix_sha256'].lower()}\nartifact_manifest_sha256={o['artifact_manifest_sha256'].lower()}\nupdate_manifest_sha256={o['update_manifest_sha256'].lower()}\ncreated_at={o['created_at']}\n").encode()
def update_payload(o):
    targets_hash=sha_bytes(json.dumps(o['targets'],separators=(',',':'),ensure_ascii=False).encode())
    return (f"phxclaw-update-manifest-v027\nchannel={o['channel']}\nsequence={o['sequence']}\nversion={o['version']}\nsource_state_sha256={o['source_state_sha256'].lower()}\nga_release_uuid={o['ga_release_uuid']}\ntargets_sha256={targets_hash}\ncreated_at={o['created_at']}\nexpires_at={o['expires_at']}\n").encode()
def rollback_payload(o):
    return (f"phxclaw-rollback-authorization-v027\nauthorization_uuid={o['authorization_uuid']}\nfrom_version={o['from_version']}\nto_version={o['to_version']}\nfrom_sequence={o['from_sequence']}\nto_sequence={o['to_sequence']}\nreason_sha256={sha_bytes(o['reason'].encode())}\ncreated_at={o['created_at']}\nexpires_at={o['expires_at']}\n").encode()
def deterministic_zip(out:Path, entries:list[tuple[str,Path]]):
    out.parent.mkdir(parents=True,exist_ok=True); seen=set()
    with zipfile.ZipFile(out,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as z:
        for arc,p in sorted(entries,key=lambda x:x[0]):
            if arc in seen: raise SystemExit('duplicate archive path: '+arc)
            seen.add(arc)
            if p.is_symlink() or not p.is_file(): raise SystemExit('non-regular file rejected while packaging: '+str(p))
            q=PurePosixPath(arc)
            if q.is_absolute() or '..' in q.parts: raise SystemExit('unsafe archive path: '+arc)
            info=zipfile.ZipInfo(arc,FIXED_ZIP_DT); info.compress_type=zipfile.ZIP_DEFLATED; info.external_attr=(0o644&0xffff)<<16
            z.writestr(info,p.read_bytes())

#!/usr/bin/env python3
from __future__ import annotations
from pathlib import Path
import argparse, base64, datetime as dt, hashlib, json, os, re, shlex, shutil, subprocess, sys, time, uuid
from urllib.parse import urlparse, parse_qs
import source_state

GATES=[
 'workspace_static','json_schema_static','migration_static','license_sbom','supply_chain',
 'cargo_fmt','cargo_check','cargo_test','cargo_clippy','postgres_migration','rls_cross_tenant',
 'skill_knowledge_lineage','mission_e2e','tauri_native_e2e','provider_model_e2e']

def rfc3339_autosi(value):
    value=value.astimezone(dt.timezone.utc)
    base=value.strftime('%Y-%m-%dT%H:%M:%S')
    us=value.microsecond
    if us==0: return base+'Z'
    if us % 1000 == 0: return base+f'.{us//1000:03d}Z'
    return base+f'.{us:06d}Z'
def utcnow(): return rfc3339_autosi(dt.datetime.now(dt.timezone.utc))
def uuid7():
    ms=int(time.time()*1000) & ((1<<48)-1)
    rnd=int.from_bytes(os.urandom(10),'big')
    rand_a=(rnd >> 68) & 0xFFF
    rand_b=rnd & ((1<<62)-1)
    value=(ms<<80) | (0x7<<76) | (rand_a<<64) | (0x2<<62) | rand_b
    return uuid.UUID(int=value)
def pg_env_from_url(raw):
    u=urlparse(raw)
    if u.scheme not in ('postgres','postgresql') or not u.hostname or not u.path or u.path=='/':
        raise ValueError('invalid PostgreSQL URL')
    env=os.environ.copy()
    env['PGHOST']=u.hostname
    if u.port: env['PGPORT']=str(u.port)
    env['PGDATABASE']=u.path.lstrip('/')
    if u.username: env['PGUSER']=u.username
    if u.password is not None: env['PGPASSWORD']=u.password
    q=parse_qs(u.query)
    if q.get('sslmode'): env['PGSSLMODE']=q['sslmode'][0]
    return env, (u.username or '')
def quote_ident(name):
    if not re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]{0,62}',name): raise ValueError('unsafe PostgreSQL role name')
    return '"'+name.replace('"','""')+'"'
def sha256_bytes(b:bytes): return hashlib.sha256(b).hexdigest()
def sha256_file(p:Path): return sha256_bytes(p.read_bytes())
def attestation_signing_payload(att):
    a=att['assessment']
    return (
      'phxclaw-release-attestation-v025\n'
      f"attestation_uuid={att['attestation_uuid']}\n"
      f"run_uuid={att['run_uuid']}\n"
      f"release_uuid={att['release_uuid']}\n"
      f"version={att['version']}\n"
      f"workspace_sha256={att['workspace_sha256'].lower()}\n"
      f"proof_bundle_sha256={att['proof_bundle_sha256'].lower()}\n"
      f"source_ready={1 if a['source_ready'] else 0}\n"
      f"static_verified={1 if a['static_verified'] else 0}\n"
      f"runtime_verified={1 if a['runtime_verified'] else 0}\n"
      f"e2e_verified={1 if a['e2e_verified'] else 0}\n"
      f"release_ready={1 if a['release_ready'] else 0}\n"
      f"created_at={att['created_at']}\n"
    ).encode('utf-8')
def verify_ed25519_signature(payload, signer_key_id, signature_b64, trust_path):
    try:
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
    except Exception as e:
        return False, f'python cryptography with Ed25519 support is required: {e}'
    try:
        store=json.loads(trust_path.read_text(encoding='utf-8'))
        signer=next((x for x in store.get('signers',[]) if x.get('key_id')==signer_key_id),None)
        if signer is None: return False, 'signer key id is not in trusted release signer store'
        public_key=base64.b64decode(signer['public_key_b64'],validate=True)
        signature=base64.b64decode(signature_b64,validate=True)
        if len(public_key)!=32 or len(signature)!=64: return False, 'invalid Ed25519 public key/signature length'
        Ed25519PublicKey.from_public_bytes(public_key).verify(signature,payload)
        return True, None
    except Exception as e: return False, f'Ed25519 signature verification failed: {e}'
def tool_version(cmd):
    try:
        r=subprocess.run(cmd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=20)
        return (r.stdout or '').strip().splitlines()[0][:300] if r.stdout else None
    except Exception: return None

def run_cmd(root:Path,out:Path,gate:str,cmd:list[str],env=None,marker=None,timeout=3600):
    start=time.monotonic(); ev=out/f'{gate}.log';
    try:
        r=subprocess.run(cmd,cwd=root,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=timeout)
        text=r.stdout or ''; ev.write_text(text,encoding='utf-8',errors='replace')
        ok=r.returncode==0 and (marker is None or marker in text)
        return {'status':'verified' if ok else 'failed','exit_code':r.returncode,'duration_ms':int((time.monotonic()-start)*1000),'evidence_sha256':sha256_file(ev),'evidence_ref':ev.name,'command':cmd,'reason':None if ok else ('required marker missing' if r.returncode==0 else 'command failed')}
    except subprocess.TimeoutExpired as e:
        data=(e.stdout or '') if isinstance(e.stdout,str) else ''
        ev.write_text(data+'\nTIMEOUT\n',encoding='utf-8')
        return {'status':'failed','exit_code':None,'duration_ms':int((time.monotonic()-start)*1000),'evidence_sha256':sha256_file(ev),'evidence_ref':ev.name,'command':cmd,'reason':'timeout'}
    except Exception as e:
        ev.write_text(f'runner error: {e}\n',encoding='utf-8')
        return {'status':'failed','exit_code':None,'duration_ms':int((time.monotonic()-start)*1000),'evidence_sha256':sha256_file(ev),'evidence_ref':ev.name,'command':cmd,'reason':str(e)}

def unavailable(reason,cmd=None):
    return {'status':'unavailable','exit_code':None,'duration_ms':0,'evidence_sha256':None,'evidence_ref':None,'command':cmd,'reason':reason}

def env_command(name):
    raw=os.environ.get(name,'').strip(); return shlex.split(raw) if raw else None

def emit_proof(out,gate,workspace,tool_name,tool_ver,result):
    p={'gate':gate,'status':result['status'],'source_state_sha256_hex':workspace,'evidence_sha256_hex':result['evidence_sha256'],'tool_name':tool_name,'tool_version':tool_ver,'evidence_ref':result['evidence_ref'],'verified_at':utcnow(),'exit_code':result['exit_code'],'duration_ms':result['duration_ms'],'command':result['command'],'reason':result['reason']}
    (out/f'{gate}.json').write_text(json.dumps(p,indent=2)+'\n',encoding='utf-8'); return p

def verify_trusted_ed25519(root:Path, att:dict, payload:bytes):
    if not att.get('signer_key_id') or not att.get('signature_b64'):
        return False,'attestation is unsigned'
    if att.get('signature_algorithm') != 'ed25519':
        return False,'signature_algorithm must be ed25519'
    trust_path=root/'config/trusted-release-signers.v025.json'
    if not trust_path.exists(): return False,'trusted signer file missing'
    try:
        trust=json.loads(trust_path.read_text())
        signer=next((x for x in trust.get('signers',[]) if x.get('key_id')==att['signer_key_id']),None)
        if signer is None: return False,'signer key id is not trusted'
        pub=base64.b64decode(signer['public_key_b64'],validate=True)
        sig=base64.b64decode(att['signature_b64'],validate=True)
        if len(pub)!=32 or len(sig)!=64: return False,'invalid Ed25519 key/signature length'
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
        Ed25519PublicKey.from_public_bytes(pub).verify(sig,payload)
        return True,None
    except ImportError:
        return False,'cryptography package not installed'
    except Exception as e:
        return False,f'Ed25519 verification failed: {e}'

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--output',type=Path,required=True); ap.add_argument('--strict',action='store_true'); ap.add_argument('--public-release',action='store_true'); args=ap.parse_args()
    root=args.root.resolve(); out=args.output.resolve(); out.mkdir(parents=True,exist_ok=True)
    manifest=source_state.manifest(root); workspace=source_state.tree_hash(manifest)
    (out/'source-state.json').write_text(json.dumps({'algorithm':'sha256-tree-v1','workspace_sha256':workspace,'file_count':len(manifest),'files':manifest},indent=2)+'\n',encoding='utf-8')
    started=utcnow(); proofs=[]
    def add(gate,tool,ver,res): proofs.append(emit_proof(out,gate,workspace,tool,ver,res))

    py=sys.executable
    # Static gates are executed, not inherited from old reports.
    static=run_cmd(root,out,'workspace_static',[py,'tools/verify_v025.py',str(root),'--mode','workspace'],timeout=300)
    add('workspace_static','verify_v025', '0.25.0', static)
    schema=run_cmd(root,out,'json_schema_static',[py,'tools/verify_v025.py',str(root),'--mode','schema'],timeout=300)
    add('json_schema_static','verify_v025','0.25.0',schema)
    mig=run_cmd(root,out,'migration_static',[py,'tools/verify_v025.py',str(root),'--mode','migration'],timeout=300)
    add('migration_static','verify_v025','0.25.0',mig)

    # License/SBOM gate: explicit files required.
    sbom_candidates=[root/'SBOM.spdx.json',root/'sbom/SBOM.spdx.json']
    notices=root/'THIRD_PARTY_NOTICES.md'
    sbom=next((p for p in sbom_candidates if p.exists()),None)
    if sbom and notices.exists():
        try:
            json.loads(sbom.read_text()); data=(sha256_file(sbom)+'\n'+sha256_file(notices)+'\n').encode(); ev=out/'license_sbom.log'; ev.write_bytes(data)
            res={'status':'verified','exit_code':0,'duration_ms':0,'evidence_sha256':sha256_file(ev),'evidence_ref':ev.name,'command':None,'reason':None}
        except Exception as e:
            ev=out/'license_sbom.log'; ev.write_text(f'SBOM parse failure: {e}\n',encoding='utf-8'); res={'status':'failed','exit_code':1,'duration_ms':0,'evidence_sha256':sha256_file(ev),'evidence_ref':ev.name,'command':None,'reason':'SBOM is present but invalid'}
    else: res=unavailable('SBOM.spdx.json and THIRD_PARTY_NOTICES.md are required')
    add('license_sbom','local-sbom-check',None,res)

    # Supply chain: prefer cargo-deny, then cargo-audit. No tool => unavailable.
    if shutil.which('cargo-deny'):
        res=run_cmd(root,out,'supply_chain',['cargo','deny','check'],timeout=1800); tool='cargo-deny'; ver=tool_version(['cargo','deny','--version'])
    elif shutil.which('cargo-audit'):
        res=run_cmd(root,out,'supply_chain',['cargo','audit'],timeout=1800); tool='cargo-audit'; ver=tool_version(['cargo','audit','--version'])
    else: res=unavailable('cargo-deny or cargo-audit not installed'); tool='cargo-supply-chain'; ver=None
    add('supply_chain',tool,ver,res)

    cargo=shutil.which('cargo'); cargo_ver=tool_version(['cargo','--version']) if cargo else None
    cargo_cmds={
      'cargo_fmt':['cargo','fmt','--all','--','--check'],
      'cargo_check':['cargo','check','--locked','--workspace','--all-targets'],
      'cargo_test':['cargo','test','--locked','--workspace','--all-targets'],
      'cargo_clippy':['cargo','clippy','--locked','--workspace','--all-targets','--','-D','warnings']}
    lockfile=(root/'Cargo.lock')
    for gate,cmd in cargo_cmds.items():
        if not cargo: cres=unavailable('cargo not installed',cmd)
        elif gate != 'cargo_fmt' and not lockfile.exists(): cres=unavailable('Cargo.lock missing; release qualification requires --locked',cmd)
        else: cres=run_cmd(root,out,gate,cmd,timeout=3600)
        add(gate,'cargo',cargo_ver,cres)

    # PostgreSQL uses two connections: owner/admin for migrations and a separate RLS-constrained role.
    psql=shutil.which('psql')
    admin_url=os.environ.get('PHXCLAW_TEST_DATABASE_URL','').strip()
    rls_url=os.environ.get('PHXCLAW_RLS_DATABASE_URL','').strip()
    psql_ver=tool_version(['psql','--version']) if psql else None
    admin_env=rls_env=None; rls_role=''
    if psql and admin_url:
        try: admin_env,_=pg_env_from_url(admin_url)
        except Exception as e: admin_env=None; pg_parse_error=str(e)
    if psql and rls_url:
        try: rls_env,rls_role=pg_env_from_url(rls_url)
        except Exception as e: rls_env=None; rls_parse_error=str(e)
    if not psql: pg_mig=unavailable('psql not installed')
    elif not admin_url: pg_mig=unavailable('PHXCLAW_TEST_DATABASE_URL not set')
    elif admin_env is None: pg_mig=unavailable('PHXCLAW_TEST_DATABASE_URL invalid: '+pg_parse_error)
    else:
        all_migs=sorted(root.glob('migrations/*.sql'))
        if not all_migs: pg_mig=unavailable('no migrations found')
        else:
            script=out/'all-migrations.sql'; script.write_text("\\set ON_ERROR_STOP on\n"+"\n".join(f"\\i '{p.as_posix()}'" for p in all_migs)+"\n",encoding='utf-8')
            pg_mig=run_cmd(root,out,'postgres_migration',['psql','-v','ON_ERROR_STOP=1','-f',str(script)],env=admin_env,timeout=1800)
    add('postgres_migration','psql',psql_ver,pg_mig)

    if psql and pg_mig['status']=='verified' and rls_url and rls_env is not None and rls_role:
        # Grant the already-provisioned non-superuser role access after migrations. Never create roles here.
        grant_sql=out/'rls-grants.sql'
        try:
            role_sql=quote_ident(rls_role)
            grant_sql.write_text(
                f"GRANT USAGE ON SCHEMA phxclaw TO {role_sql};\n"
                f"GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA phxclaw TO {role_sql};\n"
                f"GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA phxclaw TO {role_sql};\n",
                encoding='utf-8')
            grant=run_cmd(root,out,'rls_grants',['psql','-v','ON_ERROR_STOP=1','-f',str(grant_sql)],env=admin_env,timeout=300)
        except Exception as e:
            grant={'status':'failed','exit_code':None,'duration_ms':0,'evidence_sha256':None,'evidence_ref':None,'command':None,'reason':str(e)}
        if grant['status']=='verified':
            prereq=run_cmd(root,out,'rls_role_prereq',['psql','-v','ON_ERROR_STOP=1','-f','tests/postgres/v025_rls_role_prereq.sql'],env=rls_env,marker='V025_RLS_ROLE_OK',timeout=300)
        else: prereq=grant
        if prereq['status']=='verified':
            rls=run_cmd(root,out,'rls_cross_tenant',['psql','-v','ON_ERROR_STOP=1','-f','tests/postgres/v024_rls_cross_tenant.sql'],env=rls_env,marker='V024_RLS_CROSS_TENANT_PASS',timeout=600)
            att=run_cmd(root,out,'release_attestation_db',['psql','-v','ON_ERROR_STOP=1','-f','tests/postgres/v025_release_attestation.sql'],env=rls_env,marker='V025_RELEASE_ATTESTATION_PASS',timeout=600)
            if att['status']!='verified':
                rls['status']='failed'; rls['reason']='v025 release attestation DB fixture failed; see release_attestation_db.log'
                combined=(out/'rls_cross_tenant.log').read_bytes()+(out/'release_attestation_db.log').read_bytes(); (out/'rls_cross_tenant.combined').write_bytes(combined); rls['evidence_sha256']=sha256_file(out/'rls_cross_tenant.combined'); rls['evidence_ref']='rls_cross_tenant.combined'
        else:
            rls={'status':'failed','exit_code':prereq.get('exit_code'),'duration_ms':prereq.get('duration_ms',0),'evidence_sha256':prereq.get('evidence_sha256'),'evidence_ref':prereq.get('evidence_ref'),'command':prereq.get('command'),'reason':'RLS role prerequisite/grant failed'}
    elif not rls_url: rls=unavailable('PHXCLAW_RLS_DATABASE_URL not set; admin connection cannot prove RLS')
    elif rls_env is None: rls=unavailable('PHXCLAW_RLS_DATABASE_URL invalid: '+rls_parse_error)
    else: rls=unavailable('requires verified postgres_migration and a named RLS role')
    add('rls_cross_tenant','psql',psql_ver,rls)

    # F24 -> F25 lineage is a named Rust unit test so cargo output is direct evidence.
    if cargo and lockfile.exists():
        lin=run_cmd(root,out,'skill_knowledge_lineage',['cargo','test','--locked','-p','phxclaw-release-hardening','skill_release_lineage','--','--nocapture'],timeout=1800)
    elif cargo: lin=unavailable('Cargo.lock missing; release qualification requires --locked')
    else: lin=unavailable('cargo not installed')
    add('skill_knowledge_lineage','cargo',cargo_ver,lin)

    ext=[('mission_e2e','PHXCLAW_MISSION_E2E_CMD'),('tauri_native_e2e','PHXCLAW_TAURI_E2E_CMD'),('provider_model_e2e','PHXCLAW_PROVIDER_MODEL_E2E_CMD')]
    for gate,envname in ext:
        cmd=env_command(envname)
        add(gate,envname,None,run_cmd(root,out,gate,cmd,timeout=3600) if cmd else unavailable(f'{envname} not set'))

    # Re-hash after every command. A qualification run is invalid if its own gates mutate source state.
    final_manifest=source_state.manifest(root); final_workspace=source_state.tree_hash(final_manifest)
    (out/'source-state-after.json').write_text(json.dumps({'algorithm':'sha256-tree-v1','workspace_sha256':final_workspace,'file_count':len(final_manifest),'files':final_manifest},indent=2)+'\n',encoding='utf-8')
    if final_workspace != workspace:
        wp=next(p for p in proofs if p['gate']=='workspace_static')
        wp['status']='failed'; wp['reason']='source state changed during qualification'; wp['evidence_sha256']=sha256_file(out/'source-state-after.json'); wp['evidence_ref']='source-state-after.json'
        (out/'workspace_static.json').write_text(json.dumps(wp,indent=2)+'\n',encoding='utf-8')

    by={p['gate']:p for p in proofs}
    verified=[g for g in GATES if by[g]['status']=='verified']
    rust_gate_names={
      'workspace_static':'WorkspaceStatic','json_schema_static':'JsonSchemaStatic','migration_static':'MigrationStatic',
      'license_sbom':'LicenseSbom','supply_chain':'SupplyChain','cargo_fmt':'CargoFmt','cargo_check':'CargoCheck',
      'cargo_test':'CargoTest','cargo_clippy':'CargoClippy','postgres_migration':'PostgresMigration',
      'rls_cross_tenant':'RlsCrossTenant','skill_knowledge_lineage':'SkillKnowledgeLineage','mission_e2e':'MissionE2e',
      'tauri_native_e2e':'TauriNativeE2e','provider_model_e2e':'ProviderModelE2e'}
    blockers=[]; diagnostics=[]
    for g in GATES:
        status=by[g]['status']
        if status!='verified':
            blockers.append(f"{rust_gate_names[g]}: {status}")
            diagnostics.append(f"{g}: {status}"+(f" ({by[g]['reason']})" if by[g]['reason'] else ''))
    source_ready=all(by[g]['status']=='verified' for g in ['workspace_static','json_schema_static','migration_static'])
    static_verified=source_ready and all(by[g]['status']=='verified' for g in ['license_sbom','supply_chain'])
    runtime_verified=static_verified and all(by[g]['status']=='verified' for g in ['cargo_fmt','cargo_check','cargo_test','cargo_clippy','postgres_migration','rls_cross_tenant','skill_knowledge_lineage'])
    e2e_verified=runtime_verified and all(by[g]['status']=='verified' for g in ['mission_e2e','tauri_native_e2e','provider_model_e2e'])
    release_ready=e2e_verified and len(verified)==len(GATES)

    canonical_rows=[]
    for p in sorted(proofs,key=lambda x:x['gate']):
        canonical_rows.append({
          'gate':p['gate'],'status':p['status'],'source_state_sha256':p['source_state_sha256_hex'].lower(),
          'evidence_sha256':p['evidence_sha256_hex'],'tool_name':p['tool_name'],'tool_version':p['tool_version'],
          'evidence_ref':p['evidence_ref'],'verified_at':p['verified_at']})
    proof_hash=sha256_bytes(json.dumps(canonical_rows,separators=(',',':'),ensure_ascii=False).encode())
    assessment={
      'workspace_sha256_hex':workspace.lower(),'source_ready':source_ready,'static_verified':static_verified,
      'runtime_verified':runtime_verified,'e2e_verified':e2e_verified,'release_ready':release_ready,
      'verified_gates':verified,'blockers':blockers}
    run={'run_uuid':str(uuid7()),'release_uuid':str(uuid7()),'version':'0.25.0','workspace_sha256':workspace,'final_workspace_sha256':final_workspace,'source_state_stable':final_workspace==workspace,'started_at':started,'finished_at':utcnow(),'proofs':proofs,'assessment':assessment,'diagnostics':diagnostics}
    (out/'qualification-run.json').write_text(json.dumps(run,indent=2)+'\n',encoding='utf-8')
    att={
      'attestation_uuid':str(uuid7()),'run_uuid':run['run_uuid'],'release_uuid':run['release_uuid'],'version':'0.25.0',
      'workspace_sha256':workspace.lower(),'assessment':assessment,'proof_bundle_sha256':proof_hash,
      'signing_payload_sha256':'','created_at':utcnow(),'signer_key_id':None,'signature_b64':None,'signature_algorithm':None}
    def signing_payload(a):
        ass=a['assessment']
        return (f"phxclaw-release-attestation-v025\n"
          f"attestation_uuid={a['attestation_uuid']}\nrun_uuid={a['run_uuid']}\nrelease_uuid={a['release_uuid']}\n"
          f"version={a['version']}\nworkspace_sha256={a['workspace_sha256'].lower()}\nproof_bundle_sha256={a['proof_bundle_sha256'].lower()}\n"
          f"source_ready={1 if ass['source_ready'] else 0}\nstatic_verified={1 if ass['static_verified'] else 0}\n"
          f"runtime_verified={1 if ass['runtime_verified'] else 0}\ne2e_verified={1 if ass['e2e_verified'] else 0}\n"
          f"release_ready={1 if ass['release_ready'] else 0}\ncreated_at={a['created_at']}\n").encode()
    payload=signing_payload(att); att['signing_payload_sha256']=sha256_bytes(payload)
    payload_path=out/'attestation-signing-payload.bin'; payload_path.write_bytes(payload)
    att_path=out/'release-attestation.json'; att_path.write_text(json.dumps(att,indent=2)+'\n',encoding='utf-8')
    sign=env_command('PHXCLAW_RELEASE_SIGN_CMD')
    signature_verified=False
    if sign:
        # Signer receives the exact canonical payload path and emits JSON with Ed25519 metadata.
        try:
            sr=subprocess.run(sign+[str(payload_path)],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=300)
            if sr.returncode==0:
                sig=json.loads(sr.stdout)
                att['signer_key_id']=sig['signer_key_id']; att['signature_b64']=sig['signature_b64']
                att['signature_algorithm']=sig.get('signature_algorithm','ed25519')
                if att['signature_algorithm']!='ed25519': raise ValueError('signature_algorithm must be ed25519')
                base64.b64decode(att['signature_b64'],validate=True)
                att_path.write_text(json.dumps(att,indent=2)+'\n',encoding='utf-8')
        except Exception as e:
            (out/'signing-error.log').write_text(str(e)+'\n',encoding='utf-8')
    builtin_ok,builtin_reason=verify_trusted_ed25519(root,att,payload)
    signature_verified=builtin_ok
    verify_cmd=env_command('PHXCLAW_RELEASE_VERIFY_CMD')
    if builtin_ok and verify_cmd:
        vr=subprocess.run(verify_cmd+[str(payload_path),str(att_path)],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=300)
        (out/'signature-verification.log').write_text(vr.stdout or '',encoding='utf-8')
        signature_verified=(vr.returncode==0)
        if not signature_verified: builtin_reason='external signature verifier rejected attestation'
    elif not builtin_ok:
        (out/'signature-verification.log').write_text((builtin_reason or 'signature verification failed')+'\n',encoding='utf-8')
    public_release_ready=release_ready and signature_verified
    print(json.dumps({'workspace_sha256':workspace,**assessment,'diagnostics':diagnostics,'signature_verified':signature_verified,'signature_verification_reason':builtin_reason,'public_release_ready':public_release_ready,'output':str(out)},indent=2))
    if getattr(args,'public_release',False) and not public_release_ready: raise SystemExit(3)
    if args.strict and not release_ready: raise SystemExit(2)
if __name__=='__main__': main()

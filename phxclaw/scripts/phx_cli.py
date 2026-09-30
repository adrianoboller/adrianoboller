#!/usr/bin/env python3
from pathlib import Path
import argparse, base64, hashlib, json, os, shutil, subprocess, sys, tempfile, zipfile, secrets, time
from datetime import datetime, timezone

ROOT=Path(__file__).resolve().parents[1]
VERSION="0.20.0"
PLUGIN_STATE_DIR=ROOT/'var/plugin-registry'
PLUGIN_STATE=PLUGIN_STATE_DIR/'installed.json'
PLUGIN_STORE=ROOT/'var/plugin-store'
CHECKPOINT_STORE=ROOT/'var/checkpoints'

try:
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
except Exception:
    Ed25519PublicKey=None
try:
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM
except Exception:
    AESGCM=None

SECRET_ROOT=ROOT/'var/secrets'
SECRET_KEY=SECRET_ROOT/'master.key'
SECRET_INDEX=SECRET_ROOT/'index.json'
SECRET_MAGIC=b'PHXSECRET1'

def new_uuid7():
    ms=int(time.time()*1000)&((1<<48)-1); rand_a=secrets.randbits(12); rand_b=secrets.randbits(62)
    value=(ms<<80)|(0x7<<76)|(rand_a<<64)|(0b10<<62)|rand_b; h=f'{value:032x}'
    return f'{h[:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:]}'

def ensure_secret_store():
    if AESGCM is None: raise SystemExit('cryptography package required for secret broker bootstrap')
    SECRET_ROOT.mkdir(parents=True,exist_ok=True)
    if not SECRET_KEY.exists():
        SECRET_KEY.write_text(base64.b64encode(os.urandom(32)).decode()+'\n')
        try: os.chmod(SECRET_KEY,0o600)
        except Exception: pass
    if not SECRET_INDEX.exists(): SECRET_INDEX.write_text(json.dumps({'version':'1.0.0','secrets':{},'leases':{}},indent=2)+'\n')

def load_secret_index(): ensure_secret_store(); return json.loads(SECRET_INDEX.read_text())
def save_secret_index(d): SECRET_INDEX.write_text(json.dumps(d,indent=2,ensure_ascii=False)+'\n')
def secret_key_bytes():
    ensure_secret_store(); raw=base64.b64decode(SECRET_KEY.read_text().strip(),validate=True)
    if len(raw)!=32: raise SystemExit('invalid PhxClaw master key length')
    return raw
def valid_secret_name(v): return bool(v) and len(v)<=128 and all(ch.isalnum() or ch in '-_.' for ch in v)
def secret_path(uid): return SECRET_ROOT/f'{uid}.phxsecret'

def secret_encrypt(meta,value):
    key=secret_key_bytes(); nonce=os.urandom(12); ct=AESGCM(key).encrypt(nonce,value.encode(),None)
    env={'descriptor':meta,'nonce_base64':base64.b64encode(nonce).decode(),'ciphertext_base64':base64.b64encode(ct).decode()}
    b=SECRET_MAGIC+json.dumps(env,ensure_ascii=False,indent=2).encode(); path=secret_path(meta['uuid']); path.write_bytes(b)
    try: os.chmod(path,0o600)
    except Exception: pass

def secret_decrypt(uid):
    b=secret_path(uid).read_bytes()
    if not b.startswith(SECRET_MAGIC): raise SystemExit('invalid secret file magic')
    env=json.loads(b[len(SECRET_MAGIC):]); nonce=base64.b64decode(env['nonce_base64']); ct=base64.b64decode(env['ciphertext_base64'])
    value=AESGCM(secret_key_bytes()).decrypt(nonce,ct,None).decode()
    return env,value

def cmd_secrets(args):
    idx=load_secret_index()
    if args.action=='init':
        print(f'INITIALIZED secret_store={SECRET_ROOT} key_provider=bootstrap-file-key'); return
    if args.action=='list':
        for uid,m in sorted(idx['secrets'].items(),key=lambda kv:(kv[1].get('namespace',''),kv[1].get('name',''))):
            print(f"{uid} {m['namespace']}/{m['name']} v{m['version']} revoked={bool(m.get('revoked_at'))}")
        return
    if args.action=='store':
        if not valid_secret_name(args.name) or not valid_secret_name(args.namespace): raise SystemExit('invalid secret name/namespace')
        value=args.value if args.value is not None else sys.stdin.read().rstrip('\n')
        if not value: raise SystemExit('secret value must be supplied via --value or stdin')
        uid=new_uuid7(); meta={'uuid':uid,'name':args.name,'namespace':args.namespace,'version':1,'scopes':sorted(set(args.scope or [])),'sha256':hashlib.sha256(value.encode()).hexdigest(),'created_at':now(),'rotated_at':None,'revoked_at':None}
        secret_encrypt(meta,value); idx['secrets'][uid]=meta; save_secret_index(idx); print(f'STORED {uid} {args.namespace}/{args.name} value=[REDACTED]'); return
    if args.uuid not in idx['secrets']: raise SystemExit(f'secret not found: {args.uuid}')
    meta=idx['secrets'][args.uuid]
    if args.action=='show': print(json.dumps(meta,ensure_ascii=False,indent=2)); return
    if args.action=='doctor':
        env,value=secret_decrypt(args.uuid); ok=hashlib.sha256(value.encode()).hexdigest()==meta['sha256'] and env['descriptor']['version']==meta['version']
        print(f"secret={args.uuid} decrypt={'PASS' if ok else 'FAIL'} value=[REDACTED]"); raise SystemExit(0 if ok else 1)
    if args.action=='rotate':
        if meta.get('revoked_at'): raise SystemExit('secret is revoked')
        value=args.value if args.value is not None else sys.stdin.read().rstrip('\n')
        if not value: raise SystemExit('new secret value required')
        meta['version']+=1; meta['rotated_at']=now(); meta['sha256']=hashlib.sha256(value.encode()).hexdigest(); secret_encrypt(meta,value); save_secret_index(idx); print(f"ROTATED {args.uuid} v{meta['version']} value=[REDACTED]"); return
    if args.action=='revoke':
        meta['revoked_at']=now(); env,value=secret_decrypt(args.uuid); secret_encrypt(meta,value); save_secret_index(idx)
        for lease in idx['leases'].values():
            if lease['secret_uuid']==args.uuid and not lease.get('revoked_at'): lease['revoked_at']=now()
        save_secret_index(idx); print(f'REVOKED {args.uuid}'); return
    if args.action=='lease':
        if meta.get('revoked_at'): raise SystemExit('secret is revoked')
        ttl=max(1,min(int(args.ttl),3600)); scope=args.scope
        allowed=meta.get('scopes',[])
        if '*' not in allowed and scope not in allowed: raise SystemExit(f'scope denied: {scope}')
        import datetime as _dt
        issued=_dt.datetime.now(_dt.timezone.utc); expires=issued+_dt.timedelta(seconds=ttl); lid=new_uuid7()
        lease={'uuid':lid,'secret_uuid':args.uuid,'secret_version':meta['version'],'consumer':args.consumer,'scope':scope,'issued_at':issued.isoformat(),'expires_at':expires.isoformat(),'revoked_at':None}
        idx['leases'][lid]=lease; save_secret_index(idx); print(json.dumps(lease,ensure_ascii=False,indent=2)); return


def loadj(p): return json.loads((ROOT/p).read_text(encoding='utf-8'))
def now(): return datetime.now(timezone.utc).isoformat()
def sha256_file(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def sha256_bytes(b): return hashlib.sha256(b).hexdigest()
def uuid_v7(s):
    try:
        import uuid
        u=uuid.UUID(str(s)); return u.version==7
    except Exception: return False

def ensure_state():
    PLUGIN_STATE_DIR.mkdir(parents=True,exist_ok=True); PLUGIN_STORE.mkdir(parents=True,exist_ok=True)
    if not PLUGIN_STATE.exists(): PLUGIN_STATE.write_text(json.dumps({'version':'1.0.0','plugins':{}},indent=2)+'\n')
def load_state(): ensure_state(); return json.loads(PLUGIN_STATE.read_text())
def save_state(s): PLUGIN_STATE.write_text(json.dumps(s,indent=2,ensure_ascii=False)+'\n')

def path_safe(base, rel):
    rel=Path(rel)
    if rel.is_absolute() or '..' in rel.parts: raise SystemExit(f'unsafe package path: {rel}')
    out=(Path(base)/rel).resolve(); base=Path(base).resolve()
    if out!=base and base not in out.parents: raise SystemExit(f'path escapes package root: {rel}')
    return out

def read_trust_store(): return loadj('config/trust/plugin-signers.json')
def manifesto_canonico(manifest):
    # mesma forma do Rust (phxclaw_plugin_registry::manifesto_canonico): JSON compacto,
    # chaves ordenadas, assinatura vazia
    import copy, hashlib, json as _j
    m = copy.deepcopy(manifest)
    m["integrity"]["signature"] = ""
    return _j.dumps(m, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def mensagem_do_formato(manifest, formato):
    i = manifest["integrity"]
    base = (f"uuid={manifest['uuid']}\nname={manifest['name']}\n"
            f"version={manifest['version']}\nsha256={i['digest']}\n")
    if formato >= 2:
        import hashlib
        h = hashlib.sha256(manifesto_canonico(manifest).encode("utf-8")).hexdigest()
        return (f"PHXCLAW-PLUGIN-V2\n{base}manifesto_sha256={h}\n").encode("utf-8")
    return ("PHXCLAW-PLUGIN-V1\n" + base).encode("utf-8")


def signing_message(manifest):
    i=manifest['integrity']
    return (f"PHXCLAW-PLUGIN-V1\nuuid={manifest['uuid']}\nname={manifest['name']}\nversion={manifest['version']}\nsha256={i['digest']}\n").encode()

def verify_manifest_signature(manifest):
    signer_id=manifest['integrity']['signer']
    signer=next((s for s in read_trust_store()['signers'] if s['id']==signer_id and s.get('status')=='active'),None)
    if not signer: raise SystemExit(f'untrusted or inactive signer: {signer_id}')
    if not any(manifest['name'].startswith(p) for p in signer.get('allowed_name_prefixes',[])):
        raise SystemExit(f'signer {signer_id} not allowed for {manifest["name"]}')
    if Ed25519PublicKey is None: raise SystemExit('cryptography package required for Ed25519 verification')
    try:
        pub=Ed25519PublicKey.from_public_bytes(base64.b64decode(signer['public_key_base64'],validate=True))
        pub.verify(base64.b64decode(manifest['integrity']['signature'],validate=True), mensagem_do_formato(manifest, signer.get('signature_format',1)))
    except Exception as e: raise SystemExit(f'Ed25519 verification failed: {e}')

def core_compatible(req):
    # strict enough for PhxClaw manifests: comma-separated >= / > / <= / < / = constraints
    def ver(s):
        parts=s.strip().split('.')
        try:return tuple(int(x) for x in (parts+['0','0'])[:3])
        except:return None
    cur=(0,18,0)
    for raw in req.split(','):
        raw=raw.strip(); op=''
        for candidate in ('>=','<=','>','<','='):
            if raw.startswith(candidate): op=candidate; raw=raw[len(candidate):].strip(); break
        v=ver(raw)
        if not op or v is None: return False
        ok={'>=':cur>=v,'<=':cur<=v,'>':cur>v,'<':cur<v,'=':cur==v}[op]
        if not ok:return False
    return True


def package_files_digest(items):
    lines=[]
    for item in sorted(items,key=lambda x:x['path']):
        lines.append(f"{item['path']}={str(item['sha256']).lower()}\n")
    return hashlib.sha256(''.join(lines).encode()).hexdigest()

def package_signing_message(manifest,pkg):
    return (
        "PHXCLAW-PACKAGE-V1\n"
        f"uuid={manifest['uuid']}\n"
        f"name={manifest['name']}\n"
        f"version={manifest['version']}\n"
        f"manifest_sha256={pkg['manifest_sha256']}\n"
        f"files_digest={pkg['files_digest']}\n"
    ).encode()

def verify_package_signature(manifest,pkg):
    signer_id=pkg['signer']
    if signer_id != manifest['integrity']['signer']:
        raise SystemExit('package signer must match manifest signer')
    signer=next((s for s in read_trust_store()['signers'] if s['id']==signer_id and s.get('status')=='active'),None)
    if not signer: raise SystemExit(f'untrusted or inactive package signer: {signer_id}')
    if Ed25519PublicKey is None: raise SystemExit('cryptography package required for Ed25519 verification')
    try:
        pub=Ed25519PublicKey.from_public_bytes(base64.b64decode(signer['public_key_base64'],validate=True))
        pub.verify(base64.b64decode(pkg['signature'],validate=True), package_signing_message(manifest,pkg))
    except Exception as e: raise SystemExit(f'package Ed25519 verification failed: {e}')

def locate_package_root(path):
    p=Path(path).expanduser().resolve()
    tmp=None
    if p.is_file() and p.suffix.lower() in ('.zip','.phxplugin'):
        tmp=Path(tempfile.mkdtemp(prefix='phxplugin-'))
        with zipfile.ZipFile(p) as z:
            for info in z.infolist():
                q=Path(info.filename)
                if q.is_absolute() or '..' in q.parts: raise SystemExit(f'unsafe zip entry: {q}')
            z.extractall(tmp)
        cands=list(tmp.glob('PACKAGE.json'))+list(tmp.glob('*/PACKAGE.json'))
        if len(cands)!=1: raise SystemExit('plugin archive must contain exactly one PACKAGE.json at root or one top-level folder')
        return cands[0].parent,tmp
    if p.is_dir(): return p,None
    raise SystemExit(f'plugin package not found: {p}')

def verify_package(package_root):
    pkg_path=package_root/'PACKAGE.json'
    if not pkg_path.exists(): raise SystemExit('PACKAGE.json missing')
    pkg=json.loads(pkg_path.read_text())
    if pkg.get('format')!='phxclaw-plugin-package-v1': raise SystemExit('unsupported PACKAGE.json format')
    required=('manifest','manifest_sha256','license','readme','sbom','files','files_digest','signature_algorithm','signature','signer')
    for k in required:
        if k not in pkg: raise SystemExit(f'PACKAGE.json missing: {k}')
    if pkg.get('signature_algorithm')!='ed25519': raise SystemExit('plugin package must use ed25519 signature')
    listed=[]; seen=set()
    for item in pkg['files']:
        rel=item['path']
        if rel in seen: raise SystemExit(f'duplicate package file: {rel}')
        seen.add(rel); f=path_safe(package_root,rel)
        if f.is_symlink(): raise SystemExit(f'symlink not allowed in plugin package: {rel}')
        if not f.is_file(): raise SystemExit(f'package file missing: {rel}')
        got=sha256_file(f)
        if got.lower()!=str(item['sha256']).lower(): raise SystemExit(f'package hash mismatch: {rel}')
        listed.append(rel)
    for ref in (pkg['manifest'],pkg['license'],pkg['readme'],pkg['sbom']):
        f=path_safe(package_root,ref)
        if not f.is_file(): raise SystemExit(f'package reference missing: {ref}')
        if ref not in seen: raise SystemExit(f'package reference is not hash-listed: {ref}')
    actual_files=set()
    for f in package_root.rglob('*'):
        if f.is_symlink(): raise SystemExit(f'symlink not allowed in plugin package: {f.relative_to(package_root)}')
        if f.is_file() and f.name!='PACKAGE.json': actual_files.add(f.relative_to(package_root).as_posix())
    if actual_files != seen:
        extra=sorted(actual_files-seen); missing=sorted(seen-actual_files)
        raise SystemExit(f'package file inventory mismatch extra={extra} missing={missing}')
    computed_files_digest=package_files_digest(pkg['files'])
    if computed_files_digest.lower()!=pkg['files_digest'].lower(): raise SystemExit('package files_digest mismatch')
    mpath=path_safe(package_root,pkg['manifest']); manifest_bytes=mpath.read_bytes()
    if hashlib.sha256(manifest_bytes).hexdigest().lower()!=pkg['manifest_sha256'].lower(): raise SystemExit('package manifest_sha256 mismatch')
    manifest=json.loads(manifest_bytes)
    for k in ('uuid','name','version','core_api','entrypoint','capabilities','permissions','integrity','tests','rollback'):
        if k not in manifest: raise SystemExit(f'manifest missing: {k}')
    if not uuid_v7(manifest['uuid']): raise SystemExit('plugin uuid must be UUIDv7')
    if not core_compatible(manifest['core_api']): raise SystemExit(f'plugin core_api incompatible with {VERSION}: {manifest["core_api"]}')
    artifact=path_safe(package_root,manifest['integrity']['artifact'])
    artifact_rel=artifact.relative_to(package_root).as_posix()
    if artifact_rel not in seen: raise SystemExit('plugin artifact is not hash-listed in PACKAGE.json')
    if not artifact.is_file(): raise SystemExit('plugin artifact missing')
    digest=sha256_file(artifact)
    if digest.lower()!=manifest['integrity']['digest'].lower(): raise SystemExit('plugin artifact sha256 mismatch')
    if manifest['integrity'].get('signature_algorithm')!='ed25519': raise SystemExit('only ed25519 signatures are accepted')
    verify_manifest_signature(manifest)
    verify_package_signature(manifest,pkg)
    return pkg,manifest

def current_record(state,name): return state['plugins'].get(name)

def install_plugin(path):
    package_root,tmp=locate_package_root(path)
    try:
        pkg,manifest=verify_package(package_root); state=load_state(); name=manifest['name']; old=current_record(state,name)
        dest=PLUGIN_STORE/manifest['uuid']/manifest['version']
        if dest.exists(): raise SystemExit(f'already installed: {name} {manifest["version"]}')
        dest.parent.mkdir(parents=True,exist_ok=True); shutil.copytree(package_root,dest)
        history=list(old.get('history',[])) if old else []
        if old: history.append({k:old[k] for k in ('version','path','state','installed_at') if k in old})
        state['plugins'][name]={
            'uuid':manifest['uuid'],'version':manifest['version'],'path':str(dest.relative_to(ROOT)),
            'state':'disabled','installed_at':now(),'capabilities':manifest['capabilities'],'history':history
        }
        save_state(state); print(f'INSTALLED {name} {manifest["version"]} state=disabled')
    finally:
        if tmp: shutil.rmtree(tmp,ignore_errors=True)

def plugin_root_from_record(rec): return (ROOT/rec['path']).resolve()
def verify_installed(name):
    state=load_state(); rec=current_record(state,name)
    if not rec: raise SystemExit(f'plugin not installed: {name}')
    _,manifest=verify_package(plugin_root_from_record(rec))
    return state,rec,manifest

def doctor_plugin(name,run_health=True):
    state,rec,manifest=verify_installed(name); health='STATIC_OK'
    artifact=path_safe(plugin_root_from_record(rec),manifest['integrity']['artifact'])
    if run_health and manifest['entrypoint']['type']=='process' and os.access(artifact,os.X_OK):
        try:
            r=subprocess.run([str(artifact),'--health'],cwd=plugin_root_from_record(rec),capture_output=True,text=True,timeout=min(30,max(1,manifest['sandbox'].get('timeout_ms',5000)//1000)))
            if r.returncode!=0: raise SystemExit(f'health failed rc={r.returncode}: {(r.stderr or r.stdout).strip()}')
            health=(r.stdout or 'OK').strip().replace('\n',' ')[:300]
        except subprocess.TimeoutExpired: raise SystemExit('health timed out')
    rec['last_doctor_at']=now(); rec['last_health']=health; state['plugins'][name]=rec; save_state(state)
    print(f'HEALTHY {name} {rec["version"]} {health}')

def set_plugin_state(name,newstate):
    state,rec,_=verify_installed(name)
    if newstate=='enabled': doctor_plugin(name,run_health=True); state=load_state(); rec=current_record(state,name)
    rec['state']=newstate; rec['state_changed_at']=now(); state['plugins'][name]=rec; save_state(state)
    print(f'{newstate.upper()} {name} {rec["version"]}')

def rollback_plugin(name):
    state=load_state(); rec=current_record(state,name)
    if not rec: raise SystemExit(f'plugin not installed: {name}')
    hist=rec.get('history',[])
    if not hist: raise SystemExit(f'no previous version recorded for {name}')
    prev=hist.pop(); current={k:rec[k] for k in ('version','path','state','installed_at') if k in rec}
    rec['version']=prev['version']; rec['path']=prev['path']; rec['state']='disabled'; rec['installed_at']=prev.get('installed_at',now()); rec['history']=hist+[current]; rec['rolled_back_at']=now()
    _,old_manifest=verify_package(plugin_root_from_record(rec)); rec['capabilities']=old_manifest['capabilities']; state['plugins'][name]=rec; save_state(state); doctor_plugin(name,run_health=False); print(f'ROLLED_BACK {name} -> {rec["version"]} state=disabled')

def uninstall_plugin(name):
    state=load_state(); rec=current_record(state,name)
    if not rec: raise SystemExit(f'plugin not installed: {name}')
    if rec.get('state')=='enabled': raise SystemExit('disable plugin before uninstall')
    shutil.rmtree(PLUGIN_STORE/rec['uuid'],ignore_errors=True); del state['plugins'][name]; save_state(state); print(f'UNINSTALLED {name}')

def cmd_version(_): print(f"PhxClaw {VERSION}")
def _sprint_class(status):
    st=(status or '').lower()
    if st in ('done','completed','complete','closed'): return ('green','\x1b[32m','GREEN')
    if st in ('planned','backlog','todo'): return ('red','\x1b[31m','RED')
    return ('yellow','\x1b[33m','YELLOW')

def cmd_status(_):
    sprints=sorted((ROOT/'sprints').glob('f*.json')); caps=loadj('config/capability-catalog.json'); agents=list((ROOT/'config/agents').glob('*.agent.json')); bridges=loadj('config/external-capability-bridges.json')['bridges']; installed=len(load_state()['plugins'])
    counts={'green':0,'yellow':0,'red':0}
    for p in sprints:
        d=json.loads(p.read_text(encoding='utf-8')); counts[_sprint_class(d.get('status'))[0]]+=1
    print(f"version={VERSION}\nsprints={len(sprints)}\nsprints_green={counts['green']}\nsprints_yellow={counts['yellow']}\nsprints_red={counts['red']}\nagents={len(agents)}\ncapabilities={caps['count']}\nexternal_bridges={len(bridges)}\ninstalled_plugins={installed}\nrepository_visibility=private_only")

def cmd_core(args):
    binary = ROOT/'target/release/phxclaw'
    if os.name=='nt': binary = binary.with_suffix('.exe')
    if args.action=='status':
        if binary.exists(): raise SystemExit(subprocess.call([str(binary),'core','status'],cwd=ROOT))
        print(json.dumps({'product':'PhxClaw','version':VERSION,'core_binary':'not_built','source_ready':True,'static_verified':True,'release_ready':False},indent=2)); return
    if not binary.exists(): raise SystemExit('PhxClaw Rust core is not built. Run cargo build --release first.')
    raise SystemExit(subprocess.call([str(binary),'core','start'],cwd=ROOT))

def cmd_install(args):
    cmd=[sys.executable,str(ROOT/'installer/bootstrap.py'),'install']
    if args.yes: cmd.append('--yes')
    raise SystemExit(subprocess.call(cmd,cwd=ROOT))

def cmd_db(args):
    cmd=[sys.executable,str(ROOT/'installer/bootstrap.py'),args.action]
    if args.action=='install' and args.yes: cmd.append('--yes')
    raise SystemExit(subprocess.call(cmd,cwd=ROOT))

def cmd_doctor(_):
    tools=['git','python3','cargo','rustc','node','npm']; bad=0
    for t in tools:
        p=shutil.which(t); print(f"{t}: {p or 'MISSING'}")
        if t in ('git','python3') and not p: bad=1
    print('release_ready: NO (requires all release gates)'); raise SystemExit(bad)
def cmd_sprints(args):
    items=[json.loads(p.read_text()) for p in sorted((ROOT/'sprints').glob('f*.json'))]
    if args.action=='list':
        use_color=sys.stdout.isatty() and not getattr(args,'no_color',False)
        reset='\x1b[0m' if use_color else ''
        counts={'green':0,'yellow':0,'red':0}
        for d in items:
            color,ansi,label=_sprint_class(d.get('status')); counts[color]+=1
            prefix=ansi if use_color else ''
            print(f"{prefix}{d['code']:>3}  {label:<6}  {d['status']:<10}  {d['title']}{reset}")
        print(f"TOTAL {len(items)} | GREEN={counts['green']} YELLOW={counts['yellow']} RED={counts['red']}")
    else:
        code=args.code.upper(); d=next((x for x in items if x['code']==code),None)
        if not d: raise SystemExit(f"sprint not found: {code}")
        print(json.dumps(d,ensure_ascii=False,indent=2))

def cmd_plugins(args):
    bridges=loadj('config/external-capability-bridges.json')['bridges']; state=load_state()
    if args.action=='list':
        for x in bridges: print(f"bridge {x['slug']:<20} {x['state']:<24} {x['capability']}")
        for name,rec in sorted(state['plugins'].items()): print(f"plugin {name:<38} {rec['state']:<10} {rec['version']}")
    elif args.action=='show':
        x=next((x for x in bridges if x['slug']==args.name),None)
        if x: print(json.dumps(x,ensure_ascii=False,indent=2)); return
        rec=state['plugins'].get(args.name)
        if not rec: raise SystemExit(f"plugin/bridge not found: {args.name}")
        print(json.dumps(rec,ensure_ascii=False,indent=2))
    elif args.action=='install': install_plugin(args.path)
    elif args.action=='enable': set_plugin_state(args.name,'enabled')
    elif args.action=='disable': set_plugin_state(args.name,'disabled')
    elif args.action=='doctor': doctor_plugin(args.name,run_health=not args.static_only)
    elif args.action=='rollback': rollback_plugin(args.name)
    elif args.action=='uninstall': uninstall_plugin(args.name)
def cmd_agents(args):
    files=sorted((ROOT/'config/agents').glob('*.agent.json'))
    if args.action=='count': print(len(files)); return
    lim=args.limit or len(files)
    for p in files[:lim]:
        d=json.loads(p.read_text()); print(f"{d.get('agent_id',0):03d}  {d['name']}  [{d.get('role_type','')}]")
def cmd_caps(args):
    items=loadj('config/capability-catalog.json')['capabilities']
    for x in items:
        if args.filter and args.filter.lower() not in x['name'].lower(): continue
        print(f"{x['name']:<38} {x['status']:<26} {x['service']}")
def cmd_mission(args):
    p=Path(args.file)
    if not p.exists(): raise SystemExit(f"mission file not found: {p}")
    d=json.loads(p.read_text()); required=['uuid','name','project_root','steps']; missing=[k for k in required if k not in d]
    if args.action=='validate':
        if missing: raise SystemExit('missing fields: '+', '.join(missing))
        print(f"VALID mission={d.get('name')} steps={len(d.get('steps',[]))}"); return
    binary=ROOT/'target/release/phxclaw-mission-cli'
    if os.name=='nt': binary=binary.with_suffix('.exe')
    if not binary.exists(): raise SystemExit('mission runtime binary not built; run cargo build --release first')
    raise SystemExit(subprocess.call([str(binary),str(p)]))

def cmd_team(args):
    if args.action=='demo':
        script=ROOT/'scripts/team_runtime_bootstrap.py'
        raise SystemExit(subprocess.call([sys.executable,str(script),'demo'],cwd=ROOT))
    if args.action=='status':
        sprint=loadj('sprints/f20.json'); print(json.dumps({'sprint':sprint['code'],'status':sprint['status'],'runtime':'crates/phxclaw-team-runtime','bootstrap':'scripts/team_runtime_bootstrap.py'},ensure_ascii=False,indent=2))

def cmd_paths(_): print(f"root={ROOT}\ncli={ROOT/'bin/phx'}\nmission_cli=apps/phxclaw-mission-cli\nui=apps/phxclaw-ui\nplugin_store={PLUGIN_STORE}\nplugin_state={PLUGIN_STATE}\nsprints=sprints")
def cmd_privacy(_): print(json.dumps(loadj('config/private-repository-policy.json'),ensure_ascii=False,indent=2))


def cmd_channels(args):
    cfg=loadj('config/channel-providers.json')
    if args.action=='providers':
        for item in cfg['providers']:
            print(f"{item['id']:<12} enabled={str(item['enabled']).lower():<5} token_source={item['token_source']:<18} origin={item['default_origin']}")
        return
    if args.action=='status':
        print(json.dumps({'gateway':'crates/phxclaw-channel-gateway','providers':'crates/phxclaw-channel-providers','policy':cfg['policy'],'provider_count':len(cfg['providers'])},ensure_ascii=False,indent=2))
        return

def cmd_upstream(args):
    if args.project == 'claw-code':
        cfg=loadj('config/upstreams/claw-code.json')
        lock=ROOT/'private/vendor/claw-code/UPSTREAM.lock.json'
    elif args.project == 'openclaw-rs':
        lock=ROOT/'private/vendor/openclaw-rs/UPSTREAM.lock.json'
        cfg={'name':'openclaw-rs','repository':'https://github.com/neul-labs/openclaw-rs','vendor_path':'private/vendor/openclaw-rs/upstream'}
    elif args.project == 'rustclaw':
        cfg=loadj('config/upstreams/rustclaw.json')
        lock=ROOT/cfg['lock_file']
    else:
        raise SystemExit(f'unknown upstream: {args.project}')
    if args.action=='status':
        out={'config':cfg,'vendored':lock.exists()}
        if lock.exists(): out['lock']=json.loads(lock.read_text(encoding='utf-8'))
        print(json.dumps(out,ensure_ascii=False,indent=2)); return
    script_base={'claw-code':'vendor_claw_code','openclaw-rs':'vendor_openclaw_rs','rustclaw':'vendor_rustclaw'}[args.project]
    script = ROOT/'scripts'/(script_base + ('.ps1' if os.name=='nt' else '.sh'))
    if os.name=='nt':
        cmd=['powershell','-ExecutionPolicy','Bypass','-File',str(script)]
        if args.ref: cmd += ['-Ref',args.ref]
        raise SystemExit(subprocess.call(cmd,cwd=ROOT))
    env=dict(os.environ)
    if args.ref: env['PHOENIX_UPSTREAM_REF']=args.ref
    raise SystemExit(subprocess.call([str(script)],cwd=ROOT,env=env))

def cmd_repo(args):
    root=Path(args.path).resolve(); ignored={'.git','target','node_modules','var'}; rows=[]
    for p in root.rglob('*'):
        if not p.is_file() or any(part in ignored for part in p.relative_to(root).parts): continue
        try:b=p.read_bytes(); text=b.decode('utf-8')
        except Exception:continue
        rel=p.relative_to(root).as_posix(); score=min(text.count('\n')+1,5000)+min(len(b)//1024,1000)
        if any(k in rel for k in ('main.','lib.','mod.','Cargo.toml','package.json','README','schema','migration','config')): score+=250
        rows.append((score,rel,len(b),text.count('\n')+1,sha256_bytes(b)))
    rows.sort(key=lambda x:(-x[0],x[1])); print(f'files={len(rows)}')
    for score,rel,bs,lines,digest in rows[:args.limit]: print(f'{score:6d} {lines:6d} {bs:9d} {digest[:12]} {rel}')

def parser():
    p=argparse.ArgumentParser(prog='phx',description='PhxClaw command line - private, evidence-first agent platform')
    sub=p.add_subparsers(dest='cmd',required=True)
    q=sub.add_parser('version',help='show version'); q.set_defaults(fn=cmd_version)
    q=sub.add_parser('status',help='show project/runtime inventory'); q.set_defaults(fn=cmd_status)
    q=sub.add_parser('doctor',help='check required host tools'); q.set_defaults(fn=cmd_doctor)
    q=sub.add_parser('install',help='install/configure PhxClaw and managed PostgreSQL'); q.add_argument('--yes',action='store_true'); q.set_defaults(fn=cmd_install)
    sp=sub.add_parser('core',help='unified PhxClaw Core runtime'); csp=sp.add_subparsers(dest='action',required=True)
    q=csp.add_parser('status'); q.set_defaults(fn=cmd_core)
    q=csp.add_parser('start'); q.set_defaults(fn=cmd_core)
    sp=sub.add_parser('db',help='managed PostgreSQL bootstrap'); dsp=sp.add_subparsers(dest='action',required=True)
    q=dsp.add_parser('plan'); q.set_defaults(fn=cmd_db)
    q=dsp.add_parser('doctor'); q.set_defaults(fn=cmd_db)
    q=dsp.add_parser('install'); q.add_argument('--yes',action='store_true'); q.set_defaults(fn=cmd_db)
    q=sub.add_parser('paths',help='show important project paths'); q.set_defaults(fn=cmd_paths)
    q=sub.add_parser('privacy',help='show private repository policy'); q.set_defaults(fn=cmd_privacy)
    sp=sub.add_parser('sprints',help='list/show sprint roadmap'); ssp=sp.add_subparsers(dest='action',required=True)
    q=ssp.add_parser('list'); q.add_argument('--no-color',action='store_true'); q.set_defaults(fn=cmd_sprints); q=ssp.add_parser('show'); q.add_argument('code'); q.set_defaults(fn=cmd_sprints)
    sp=sub.add_parser('plugins',help='private plugin lifecycle and external bridges'); ssp=sp.add_subparsers(dest='action',required=True)
    q=ssp.add_parser('list'); q.set_defaults(fn=cmd_plugins)
    q=ssp.add_parser('show'); q.add_argument('name'); q.set_defaults(fn=cmd_plugins)
    q=ssp.add_parser('install'); q.add_argument('path'); q.set_defaults(fn=cmd_plugins)
    q=ssp.add_parser('enable'); q.add_argument('name'); q.set_defaults(fn=cmd_plugins)
    q=ssp.add_parser('disable'); q.add_argument('name'); q.set_defaults(fn=cmd_plugins)
    q=ssp.add_parser('doctor'); q.add_argument('name'); q.add_argument('--static-only',action='store_true'); q.set_defaults(fn=cmd_plugins)
    q=ssp.add_parser('rollback'); q.add_argument('name'); q.set_defaults(fn=cmd_plugins)
    q=ssp.add_parser('uninstall'); q.add_argument('name'); q.set_defaults(fn=cmd_plugins)
    sp=sub.add_parser('agents',help='inspect declarative agents'); ssp=sp.add_subparsers(dest='action',required=True)
    q=ssp.add_parser('count'); q.set_defaults(fn=cmd_agents); q=ssp.add_parser('list'); q.add_argument('--limit',type=int,default=0); q.set_defaults(fn=cmd_agents)
    q=sub.add_parser('capabilities',help='list capabilities'); q.add_argument('--filter'); q.set_defaults(fn=cmd_caps)
    sp=sub.add_parser('mission',help='validate/run a mission'); ssp=sp.add_subparsers(dest='action',required=True)
    q=ssp.add_parser('validate'); q.add_argument('file'); q.set_defaults(fn=cmd_mission); q=ssp.add_parser('run'); q.add_argument('file'); q.set_defaults(fn=cmd_mission)
    sp=sub.add_parser('team',help='F20 team runtime'); ssp=sp.add_subparsers(dest='action',required=True)
    q=ssp.add_parser('status'); q.set_defaults(fn=cmd_team)
    q=ssp.add_parser('demo'); q.set_defaults(fn=cmd_team)
    sp=sub.add_parser('secrets',help='F23 encrypted secret broker bootstrap'); ssp=sp.add_subparsers(dest='action',required=True)
    q=ssp.add_parser('init'); q.set_defaults(fn=cmd_secrets)
    q=ssp.add_parser('list'); q.set_defaults(fn=cmd_secrets)
    q=ssp.add_parser('store'); q.add_argument('name'); q.add_argument('--namespace',default='project'); q.add_argument('--scope',action='append',default=[]); q.add_argument('--value'); q.set_defaults(fn=cmd_secrets)
    q=ssp.add_parser('show'); q.add_argument('uuid'); q.set_defaults(fn=cmd_secrets)
    q=ssp.add_parser('doctor'); q.add_argument('uuid'); q.set_defaults(fn=cmd_secrets)
    q=ssp.add_parser('rotate'); q.add_argument('uuid'); q.add_argument('--value'); q.set_defaults(fn=cmd_secrets)
    q=ssp.add_parser('revoke'); q.add_argument('uuid'); q.set_defaults(fn=cmd_secrets)
    q=ssp.add_parser('lease'); q.add_argument('uuid'); q.add_argument('--consumer',required=True); q.add_argument('--scope',required=True); q.add_argument('--ttl',type=int,default=60); q.set_defaults(fn=cmd_secrets)
    sp=sub.add_parser('channels',help='F21 channel gateway and secret-backed providers'); ssp=sp.add_subparsers(dest='action',required=True)
    q=ssp.add_parser('status'); q.set_defaults(fn=cmd_channels)
    q=ssp.add_parser('providers'); q.set_defaults(fn=cmd_channels)
    sp=sub.add_parser('upstream',help='inspect/vendor approved upstream sources'); usp=sp.add_subparsers(dest='project',required=True)
    cp=usp.add_parser('claw-code',help='ultraworkers/claw-code MIT upstream'); csp=cp.add_subparsers(dest='action',required=True)
    q=csp.add_parser('status'); q.set_defaults(fn=cmd_upstream)
    q=csp.add_parser('vendor'); q.add_argument('--ref',default=''); q.set_defaults(fn=cmd_upstream)
    op=usp.add_parser('openclaw-rs',help='Neul Labs openclaw-rs MIT source'); osp=op.add_subparsers(dest='action',required=True)
    q=osp.add_parser('status'); q.set_defaults(fn=cmd_upstream)
    q=osp.add_parser('vendor'); q.add_argument('--ref',default=''); q.set_defaults(fn=cmd_upstream)
    rp=usp.add_parser('rustclaw',help='RustClaw MIT verified upstream/vendor snapshot'); rsp=rp.add_subparsers(dest='action',required=True)
    q=rsp.add_parser('status'); q.set_defaults(fn=cmd_upstream)
    q=rsp.add_parser('vendor'); q.add_argument('--ref',default=''); q.set_defaults(fn=cmd_upstream)
    sp=sub.add_parser('repo',help='deterministic repository inventory'); ssp=sp.add_subparsers(dest='action',required=True)
    q=ssp.add_parser('scan'); q.add_argument('path',nargs='?',default='.'); q.add_argument('--limit',type=int,default=20); q.set_defaults(fn=cmd_repo)
    return p
if __name__=='__main__':
    a=parser().parse_args(); a.fn(a)

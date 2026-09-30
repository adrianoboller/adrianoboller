#!/usr/bin/env python3
from __future__ import annotations
import argparse, hashlib, json, os, platform, secrets, shutil, stat, subprocess, sys, tempfile, urllib.request, zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CFG = json.loads((ROOT/'config/install/postgresql.json').read_text())
RUNTIME_CFG = ROOT/'config/runtime.local.json'

class InstallError(RuntimeError): pass

def sha256(path: Path) -> str:
    h=hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda:f.read(1024*1024), b''): h.update(chunk)
    return h.hexdigest().upper()

def run(cmd, *, env=None, input_text=None, check=True, cwd=None):
    print('+', ' '.join(map(str,cmd)))
    return subprocess.run(list(map(str,cmd)), text=True, input=input_text, env=env, cwd=cwd, check=check)

def detect_host():
    s=platform.system().lower()
    if s=='windows': return 'windows'
    if s=='darwin': return 'macos'
    if s=='linux':
        txt=Path('/etc/os-release').read_text(errors='ignore') if Path('/etc/os-release').exists() else ''
        if any(x in txt.lower() for x in ['debian','ubuntu']): return 'debian'
        if any(x in txt.lower() for x in ['rhel','fedora','rocky','almalinux','centos']): return 'redhat'
        return 'linux'
    return s

def postgres_bins_from_existing():
    names=['psql','pg_ctl','initdb','postgres']
    found={n:shutil.which(n) for n in names}
    return found if found['psql'] and (found['postgres'] or found['pg_ctl']) else None

def windows_bundle_paths():
    runtime=ROOT/CFG['managed_cluster']['runtime_dir']
    return runtime, runtime/'pgsql'/'bin'

def safe_extract_zip(zp: Path, dest: Path):
    dest=dest.resolve(); dest.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(zp) as z:
        for m in z.infolist():
            p=(dest/m.filename).resolve()
            if p!=dest and dest not in p.parents: raise InstallError(f'unsafe zip member: {m.filename}')
        z.extractall(dest)

def download_windows_bundle():
    prod=CFG['production']; runtime,bindir=windows_bundle_paths(); cache=ROOT/'var/cache/postgresql'; cache.mkdir(parents=True,exist_ok=True)
    zp=cache/f"postgresql-{prod['windows_bundle_build']}-windows-x64-binaries.zip"
    if not zp.exists():
        print('Downloading PostgreSQL from EDB...')
        urllib.request.urlretrieve(prod['windows_bundle_url'], zp)
    got=sha256(zp); exp=prod['windows_bundle_sha256'].upper()
    if got!=exp: raise InstallError(f'PostgreSQL SHA-256 mismatch: {got} != {exp}')
    if not (bindir/'initdb.exe').exists(): safe_extract_zip(zp, runtime)
    return bindir

def ensure_native_postgres(host: str, yes: bool):
    existing=postgres_bins_from_existing()
    if existing: return Path(existing['psql']).parent
    major=CFG['production']['major']
    if host=='windows': return download_windows_bundle()
    if not yes: raise InstallError('PostgreSQL is missing. Re-run with --yes to allow package installation.')
    if host=='debian':
        run(['sudo','apt','install','-y','postgresql-common'])
        run(['sudo','/usr/share/postgresql-common/pgdg/apt.postgresql.org.sh'])
        run(['sudo','apt','update'])
        run(['sudo','apt','install','-y',f'postgresql-{major}',f'postgresql-client-{major}'])
    elif host=='macos':
        if not shutil.which('brew'): raise InstallError('Homebrew is required for automatic macOS PostgreSQL installation')
        run(['brew','install',CFG['production']['macos_formula']]); run(['brew','services','start',CFG['production']['macos_formula']])
    elif host=='redhat':
        raise InstallError('Automatic PGDG repository bootstrap for Red Hat is intentionally not guessed. Use the generated plan from `phx db plan redhat`, then rerun the installer.')
    else:
        raise InstallError(f'unsupported automatic PostgreSQL install host: {host}')
    existing=postgres_bins_from_existing()
    if not existing: raise InstallError('PostgreSQL installation completed but binaries were not found on PATH')
    return Path(existing['psql']).parent

def write_secure(path: Path, data: str):
    path.parent.mkdir(parents=True, exist_ok=True); path.write_text(data)
    try: path.chmod(stat.S_IRUSR|stat.S_IWUSR)
    except Exception: pass

def provision_managed_windows(bindir: Path):
    cfg=CFG['managed_cluster']; runtime=ROOT/cfg['runtime_dir']; data=ROOT/cfg['data_dir']; logs=ROOT/cfg['log_dir']; logs.mkdir(parents=True,exist_ok=True)
    initdb=bindir/'initdb.exe'; pg_ctl=bindir/'pg_ctl.exe'; psql=bindir/'psql.exe'
    superpass=secrets.token_urlsafe(36); apppass=secrets.token_urlsafe(36)
    if not (data/'PG_VERSION').exists():
        pw=runtime/'initdb.pw'; write_secure(pw, superpass+'\n')
        try: run([initdb,'-D',data,'-U','postgres',f"--auth={cfg['auth']}",'--encoding=UTF8',f'--pwfile={pw}'])
        finally: pw.unlink(missing_ok=True)
        with (data/'postgresql.conf').open('a',encoding='utf-8') as f:
            f.write(f"\n# PhxClaw managed settings\nlisten_addresses = '127.0.0.1,::1'\nport = {cfg['port']}\npassword_encryption = 'scram-sha-256'\n")
    run([pg_ctl,'-D',data,'-l',logs/'postgres.log','start'])
    env=os.environ.copy(); env['PGPASSWORD']=superpass
    owner=cfg['owner_role']; app=cfg['application_role']; db=cfg['database']
    sql=runtime/'provision.sql'
    esc=apppass.replace("'","''")
    sql_text=f"""DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='{owner}') THEN CREATE ROLE {owner} NOLOGIN; END IF; END $$;\nDO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='{app}') THEN CREATE ROLE {app} LOGIN PASSWORD '{esc}'; ELSE ALTER ROLE {app} LOGIN PASSWORD '{esc}'; END IF; END $$;\nSELECT 'CREATE DATABASE {db} OWNER {owner}' WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname='{db}')\\gexec\nGRANT {owner} TO postgres;\nGRANT CONNECT ON DATABASE {db} TO {app};\n"""
    write_secure(sql, sql_text)
    try: run([psql,'-h','127.0.0.1','-p',str(cfg['port']),'-U','postgres','-d','postgres','-v','ON_ERROR_STOP=1','-f',sql],env=env)
    finally: sql.unlink(missing_ok=True)
    return superpass, apppass, psql

def store_secret(name: str, value: str, scopes: list[str]) -> str:
    cmd=[sys.executable,str(ROOT/'scripts/phx_cli.py'),'secrets','store',name,'--namespace','database']
    for s in scopes: cmd += ['--scope',s]
    p=subprocess.run(cmd,cwd=ROOT,text=True,input=value+'\n',capture_output=True)
    if p.returncode: raise InstallError('Secret Broker bootstrap failed: '+(p.stderr or p.stdout))
    parts=p.stdout.strip().split()
    if len(parts)<2 or parts[0] != 'STORED': raise InstallError('Unexpected Secret Broker response')
    return parts[1]

def apply_migrations(psql: Path, superpass: str):
    cfg=CFG['managed_cluster']; env=os.environ.copy(); env['PGPASSWORD']=superpass
    for m in sorted((ROOT/'migrations').glob('*.sql')):
        run([psql,'-h','127.0.0.1','-p',str(cfg['port']),'-U','postgres','-d',cfg['database'],'-v','ON_ERROR_STOP=1','-f',m],env=env)

def write_runtime_config(secret_uuid: str):
    cfg=CFG['managed_cluster']
    data={'version':'0.20.0','database':{'driver':'postgresql','host':'127.0.0.1','port':cfg['port'],'database':cfg['database'],'user':cfg['application_role'],'password_secret_uuid':secret_uuid,'sslmode':'disable'},'core':{'repository_visibility':'private_only'}}
    write_secure(RUNTIME_CFG,json.dumps(data,indent=2)+'\n')

def install(args):
    host=detect_host(); print('host=',host)
    bindir=ensure_native_postgres(host,args.yes)
    if host!='windows':
        print('PostgreSQL installed/detected. Provisioning of system-managed clusters is intentionally delegated to `phx db provision` to avoid guessing distro service ownership.')
        return
    superpass,apppass,psql=provision_managed_windows(bindir)
    try:
        secret_uuid=store_secret('postgresql-app-password',apppass,['database:connect','database:migrate'])
        write_runtime_config(secret_uuid)
        apply_migrations(psql,superpass)
    finally:
        superpass=''; apppass=''
    print('PhxClaw PostgreSQL bootstrap: OK')
    print('runtime_config=',RUNTIME_CFG)

def plan():
    host=detect_host(); prod=CFG['production']; c=CFG['managed_cluster']
    print(json.dumps({'host':host,'postgresql':prod,'managed_cluster':c,'existing':bool(postgres_bins_from_existing()),'policy':CFG['policy']},indent=2))

def doctor():
    bins=postgres_bins_from_existing(); managed_bin=windows_bundle_paths()[1] if detect_host()=='windows' else None
    if not bins and managed_bin and (managed_bin/'psql.exe').exists(): bins={'psql':str(managed_bin/'psql.exe'),'postgres':str(managed_bin/'postgres.exe'),'pg_ctl':str(managed_bin/'pg_ctl.exe'),'initdb':str(managed_bin/'initdb.exe')}
    print(json.dumps({'postgres_binaries':bins,'runtime_config_exists':RUNTIME_CFG.exists(),'migrations':len(list((ROOT/'migrations').glob('*.sql')))},indent=2))
    return 0 if bins else 1

def main():
    ap=argparse.ArgumentParser(description='PhxClaw unified installer/bootstrap')
    sp=ap.add_subparsers(dest='cmd',required=True)
    sp.add_parser('plan'); ins=sp.add_parser('install'); ins.add_argument('--yes',action='store_true'); sp.add_parser('doctor')
    args=ap.parse_args()
    try:
        if args.cmd=='plan': plan(); return 0
        if args.cmd=='install': install(args); return 0
        return doctor()
    except InstallError as e:
        print('ERROR:',e,file=sys.stderr); return 2

if __name__=='__main__': raise SystemExit(main())

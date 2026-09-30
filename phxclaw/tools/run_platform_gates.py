#!/usr/bin/env python3
from __future__ import annotations
from pathlib import Path
import argparse, hashlib, json, os, platform, subprocess, sys
from packaging.version import Version

def sha_file(p:Path):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for c in iter(lambda:f.read(1024*1024),b''): h.update(c)
    return h.hexdigest()
def run_log(cmd,log,cwd,shell=True,timeout=3600):
    r=subprocess.run(cmd,cwd=cwd,shell=shell,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=timeout)
    log.write_text((r.stdout or '')+f'\nexit_code={r.returncode}\n',encoding='utf-8'); return r.returncode==0
def env_gate(name,log,root):
    cmd=os.environ.get(name,'').strip()
    if not cmd:
        log.write_text(name+' missing\n',encoding='utf-8'); return False
    return run_log(cmd,log,root,True)
def normalize_arch(x):
    x=x.lower(); return {'amd64':'x86_64','x86_64':'x86_64','arm64':'aarch64','aarch64':'aarch64'}.get(x,x)
def normalize_platform():
    x=platform.system().lower(); return {'linux':'linux','windows':'windows','darwin':'macos'}.get(x,x)
def current_source(root):
    r=subprocess.run([sys.executable,str(root/'tools/source_state.py'),str(root)],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=300)
    if r.returncode: raise SystemExit('source-state calculation failed: '+r.stderr[:800])
    return r.stdout.strip().splitlines()[-1].lower()
def verify_rc(root,rc):
    r=subprocess.run([sys.executable,str(root/'tools/verify_release_candidate.py'),str(rc),'--trust-store',str(root/'config/trusted-release-signers.v025.json')],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=300)
    if r.returncode: raise SystemExit('RC verification failed: '+(r.stderr or r.stdout)[:1000])
    return json.loads(r.stdout)
def windows_signature(artifact,log):
    thumbs={x.strip().upper().replace(' ','') for x in os.environ.get('PHXCLAW_WINDOWS_SIGNER_THUMBPRINTS','').split(',') if x.strip()}
    if not thumbs: log.write_text('PHXCLAW_WINDOWS_SIGNER_THUMBPRINTS missing\n'); return False
    escaped=str(artifact).replace("'","''")
    ps=f"$s=Get-AuthenticodeSignature -LiteralPath '{escaped}'; [pscustomobject]@{{Status=$s.Status.ToString();Thumbprint=$s.SignerCertificate.Thumbprint;TimeStamperThumbprint=if($s.TimeStamperCertificate){{$s.TimeStamperCertificate.Thumbprint}}else{{$null}}}} | ConvertTo-Json -Compress"
    r=subprocess.run(['powershell','-NoProfile','-NonInteractive','-Command',ps],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=120)
    text=r.stdout or ''
    try:
        o=json.loads(text); valid=(r.returncode==0 and o.get('Status')=='Valid' and str(o.get('Thumbprint','')).upper().replace(' ','') in thumbs and bool(o.get('TimeStamperThumbprint')))
    except Exception: valid=False
    # OS-native chain verification in addition to PowerShell metadata.
    s=subprocess.run('signtool verify /pa /all /v "'+str(artifact)+'"',shell=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=120)
    log.write_text(text+'\n--- signtool ---\n'+(s.stdout or '')+f'\nsigntool_exit={s.returncode}\n',encoding='utf-8')
    return valid and s.returncode==0
def mac_signature(artifact,signlog,notarylog):
    teams={x.strip() for x in os.environ.get('PHXCLAW_MACOS_TEAM_IDS','').split(',') if x.strip()}
    if not teams:
        signlog.write_text('PHXCLAW_MACOS_TEAM_IDS missing\n',encoding='utf-8')
        notarylog.write_text('not checked\n',encoding='utf-8')
        return False,False
    if artifact.suffix.lower()!='.dmg':
        signlog.write_text('macOS GA artifact must be a signed/notarized DMG\n',encoding='utf-8')
        notarylog.write_text('not checked: invalid distribution format\n',encoding='utf-8')
        return False,False
    r1=subprocess.run(['codesign','--verify','--deep','--strict','--verbose=4',str(artifact)],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=120)
    r2=subprocess.run(['codesign','-dv','--verbose=4',str(artifact)],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=120)
    text=(r1.stdout or '')+'\n'+(r2.stdout or '')
    team=None
    for line in text.splitlines():
        if line.startswith('TeamIdentifier='): team=line.split('=',1)[1].strip()
    signed=r1.returncode==0 and team in teams
    signlog.write_text(text+f'\nteam_id={team or ""}\nallowed_team={team in teams if team else False}\n',encoding='utf-8')
    r3=subprocess.run(['spctl','--assess','--type','open','--context','context:primary-signature','--verbose=4',str(artifact)],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=120)
    r4=subprocess.run(['xcrun','stapler','validate','-v',str(artifact)],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=120)
    notarylog.write_text((r3.stdout or '')+'\n'+(r4.stdout or '')+f'\nspctl_exit={r3.returncode}\nstapler_exit={r4.returncode}\n',encoding='utf-8')
    return signed, r3.returncode==0 and r4.returncode==0

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--artifact',type=Path,required=True); ap.add_argument('--rc',type=Path,required=True); ap.add_argument('--candidate-version',required=True); ap.add_argument('--platform',choices=['linux','windows','macos'],required=True); ap.add_argument('--architecture',choices=['x86_64','aarch64'],required=True); ap.add_argument('--evidence-dir',type=Path,required=True); ap.add_argument('--first-release',action='store_true'); ap.add_argument('--previous-version'); args=ap.parse_args()
    root=args.root.resolve(); art=args.artifact.resolve(); rc=args.rc.resolve(); ed=args.evidence_dir.resolve(); ed.mkdir(parents=True,exist_ok=True)
    if art.is_symlink() or not art.is_file(): raise SystemExit('artifact must be regular non-symlink')
    if rc.is_symlink() or not rc.is_file(): raise SystemExit('RC must be regular non-symlink')
    host_platform=normalize_platform(); host_arch=normalize_arch(platform.machine())
    if host_platform!=args.platform or host_arch!=args.architecture: raise SystemExit(f'native runner mismatch: host={host_platform}/{host_arch} requested={args.platform}/{args.architecture}')
    rr=verify_rc(root,rc); source=current_source(root)
    if rr.get('candidate_version')!=args.candidate_version or rr.get('source_state_sha256')!=source: raise SystemExit('native checkout/RC identity mismatch')
    if args.first_release:
        if args.previous_version: raise SystemExit('first release must not declare previous version')
    else:
        if not args.previous_version: raise SystemExit('--previous-version required unless --first-release')
        if Version(args.previous_version)>=Version(args.candidate_version): raise SystemExit('previous version must be lower than candidate')
    checks={}
    checks['native_tests']='verified' if run_log('cargo test --locked --workspace',ed/'native-tests.log',root,True) else 'failed'
    checks['fresh_install']='verified' if env_gate('PHXCLAW_FRESH_INSTALL_CMD',ed/'fresh-install.log',root) else 'failed'
    if args.first_release:
        (ed/'upgrade-n-minus-1.log').write_text('not_applicable: first release\n'); (ed/'rollback-or-restore.log').write_text('not_applicable: first release\n')
        checks['upgrade_n_minus_1']=checks['rollback_or_restore']='not_applicable'
    else:
        checks['upgrade_n_minus_1']='verified' if env_gate('PHXCLAW_UPGRADE_N_MINUS_1_CMD',ed/'upgrade-n-minus-1.log',root) else 'failed'
        checks['rollback_or_restore']='verified' if env_gate('PHXCLAW_ROLLBACK_RESTORE_CMD',ed/'rollback-or-restore.log',root) else 'failed'
    if args.platform=='windows':
        checks['code_signing']='verified' if windows_signature(art,ed/'code-signing.log') else 'failed'; (ed/'notarization.log').write_text('not_applicable: Windows\n'); checks['notarization']='not_applicable'
    elif args.platform=='macos':
        signed,notary=mac_signature(art,ed/'code-signing.log',ed/'notarization.log'); checks['code_signing']='verified' if signed else 'failed'; checks['notarization']='verified' if notary else 'failed'
    else:
        (ed/'code-signing.log').write_text('not_applicable: Linux artifact covered by PhxClaw release signature\n'); (ed/'notarization.log').write_text('not_applicable: Linux\n'); checks['code_signing']=checks['notarization']='not_applicable'
    logs={}
    for name in ['native-tests.log','fresh-install.log','upgrade-n-minus-1.log','rollback-or-restore.log','code-signing.log','notarization.log']:
        p=ed/name
        if not p.is_file() or p.stat().st_size==0: raise SystemExit('required evidence log missing: '+name)
        logs[name]={'sha256':sha_file(p),'size':p.stat().st_size}
    status={'origin':'phxclaw-native-platform-runner-v027','platform':args.platform,'architecture':args.architecture,'candidate_version':args.candidate_version,'source_state_sha256':source,'rc_archive_sha256':sha_file(rc),'artifact_name':art.name,'artifact_sha256':sha_file(art),'artifact_size':art.stat().st_size,'first_release':bool(args.first_release),'previous_version':args.previous_version,'checks':checks,'logs':logs}
    (ed/'status.json').write_text(json.dumps(status,indent=2)+'\n',encoding='utf-8')
    ok=checks['native_tests']=='verified' and checks['fresh_install']=='verified' and (args.first_release or (checks['upgrade_n_minus_1']=='verified' and checks['rollback_or_restore']=='verified')) and (args.platform!='windows' or checks['code_signing']=='verified') and (args.platform!='macos' or (checks['code_signing']=='verified' and checks['notarization']=='verified'))
    print(json.dumps({'status':'verified' if ok else 'failed','platform':args.platform,'architecture':args.architecture,'source_state_sha256':source,'rc_archive_sha256':status['rc_archive_sha256'],'checks':checks},indent=2)); return 0 if ok else 3
if __name__=='__main__': raise SystemExit(main())

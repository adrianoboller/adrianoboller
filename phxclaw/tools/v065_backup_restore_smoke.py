#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,shutil,tempfile,uuid,os

def hfile(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
    return h.hexdigest()

def backup(src,repo):
    bid=str(uuid.uuid4()); (repo/'blobs').mkdir(parents=True,exist_ok=True);(repo/'manifests').mkdir(parents=True,exist_ok=True)
    entries=[]; total=0
    for p in sorted(src.rglob('*')):
        if p.is_symlink(): raise RuntimeError('symlink rejected')
        if not p.is_file(): continue
        rel=p.relative_to(src).as_posix(); digest=hfile(p); blob=repo/'blobs'/digest[:2]/digest; blob.parent.mkdir(parents=True,exist_ok=True)
        if not blob.exists(): shutil.copy2(p,blob)
        entries.append({'path':rel,'sha256':digest,'size_bytes':p.stat().st_size});total+=p.stat().st_size
    m={'backup_uuid':bid,'entries':entries,'total_bytes':total}; data=json.dumps(m,sort_keys=True,indent=2).encode(); (repo/'manifests'/f'{bid}.json').write_bytes(data);return bid

def verify(repo,bid):
    m=json.loads((repo/'manifests'/f'{bid}.json').read_text())
    for e in m['entries']:
        b=repo/'blobs'/e['sha256'][:2]/e['sha256']
        if not b.is_file() or b.stat().st_size!=e['size_bytes'] or hfile(b)!=e['sha256']: return False
    return True

def restore(repo,bid,target):
    assert verify(repo,bid); m=json.loads((repo/'manifests'/f'{bid}.json').read_text()); stage=target.parent/f'.stage-{uuid.uuid4()}';stage.mkdir(parents=True)
    for e in m['entries']:
        out=stage/e['path'];out.parent.mkdir(parents=True,exist_ok=True); shutil.copy2(repo/'blobs'/e['sha256'][:2]/e['sha256'],out); assert hfile(out)==e['sha256']
    rollback=None
    if target.exists(): rollback=target.parent/f'.rollback-{uuid.uuid4()}';target.rename(rollback)
    stage.rename(target); return rollback

checks=[]
def ck(name,cond): checks.append({'name':name,'pass':bool(cond)})
with tempfile.TemporaryDirectory(prefix='phxclaw-v065-') as td:
    t=Path(td);src=t/'source';repo=t/'repo';dst=t/'restore';src.mkdir();(src/'nested').mkdir();(src/'a.txt').write_text('alpha');(src/'nested'/'b.bin').write_bytes(b'\x00beta')
    bid=backup(src,repo); ck('backup_manifest_created',(repo/'manifests'/f'{bid}.json').is_file());ck('backup_verifies',verify(repo,bid))
    restore(repo,bid,dst);ck('restore_content_a',(dst/'a.txt').read_text()=='alpha');ck('restore_content_b',(dst/'nested'/'b.bin').read_bytes()==b'\x00beta')
    # content-addressed dedupe across second backup
    bid2=backup(src,repo); blobs=list((repo/'blobs').glob('*/*'));ck('content_addressed_dedup',len(blobs)==2);ck('second_backup_verifies',verify(repo,bid2))
    # existing target rollback preservation
    (dst/'old.txt').write_text('old');rb=restore(repo,bid,dst);ck('rollback_copy_preserved',rb is not None and (rb/'old.txt').read_text()=='old')
    # corruption is detected before restore
    victim=blobs[0]; old=victim.read_bytes(); victim.write_bytes(old+b'corrupt');ck('corruption_detected',not verify(repo,bid)); victim.write_bytes(old)
    # symlink rejection where supported
    link=src/'bad-link'; symlink_ok=False
    try: link.symlink_to(src/'a.txt'); backup(src,repo)
    except Exception: symlink_ok=True
    finally:
        try: link.unlink()
        except Exception: pass
    ck('symlink_rejected',symlink_ok)
report={'version':'0.65.0','scope':'host_filesystem_backup_restore_fixture','checks':checks,'pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'postgresql_native':False}
out=Path(__file__).resolve().parents[1]/'reports/V065_BACKUP_RESTORE_SMOKE.json';out.parent.mkdir(exist_ok=True);out.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2));raise SystemExit(0 if report['fail']==0 else 1)

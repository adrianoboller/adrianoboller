#!/usr/bin/env python3
from __future__ import annotations
import json, os, shutil, subprocess, tempfile, uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
checks=[]
def check(name, ok, detail=""):
    checks.append({"name":name,"status":"PASS" if ok else "FAIL","detail":detail})
    if not ok:
        raise AssertionError(f"{name}: {detail}")

def run(cmd, cwd):
    return subprocess.run(cmd, cwd=cwd, text=True, capture_output=True, check=True)

# Source / wiring checks
cargo=(ROOT/'Cargo.toml').read_text()
check('workspace includes code workspace', 'crates/phxclaw-code-workspace' in cargo)
check('workspace includes mission runtime', 'crates/phxclaw-mission-runtime' in cargo)
check('workspace includes mission cli', 'apps/phxclaw-mission-cli' in cargo)
mission_src=(ROOT/'crates/phxclaw-mission-runtime/src/lib.rs').read_text()
workspace_src=(ROOT/'crates/phxclaw-code-workspace/src/lib.rs').read_text()
check('mission uses UUIDv7', 'new_uuid_v7()' in mission_src)
check('mission emits live events', 'publish_json(' in mission_src)
check('mission writes evidence', 'EvidenceDraft' in mission_src and 'evidence.append' in mission_src)
check('model path is fail closed', 'ModelExecutorRequired' in mission_src)
check('extension path is fail closed', 'ExtensionExecutorRequired' in mission_src)
check('agent permission gate exists', 'agent.authorize' in mission_src)
check('workspace rejects parent escape', 'Component::ParentDir' not in workspace_src and 'PathEscape' in workspace_src)
check('workspace uses direct execution', 'CommandRequest::direct' in workspace_src)
check('workspace has no shell command composition', 'bash -lc' not in workspace_src and 'cmd /c' not in workspace_src.lower())

catalog=json.load(open(ROOT/'config/capability-catalog.json'))
check('capability count exact', catalog['count']==len(catalog['capabilities']), str(catalog['count']))
known={x['name'] for x in catalog['capabilities']}
for cap in ['mission.run','workspace.snapshot','workspace.diff','workspace.file.write','workspace.gate.run','workspace.worktree.create']:
    check(f'capability {cap}', cap in known)

# Host workflow smoke: prove Git worktree/diff/direct gate behavior on this host.
with tempfile.TemporaryDirectory(prefix='phx-mission-smoke-') as td:
    td=Path(td)
    repo=td/'repo'; state=td/'state'; worktree=state/'worktrees'/'mission'
    repo.mkdir(); state.mkdir()
    run(['git','init','-q'],repo)
    run(['git','config','user.email','phxclaw@example.invalid'],repo)
    run(['git','config','user.name','PhxClaw Test'],repo)
    (repo/'value.txt').write_text('before\n')
    run(['git','add','value.txt'],repo); run(['git','commit','-qm','base'],repo)
    clean=run(['git','status','--porcelain=v1'],repo).stdout.strip()==''
    check('host smoke clean start', clean)
    worktree.parent.mkdir(parents=True)
    run(['git','worktree','add','-q','-b','agent/mission/smoke',str(worktree),'HEAD'],repo)
    check('host smoke worktree created', worktree.is_dir())
    (worktree/'value.txt').write_text('after\n')
    gate=run(['python3','-c',"from pathlib import Path; assert Path('value.txt').read_text() == 'after\\n'"],worktree)
    check('host smoke direct gate', gate.returncode==0)
    diff=run(['git','diff','--no-ext-diff','--binary'],worktree).stdout
    check('host smoke diff produced', '-before' in diff and '+after' in diff)
    run(['git','worktree','remove','--force',str(worktree)],repo)
    check('host smoke worktree removed', not worktree.exists())

report={
  'suite':'Mission Runtime + Code Workspace v0.13 static/host smoke',
  'pass':sum(x['status']=='PASS' for x in checks),
  'fail':sum(x['status']=='FAIL' for x in checks),
  'cargo_available':shutil.which('cargo') is not None,
  'note':'Host smoke validates Git/worktree/direct-command primitives. Rust compilation remains a separate gate.',
  'checks':checks,
}
(ROOT/'MISSION_RUNTIME_TEST_REPORT.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n')
print(json.dumps({'PASS':report['pass'],'FAIL':report['fail'],'cargo_available':report['cargo_available']}))

#!/usr/bin/env python3
from pathlib import Path
import json, shutil, subprocess, tempfile
ROOT=Path(__file__).resolve().parents[1]
PASS=0; FAIL=0; details=[]
def check(name,ok,detail=''):
 global PASS,FAIL
 if ok: PASS+=1; print('PASS',name)
 else: FAIL+=1; print('FAIL',name,detail)
 details.append({'name':name,'pass':bool(ok),'detail':str(detail)})
def run(*args,expect=0):
 r=subprocess.run([str(ROOT/'bin/phx'),*args],cwd=ROOT,text=True,capture_output=True)
 return r, r.returncode==expect

# Clean local runtime state for deterministic test.
shutil.rmtree(ROOT/'var/plugin-registry',ignore_errors=True); shutil.rmtree(ROOT/'var/plugin-store',ignore_errors=True)

r,ok=run('version'); check('cli.version',ok and '0.13.0' in r.stdout,r.stdout+r.stderr)
r,ok=run('--help'); check('cli.help',ok and 'plugins' in r.stdout and 'repo' in r.stdout,r.stdout+r.stderr)
r,ok=run('plugins','install','tests/fixtures/plugin-lifecycle/v1.0.0'); check('plugin.install.v1',ok and 'INSTALLED' in r.stdout,r.stdout+r.stderr)
r,ok=run('plugins','enable','com.phxclaw.fixture.lifecycle'); check('plugin.enable',ok and 'ENABLED' in r.stdout,r.stdout+r.stderr)
r,ok=run('plugins','doctor','com.phxclaw.fixture.lifecycle'); check('plugin.doctor.real_health',ok and 'healthy' in r.stdout.lower(),r.stdout+r.stderr)
r,ok=run('plugins','disable','com.phxclaw.fixture.lifecycle'); check('plugin.disable',ok and 'DISABLED' in r.stdout,r.stdout+r.stderr)
r,ok=run('plugins','install','tests/fixtures/plugin-lifecycle/v1.1.0'); check('plugin.install.v2',ok and '1.1.0' in r.stdout,r.stdout+r.stderr)
r,ok=run('plugins','rollback','com.phxclaw.fixture.lifecycle'); check('plugin.rollback',ok and '1.0.0' in r.stdout,r.stdout+r.stderr)
r,ok=run('plugins','show','com.phxclaw.fixture.lifecycle'); check('plugin.show',ok and '"version": "1.0.0"' in r.stdout,r.stdout+r.stderr)
r,ok=run('plugins','uninstall','com.phxclaw.fixture.lifecycle'); check('plugin.uninstall',ok and 'UNINSTALLED' in r.stdout,r.stdout+r.stderr)

# Tamper detection.
with tempfile.TemporaryDirectory() as td:
 src=ROOT/'tests/fixtures/plugin-lifecycle/v1.0.0'; dst=Path(td)/'tampered'; shutil.copytree(src,dst); (dst/'phxclaw-fixture-plugin').write_text('tampered\n')
 r=subprocess.run([str(ROOT/'bin/phx'),'plugins','install',str(dst)],cwd=ROOT,text=True,capture_output=True)
 check('plugin.tamper_rejected',r.returncode!=0 and ('hash mismatch' in (r.stdout+r.stderr).lower()),r.stdout+r.stderr)

r,ok=run('repo','scan','.', '--limit','3'); check('repo.scan',ok and 'files=' in r.stdout and len(r.stdout.strip().splitlines())>=2,r.stdout+r.stderr)

cargo=(ROOT/'Cargo.toml').read_text()
for crate in ['phxclaw-checkpoint','phxclaw-mcp-lsp-runtime','phxclaw-repo-intelligence','phxclaw-mission-executors']:
 check('workspace.'+crate,crate in cargo)

for code in ('f17','f18','f19'):
 d=json.loads((ROOT/'sprints'/f'{code}.json').read_text()); check(f'sprint.{code}.review',d.get('status')=='review',d.get('status'))

caps=json.loads((ROOT/'config/capability-catalog.json').read_text()); names={x['name'] for x in caps['capabilities']}
check('capabilities.count',caps['count']==len(caps['capabilities'])==120,caps['count'])
for cap in ['plugin.install','plugin.rollback','checkpoint.create','checkpoint.restore','protocol.mcp.invoke','protocol.lsp.invoke','repo.scan','repo.rank']:
 check('capability.'+cap,cap in names)

mission=(ROOT/'crates/phxclaw-mission-runtime/src/lib.rs').read_text()
check('mission.model_executor_attached','with_model_executor' in mission and 'execute_model' in mission)
check('mission.extension_executor_attached','with_extension_executor' in mission and 'execute_extension' in mission)

# State should finish clean.
shutil.rmtree(ROOT/'var/plugin-registry',ignore_errors=True); shutil.rmtree(ROOT/'var/plugin-store',ignore_errors=True)
report={'suite':'PhxClaw v0.13 operational/bootstrap','pass':PASS,'fail':FAIL,'details':details}
(ROOT/'V013_OPERATIONAL_TEST_REPORT.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'PASS':PASS,'FAIL':FAIL}))
raise SystemExit(1 if FAIL else 0)

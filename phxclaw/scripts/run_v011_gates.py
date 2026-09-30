#!/usr/bin/env python3
from __future__ import annotations
import datetime as dt, json, shutil, subprocess, sys, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
SUITES=[
 ('mission_runtime','test_mission_runtime_static.py',30),
 ('research_pipeline','test_research_pipeline_static.py',30),
 ('f15_f16','test_f15_f16_static.py',30),
 ('desktop_host','test_desktop_host_static.py',30),
 ('plugin_ecosystem','test_plugin_ecosystem_static.py',30),
 ('olmocr_adapter','test_olmocr_plugin.py',45),
 ('process_smoke','test_v09_smoke.py',45),
 ('bootstrap_verifier','verify_bootstrap.py',45),
]
results=[]
for name,script,timeout in SUITES:
    started=time.monotonic()
    try:
        p=subprocess.run([sys.executable,str(ROOT/'scripts'/script)],cwd=ROOT,capture_output=True,text=True,timeout=timeout)
        status='PASS' if p.returncode==0 else 'FAIL'
        detail=(p.stdout+p.stderr)[-4000:]
    except subprocess.TimeoutExpired as exc:
        status='FAIL'; detail=f'timeout after {timeout}s: {(exc.stdout or "")[-1000:]}'
    results.append({'suite':name,'status':status,'elapsed_ms':round((time.monotonic()-started)*1000),'detail':detail})
    print(f'{name}: {status}')
report={
 'version':'0.11.0',
 'generated_at':dt.datetime.now(dt.timezone.utc).isoformat(),
 'cargo_available':shutil.which('cargo') is not None,
 'rustc_available':shutil.which('rustc') is not None,
 'pass':sum(r['status']=='PASS' for r in results),
 'fail':sum(r['status']=='FAIL' for r in results),
 'results':results,
 'note':'cargo/rustc availability is reported separately; static/host gates do not substitute Rust compilation.'
}
(ROOT/'V011_GATE_REPORT.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n')
print(json.dumps({k:report[k] for k in ('pass','fail','cargo_available','rustc_available')}))
raise SystemExit(1 if report['fail'] else 0)

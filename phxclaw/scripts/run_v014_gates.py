#!/usr/bin/env python3
from pathlib import Path
import json, subprocess, sys, time
ROOT=Path(__file__).resolve().parents[1]
SUITES=[
 ('desktop','test_desktop_host_static.py',60),('f15_f16','test_f15_f16_static.py',60),('research','test_research_pipeline_static.py',60),('plugins','test_plugin_ecosystem_static.py',60),('olmocr','test_olmocr_plugin.py',60),('mission','test_mission_runtime_static.py',60),('uploaded_plugins_f20','test_v014_plugins_f20.py',90),('bootstrap','verify_bootstrap.py',90)]
results=[]; passed=failed=0
for name,script,timeout in SUITES:
 t=time.time()
 try:
  r=subprocess.run([sys.executable,str(ROOT/'scripts'/script)],cwd=ROOT,text=True,capture_output=True,timeout=timeout)
  ok=r.returncode==0
 except subprocess.TimeoutExpired as e:
  ok=False; r=type('R',(object,),{'returncode':124,'stdout':e.stdout or '','stderr':e.stderr or 'timeout'})()
 results.append({'suite':name,'script':script,'pass':ok,'returncode':r.returncode,'elapsed_ms':round((time.time()-t)*1000),'stdout_tail':(r.stdout or '')[-3000:],'stderr_tail':(r.stderr or '')[-3000:]})
 print(('PASS' if ok else 'FAIL'),name)
 if ok: passed+=1
 else: failed+=1
report={'version':'0.14.0','suite':'PhxClaw v0.14 consolidated','pass':passed,'fail':failed,'results':results}
(ROOT/'V014_GATE_REPORT.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'PASS':passed,'FAIL':failed}))
raise SystemExit(1 if failed else 0)

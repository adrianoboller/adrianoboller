#!/usr/bin/env python3
from pathlib import Path
import json, subprocess, shutil, tempfile, uuid, sys
ROOT=Path(__file__).resolve().parents[1]
PASS=FAIL=0; DETAILS=[]
def check(name,ok,detail=''):
 global PASS,FAIL
 if ok: PASS+=1; print('PASS',name)
 else: FAIL+=1; print('FAIL',name,detail)
 DETAILS.append({'name':name,'pass':bool(ok),'detail':str(detail)})
def env(cap,payload=None): return json.dumps({'protocol':'phxclaw-process-v1','message_uuid':str(uuid.uuid4()),'correlation_uuid':None,'kind':'execute','sent_at':'2026-01-01T00:00:00Z','payload':{'capability':cap,'payload':payload or {}}})+'\n'
def call(plugin,cap,payload=None):
 r=subprocess.run([str(plugin)],input=env(cap,payload),text=True,capture_output=True,cwd=plugin.parent,timeout=30); return r,json.loads(r.stdout.strip().splitlines()[-1]) if r.stdout.strip() else {}
for rel in ['private/plugins-src/phoenix-dashboards/phxclaw-dashboards-plugin','private/plugins-src/phoenix-web-fxsdk/phxclaw-web-fxsdk-plugin']:
 p=ROOT/rel; r=subprocess.run([str(p),'--health'],text=True,capture_output=True); check(rel+'.health',r.returncode==0 and 'healthy' in r.stdout.lower(),r.stdout+r.stderr)
p=ROOT/'private/plugins-src/phoenix-dashboards/phxclaw-dashboards-plugin'; r,j=call(p,'dashboard.catalog.list'); check('dash.catalog',j.get('status')=='ok' and len(j.get('payload',{}).get('charts',[]))>=60,j)
r,j=call(p,'dashboard.render.basic_html',{'tipo':'linha','titulo':'Teste','categorias':['A','B'],'series':[{'nome':'S','valores':[1,2]}],'output_name':'smoke.html'}); out=p.parent/j.get('payload',{}).get('output_path',''); check('dash.render',j.get('status')=='ok' and out.is_file() and out.stat().st_size>100000,j)
p=ROOT/'private/plugins-src/phoenix-web-fxsdk/phxclaw-web-fxsdk-plugin'; r,j=call(p,'fx.catalog.summary'); check('fx.summary',j.get('payload',{}).get('tokens')==56,j)
r,j=call(p,'fx.tokens.emit_css',{'theme_id':'zeus'}); check('fx.css',j.get('status')=='ok' and ':root' in j.get('payload',{}).get('css',''),j)
r,j=call(p,'fx.ui.search',{'query':'datagrid'}); check('fx.ui.search',j.get('status')=='ok' and j.get('payload',{}).get('count',0)>0,j)
# private signed package lifecycle
for pkg,name in [('PhxClaw_Plugin_Phoenix_Dashboards_v0.1.phxplugin','com.phxclaw.private.dashboards'),('PhxClaw_Plugin_Web_Absorber_FXSDK_v0.1.phxplugin','com.phxclaw.private.web-fxsdk')]:
 shutil.rmtree(ROOT/'var/plugin-registry',ignore_errors=True); shutil.rmtree(ROOT/'var/plugin-store',ignore_errors=True)
 r=subprocess.run([str(ROOT/'bin/phx'),'plugins','install',str(Path('/mnt/data')/pkg)],cwd=ROOT,text=True,capture_output=True); check(name+'.install',r.returncode==0 and 'INSTALLED' in r.stdout,r.stdout+r.stderr)
 r=subprocess.run([str(ROOT/'bin/phx'),'plugins','doctor',name],cwd=ROOT,text=True,capture_output=True); check(name+'.doctor',r.returncode==0 and 'HEALTHY' in r.stdout,r.stdout+r.stderr)
 r=subprocess.run([str(ROOT/'bin/phx'),'plugins','enable',name],cwd=ROOT,text=True,capture_output=True); check(name+'.enable',r.returncode==0 and 'ENABLED' in r.stdout,r.stdout+r.stderr)
 r=subprocess.run([str(ROOT/'bin/phx'),'plugins','disable',name],cwd=ROOT,text=True,capture_output=True); check(name+'.disable',r.returncode==0 and 'DISABLED' in r.stdout,r.stdout+r.stderr)
 r=subprocess.run([str(ROOT/'bin/phx'),'plugins','uninstall',name],cwd=ROOT,text=True,capture_output=True); check(name+'.uninstall',r.returncode==0 and 'UNINSTALLED' in r.stdout,r.stdout+r.stderr)
# F20 bootstrap
r=subprocess.run([sys.executable,str(ROOT/'scripts/team_runtime_bootstrap.py'),'demo'],cwd=ROOT,text=True,capture_output=True,timeout=30); check('f20.team.demo',r.returncode==0 and 'fencing_rejects_stale_worker' in r.stdout,r.stdout+r.stderr)
report={'suite':'PhxClaw v0.14 uploaded packages + F20','pass':PASS,'fail':FAIL,'details':DETAILS}; (ROOT/'V014_PLUGIN_F20_TEST_REPORT.json').write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps({'PASS':PASS,'FAIL':FAIL})); raise SystemExit(1 if FAIL else 0)

#!/usr/bin/env python3
from pathlib import Path
import argparse,hashlib,json,os,shutil,subprocess,sys,time,urllib.request
sys.path.insert(0,str(Path(__file__).parent));from source_state_v056 import digest

def evhash(e):
 x={k:v for k,v in e.items() if k!='evidence_sha256'};return hashlib.sha256(json.dumps(x,sort_keys=True,separators=(',',':')).encode()).hexdigest()
def run(cmd,cwd,timeout=3600):
 t=time.time()
 try:p=subprocess.run(cmd,cwd=cwd,text=True,capture_output=True,timeout=timeout);return p.returncode,p.stdout[-8000:],p.stderr[-8000:],round(time.time()-t,3)
 except Exception as e:return 127,'',str(e),round(time.time()-t,3)
def main():
 ap=argparse.ArgumentParser();ap.add_argument('root',type=Path);ap.add_argument('--matrix',type=Path,required=True);ap.add_argument('--out-dir',type=Path,required=True);ap.add_argument('--strict',action='store_true');a=ap.parse_args();root=a.root.resolve();out=a.out_dir.resolve();out.mkdir(parents=True,exist_ok=True)
 source,_=digest(root);matrix=json.loads(a.matrix.read_text());evidence=[];details={}
 def add(gate,result,detail,tool='v0.56'):
  details[gate]=detail
  for s in matrix['sprints']:
   if gate in s['required_gates']:
    e={'sprint_id':s['sprint_id'],'gate_id':gate,'source_state_sha256':source,'result':result,'tool_version':tool,'details':detail};e['evidence_sha256']=evhash(e);evidence.append(e)
 # global cargo gates
 cargo=shutil.which('cargo');rustc=shutil.which('rustc')
 if cargo and rustc and (root/'Cargo.toml').exists():
  if not (root/'Cargo.lock').exists():
   rc,so,se,sec=run(['cargo','generate-lockfile'],root);details['cargo_generate_lock']={'exit_code':rc,'stdout':so,'stderr':se,'seconds':sec}
  cmds={'cargo_fmt':['cargo','fmt','--check'],'cargo_check':['cargo','check','--workspace','--locked'],'cargo_test':['cargo','test','--workspace','--locked'],'cargo_clippy':['cargo','clippy','--workspace','--all-targets','--locked','--','-D','warnings']}
  for g,c in cmds.items():
   rc,so,se,sec=run(c,root);add(g,'pass' if rc==0 else 'fail',{'command':c,'exit_code':rc,'stdout_tail':so,'stderr_tail':se,'seconds':sec},'cargo')
 else:
  for g in ['cargo_fmt','cargo_check','cargo_test','cargo_clippy']:add(g,'unavailable',{'reason':'cargo/rustc/Cargo.toml unavailable'},'host-probe')
 # bootstrap static, workspace metadata
 if cargo and (root/'Cargo.toml').exists():
  rc,so,se,sec=run(['cargo','metadata','--locked','--no-deps','--format-version','1'],root);add('workspace_metadata','pass' if rc==0 else 'fail',{'exit_code':rc,'stderr_tail':se,'seconds':sec},'cargo')
 else:add('workspace_metadata','unavailable',{'reason':'cargo/project unavailable'},'host-probe')
 ver=None
 for q in [root/'scripts/verify_bootstrap.py',root/'tools/verify_bootstrap.py']:
  if q.exists():ver=q;break
 if ver:
  rc,so,se,sec=run([sys.executable,str(ver)],root);add('bootstrap_static','pass' if rc==0 else 'fail',{'script':str(ver),'exit_code':rc,'stdout_tail':so,'stderr_tail':se},'bootstrap')
 else:add('bootstrap_static','unavailable',{'reason':'bootstrap verifier not found'},'host-probe')
 # uuid integrity conservative: use existing verifier only; never auto-pass from regex scanning
 add('uuidv7_integrity','unavailable',{'reason':'requires consolidated-project UUID verifier evidence'},'host-probe')
 # PostgreSQL gates deliberately require disposable test DB and explicit runner; no implicit mutation
 pg=shutil.which('psql');admin=os.getenv('DATABASE_URL');rls=os.getenv('PHXCLAW_RLS_DATABASE_URL')
 for g in ['postgresql_e2e','rls_e2e','plugin_postgresql_e2e']:
  add(g,'unavailable',{'reason':'requires psql + disposable PhxClaw test DB + project E2E runner','psql':bool(pg),'admin_url':bool(admin),'rls_url':bool(rls)},'host-probe')
 # Ollama real loopback probe
 try:
  with urllib.request.urlopen('http://127.0.0.1:11434/api/version',timeout=1.5) as r: resp=json.loads(r.read());add('ollama_real','pass',{'version':resp.get('version')},'ollama-api')
 except Exception as e:add('ollama_real','unavailable',{'reason':str(e)},'ollama-api')
 # every remaining sprint-specific gate stays unavailable until its explicit native E2E produces proof
 already={e['gate_id'] for e in evidence}
 allg={g for s in matrix['sprints'] for g in s['required_gates']}
 for g in sorted(allg-already):add(g,'unavailable',{'reason':'explicit native/E2E evidence not executed on this host'},'v0.56')
 (out/'V056_EVIDENCE.json').write_text(json.dumps(evidence,indent=2)+'\n');(out/'V056_GATE_DETAILS.json').write_text(json.dumps(details,indent=2)+'\n')
 rec=Path(__file__).with_name('sprint_gate_reconcile_v056.py');cmd=[sys.executable,str(rec),str(out/'V056_EVIDENCE.json'),'--matrix',str(a.matrix),'--source-state',source,'--out',str(out/'V056_SPRINT_STATUS.json')];rc,so,se,sec=run(cmd,root)
 print(so,end='');
 status=json.loads((out/'V056_SPRINT_STATUS.json').read_text());final={'version':'0.56.0','source_state_sha256':source,'green':status['green'],'yellow':status['yellow'],'target_green':status['target_green_count'],'target_met':status['target_met'],'release_ready':status['green']==26};(out/'V056_CAMPAIGN_SUMMARY.json').write_text(json.dumps(final,indent=2)+'\n');print(json.dumps(final,indent=2))
 if a.strict and not status['target_met']:raise SystemExit(4)
if __name__=='__main__':main()

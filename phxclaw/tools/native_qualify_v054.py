#!/usr/bin/env python3
from __future__ import annotations
import argparse, json, os, shutil, subprocess, sys, time
from pathlib import Path

def run(cmd,cwd,env=None):
 t=time.time(); p=subprocess.run(cmd,cwd=cwd,env=env,text=True,capture_output=True); return {'command':cmd,'exit_code':p.returncode,'seconds':round(time.time()-t,3),'stdout_tail':p.stdout[-4000:],'stderr_tail':p.stderr[-4000:]}

def main():
 ap=argparse.ArgumentParser(); ap.add_argument('root',type=Path); ap.add_argument('--out',type=Path,required=True); ap.add_argument('--strict',action='store_true'); ap.add_argument('--ollama-chat-model'); ap.add_argument('--ollama-embed-model'); a=ap.parse_args(); root=a.root.resolve()
 tools={x:shutil.which(x) for x in ['cargo','rustc','psql','postgres']}; report={'version':'0.54.0','tools':tools,'gates':[],'release_ready':False}
 def unavailable(name,why): report['gates'].append({'name':name,'status':'UNAVAILABLE','detail':why})
 if tools['cargo'] and tools['rustc']:
  if not (root/'Cargo.lock').exists(): report['gates'].append({'name':'cargo_lock','status':'FAIL','detail':'Cargo.lock missing; run cargo generate-lockfile on trusted Rust host'})
  else:
   for name,cmd in [('cargo_fmt',['cargo','fmt','--check']),('cargo_check',['cargo','check','--workspace','--locked']),('cargo_test',['cargo','test','--workspace','--locked']),('cargo_clippy',['cargo','clippy','--workspace','--all-targets','--locked','--','-D','warnings'])]:
    r=run(cmd,root); report['gates'].append({'name':name,'status':'PASS' if r['exit_code']==0 else 'FAIL','evidence':r})
 else: unavailable('rust_native','cargo/rustc unavailable')
 if tools['psql'] and os.getenv('DATABASE_URL') and os.getenv('PHXCLAW_RLS_DATABASE_URL'):
  report['gates'].append({'name':'postgresql_real','status':'READY_TO_RUN','detail':'Use project migration/E2E runner with admin DATABASE_URL and non-superuser PHXCLAW_RLS_DATABASE_URL'})
 else: unavailable('postgresql_real','psql or required DB URLs unavailable')
 probe=Path(__file__).with_name('ollama_probe_v054.py'); ollama_out=a.out.parent/'V054_OLLAMA_PROBE.json'
 cmd=[sys.executable,str(probe),'--out',str(ollama_out)]
 if a.ollama_chat_model: cmd += ['--chat-model',a.ollama_chat_model]
 if a.ollama_embed_model: cmd += ['--embed-model',a.ollama_embed_model]
 r=run(cmd,root); report['gates'].append({'name':'ollama_real','status':'PASS' if r['exit_code']==0 else 'UNAVAILABLE','evidence':r,'report':str(ollama_out)})
 report['release_ready']=all(g['status']=='PASS' for g in report['gates']) and len(report['gates'])>0
 a.out.parent.mkdir(parents=True,exist_ok=True); a.out.write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps(report,indent=2))
 if a.strict and not report['release_ready']: raise SystemExit(4)
if __name__=='__main__': main()

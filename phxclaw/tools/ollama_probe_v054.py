#!/usr/bin/env python3
from __future__ import annotations
import argparse, json, time, urllib.request, urllib.parse
from pathlib import Path

def req(base,path,body=None,timeout=60):
 url=base.rstrip('/')+path
 data=None if body is None else json.dumps(body).encode()
 r=urllib.request.Request(url,data=data,headers={'Content-Type':'application/json'})
 with urllib.request.urlopen(r,timeout=timeout) as x:return json.loads(x.read().decode())

def main():
 ap=argparse.ArgumentParser(); ap.add_argument('--base',default='http://127.0.0.1:11434'); ap.add_argument('--chat-model'); ap.add_argument('--embed-model'); ap.add_argument('--out',type=Path,required=True); a=ap.parse_args()
 u=urllib.parse.urlparse(a.base)
 if u.hostname not in {'127.0.0.1','localhost','::1'}: raise SystemExit('remote Ollama endpoint refused by v0.54 probe')
 report={'base':a.base,'checks':[],'models':{},'status':'partial'}
 try:
  ver=req(a.base,'/api/version'); report['version']=ver.get('version'); report['checks'].append({'name':'version','pass':True})
  tags=req(a.base,'/api/tags'); names=[m.get('name') or m.get('model') for m in tags.get('models',[])]; report['available_models']=names; report['checks'].append({'name':'tags','pass':True,'count':len(names)})
  ps=req(a.base,'/api/ps'); report['running_models']=ps.get('models',[]); report['checks'].append({'name':'ps','pass':True})
  if a.chat_model:
   schema={'type':'object','required':['category','confidence'],'properties':{'category':{'type':'string'},'confidence':{'type':'number','minimum':0,'maximum':1}}}
   t=time.perf_counter(); chat=req(a.base,'/api/chat',{'model':a.chat_model,'messages':[{'role':'user','content':'Classify this task: fix a failing Rust unit test. Return JSON only.'}],'format':schema,'stream':False,'keep_alive':'5m','options':{'temperature':0}}); elapsed=time.perf_counter()-t
   content=(chat.get('message') or {}).get('content',''); parsed=json.loads(content); ok=isinstance(parsed.get('category'),str) and isinstance(parsed.get('confidence'),(int,float))
   eval_count=chat.get('eval_count'); eval_duration=chat.get('eval_duration'); tps=(eval_count/(eval_duration/1e9)) if eval_count and eval_duration else None
   report['models'][a.chat_model]={'structured_output':ok,'elapsed_seconds':elapsed,'eval_count':eval_count,'eval_duration_ns':eval_duration,'tokens_per_second':tps,'load_duration_ns':chat.get('load_duration')}
   report['checks'].append({'name':'structured_output','pass':ok})
  if a.embed_model:
   emb=req(a.base,'/api/embed',{'model':a.embed_model,'input':['PhxClaw local semantic retrieval'],'keep_alive':'5m'}); vecs=emb.get('embeddings',[]); ok=bool(vecs and isinstance(vecs[0],list) and len(vecs[0])>0); report['models'][a.embed_model]={'embedding_dimensions':len(vecs[0]) if ok else 0}; report['checks'].append({'name':'embedding','pass':ok})
  report['status']='pass' if all(c['pass'] for c in report['checks']) else 'fail'
 except Exception as e:
  report['error']=str(e); report['status']='unavailable_or_fail'
 a.out.parent.mkdir(parents=True,exist_ok=True); a.out.write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps(report,indent=2))
 raise SystemExit(0 if report['status']=='pass' else 3)
if __name__=='__main__': main()

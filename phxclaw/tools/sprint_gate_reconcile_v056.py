#!/usr/bin/env python3
import argparse,json,hashlib
from pathlib import Path
def canon(o):return json.dumps(o,sort_keys=True,separators=(',',':')).encode()
def main():
 ap=argparse.ArgumentParser();ap.add_argument('evidence',type=Path);ap.add_argument('--matrix',type=Path,required=True);ap.add_argument('--source-state',required=True);ap.add_argument('--out',type=Path);a=ap.parse_args()
 evs=json.loads(a.evidence.read_text());mat=json.loads(a.matrix.read_text());out=[]
 for s in mat['sprints']:
  seen={}
  for e in evs:
   if e.get('sprint_id')==s['sprint_id'] and e.get('source_state_sha256')==a.source_state: seen[e['gate_id']]=e['result']
  req=s['required_gates'];missing=[g for g in req if g not in seen];fail=[g for g in req if seen.get(g)=='fail'];un=[g for g in req if seen.get(g)=='unavailable']
  green=all(seen.get(g)=='pass' for g in req) and not missing
  st={'sprint_id':s['sprint_id'],'title':s['title'],'priority_wave':s['priority_wave'],'color':'green' if green else 'yellow','missing_gates':missing,'failing_gates':fail,'unavailable_gates':un};st['evidence_sha256']=hashlib.sha256(canon(st)).hexdigest();out.append(st)
 report={'source_state_sha256':a.source_state,'green':sum(x['color']=='green' for x in out),'yellow':sum(x['color']=='yellow' for x in out),'red':0,'target_green_count':mat['campaign']['target_green_count'],'target_met':sum(x['color']=='green' for x in out)>=mat['campaign']['target_green_count'],'sprints':out}
 txt=json.dumps(report,indent=2);print(txt)
 if a.out:a.out.parent.mkdir(parents=True,exist_ok=True);a.out.write_text(txt+'\n')
if __name__=='__main__':main()

#!/usr/bin/env python3
import argparse,json,shutil,socket,subprocess,urllib.request
from pathlib import Path
def probe(cmd):
 p=shutil.which(cmd);return {'available':bool(p),'path':p}
def main():
 ap=argparse.ArgumentParser();ap.add_argument('--root',type=Path,default=Path('.'));ap.add_argument('--out',type=Path);a=ap.parse_args();root=a.root.resolve()
 tools={x:probe(x) for x in ['cargo','rustc','psql','postgres','git','ffmpeg','node']}
 oll={'available':False,'base':'http://127.0.0.1:11434'}
 try:
  with urllib.request.urlopen(oll['base']+'/api/version',timeout=1.5) as r:oll.update(available=True,response=json.loads(r.read()))
 except Exception as e:oll['detail']=str(e)
 base=(root/'PhoenixClaw_Core_Bootstrap_v0.20.zip').exists()
 report={'version':'0.56.0','root':str(root),'base_v020_present':base,'tools':tools,'ollama':oll,'network_dns':{},'ready_for_native_campaign':bool(base and tools['cargo']['available'] and tools['rustc']['available'] and tools['psql']['available'] and tools['postgres']['available'])}
 for host in ['static.rust-lang.org','apt.postgresql.org','ollama.com']:
  try:report['network_dns'][host]=socket.gethostbyname(host)
  except Exception as e:report['network_dns'][host]='UNAVAILABLE: '+str(e)
 txt=json.dumps(report,indent=2);print(txt)
 if a.out:a.out.parent.mkdir(parents=True,exist_ok=True);a.out.write_text(txt+'\n')
if __name__=='__main__':main()

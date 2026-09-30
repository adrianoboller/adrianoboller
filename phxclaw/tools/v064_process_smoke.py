#!/usr/bin/env python3
from pathlib import Path
import json, subprocess, sys, time
ROOT=Path(__file__).resolve().parents[1]

def rpc_line(proc,msg):
    proc.stdin.write((json.dumps(msg)+'\n').encode());proc.stdin.flush()
    return json.loads(proc.stdout.readline())

def lsp_send(proc,msg):
    b=json.dumps(msg,separators=(',',':')).encode();proc.stdin.write(f'Content-Length: {len(b)}\r\n\r\n'.encode()+b);proc.stdin.flush()
def lsp_read(proc):
    n=None
    while True:
        line=proc.stdout.readline()
        if line in (b'\r\n',b'\n'): break
        if not line: raise RuntimeError('eof')
        k,_,v=line.decode().partition(':')
        if k.lower()=='content-length': n=int(v.strip())
    return json.loads(proc.stdout.read(n))
checks=[]
def ck(name,ok,detail=''):
    checks.append({'name':name,'pass':bool(ok),'detail':detail})
py=sys.executable
m=subprocess.Popen([py,str(ROOT/'tests/v064/mcp_stdio_fixture.py')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
r=rpc_line(m,{'jsonrpc':'2.0','id':'1','method':'initialize','params':{}});ck('mcp_real_process_initialize',r.get('result',{}).get('protocolVersion')=='2025-11-25')
r=rpc_line(m,{'jsonrpc':'2.0','id':'2','method':'tools/list','params':{}});ck('mcp_real_process_tools',r.get('result',{}).get('tools',[{}])[0].get('name')=='echo')
r=rpc_line(m,{'jsonrpc':'2.0','id':'3','method':'tools/call','params':{'name':'echo','arguments':{'x':1}}});ck('mcp_real_process_call',r.get('result',{}).get('isError') is False)
m.terminate();m.wait(timeout=2)
l=subprocess.Popen([py,str(ROOT/'tests/v064/lsp_stdio_fixture.py')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
lsp_send(l,{'jsonrpc':'2.0','id':1,'method':'initialize','params':{}});r=lsp_read(l);ck('lsp_real_process_initialize',r.get('result',{}).get('capabilities',{}).get('hoverProvider') is True)
lsp_send(l,{'jsonrpc':'2.0','id':2,'method':'textDocument/hover','params':{}});r=lsp_read(l);ck('lsp_real_process_hover',r.get('result',{}).get('contents',{}).get('value')=='fixture hover')
lsp_send(l,{'jsonrpc':'2.0','id':3,'method':'shutdown','params':None});r=lsp_read(l);ck('lsp_real_process_shutdown','result' in r)
lsp_send(l,{'jsonrpc':'2.0','method':'exit','params':None});l.wait(timeout=2);ck('lsp_real_process_exit',l.returncode==0)
report={'version':'0.64.0','checks':checks,'pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks)}
out=ROOT/'reports/V064_PROCESS_SMOKE.json';out.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2));raise SystemExit(1 if report['fail'] else 0)

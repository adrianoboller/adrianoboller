#!/usr/bin/env python3
import json, sys

def read_msg():
    n=None
    while True:
        line=sys.stdin.buffer.readline()
        if not line: return None
        if line in (b'\r\n',b'\n'): break
        k,_,v=line.decode('ascii').partition(':')
        if k.lower()=='content-length': n=int(v.strip())
    if n is None: raise SystemExit(2)
    return json.loads(sys.stdin.buffer.read(n))

def send(v):
    body=json.dumps(v,separators=(',',':')).encode()
    sys.stdout.buffer.write(f'Content-Length: {len(body)}\r\n\r\n'.encode()+body);sys.stdout.buffer.flush()
while True:
    msg=read_msg()
    if msg is None: break
    m=msg.get('method')
    if m=='exit': break
    if 'id' not in msg: continue
    if m=='initialize': result={'capabilities':{'hoverProvider':True},'serverInfo':{'name':'phx-lsp-fixture','version':'0.64'}}
    elif m=='shutdown': result=None
    elif m=='textDocument/hover': result={'contents':{'kind':'plaintext','value':'fixture hover'}}
    else:
        send({'jsonrpc':'2.0','id':msg['id'],'error':{'code':-32601,'message':'not found'}});continue
    send({'jsonrpc':'2.0','id':msg['id'],'result':result})

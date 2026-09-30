#!/usr/bin/env python3
import json, sys, time
for raw in sys.stdin:
    if not raw.strip(): continue
    msg=json.loads(raw)
    method=msg.get('method')
    if method=='notifications/cancelled':
        continue
    if 'id' not in msg: continue
    if method=='initialize':
        result={'protocolVersion':'2025-11-25','capabilities':{'tools':{}},'serverInfo':{'name':'phx-fixture','version':'0.64'}}
    elif method=='server/discover':
        result={'protocolVersion':'2026-07-28','serverInfo':{'name':'phx-fixture','version':'0.64'},'capabilities':{}}
    elif method=='tools/list':
        result={'tools':[{'name':'echo','description':'fixture','inputSchema':{'type':'object'}}]}
    elif method=='tools/call':
        result={'content':[{'type':'text','text':json.dumps(msg.get('params',{}),sort_keys=True)}], 'isError':False}
    elif method=='fixture/delay':
        time.sleep(0.2); result={'ok':True}
    else:
        print(json.dumps({'jsonrpc':'2.0','id':msg['id'],'error':{'code':-32601,'message':'not found'}}),flush=True); continue
    print(json.dumps({'jsonrpc':'2.0','id':msg['id'],'result':result}),flush=True)

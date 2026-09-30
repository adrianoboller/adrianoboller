#!/usr/bin/env python3
import argparse, json, time, uuid
from dataclasses import dataclass

def uv7():
    ms=int(time.time()*1000); r=uuid.uuid4().int
    hi=(ms & ((1<<48)-1))<<80; hi|=7<<76; hi|=(r & ((1<<12)-1))<<64; hi|=2<<62; hi|=r & ((1<<62)-1)
    return str(uuid.UUID(int=hi))

@dataclass
class Binding:
    principal:str; state:str

class Gateway:
    def __init__(self):
        self.identities={}; self.sessions={}; self.seen=set(); self.events=[]
    def bind(self,ch,acct,user,principal,state='active'):
        self.identities[(ch,acct,user)]=Binding(principal,state); self.events.append(('identity.bound',principal))
    def state(self,ch,acct,user,state): self.identities[(ch,acct,user)].state=state
    def inbound(self,ch,acct,user,conv,msgid,text):
        if (ch,acct,msgid) in self.seen: raise ValueError('duplicate')
        b=self.identities.get((ch,acct,user))
        if not b: raise ValueError('identity_missing')
        if b.state!='active': raise ValueError('identity_'+b.state)
        self.seen.add((ch,acct,msgid)); key=(b.principal,ch,acct,conv)
        sid=self.sessions.setdefault(key,uv7()); mid=uv7(); self.events.append(('channel.inbound.accepted',mid,sid,b.principal))
        return {'message_uuid':mid,'session_uuid':sid,'principal_uuid':b.principal,'text':text}
    def outbound(self,sid,text):
        key=next((k for k,v in self.sessions.items() if v==sid),None)
        if not key: raise ValueError('unknown_session')
        principal,ch,acct,conv=key
        binding=next((b for (c,a,_),b in self.identities.items() if c==ch and a==acct and b.principal==principal),None)
        if not binding or binding.state!='active': raise ValueError('identity_blocked')
        out={'message_uuid':uv7(),'session_uuid':sid,'channel':ch,'account_id':acct,'conversation_id':conv,'text':text}; self.events.append(('channel.outbound.requested',out['message_uuid'],sid)); return out

def demo():
    g=Gateway(); p1=uv7(); p2=uv7(); checks=[]
    g.bind('telegram','main','u1',p1,'active'); checks.append(('identity_bind',True))
    a=g.inbound('telegram','main','u1','c1','m1','oi'); checks.append(('inbound_accept',a['principal_uuid']==p1))
    b=g.inbound('telegram','main','u1','c1','m2','novo'); checks.append(('session_reuse',a['session_uuid']==b['session_uuid']))
    try: g.inbound('telegram','main','u1','c1','m2','dup'); checks.append(('dedup',False))
    except ValueError as e: checks.append(('dedup',str(e)=='duplicate'))
    out=g.outbound(a['session_uuid'],'resposta'); checks.append(('outbound_prepare',out['channel']=='telegram'))
    g.bind('whatsapp','sales','u2',p2,'pending')
    try: g.inbound('whatsapp','sales','u2','c2','m3','oi'); checks.append(('pending_reject',False))
    except ValueError as e: checks.append(('pending_reject',str(e)=='identity_pending'))
    g.state('telegram','main','u1','blocked')
    try: g.outbound(a['session_uuid'],'blocked'); checks.append(('blocked_outbound',False))
    except ValueError: checks.append(('blocked_outbound',True))
    checks.append(('events_recorded',len(g.events)>=4))
    report={'suite':'F21 Channel Gateway operational bootstrap','pass':sum(v for _,v in checks),'fail':sum(not v for _,v in checks),'checks':[{'name':n,'pass':v} for n,v in checks],'session_uuid':a['session_uuid']}
    print(json.dumps(report,indent=2)); return 1 if report['fail'] else 0

if __name__=='__main__':
    ap=argparse.ArgumentParser(); ap.add_argument('command',nargs='?',default='demo',choices=['demo']); args=ap.parse_args(); raise SystemExit(demo())

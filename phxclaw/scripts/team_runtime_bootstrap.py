#!/usr/bin/env python3
from __future__ import annotations
import argparse, json, secrets, threading, time
from dataclasses import dataclass, field, asdict
from typing import Any

def uuid7():
    ms=int(time.time()*1000)&((1<<48)-1); ra=secrets.randbits(12); rb=secrets.randbits(62); v=(ms<<80)|(0x7<<76)|(ra<<64)|(0b10<<62)|rb; h=f"{v:032x}"; return f"{h[:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:]}"
@dataclass
class Task:
    name:str; capability:str; depends_on:list[str]=field(default_factory=list); priority:int=0; uuid:str=field(default_factory=uuid7); state:str='pending'; worker_id:str|None=None; fencing_token:int=0; lease_until:float|None=None; heartbeat_at:float|None=None; attempts:int=0; cancel_requested:bool=False; output:Any=None
@dataclass
class Team:
    name:str; max_parallelism:int=4; uuid:str=field(default_factory=uuid7); correlation_uuid:str=field(default_factory=uuid7); state:str='running'; tasks:dict[str,Task]=field(default_factory=dict)
class TeamRuntime:
    def __init__(self): self._lock=threading.RLock(); self.teams={}
    def create(self,team:Team):
        with self._lock:
            ids=set(team.tasks)
            for t in team.tasks.values():
                miss=[d for d in t.depends_on if d not in ids]
                if miss: raise ValueError(f'missing dependency {miss[0]}')
            self.teams[team.uuid]=team; return team.uuid
    def claim(self,team_id,worker,ttl=1.0):
        with self._lock:
            team=self.teams[team_id]; running=sum(t.state=='running' for t in team.tasks.values())
            if team.state!='running' or running>=team.max_parallelism:return None
            done={i for i,t in team.tasks.items() if t.state=='succeeded'}
            ready=[t for t in team.tasks.values() if t.state=='pending' and not t.cancel_requested and all(d in done for d in t.depends_on)]
            if not ready:return None
            ready.sort(key=lambda t:(-t.priority,t.uuid)); t=ready[0]; t.state='running'; t.worker_id=worker; t.fencing_token+=1; t.attempts+=1; t.heartbeat_at=time.time(); t.lease_until=time.time()+max(.01,ttl)
            return {'team_uuid':team_id,'task_uuid':t.uuid,'worker_id':worker,'fencing_token':t.fencing_token,'lease_until':t.lease_until,'capability':t.capability}
    def _task(self,g): return self.teams[g['team_uuid']].tasks[g['task_uuid']]
    def _check(self,g):
        t=self._task(g)
        if t.state!='running' or t.worker_id!=g['worker_id'] or t.fencing_token!=g['fencing_token']: raise RuntimeError('stale_fencing_token')
        if t.cancel_requested: raise RuntimeError('cancelled')
        return t
    def heartbeat(self,g,ttl=1.0):
        with self._lock:
            t=self._check(g); t.heartbeat_at=time.time(); t.lease_until=time.time()+max(.01,ttl)
    def succeed(self,g,output=None):
        with self._lock:
            t=self._check(g); t.state='succeeded'; t.output=output; t.worker_id=None; t.lease_until=None
            team=self.teams[g['team_uuid']]
            if all(x.state=='succeeded' for x in team.tasks.values()): team.state='succeeded'
    def fail(self,g,output=None):
        with self._lock:
            t=self._check(g); t.state='failed'; t.output=output; t.worker_id=None; t.lease_until=None; self.teams[g['team_uuid']].state='failed'
    def recover(self):
        out=[]; now=time.time()
        with self._lock:
            for team in self.teams.values():
                if team.state!='running': continue
                for t in team.tasks.values():
                    if t.state=='running' and t.lease_until is not None and t.lease_until<=now:
                        t.state='pending'; t.worker_id=None; t.lease_until=None; t.heartbeat_at=None; t.fencing_token+=1; out.append(t.uuid)
        return out
    def cancel(self,team_id):
        with self._lock:
            team=self.teams[team_id]; team.state='cancelled'
            for t in team.tasks.values():
                if t.state=='pending': t.state='cancelled'
                elif t.state=='running': t.cancel_requested=True
    def snapshot(self,team_id): return asdict(self.teams[team_id])

def demo():
    rt=TeamRuntime(); a=Task('research','research.collect'); b=Task('repo','repo.scan'); c=Task('synthesis','model.reasoning',depends_on=[a.uuid,b.uuid]); team=Team('demo',max_parallelism=2,tasks={a.uuid:a,b.uuid:b,c.uuid:c}); rt.create(team)
    g1=rt.claim(team.uuid,'worker-a',.2); g2=rt.claim(team.uuid,'worker-b',.2); assert g1 and g2 and g1['task_uuid']!=g2['task_uuid']; rt.heartbeat(g1,.2); rt.succeed(g1,{'ok':1}); rt.succeed(g2,{'ok':1}); g3=rt.claim(team.uuid,'worker-c',.2); assert g3 and g3['task_uuid']==c.uuid; rt.succeed(g3,{'ok':1}); assert rt.teams[team.uuid].state=='succeeded'
    d=Task('recovery','workspace.diff'); team2=Team('recovery',max_parallelism=1,tasks={d.uuid:d}); rt.create(team2); old=rt.claim(team2.uuid,'old',.01); time.sleep(.03); assert d.uuid in rt.recover(); new=rt.claim(team2.uuid,'new',.2); assert new and new['fencing_token']>old['fencing_token']; stale=False
    try: rt.succeed(old,{})
    except RuntimeError: stale=True
    assert stale; rt.succeed(new,{'recovered':True})
    e=Task('cancel','workspace.diff'); team3=Team('cancel',tasks={e.uuid:e}); rt.create(team3); rt.cancel(team3.uuid); assert rt.teams[team3.uuid].state=='cancelled' and e.state=='cancelled'
    return {'status':'ok','checks':['parallel_claim','dependency_gate','heartbeat','lease_recovery','fencing_rejects_stale_worker','cancel'],'teams':[rt.snapshot(team.uuid),rt.snapshot(team2.uuid),rt.snapshot(team3.uuid)]}

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('command',choices=['demo']); a=ap.parse_args(); print(json.dumps(demo(),ensure_ascii=False,indent=2)); return 0
if __name__=='__main__': raise SystemExit(main())

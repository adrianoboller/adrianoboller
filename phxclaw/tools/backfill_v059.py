#!/usr/bin/env python3
"""PhxClaw v0.59 historical backfill.

Requires psycopg 3 only for live execution. --plan-only works without a database.
Database credentials are read from PHXCLAW_DATABASE_URL; passwords are never accepted as CLI flags.
"""
from __future__ import annotations
import argparse,hashlib,json,os,re,sys,time
from pathlib import Path
from datetime import datetime,timezone
from uuid import UUID
try:
    import psycopg
    from psycopg.rows import dict_row
except Exception:
    psycopg=None

SECRET_KEYS={"password","token","api_key","apikey","secret","private_key","access_token","refresh_token"}
SCOPE_POLICIES={"direct_project","reference_required","portfolio_scope","tenant_scope"}
HEX64=re.compile(r"^[0-9a-f]{64}$")
SAFE_IDENT=re.compile(r"^[a-z][a-z0-9_]*$")

def canonical(obj): return json.dumps(obj,sort_keys=True,separators=(",",":"),default=str).encode()
def sha256_obj(obj): return hashlib.sha256(canonical(obj)).hexdigest()
def uuid7():
    # RFC 9562 UUIDv7, generated without third-party dependencies.
    import secrets
    ms=int(time.time_ns()//1_000_000)&((1<<48)-1); rand=secrets.randbits(74)
    value=(ms<<80)|(0x7<<76)|(((rand>>62)&0xFFF)<<64)|(0b10<<62)|(rand&((1<<62)-1))
    return UUID(int=value)
def valid_uuid(v):
    try:return UUID(str(v))
    except:return None
def qident(v): return '"'+str(v).replace('\"','\"\"')+'"'
def clean_json(v):
    if isinstance(v,dict): return {k:clean_json(x) for k,x in v.items() if k.lower() not in SECRET_KEYS}
    if isinstance(v,list): return [clean_json(x) for x in v]
    return v
def first_hash(row,cols):
    for c in cols:
        v=row.get(c)
        if isinstance(v,str) and HEX64.match(v.lower()): return v.lower(),c
    return None,None
def title_for(row,entry,obj_uuid):
    for c in entry.get('title_columns',[]):
        v=row.get(c)
        if v not in (None,''): return str(v)[:240]
    return f"{entry['source_table']} {obj_uuid}"
def link_kind(col):
    if col in ('task_uuid','run_uuid','cycle_uuid','scenario_uuid','search_uuid','plan_uuid','twin_uuid','controller_uuid','forecast_uuid'): return 'related_to'
    if col in ('agent_uuid','model_profile_uuid'): return 'uses'
    if col in ('knowledge_uuid','candidate_uuid'): return 'derived_from'
    if col in ('case_uuid','recommendation_uuid','supervisor_plan_uuid'): return 'decided_by'
    if col=='contradiction_uuid': return 'related_to'
    return 'related_to'

def registry_path():
    here=Path(__file__).resolve(); candidates=[here.parents[1]/'config/historical-backfill-registry.v059.json',here.parents[2]/'overlay/config/historical-backfill-registry.v059.json']
    for p in candidates:
        if p.exists(): return p
    raise SystemExit('registry not found')
def load_registry():
    p=registry_path(); raw=p.read_bytes(); reg=json.loads(raw)
    for entry in reg.get('sources',[]):
        if not SAFE_IDENT.fullmatch(entry.get('source_table','')): raise SystemExit('unsafe registry table identifier')
        for key in ('parent_ref_columns','link_ref_columns','title_columns','source_state_columns','evidence_columns','timestamp_columns'):
            if any(not SAFE_IDENT.fullmatch(c) for c in entry.get(key,[])): raise SystemExit('unsafe registry column identifier')
    return reg,hashlib.sha256(raw).hexdigest()

def table_exists(cur,t): cur.execute('select to_regclass(%s) as r',(t,)); return cur.fetchone()['r'] is not None
def columns(cur,t):
    cur.execute("select column_name,data_type from information_schema.columns where table_schema=current_schema() and table_name=%s order by ordinal_position",(t,)); return {r['column_name']:r['data_type'] for r in cur.fetchall()}
def primary_key_columns(cur,t):
    cur.execute("""select a.attname from pg_index i join pg_class c on c.oid=i.indrelid join pg_attribute a on a.attrelid=c.oid and a.attnum=any(i.indkey) where c.relname=%s and i.indisprimary order by array_position(i.indkey,a.attnum)""",(t,)); return [r['attname'] for r in cur.fetchall()]
def identity_for(cur,tenant,table,row,pk_cols):
    pk={k:row.get(k) for k in pk_cols}; key=sha256_obj({'table':table,'pk':pk if pk else clean_json(row)})
    # Reuse an existing non-tenant/project UUID PK when unambiguous.
    ids=[valid_uuid(row.get(k)) for k in pk_cols if k not in ('tenant_uuid','project_uuid') and row.get(k)]
    ids=[x for x in ids if x]
    if len(ids)==1:return ids[0],key
    cur.execute("select object_uuid from phx_historical_backfill_identity_map where tenant_uuid=%s and source_table=%s and source_key_sha256=%s",(tenant,table,key)); r=cur.fetchone()
    if r:return r['object_uuid'],key
    u=uuid7(); cur.execute("insert into phx_historical_backfill_identity_map(tenant_uuid,source_table,source_key_sha256,object_uuid) values(%s,%s,%s,%s)",(tenant,table,key,u)); return u,key
def resolve_bound_node(cur,tenant,obj_uuid,project=None):
    if not obj_uuid:return None
    if project:
        cur.execute("select project_uuid,node_uuid,object_table from phx_project_trace_bindings where tenant_uuid=%s and project_uuid=%s and object_uuid=%s limit 2",(tenant,project,obj_uuid))
    else:
        cur.execute("select project_uuid,node_uuid,object_table from phx_project_trace_bindings where tenant_uuid=%s and object_uuid=%s limit 2",(tenant,obj_uuid))
    rs=cur.fetchall(); return rs[0] if len(rs)==1 else None
def ensure_scope(cur,tenant,kind,key):
    cur.execute("select project_uuid from phx_historical_backfill_scope_map where tenant_uuid=%s and scope_kind=%s and scope_key=%s",(tenant,kind,key)); r=cur.fetchone()
    if r:return r['project_uuid']
    p=uuid7(); cur.execute("insert into phx_historical_backfill_scope_map(tenant_uuid,scope_kind,scope_key,project_uuid) values(%s,%s,%s,%s)",(tenant,kind,key,p)); return p
def resolve_project(cur,tenant,row,entry):
    p=valid_uuid(row.get('project_uuid'))
    if p:return p,'direct'
    for col in entry.get('parent_ref_columns',[])+entry.get('link_ref_columns',[]):
        u=valid_uuid(row.get(col)); b=resolve_bound_node(cur,tenant,u) if u else None
        if b:return b['project_uuid'],f'reference:{col}'
    pol=entry['scope_policy']
    if pol=='portfolio_scope': return ensure_scope(cur,tenant,'portfolio','default'),'portfolio_scope'
    if pol=='tenant_scope': return ensure_scope(cur,tenant,'tenant','history'),'tenant_scope'
    return None,'unresolved'
def ensure_root(cur,tenant,project,source_state):
    cur.execute("select node_uuid from phx_project_trace_nodes where tenant_uuid=%s and project_uuid=%s and node_kind='project' and parent_uuid is null order by created_at limit 1",(tenant,project)); r=cur.fetchone()
    if r:return r['node_uuid']
    title=f'Project {project}'
    if table_exists(cur,'phx_projects'):
        cols=columns(cur,'phx_projects')
        namecol='name' if 'name' in cols else ('slug' if 'slug' in cols else None)
        if namecol:
            cur.execute(f'select {namecol} as n from phx_projects where tenant_uuid=%s and project_uuid=%s limit 1',(tenant,project)); rr=cur.fetchone();
            if rr and rr['n']: title=str(rr['n'])
    cur.execute("insert into phx_project_trace_nodes(tenant_uuid,project_uuid,node_uuid,parent_uuid,root_uuid,depth,path_ids,node_kind,logical_key,title,source_state_sha256,metadata) values(%s,%s,%s,null,%s,0,array[%s]::uuid[],'project',%s,%s,%s,%s) on conflict do nothing",(tenant,project,project,project,project,f'project:{project}',title,source_state,json.dumps({'historical_backfill_root':True})))
    return project
def ensure_domain(cur,tenant,project,root,domain,source_state):
    key=f'backfill-domain:{domain}'
    cur.execute("select node_uuid from phx_project_trace_nodes where tenant_uuid=%s and project_uuid=%s and node_kind='domain' and logical_key=%s order by created_at limit 1",(tenant,project,key)); r=cur.fetchone()
    if r:return r['node_uuid']
    u=uuid7(); cur.execute("insert into phx_project_trace_nodes(tenant_uuid,project_uuid,node_uuid,parent_uuid,root_uuid,depth,path_ids,node_kind,logical_key,title,source_state_sha256,metadata) values(%s,%s,%s,%s,%s,1,array[%s,%s]::uuid[],'domain',%s,%s,%s,%s)",(tenant,project,u,root,root,root,u,key,domain.replace('_',' ').title(),source_state,json.dumps({'historical_backfill_domain':True}))); return u
def row_source_state(row,entry,row_hash):
    h,c=first_hash(row,entry.get('source_state_columns',[])); return (h,c,False) if h else (row_hash,'canonical_row_sha256',True)
def row_evidence(row,entry,row_hash):
    h,c=first_hash(row,entry.get('evidence_columns',[])); return (h,c) if h else (row_hash,'canonical_row_sha256')
def source_uri(table,key): return f'postgresql://{table}/{key}'

def process_row(cur,tenant,run_uuid,row,entry,pk_cols):
    raw_row_hash=sha256_obj(row); row=clean_json(row); row_hash=raw_row_hash; obj,key=identity_for(cur,tenant,entry['source_table'],row,pk_cols)
    # Idempotency: binding means this row already exists in the trace tree.
    cur.execute("select project_uuid,node_uuid from phx_project_trace_bindings where tenant_uuid=%s and object_table=%s and object_uuid=%s limit 1",(tenant,entry['source_table'],obj)); ex=cur.fetchone()
    if ex:return 'skipped',None
    project,project_mode=resolve_project(cur,tenant,row,entry)
    ev,_=row_evidence(row,entry,row_hash)
    if not project:
        o=uuid7(); cur.execute("insert into phx_historical_backfill_orphans(tenant_uuid,run_uuid,orphan_uuid,source_table,source_key_sha256,row_sha256,reason_code,candidate_refs,evidence_sha256) values(%s,%s,%s,%s,%s,%s,'project_unresolved',%s,%s)",(tenant,run_uuid,o,entry['source_table'],key,row_hash,json.dumps({c:str(row.get(c)) for c in entry.get('parent_ref_columns',[]) if row.get(c)}),ev)); return 'orphan',str(o)
    state,state_col,inferred=row_source_state(row,entry,row_hash); root=ensure_root(cur,tenant,project,state); domain_node=ensure_domain(cur,tenant,project,root,entry['domain'],state)
    parent=domain_node
    for col in entry.get('parent_ref_columns',[]):
        u=valid_uuid(row.get(col)); b=resolve_bound_node(cur,tenant,u,project) if u else None
        if b: parent=b['node_uuid']; break
    node=uuid7(); title=title_for(row,entry,obj)
    metadata={'historical_source_table':entry['source_table'],'historical_source_version':entry['source_version'],'backfill_run_uuid':str(run_uuid),'project_resolution':project_mode,'source_state_origin':state_col,'source_state_inferred':inferred,'source_key_sha256':key}
    cur.execute("insert into phx_project_trace_nodes(tenant_uuid,project_uuid,node_uuid,parent_uuid,root_uuid,depth,path_ids,node_kind,logical_key,title,object_type,object_uuid,source_state_sha256,evidence_sha256,metadata) values(%s,%s,%s,%s,%s,0,array[%s]::uuid[],%s,%s,%s,%s,%s,%s,%s,%s)",(tenant,project,node,parent,root,node,entry['node_kind'],f"{entry['source_table']}:{key}",title,entry['source_table'],obj,state,ev,json.dumps(metadata)))
    # v0.58 trigger derives depth/root/path from parent.
    cur.execute("insert into phx_project_trace_bindings(tenant_uuid,project_uuid,binding_uuid,node_uuid,object_table,object_uuid,object_sha256,source_state_sha256) values(%s,%s,%s,%s,%s,%s,%s,%s)",(tenant,project,uuid7(),node,entry['source_table'],obj,row_hash,state))
    prov={'source':'historical_backfill','source_table':entry['source_table'],'source_version':entry['source_version'],'run_uuid':str(run_uuid),'row_sha256':row_hash}
    cur.execute("insert into phx_project_source_documents(tenant_uuid,project_uuid,source_uuid,node_uuid,source_kind,uri,content_sha256,source_state_sha256,provenance) values(%s,%s,%s,%s,'postgresql-row',%s,%s,%s,%s)",(tenant,project,uuid7(),node,source_uri(entry['source_table'],key),row_hash,state,json.dumps(prov)))
    # Typed cross-links to already-bound objects. Parent relation is already encoded structurally.
    seen=set()
    for col in entry.get('link_ref_columns',[]):
        u=valid_uuid(row.get(col))
        if not u or u in seen: continue
        seen.add(u); b=resolve_bound_node(cur,tenant,u,project)
        if b and b['node_uuid']!=node and b['node_uuid']!=parent:
            cur.execute("insert into phx_project_trace_links(tenant_uuid,project_uuid,link_uuid,from_node_uuid,to_node_uuid,link_kind,evidence_sha256,source_state_sha256,metadata) values(%s,%s,%s,%s,%s,%s,%s,%s,%s)",(tenant,project,uuid7(),node,b['node_uuid'],link_kind(col),ev,state,json.dumps({'historical_ref_column':col})))
    return 'inserted',str(node)

def execute(args):
    reg,reg_hash=load_registry(); sources=[x for x in reg['sources'] if x.get('enabled')];
    if getattr(args,'only_table',None):
        wanted=set(args.only_table); sources=[x for x in sources if x['source_table'] in wanted]
        missing=wanted-{x['source_table'] for x in sources}
        if missing: raise SystemExit('unknown registry table(s): '+','.join(sorted(missing)))
    if args.plan_only:
        print(json.dumps({'mode':'plan_only','registry_sha256':reg_hash,'source_count':len(sources),'sources':[{'table':x['source_table'],'priority':x['priority'],'scope_policy':x['scope_policy']} for x in sources]},indent=2));return 0
    if psycopg is None: raise SystemExit('psycopg 3 is required for live backfill')
    dsn=os.environ.get('PHXCLAW_DATABASE_URL');
    if not dsn: raise SystemExit('PHXCLAW_DATABASE_URL is required; credentials are never accepted via CLI flags')
    tenant=UUID(args.tenant); batch=args.batch_size; run=UUID(args.resume_run) if args.resume_run else uuid7()
    with psycopg.connect(dsn,row_factory=dict_row) as conn:
      with conn.cursor() as cur:
        cur.execute("select pg_try_advisory_lock(hashtextextended(%s,0)) as ok",(f'phx-v059:{tenant}',)); lock=cur.fetchone()
        if not lock or not lock['ok']: raise SystemExit('another v0.59 backfill is already active for this tenant')
        cur.execute("select set_config('phx.tenant_uuid',%s,false)",(str(tenant),))
        # prerequisite
        for t in ('phx_project_trace_nodes','phx_project_trace_bindings','phx_project_source_documents','phx_historical_backfill_runs'):
            if not table_exists(cur,t): raise SystemExit(f'missing prerequisite table: {t}')
        if args.resume_run:
            cur.execute("select registry_sha256,batch_size from phx_historical_backfill_runs where tenant_uuid=%s and run_uuid=%s",(tenant,run)); rr=cur.fetchone()
            if not rr: raise SystemExit('resume run not found')
            if rr['registry_sha256'].strip()!=reg_hash: raise SystemExit('registry drift: refusing resume')
            batch=rr['batch_size']
        else:
            state=args.source_state or hashlib.sha256((str(tenant)+reg_hash).encode()).hexdigest()
            cur.execute("insert into phx_historical_backfill_runs(tenant_uuid,run_uuid,source_state_sha256,registry_sha256,batch_size,mode,requested_by) values(%s,%s,%s,%s,%s,'backfill',%s)",(tenant,run,state,reg_hash,batch,args.requested_by))
        conn.commit()
      table_reports=[]
      for entry in sources:
        table=entry['source_table']
        with conn.cursor() as cur:
          cur.execute("select set_config('phx.tenant_uuid',%s,false)",(str(tenant),))
          if not table_exists(cur,table):
            ev=sha256_obj({'run':str(run),'table':table,'status':'missing_table'})
            cur.execute("insert into phx_historical_backfill_coverage(tenant_uuid,run_uuid,snapshot_uuid,source_table,source_rows,processed_rows,bound_rows,orphan_rows,skipped_rows,coverage_ratio,status,evidence_sha256) values(%s,%s,%s,%s,0,0,0,0,0,0,'missing_table',%s)",(tenant,run,uuid7(),table,ev));conn.commit();table_reports.append({'table':table,'status':'missing_table'});continue
          cols=columns(cur,table); pk=primary_key_columns(cur,table)
          cur.execute(f'SELECT count(*) AS n FROM {qident(table)} WHERE tenant_uuid=%s',(tenant,)); total=cur.fetchone()['n']
          cur.execute("select checkpoint_seq,offset_rows,processed_count,inserted_count,skipped_count,orphan_count,cursor_json from phx_historical_backfill_checkpoints where tenant_uuid=%s and run_uuid=%s and source_table=%s order by checkpoint_seq desc limit 1",(tenant,run,table)); cp=cur.fetchone()
          if cp:
            seq=cp['checkpoint_seq']; offset=cp['offset_rows']; processed=cp['processed_count']; inserted=cp['inserted_count']; skipped=cp['skipped_count']; orphaned=cp['orphan_count']; cursor=cp['cursor_json'] or {}
          else:
            seq=offset=processed=inserted=skipped=orphaned=0; cursor={}
        last_pk=cursor.get('last_pk') if pk else None
        while processed<total:
          with conn.cursor() as cur:
            cur.execute("select set_config('phx.tenant_uuid',%s,false)",(str(tenant),))
            if pk:
                order_sql=','.join(qident(c) for c in pk); params=[tenant]; where='tenant_uuid=%s'
                if last_pk:
                    where+=' AND ('+','.join(qident(c) for c in pk)+') > ('+','.join(['%s']*len(pk))+')'; params.extend(last_pk[c] for c in pk)
                params.append(batch); query=f'SELECT to_jsonb(t) AS row_json FROM {qident(table)} t WHERE {where} ORDER BY {order_sql} LIMIT %s'
                cur.execute(query,params)
            else:
                # Explicit fallback only for legacy tables without a primary key. Coverage reports keep this visible.
                cur.execute(f'SELECT to_jsonb(t) AS row_json FROM {qident(table)} t WHERE tenant_uuid=%s ORDER BY ctid OFFSET %s LIMIT %s',(tenant,offset,batch))
            rows=[r['row_json'] for r in cur.fetchall()]
            if not rows: break
            b_ins=b_skip=b_orph=0
            for row in rows:
                try:
                    status,_=process_row(cur,tenant,run,row,entry,pk)
                except Exception as e:
                    status='orphan'; rowh=sha256_obj(row); key=sha256_obj({'table':table,'row':clean_json(row)}); ev=sha256_obj({'error':type(e).__name__,'table':table,'key':key});
                    cur.execute("insert into phx_historical_backfill_orphans(tenant_uuid,run_uuid,orphan_uuid,source_table,source_key_sha256,row_sha256,reason_code,candidate_refs,evidence_sha256) values(%s,%s,%s,%s,%s,%s,'row_processing_error',%s,%s)",(tenant,run,uuid7(),table,key,rowh,json.dumps({'error_type':type(e).__name__}),ev))
                if status=='inserted':b_ins+=1
                elif status=='skipped':b_skip+=1
                else:b_orph+=1
            processed+=len(rows); offset=processed; inserted+=b_ins; skipped+=b_skip; orphaned+=b_orph; seq+=1
            if pk: last_pk={c:rows[-1].get(c) for c in pk}; cursor={'mode':'pk_keyset','last_pk':clean_json(last_pk)}
            else: cursor={'mode':'ctid_fallback','offset':offset}
            ev=sha256_obj({'run':str(run),'table':table,'seq':seq,'offset':offset,'cursor':cursor,'processed':processed,'inserted':inserted,'skipped':skipped,'orphaned':orphaned})
            cur.execute("insert into phx_historical_backfill_checkpoints(tenant_uuid,run_uuid,checkpoint_uuid,source_table,checkpoint_seq,offset_rows,processed_count,inserted_count,skipped_count,orphan_count,cursor_json,evidence_sha256) values(%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s)",(tenant,run,uuid7(),table,seq,offset,processed,inserted,skipped,orphaned,json.dumps(cursor),ev));conn.commit()
        ratio=1.0 if total==0 else max(0.0,min(1.0,(inserted+skipped)/total)); status='complete' if processed>=total and orphaned==0 else 'partial'; ev=sha256_obj({'run':str(run),'table':table,'total':total,'inserted':inserted,'skipped':skipped,'orphaned':orphaned,'ratio':ratio})
        with conn.cursor() as cur:
          cur.execute("select set_config('phx.tenant_uuid',%s,false)",(str(tenant),));cur.execute("insert into phx_historical_backfill_coverage(tenant_uuid,run_uuid,snapshot_uuid,source_table,source_rows,processed_rows,bound_rows,orphan_rows,skipped_rows,coverage_ratio,status,evidence_sha256) values(%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s)",(tenant,run,uuid7(),table,total,processed,inserted+skipped,orphaned,skipped,ratio,status,ev));conn.commit()
        table_reports.append({'table':table,'status':status,'source_rows':total,'processed':processed,'inserted':inserted,'skipped':skipped,'orphans':orphaned,'coverage':ratio})
      summary={'run_uuid':str(run),'tenant_uuid':str(tenant),'registry_sha256':reg_hash,'tables':table_reports,'summary':{'registry_tables':len(sources),'complete':sum(x.get('status')=='complete' for x in table_reports),'partial':sum(x.get('status')=='partial' for x in table_reports),'missing':sum(x.get('status')=='missing_table' for x in table_reports),'orphans':sum(x.get('orphans',0) for x in table_reports)}}
      print(json.dumps(summary,indent=2));
      if args.report: Path(args.report).write_text(json.dumps(summary,indent=2)+"\n")
    return 0

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('--tenant',help='Tenant UUID; required for live execution'); ap.add_argument('--batch-size',type=int,default=500); ap.add_argument('--resume-run'); ap.add_argument('--source-state'); ap.add_argument('--requested-by',default='phxclaw-v059'); ap.add_argument('--report'); ap.add_argument('--only-table',action='append',help='Restrict to one registry table; repeatable and useful for orphan retries'); ap.add_argument('--plan-only',action='store_true'); a=ap.parse_args()
    if not a.plan_only and not a.tenant: ap.error('--tenant is required for live execution')
    if not (1<=a.batch_size<=10000): ap.error('--batch-size must be 1..10000')
    raise SystemExit(execute(a))
if __name__=='__main__': main()

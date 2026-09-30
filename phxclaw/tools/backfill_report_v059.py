#!/usr/bin/env python3
import argparse,json,os,sys
try: import psycopg
except Exception: psycopg=None
ap=argparse.ArgumentParser();ap.add_argument('--tenant',required=True);ap.add_argument('--run',required=True);a=ap.parse_args()
if psycopg is None: raise SystemExit('psycopg 3 required')
dsn=os.environ.get('PHXCLAW_DATABASE_URL');
if not dsn: raise SystemExit('PHXCLAW_DATABASE_URL required')
with psycopg.connect(dsn) as c:
 with c.cursor() as q:
  q.execute("select set_config('phx.tenant_uuid',%s,false)",(a.tenant,));
  q.execute('select * from phx_historical_backfill_latest_coverage_v where tenant_uuid=%s and run_uuid=%s order by source_table',(a.tenant,a.run)); cols=[d.name for d in q.description]; rows=[dict(zip(cols,r)) for r in q.fetchall()]
  q.execute('select reason_code,count(*) from phx_historical_backfill_orphans where tenant_uuid=%s and run_uuid=%s group by reason_code order by 2 desc',(a.tenant,a.run)); orph=[{'reason':r[0],'count':r[1]} for r in q.fetchall()]
print(json.dumps({'tables':rows,'orphans':orph},default=str,indent=2))

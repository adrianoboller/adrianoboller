#!/usr/bin/env python3
from pathlib import Path
import json,importlib.util,tempfile,subprocess,sys,hashlib,shutil
base=Path(__file__).resolve().parents[1]; tool=base/'tools/backfill_v059.py'
spec=importlib.util.spec_from_file_location('bf',tool);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
checks=[]
def c(n,o):checks.append({'name':n,'pass':bool(o)})
reg=json.loads((base/'config/historical-backfill-registry.v059.json').read_text());inv=json.loads((base/'config/historical-table-inventory.v059.json').read_text())
c('registry_inventory_same',{x['source_table'] for x in reg['sources']}=={x['source_table'] for x in inv['tables']})
u=m.uuid7();c('uuid7_version',u.version==7);c('uuid7_variant',u.variant=='specified in RFC 4122')
c('uuid7_timestamp_prefix',int(u.hex[:12],16)>0)
c('canonical_hash_order',m.sha256_obj({'a':1,'b':2})==m.sha256_obj({'b':2,'a':1}))
clean=m.clean_json({'a':1,'password':'secret-value','nested':{'api_key':'abcdefgh','x':2}});c('secret_scrub','password' not in clean and 'api_key' not in clean['nested'])
c('link_agent',m.link_kind('agent_uuid')=='uses');c('link_knowledge',m.link_kind('knowledge_uuid')=='derived_from');c('link_decision',m.link_kind('case_uuid')=='decided_by')
prio=[x['priority'] for x in reg['sources']];c('priority_sorted',prio==sorted(prio))
c('has_all_domains',len({x['domain'] for x in reg['sources']})>=12)
c('project_tables_direct',all(x['scope_policy']=='direct_project' for x in reg['sources'] if 'project_uuid' in next(i['columns'] for i in inv['tables'] if i['source_table']==x['source_table'])))
# plan-only must run without psycopg/database
p=subprocess.run([sys.executable,str(tool),'--only-table','phx_experience_episodes','--plan-only'],capture_output=True,text=True);c('plan_only_no_db',p.returncode==0 and 'source_count' in p.stdout and 'phx_experience_episodes' in p.stdout and 'source_count' in p.stdout)
fail=[x for x in checks if not x['pass']];print(json.dumps({'suite':'v0.59 contracts','pass':len(checks)-len(fail),'fail':len(fail),'checks':checks},indent=2));sys.exit(1 if fail else 0)

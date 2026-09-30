#!/usr/bin/env python3
from pathlib import Path
import json,sys,re
base=Path(__file__).resolve().parents[1]
over=base/'overlay' if (base/'overlay').exists() else base
checks=[]
def c(name,ok): checks.append({'name':name,'pass':bool(ok)})
sql=(over/'migrations/0058_project_trace_tree.sql').read_text()
lib=(over/'crates/phxclaw-project-trace-tree/src/lib.rs').read_text()
cfg=json.loads((over/'config/project-traceability.v058.json').read_text())
caps=json.loads((over/'config/capabilities-v058.json').read_text())['capabilities']
c('capability_count_52',len(caps)==52)
c('parent_fk','parent_uuid' in sql and 'REFERENCES phx_project_trace_nodes' in sql)
c('materialized_uuid_path','path_ids uuid[]' in sql and 'USING gin(path_ids)' in sql)
c('full_text_index','search_document tsvector' in sql and 'USING gin(search_document)' in sql)
c('metadata_gin','jsonb_path_ops' in sql)
c('typed_links','phx_project_trace_links' in sql and "'supports','refutes','derived_from','caused_by'" in sql)
c('source_registry','phx_project_source_documents' in sql and 'content_sha256 char(64)' in sql)
c('source_commit_index','commit_sha' in sql and 'phx_project_source_documents_commit_idx' in sql)
c('bindings','phx_project_trace_bindings' in sql)
c('events_append_only','phx_project_trace_events_immutable' in sql)
c('nodes_append_only','phx_project_trace_nodes_immutable' in sql)
c('links_append_only','phx_project_trace_links_immutable' in sql)
c('sources_append_only','phx_project_source_documents_immutable' in sql)
c('rls_force',sql.count('FORCE ROW LEVEL SECURITY')>=5)
c('ancestors_function','phx_project_trace_ancestors' in sql)
c('descendants_function','phx_project_trace_descendants' in sql and 'a.depth + LEAST' in sql)
c('decision_trace_view','phx_project_decision_trace_v' in sql)
c('unified_search_function','phx_project_trace_search' in sql and 'websearch_to_tsquery' in sql)
c('secret_guard_rust','contains_secret_fields' in lib)
c('source_hash_validation','SourceHash' in lib)
c('max_depth_64',cfg['hierarchy']['max_depth']==64)
c('v057_bindings',cfg['bindings']['experience_ledger'] and cfg['bindings']['fruitful_unfruitful'] and cfg['bindings']['self_improvement'])
c('ui',(over/'ui/project-trace-tree.html').exists())
fail=[x for x in checks if not x['pass']]
out={'suite':'v0.58 static','pass':len(checks)-len(fail),'fail':len(fail),'checks':checks}
print(json.dumps(out,indent=2)); sys.exit(1 if fail else 0)

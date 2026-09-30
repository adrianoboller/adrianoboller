#!/usr/bin/env python3
from pathlib import Path
import json,tomllib,sys
R=Path(__file__).resolve().parents[1];checks=[]
def ck(n,o,d=''):checks.append((n,bool(o),d))
ws=tomllib.loads((R/'Cargo.toml').read_text());deps=ws['workspace']['dependencies'];ck('version',ws['workspace']['package']['version']=='0.63.0')
for k,v in {'tree-sitter':'=0.27.0','tree-sitter-rust':'=0.24.2','tree-sitter-python':'=0.25.0','tree-sitter-javascript':'=0.25.0','tree-sitter-typescript':'=0.23.2','tree-sitter-go':'=0.25.0','tree-sitter-c':'=0.24.2','tree-sitter-cpp':'=0.23.4','tree-sitter-java':'=0.23.5'}.items():ck('dep '+k,deps.get(k)==v,str(deps.get(k)))
s=(R/'crates/phxclaw-repo-intelligence/src/lib.rs').read_text()
for t in ['Parser::new','tree_sitter_rust::LANGUAGE','tree_sitter_python::LANGUAGE','LANGUAGE_TYPESCRIPT','tree_sitter_go::LANGUAGE','tree_sitter_c::LANGUAGE','tree_sitter_cpp::LANGUAGE','tree_sitter_java::LANGUAGE','RepoSymbol','RepoCall','RepoDependency','pagerank_ppm','ParsedWithErrors'] : ck('source '+t,t in s)
sql=(R/'migrations/0063_repo_intelligence_tree_sitter.sql').read_text()
for t in ['repo_analysis_runs','repo_symbols','repo_calls','repo_dependencies','repo_hotspots','FORCE ROW LEVEL SECURITY','append-only']:ck('sql '+t,t in sql)
c=json.loads((R/'config/project-state-canonical.v063.json').read_text());f=next(x for x in c['sprints'] if x['id']=='F19');ck('F19 source ready',f['implementation_state']=='source_ready' and not f['gaps']);ck('canonical 0.63',c['version']=='0.63.0')
try:json.loads((R/'schemas/repo-intelligence-v063.schema.json').read_text());ck('schema valid',True)
except Exception as e:ck('schema valid',False,str(e))
passed=sum(x[1] for x in checks);rep={'version':'0.63.0','pass':passed,'fail':len(checks)-passed,'checks':[{'name':n,'ok':o,'detail':d} for n,o,d in checks]};(R/'reports/V063_STATIC_VERIFY.json').write_text(json.dumps(rep,indent=2)+'\n');(R/'reports/V063_STATIC_VERIFY.md').write_text('# v0.63 Static Verify\n\n'+f'PASS {passed} / FAIL {len(checks)-passed}\n'+'\n'.join(f"- {'PASS' if o else 'FAIL'} — {n}" for n,o,d in checks)+'\n');print(json.dumps(rep,indent=2));sys.exit(1 if rep['fail'] else 0)

#!/usr/bin/env python3
from pathlib import Path
import json,re,sys
HERE=Path(__file__).resolve();ROOT=HERE.parents[1];OV=ROOT/'overlay' if (ROOT/'overlay').exists() else ROOT
checks=[]
def ck(n,c,d=''):checks.append({'name':n,'pass':bool(c),'detail':d})
def txt(p):return (OV/p).read_text(encoding='utf-8')
for p in ['config/capabilities-v051.json','schemas/portfolio-optimization-search.schema.json','schemas/portfolio-optimization-candidate.schema.json','schemas/portfolio-optimization-result.schema.json','schemas/portfolio-optimization-delegation.schema.json']:
    try:json.loads(txt(p));ck('json_'+p,True)
    except Exception as e:ck('json_'+p,False,str(e))
cap=json.loads(txt('config/capabilities-v051.json'));ck('capability_count',cap.get('count')==72 and len(cap.get('capabilities',[]))==72);ck('uuidv7_caps',all(re.match(r'^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$',x['uuid']) for x in cap['capabilities']))
rust=txt('crates/phxclaw-portfolio-optimizer/src/lib.rs')
for token in ['SearchSpace','HardConstraints','ObjectiveWeights','CandidateSpec','EvaluatedCandidate','SearchPlan','SearchResult','generate_candidates','pareto_frontier','dominates','utility','rank_stability','weight_rank_stability','optimize_coarse_to_fine','delegate_recommendation','assert_no_direct_mutation']:
    ck('rust_'+token,token in rust)
for phrase in ['DIRECT_MUTATION_ALLOWED: bool = false','executive_decision_center','MAX_CANDIDATES_HARD','WeightsRequired','InsufficientConfidence']:
    ck('authority_'+phrase[:18],phrase in rust)
cfgp='config/portfolio-optimizer.v051.json' if (ROOT/'overlay').exists() else 'config/phxclaw.config.json';cfg=json.loads(txt(cfgp));e=cfg['portfolio_optimizer'];ck('direct_mutation_false',e['direct_mutation'] is False);ck('pareto_enabled',e['objectives']['use_pareto_frontier'] is True);ck('weights_required',e['objectives']['require_explicit_weights_for_single_winner'] is True);ck('never_auto_execute',e['recommendation']['never_auto_execute'] is True);ck('unfruitful_promoted_only',e['knowledge']['unfruitful_promoted_as_exclusion'] is True);ck('search_budget',e['search']['max_candidates']<=25000 and e['search']['final_iterations']<=100000)
sql=txt('migrations/0051_portfolio_optimizer.sql');ck('sql_force_rls',sql.count('FORCE ROW LEVEL SECURITY')>=1);ck('sql_rls_exact',"nullif(current_setting('phxclaw.tenant_uuid', true), '')::uuid" in sql);ck('sql_append_only','phx_portfolio_optimizer_append_only' in sql);ck('sql_direct_mutation_false','direct_mutation boolean NOT NULL DEFAULT false CHECK (direct_mutation=false)' in sql);ck('sql_fk_search','REFERENCES phx_portfolio_optimization_searches' in sql)
ck('ui_exists',(OV/'ui/portfolio-optimizer.html').exists());ck('no_legacy_brand',not any(x in rust for x in ['PhoenixClaw','phoenixclaw','PHOENIXCLAW']))
passed=sum(x['pass'] for x in checks);failed=len(checks)-passed;rep={'suite':'v0.51 static','pass':passed,'fail':failed,'checks':checks};(ROOT/'reports').mkdir(exist_ok=True);(ROOT/'reports/V051_STATIC_REPORT.json').write_text(json.dumps(rep,indent=2)+'\n');print(json.dumps({'pass':passed,'fail':failed},indent=2));sys.exit(1 if failed else 0)

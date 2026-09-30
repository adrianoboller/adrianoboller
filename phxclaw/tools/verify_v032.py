#!/usr/bin/env python3
from pathlib import Path
import hashlib, json, sys, tomllib
HERE=Path(__file__).resolve(); PKG=HERE.parents[1]; ROOT=PKG/'overlay' if (PKG/'overlay/capabilities/V032_CAPABILITIES_DELTA.json').is_file() else PKG; REPORT=PKG/'reports' if (PKG/'overlay').exists() else ROOT/'reports'; checks=[]
def ck(n,o,d=''): checks.append({'name':n,'pass':bool(o),'detail':d}); print(('PASS' if o else 'FAIL'),n,d)
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
cap=json.loads((ROOT/'capabilities/V032_CAPABILITIES_DELTA.json').read_text()); ck('cap_delta',cap['delta_count']==26); ck('cap_total',cap['projected_total']==421); ck('cap_unique',len(set(cap['capabilities']))==26)
for c in ['phxclaw-ai-benchmark','phxclaw-adaptive-model-intelligence']:
 p=ROOT/f'crates/{c}/Cargo.toml'; ck('crate_'+c,p.is_file())
 if p.is_file():
  try: data=tomllib.loads(p.read_text()); ck('toml_'+c,True); ck('version_'+c,data['package']['version']=='0.32.0'); ck('unsafe_forbid_'+c,'unsafe_code = "forbid"' in p.read_text())
  except Exception as e: ck('toml_'+c,False,str(e))
bench=(ROOT/'crates/phxclaw-ai-benchmark/src/lib.rs').read_text()
for n,s in [('task_family','enum TaskFamily'),('complexity','enum ComplexityBand'),('dataset_hash','dataset_sha256'),('scorer_hash','scorer_sha256'),('environment_hash','environment_sha256'),('duplicate_guard','DuplicateObservation'),('aggregate','aggregate_profile'),('promotion','can_promote'),('coverage','evidence_coverage_basis_points'),('regression_quality','max_quality_regression_basis_points'),('regression_success','max_success_regression_basis_points'),('evidence_class','EvidenceClass'),('fixture_promotion_block','FixtureNotPromotable'),('promotion_freshness','StaleProfile')]: ck('benchmark_'+n,s in bench)
ck('benchmark_no_raw_prompt','prompt_content' not in bench and 'raw_prompt' not in bench); ck('benchmark_no_raw_output','output_content' not in bench and 'raw_output' not in bench)
adapt=(ROOT/'crates/phxclaw-adaptive-model-intelligence/src/lib.rs').read_text()
for n,s in [('base_router','route(now,&broad'),('profile_required','profile_required'),('freshness','fn fresh'),('samples','min_samples'),('coverage','min_coverage_basis_points'),('task_family','task_family'),('complexity','complexity_ok'),('deterministic_candidates','candidates.sort_by'),('decision_hash','adaptive_decision_sha256'),('base_eligible_only','eligible_keys.contains'),('promoted_profile_only','PromotedProfile'),('promotion_policy_hash','promoted.policy_sha256')]: ck('adaptive_'+n,s in adapt)
policy=json.loads((ROOT/'config/ai-benchmark-policy.v032.json').read_text()); ck('policy_no_raw_prompts',policy['store_raw_prompts'] is False); ck('policy_no_raw_outputs',policy['store_raw_outputs'] is False); ck('fixtures_never_promote','fixtures_never_promote' in policy['production_rules']); ck('stale_profiles_never_route','stale_profiles_never_route' in policy['production_rules'])
sql=(ROOT/'migrations/0032_ai_benchmark_adaptive.sql').read_text()
for n,s in [('force_rls','FORCE ROW LEVEL SECURITY'),('tenant_setting',"current_setting(''phxclaw.tenant_uuid'', true), '''')"),('suite_table','phxclaw_ai_benchmark_suites'),('observation_table','phxclaw_ai_benchmark_observations'),('profile_table','phxclaw_ai_performance_profiles'),('evidence_class_sql',"evidence_class text NOT NULL CHECK(evidence_class IN ('production','fixture'))"),('promotion_table','phxclaw_ai_profile_promotions'),('adaptive_evidence','phxclaw_ai_adaptive_route_evidence'),('composite_fk','FOREIGN KEY(tenant_uuid,suite_uuid,case_uuid)'),('unique_run_case','UNIQUE(tenant_uuid,run_uuid,case_uuid)'),('append_observation','ai_benchmark_observation_append_only'),('append_profile','ai_profile_append_only'),('append_promotion','ai_profile_promotion_append_only'),('append_adaptive','ai_adaptive_evidence_append_only')]: ck('sql_'+n,s in sql)
ck('sql_no_raw_prompt_column','raw_prompt' not in sql and 'prompt_content' not in sql); ck('sql_no_raw_output_column','raw_output' not in sql and 'output_content' not in sql)
# v0.31 migration defect must be fixed either in installed tree or bundled repair.
if (ROOT/'migrations/0031_unified_ai_fabric.sql').is_file(): fixed=ROOT/'migrations/0031_unified_ai_fabric.sql'
else:
 candidates=[PKG/'repairs/0031_unified_ai_fabric.sql.fixed',PKG.parent/'repairs/0031_unified_ai_fabric.sql.fixed']
 fixed=next((x for x in candidates if x.is_file()),candidates[0])
fs=fixed.read_text(); ck('v031_repair_present',"nullif(current_setting(''phxclaw.tenant_uuid'', true), '''')::uuid" in fs); ck('v031_bad_policy_removed',"current_setting(phxclaw.tenant_uuid" not in fs and "true), )::uuid" not in fs)
for s in (ROOT/'schemas').glob('*v032.schema.json'):
 try: json.loads(s.read_text()); ck('schema_'+s.name,True)
 except Exception as e: ck('schema_'+s.name,False,str(e))
doc=(ROOT/'docs/AI_BENCHMARK_ADAPTIVE_INTELLIGENCE_V032.md').read_text(); ck('doc_no_global_rank','No provider/model receives a permanent global rank' in doc); ck('doc_v031_authority','v0.31 router remains the authority' in doc); ck('doc_fixture_boundary','Fixture results validate' in doc)
ci=(ROOT/'ci/run-v032-native.sh').read_text(); ck('ci_locked','--locked' in ci); ck('ci_clippy','cargo clippy' in ci); ck('ci_tests','test_v032_local.py' in ci)
report={'suite':'PhxClaw v0.32 static verifier','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}; REPORT.mkdir(parents=True,exist_ok=True); (REPORT/'V032_STATIC_VERIFY_REPORT.json').write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps({'pass':report['pass'],'fail':report['fail']})); sys.exit(1 if report['fail'] else 0)

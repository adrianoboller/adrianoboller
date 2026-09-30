#!/usr/bin/env python3
from pathlib import Path
import json, re, sys, tomllib
root=Path(sys.argv[1] if len(sys.argv)>1 else '.').resolve()
if (root/'overlay').is_dir() and (root/'PACKAGE.json').exists(): base=root/'overlay'
else: base=root
checks=[]
def ck(name, cond, detail=''):
 checks.append({'name':name,'pass':bool(cond),'detail':detail}); print(('PASS ' if cond else 'FAIL ')+name+((' '+detail) if detail else ''))
req=['crates/phxclaw-autonomous-engineering/Cargo.toml','crates/phxclaw-autonomous-engineering/src/lib.rs','config/autonomous-engineering-policy.v037.json','config/engineering-stage-skill-map.v037.json','migrations/0037_autonomous_engineering_workflow.sql','schemas/engineering-workflow-v037.schema.json','schemas/engineering-stage-evidence-v037.schema.json','schemas/engineering-checkpoint-v037.schema.json','tests/sql/v037_rls_e2e.sql']
for x in req: ck('file/'+x,(base/x).is_file())
# parse json/toml
for p in base.rglob('*.json'):
 if 'reports' in p.relative_to(base).parts: continue
 try: json.load(open(p,encoding='utf-8')); ck('json/'+str(p.relative_to(base)),True)
 except Exception as e: ck('json/'+str(p.relative_to(base)),False,str(e))
for p in base.rglob('Cargo.toml'):
 try: tomllib.loads(p.read_text()); ck('toml/'+str(p.relative_to(base)),True)
 except Exception as e: ck('toml/'+str(p.relative_to(base)),False,str(e))
pol=json.load(open(base/'config/autonomous-engineering-policy.v037.json'))
sm=json.load(open(base/'config/engineering-stage-skill-map.v037.json'))
ck('ten_stages',len(pol['stages'])==10 and len(sm['stages'])==10)
ck('workflow_sequence',pol['stages']==['intake','research','code_map','hypothesis','plan','implement','test','review','explain','deliver'])
ck('checkpoint_before_mutation',pol['mutation']['checkpoint_before_mutation'] is True)
ck('arbitrary_shell_denied',pol['mutation']['arbitrary_shell'] is False)
ck('external_publish_human',pol['approvals']['external_publish']=='human_required')
ck('independent_review',pol['review']['independent_reviewer_for_critical'] is True)
ck('observations_not_truth',pol['knowledge']['observations_are_not_truth'] is True)
ck('skill_catalog_20',sm['catalog_count_expected']==20)
ids={i for s in sm['stages'] for i in s['skill_ids']}; ck('skill_ids_count',len(ids)==20, str(len(ids)))
# ensure exact 20 ids known from v0.36
known={'agent_reach','last_30_days','deep_research','user_research','qmd_search','graphify','ponytail','napkin','tech_debt_audit','understand_anything','ui_ux_pro_max','frontend_slides','scroll_world','visual_explainer','fireworks_tech_graph','claude_seo','humanizer','auto_research_sleep','video_shotcraft','ffmpeg_skill'}
ck('skill_ids_exact_v036',ids==known)
rs=(base/'crates/phxclaw-autonomous-engineering/src/lib.rs').read_text()
for token in ['EngineeringStage','Checkpoint','ApprovalProof','failure_action','validate_delivery','verify_transition_fencing','SelfApprovalForbidden','SourceStateMismatch']:
 ck('rust/'+token,token in rs)
sql=(base/'migrations/0037_autonomous_engineering_workflow.sql').read_text()
ck('rls_table_count',sql.count('FORCE ROW LEVEL SECURITY')==8,str(sql.count('FORCE ROW LEVEL SECURITY')))
ck('rls_current_setting_exact',"current_setting('phxclaw.tenant_uuid', true)" in sql)
ck('append_only_triggers',sql.count('_append_only BEFORE UPDATE OR DELETE')==6,str(sql.count('_append_only BEFORE UPDATE OR DELETE')))
ck('tenant_composite_fk',sql.count('FOREIGN KEY (tenant_uuid')>=8,str(sql.count('FOREIGN KEY (tenant_uuid')))
ck('no_shell_exec_contract','Command::new' not in rs and 'std::process' not in rs)
e2e=(base/'tests/sql/v037_rls_e2e.sql').read_text(); ck('e2e_non_super', 'rolsuper' in e2e and 'rolbypassrls' in e2e); ck('e2e_cross_tenant','cross-tenant RLS leak' in e2e)
cap=json.load(open(base/'capabilities/V037_CAPABILITIES_DELTA.json')); ck('cap_delta',cap['delta_count']==48); ck('cap_total',cap['projected_total']==644)
passed=sum(x['pass'] for x in checks); failed=len(checks)-passed
out={'suite':'PhxClaw v0.37 static verifier','pass':passed,'fail':failed,'checks':checks}
outp=(base/'reports/V037_STATIC_VERIFY_REPORT.json'); outp.parent.mkdir(parents=True,exist_ok=True); outp.write_text(json.dumps(out,indent=2)+'\n')
print(f'SUMMARY {passed} PASS / {failed} FAIL'); raise SystemExit(1 if failed else 0)

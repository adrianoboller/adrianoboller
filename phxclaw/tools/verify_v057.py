#!/usr/bin/env python3
from pathlib import Path
import json, sys, re
P=Path(__file__).resolve().parents[1]; O=P/'overlay' if (P/'overlay').exists() else P; checks=[]
def c(n,v,d=''): checks.append({'name':n,'pass':bool(v),'detail':d})
cfgp=O/'config/continuous-learning.v057.json'
if cfgp.exists(): cfg=json.loads(cfgp.read_text())
else:
    allcfg=json.loads((O/'config/phxclaw.config.json').read_text()); cfg={'continuous_learning':allcfg.get('continuous_learning',{})}
cl=cfg['continuous_learning']
c('enabled',cl.get('enabled') is True)
c('autonomy_l2',cl['autonomy']['default_level']==2)
c('core_auto_merge_off',cl['autonomy']['core_auto_merge'] is False)
c('never_autonomous',all(x in cl['autonomy']['never_autonomous'] for x in ['microkernel','constitution','security_policy','secret_broker','promotion_gate','release_policy']))
c('experience_append_only',cl['experience_ledger']['append_only'] is True)
c('fruitful_governed',cl['fruitful_knowledge']['reuse_governed_only_for_strong_preference'] is True)
c('unfruitful_context_guard',cl['unfruitful_knowledge']['block_only_when_governed_fresh_and_context_compatible'] is True)
c('fixture_no_promotion',cl['experiment_lab']['fixture_cannot_promote_production'] is True)
c('skill_human_gate',cl['skill_evolution']['automatic_promotion'] is False)
c('workflow_human_gate',cl['workflow_evolution']['automatic_promotion'] is False)
c('qualification_authority',cl['qualification_integration']['may_mark_sprint_green'] is False and cl['qualification_integration']['sprint_gate_reconciler_remains_authoritative'] is True)
cap=json.loads((O/'config/capabilities-v057.json').read_text()); c('cap_count_98',cap['count']==98 and len(cap['capabilities'])==98)
lib=(O/'crates/phxclaw-self-evolving-intelligence/src/lib.rs').read_text(); c('rust_fixture_guard','FixturePromotion' in lib and 'FIXTURE_CAN_PROMOTE_PRODUCTION: bool = false' in lib)
c('rust_core_guard','CORE_AUTO_MERGE: bool = false' in lib and 'CorePolicy' in lib)
c('rust_context_guard','unfruitful_guard_blocks' in lib and 'context_fingerprint_sha256 == required_context_sha256' in lib)
c('rust_prompt_auto_only','candidate.kind == CandidateKind::Prompt' in lib and 'candidate.risk == RiskLevel::Low' in lib)
sql=(O/'migrations/0057_self_evolving_intelligence.sql').read_text(); c('force_rls',sql.count('FORCE ROW LEVEL SECURITY')>=10)
c('rls_quoting',"current_setting('phx.tenant_uuid', true)" in sql)
c('append_only_triggers',sql.count('EXECUTE FUNCTION phx_v057_deny_mutation()')>=4)
c('self_improvement_no_auto_merge','core_auto_merge boolean NOT NULL DEFAULT false CHECK (core_auto_merge = false)' in sql)
for f in ['experience-episode-v057.schema.json','context-fingerprint-v057.schema.json','knowledge-pattern-v057.schema.json','evolution-candidate-v057.schema.json','promotion-decision-v057.schema.json','self-improvement-change-v057.schema.json']:
    c('schema_'+f,(O/'schemas'/f).exists())
c('ui',(O/'ui/learning-evolution.html').exists())
report={'suite':'PhxClaw v0.57 static','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}; print(json.dumps(report,indent=2)); sys.exit(1 if report['fail'] else 0)

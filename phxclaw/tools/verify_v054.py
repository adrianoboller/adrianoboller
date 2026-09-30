#!/usr/bin/env python3
from pathlib import Path
import json,re,sys
P=Path(__file__).resolve().parents[1]
O=P/'overlay' if (P/'overlay').exists() else P
checks=[]
def c(n,v,d=''): checks.append({'name':n,'pass':bool(v),'detail':d})
cap=json.loads((O/'config/capabilities-v054.json').read_text()); c('capability_count',len(cap['capabilities'])==78,str(len(cap['capabilities'])))
cfg_path=(O/'config/consolidation-performance.v054.json') if (O/'config/consolidation-performance.v054.json').exists() else (O/'config/phxclaw.config.json'); cfg=json.loads(cfg_path.read_text()); pc=cfg['consolidation_performance']
c('green_target',pc['release_focus']['target_green_sprints']==10)
c('feature_freeze',pc['release_focus']['freeze_net_new_feature_expansion'] is True)
c('ollama_loopback',pc['ollama']['endpoint'].startswith('http://127.0.0.1:'))
c('ollama_remote_blocked',pc['ollama']['remote_allowed'] is False)
c('structured_outputs',pc['ollama']['fast_lane']['structured_outputs_required'] is True and pc['prompt_compiler']['structured_output_required'] is True)
c('warm_pool',pc['ollama']['warm_pool']['enabled'] is True and pc['ollama']['warm_pool']['adaptive_keep_alive'] is True)
c('context_budget',set(pc['context_budget']['by_complexity_tokens'])=={'low','medium','high','extreme'})
c('semantic_cache_scope','source_state_sha256' in pc['semantic_cache']['scope'] and 'policy_sha256' in pc['semantic_cache']['scope'])
c('affinity',pc['agent_model_affinity']['min_samples']>=3)
c('outcome_failure_origin',pc['outcome_memory']['separate_model_failure_from_prompt_tool_context_failure'] is True)
rs=(O/'crates/phxclaw-performance-fabric/src/lib.rs').read_text()
for token in ['RestrictedRequiresLocal','supports_structured_output','AffinityProfile','ContextBudgetDecision','SemanticCacheKey','WarmPoolPlan','failure_origin','ExecutionTier::OllamaFast']:
 c('rust_'+re.sub(r'\W+','_',token),token in rs)
sql=(O/'migrations/0054_consolidation_performance.sql').read_text()
c('force_rls',sql.count('FORCE ROW LEVEL SECURITY')>=1 and "current_setting($q$phxclaw.tenant_uuid$q$" in sql)
c('append_only',sql.count('trg_append_only_v054')>=1)
manifest=json.loads((O/'config/consolidation-manifest.v054.json').read_text()); c('overlay_manifest_33',len(manifest['overlays'])==33)
for f in ['assemble_checkout_v054.py','ollama_probe_v054.py','native_qualify_v054.py','apply_v054.py']:
 c('tool_'+f,(P/'tools'/f).exists() or (O.parent/'tools'/f).exists())
failed=[x for x in checks if not x['pass']]; report={'suite':'v0.54 static','pass':len(checks)-len(failed),'fail':len(failed),'checks':checks};
print(json.dumps(report,indent=2)); sys.exit(1 if failed else 0)

#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import re
import uuid
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
results=[]

def check(name, ok, detail=None):
    results.append({"name":name,"status":"PASS" if ok else "FAIL","detail":detail})

# Workspace / code presence
cargo=(ROOT/'Cargo.toml').read_text(encoding='utf-8')
check('workspace.version', 'version = "0.20.0"' in cargo)
for rel in [
    'crates/phxclaw-agent-catalog/src/lib.rs',
    'crates/phxclaw-research-pipeline/src/lib.rs',
    'crates/phxclaw-skill-runtime/src/lib.rs',
    'crates/phxclaw-memory-context/src/lib.rs',
    'crates/phxclaw-source-registry/src/lib.rs',
    'migrations/0012_research_context_pipeline.sql',
]:
    check('artifact.'+Path(rel).stem, (ROOT/rel).is_file(), rel)

# Agent registry + routing prerequisites
agents={}
for p in sorted((ROOT/'config/agents').glob('*.agent.json')):
    d=json.loads(p.read_text(encoding='utf-8')); agents[d['name']]=d
check('agents.count', len(agents)==110, len(agents))
research=agents.get('Research Agent',{})
needed_agent_caps={'research.collect','knowledge.rust.read','knowledge.source_registry.read','context.compile','skill.read'}
check('research_agent.capabilities', needed_agent_caps.issubset(set(research.get('capabilities',[]))), sorted(needed_agent_caps-set(research.get('capabilities',[]))))
check('research_agent.source_acl', 'rust-official' in research.get('knowledge_sources',[]))

# Skill lazy index + canonical hash
idx=json.loads((ROOT/'config/skills/registry.index.json').read_text(encoding='utf-8'))
check('skill_index.count', len(idx.get('skills',[]))>=1, len(idx.get('skills',[])))
entry=next((x for x in idx.get('skills',[]) if x.get('name')=='rust.research.official'),None)
check('skill_index.rust', entry is not None, entry)
if entry:
    sp=ROOT/'config/skills'/entry['path']
    skill=json.loads(sp.read_text(encoding='utf-8'))
    expected=skill['sha256']; clone=dict(skill); clone['sha256']=''
    actual=hashlib.sha256(json.dumps(clone,ensure_ascii=False,separators=(',',':'),sort_keys=True).encode()).hexdigest()
    check('skill.hash', expected==actual, {'expected':expected,'actual':actual})
    check('skill.state', skill['state']=='validated', skill['state'])
    check('skill.source', 'rust-official' in skill.get('knowledge_sources',[]), skill.get('knowledge_sources'))
    check('skill.required_capabilities', needed_agent_caps.issubset(set(skill.get('required_capabilities',[]))), sorted(needed_agent_caps-set(skill.get('required_capabilities',[]))))
    bad_evidence=[e['uuid'] for e in skill.get('evidence',[]) if uuid.UUID(e['uuid']).version!=7]
    check('skill.evidence_uuidv7', not bad_evidence, bad_evidence)

# Development vs production policy must be explicit and fail-closed in production.
pcfg=json.loads((ROOT/'config/research-pipeline.json').read_text(encoding='utf-8'))
dev=pcfg['profiles']['development']; prod=pcfg['profiles']['production']
check('policy.dev_validated', dev['allow_validated_skills'] is True and dev['require_promoted_skills'] is False, dev)
check('policy.prod_promoted_only', prod['require_promoted_skills'] is True and prod['allow_validated_skills'] is False, prod)

# Deterministic offline retrieval reference test using the same scoring rules.
records=[json.loads(line) for line in (ROOT/'tests/fixtures/rust-offline-sample/index/documents.jsonl').read_text(encoding='utf-8').splitlines() if line.strip()]
query='How do I propagate Result errors in Rust?'
terms={x.lower() for x in re.split(r'[^\w-]+',query) if len(x)>=2}
phrase=query.strip().lower()
def score(r):
    title=r['title'].lower(); path=r['path'].lower(); text=r['text'].lower(); s=40 if phrase and phrase in text else 0
    for term in terms:
        if term in title: s+=12
        if term in path: s+=6
        s+=min(text.count(term),24)*2
    return s
ranked=sorted(((score(r),r) for r in records if score(r)>0), key=lambda x:(-x[0],x[1]['path']))
check('offline_search.hit', bool(ranked), len(ranked))
if ranked:
    check('offline_search.result_first', 'result' in ranked[0][1]['path'].lower() or 'result' in ranked[0][1]['title'].lower(), ranked[0])

# Event/evidence integration is declared in code.
pipe=(ROOT/'crates/phxclaw-research-pipeline/src/lib.rs').read_text(encoding='utf-8')
for token in ['research.pipeline','agent.runtime','skill.runtime','knowledge.source','context.compiler','evidence.ledger']:
    check('event.'+token, f'"{token}"' in pipe)
for token in ['route_named','resolve_for_query','search_offline','compile_for_query','evidence_ledger.append']:
    check('pipeline.'+token, token in pipe)

caps=json.loads((ROOT/'config/capability-catalog.json').read_text(encoding='utf-8'))
capnames={c['name'] for c in caps['capabilities']}
newcaps={'agent.catalog.route','skill.resolve.lazy','context.materialize','knowledge.offline.search','research.pipeline.prepare'}
check('capabilities.new', newcaps.issubset(capnames), sorted(newcaps-capnames))
check('capabilities.count', caps.get('count')==len(caps['capabilities']) and caps.get('count',0)>=141, caps.get('count'))

report={
    'version':'0.20.0',
    'generated_at':datetime.now(timezone.utc).isoformat(),
    'pass':sum(x['status']=='PASS' for x in results),
    'fail':sum(x['status']=='FAIL' for x in results),
    'skip':0,
    'results':results,
}
print(json.dumps(report,ensure_ascii=False,indent=2))
(ROOT/'RESEARCH_PIPELINE_TEST_REPORT.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
raise SystemExit(1 if report['fail'] else 0)

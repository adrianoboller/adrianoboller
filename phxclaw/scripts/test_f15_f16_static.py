#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import uuid
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
results=[]

def check(name, ok, detail=None):
    results.append({"name":name,"status":"PASS" if ok else "FAIL","detail":detail})

idx=json.loads((ROOT/'config/agents/registry.index.json').read_text(encoding='utf-8'))
agent_files=sorted((ROOT/'config/agents').glob('*.agent.json'))
check('agents.count', len(agent_files)==110, len(agent_files))
check('agents.index_count', idx.get('count')==110, idx.get('count'))

ids=[]; names=[]; bad_v7=[]; bad_source=[]
for p in agent_files:
    d=json.loads(p.read_text(encoding='utf-8'))
    names.append(d['name']); ids.append(d['uuid'])
    try:
        if uuid.UUID(d['uuid']).version != 7: bad_v7.append(d['name'])
    except Exception:
        bad_v7.append(d['name'])
    src=d.get('source',{})
    if src.get('sheet')!='Equipe 110' or not isinstance(src.get('row'),int): bad_source.append(d['name'])
check('agents.uuidv7', not bad_v7, bad_v7)
check('agents.uuid_unique', len(ids)==len(set(ids)), len(set(ids)))
check('agents.name_unique', len(names)==len(set(names)), len(set(names)))
check('agents.source_rows', not bad_source, bad_source)

rust=json.loads((ROOT/'config/knowledge-sources/rust-official.json').read_text(encoding='utf-8'))
expected={'Research Agent','Research Lead / Pesquisador PDCA','Documentador','Perséfone'}
allowed=set(rust.get('allowed_agents',[]))
check('rust_source.authoritative', rust.get('authoritative') is True, rust.get('authoritative'))
check('rust_source.mode_hybrid', rust.get('mode')=='hybrid', rust.get('mode'))
check('rust_source.required_capability', 'knowledge.rust.read' in rust.get('required_capabilities',[]), rust.get('required_capabilities'))
check('rust_source.allowed_agents', allowed==expected, sorted(allowed))
check('rust_source.verified_version', rust.get('verified_version')=='1.98.1', rust.get('verified_version'))

agents_by_name={json.loads(p.read_text(encoding='utf-8'))['name']:json.loads(p.read_text(encoding='utf-8')) for p in agent_files}
missing_access=[]
for name in expected:
    d=agents_by_name[name]
    if 'rust-official' not in d.get('knowledge_sources',[]) or 'knowledge.rust.read' not in d.get('capabilities',[]):
        missing_access.append(name)
check('rust_source.agent_access', not missing_access, missing_access)

for rel in ['scripts/sync_rust_offline_docs.sh','scripts/sync_rust_offline_docs.ps1','scripts/index_offline_docs.py']:
    check('offline.'+Path(rel).name, (ROOT/rel).exists(), rel)

caps=json.loads((ROOT/'config/capability-catalog.json').read_text(encoding='utf-8'))
capnames={c['name'] for c in caps['capabilities']}
required={'skill.read','skill.resolve','skill.resolve.lazy','skill.validate','skill.promote','memory.read','memory.write','context.compile','context.materialize','knowledge.source_registry.read','knowledge.rust.read','knowledge.offline.sync','knowledge.offline.search','agent.registry.generate','agent.catalog.route','research.pipeline.prepare'}
check('capability.f15_f16', required.issubset(capnames), sorted(required-capnames))
check('capability.count', caps.get('count')==len(caps.get('capabilities',[]))==141, caps.get('count'))

report={
    'version':'0.14.0',
    'generated_at':datetime.now(timezone.utc).isoformat(),
    'pass':sum(r['status']=='PASS' for r in results),
    'fail':sum(r['status']=='FAIL' for r in results),
    'skip':0,
    'results':results,
}
print(json.dumps(report,ensure_ascii=False,indent=2))
(ROOT/'F15_F16_TEST_REPORT.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
raise SystemExit(1 if report['fail'] else 0)

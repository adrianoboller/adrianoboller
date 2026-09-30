#!/usr/bin/env python3
from pathlib import Path
import json, re, hashlib, sys, uuid
ROOT=Path(__file__).resolve().parents[1]
passes=[]; fails=[]
def ok(name, cond, detail=''):
    (passes if cond else fails).append((name, detail))

def is_uuid7(s):
    try:
        u=uuid.UUID(s)
        return u.version==7 and u.variant==uuid.RFC_4122
    except Exception:
        return False

# Structural gates
required=[
 'MANIFEST.json','VERSION','README.md','crates/phxclaw-provenance-core/Cargo.toml',
 'crates/phxclaw-provenance-core/src/lib.rs','schemas/source-provenance.schema.json',
 'policies/source-ingestion-policy.json','inventory/source_inventory_2026-09-29.json',
 'sql/v060_source_provenance.sql','docs/ADR-0060-source-provenance-firewall.md',
 'docs/THIRD_PARTY_SOURCES_v060.md','integration/INTEGRATION_v060.md',
 'ui/PhxClaw_ProvenanceFirewall_UI_v060.html']
for p in required: ok('file:'+p,(ROOT/p).is_file())

man=json.loads((ROOT/'MANIFEST.json').read_text())
pol=json.loads((ROOT/'policies/source-ingestion-policy.json').read_text())
inv=json.loads((ROOT/'inventory/source_inventory_2026-09-29.json').read_text())
schema=json.loads((ROOT/'schemas/source-provenance.schema.json').read_text())
ok('manifest.version',man['version']=='0.60.0')
ok('manifest.uuidv7',is_uuid7(man['package_uuid']),man['package_uuid'])
ok('policy.uuidv7',is_uuid7(pol['policy_uuid']),pol['policy_uuid'])
ok('policy.fail_closed',pol.get('fail_closed') is True)
ok('policy.default_quarantine',pol.get('default_decision')=='QUARANTINE')
ok('inventory.uuidv7',is_uuid7(inv['inventory_uuid']),inv['inventory_uuid'])
ok('inventory.count',len(inv['sources'])==6,str(len(inv['sources'])))

counts={'ALLOW':0,'QUARANTINE':0,'DENY':0}
for src in inv['sources']:
    counts[src['decision']]+=1
    ok('uuidv7:'+src['source_name'],is_uuid7(src['record_uuid']),src['record_uuid'])
    ok('sha256:'+src['source_name'],bool(re.fullmatch(r'[a-f0-9]{64}',src['sha256'])))
    if src['decision']=='ALLOW':
        ok('allow-has-license:'+src['source_name'],src['license']['local_evidence'] is True)
        ok('allow-origin-known:'+src['source_name'],src['origin']['known'] is True)
        ok('allow-not-leak:'+src['source_name'],src.get('declared_leak') is False)
    if src['decision']=='DENY':
        ok('deny-has-hard-reason:'+src['source_name'],src.get('declared_leak') or src.get('redistribution_prohibited') or src['license']['class']=='proprietary')
ok('decision-counts',counts=={'ALLOW':2,'QUARANTINE':3,'DENY':1},str(counts))

# Contamination gate: prohibited source names may appear only in metadata/docs/UI, never under crates.
crate_text='\n'.join(p.read_text(errors='ignore') for p in (ROOT/'crates').rglob('*') if p.is_file())
ok('no-proprietary-source-import', 'claude-code-main' not in crate_text)
ok('no-unsafe-rust', '#![forbid(unsafe_code)]' in (ROOT/'crates/phxclaw-provenance-core/src/lib.rs').read_text())

# Self-integrity manifest
hashes={}
for p in sorted(ROOT.rglob('*')):
    if p.is_file() and p.name not in {'SHA256SUMS','VERIFY_REPORT.md'}:
        hashes[str(p.relative_to(ROOT))]=hashlib.sha256(p.read_bytes()).hexdigest()
(ROOT/'SHA256SUMS').write_text('\n'.join(f'{h}  {p}' for p,h in hashes.items())+'\n')

print(f'PASS {len(passes)} / FAIL {len(fails)}')
for name,detail in passes: print('PASS',name,detail)
for name,detail in fails: print('FAIL',name,detail)
report=['# PhxClaw v0.60 verifier report','',f'PASS: {len(passes)}',f'FAIL: {len(fails)}','']
report += [f'- PASS `{n}` {d}'.rstrip() for n,d in passes]
report += [f'- FAIL `{n}` {d}'.rstrip() for n,d in fails]
(ROOT/'VERIFY_REPORT.md').write_text('\n'.join(report)+'\n')
sys.exit(1 if fails else 0)

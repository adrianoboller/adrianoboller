#!/usr/bin/env python3
from pathlib import Path
import base64, hashlib, json, sys, tomllib, uuid
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
HERE=Path(__file__).resolve(); PKG=HERE.parents[1]; ROOT=PKG/'overlay' if (PKG/'overlay/capabilities/V031_CAPABILITIES_DELTA.json').is_file() else PKG; REPORT=PKG/'reports' if (PKG/'overlay').exists() else ROOT/'reports'; checks=[]
def ck(n,o,d=''): checks.append({'name':n,'pass':bool(o),'detail':d}); print(('PASS' if o else 'FAIL'),n,d)
cap=json.loads((ROOT/'capabilities/V031_CAPABILITIES_DELTA.json').read_text()); ck('cap_delta',cap['delta_count']==28); ck('cap_total',cap['projected_total']==395); ck('cap_unique',len(set(cap['capabilities']))==28)
for c in ['phxclaw-ai-fabric','phxclaw-openai-provider','phxclaw-anthropic-provider','phxclaw-gemini-provider']:
 p=ROOT/f'crates/{c}/Cargo.toml'; ck('crate_'+c,p.is_file());
 if p.is_file():
  try: tomllib.loads(p.read_text()); ck('toml_'+c,True)
  except Exception as e: ck('toml_'+c,False,str(e))
ai=(ROOT/'crates/phxclaw-ai-fabric/src/lib.rs').read_text();
for n,s in [('restricted','restricted_local_only'),('confidential','confidential_cloud_requires_explicit_allow'),('fresh_catalog','stale_catalog'),('fresh_health','stale_health'),('budget','budget_limit'),('unknown_cost','unknown_cost'),('circuit','circuit_not_closed'),('fallback','fallback_chain'),('prompt_hash','prompt_sha256'),('evidence','evidence_projection'),('deterministic','eligible.sort_by'),('decision_canonical_sort','decisions.sort_by'),('complexity_gate','reasoning_tier_too_low')]: ck('ai_'+n,s in ai)
ck('ai_no_prompt_content','prompt_content' not in ai)
sql=(ROOT/'migrations/0031_unified_ai_fabric.sql').read_text();
for n,s in [('force_rls','FORCE ROW LEVEL SECURITY'),('tenant_setting',"current_setting(''phxclaw.tenant_uuid'', true)"),('budget_reserve','phxclaw_ai_reserve_budget'),('budget_settle','phxclaw_ai_settle_budget'),('row_lock','FOR UPDATE'),('budget_idempotent',"existing.state IN ('reserved','settled')"),('settle_idempotent',"IF r.state='settled'"),('append_health','ai_health_append_only'),('append_route','ai_route_append_only'),('append_events','ai_events_append_only')]: ck('sql_'+n,s in sql)
for c in ['openai','anthropic','gemini']:
 src=(ROOT/f'crates/phxclaw-{c}-provider/src/lib.rs').read_text(); ck(c+'_https','u.scheme()!="https"' in src); ck(c+'_userinfo','u.username().is_empty()' in src); ck(c+'_secret_uuid','secret_uuid:Uuid' in src); ck(c+'_no_raw_key','api_key' not in src.lower() and 'sk-' not in src.lower())
ck('openai_responses','/v1/responses' in (ROOT/'crates/phxclaw-openai-provider/src/lib.rs').read_text()); ck('openai_store_false','"store":false' in (ROOT/'crates/phxclaw-openai-provider/src/lib.rs').read_text())
ck('anthropic_messages','/v1/messages' in (ROOT/'crates/phxclaw-anthropic-provider/src/lib.rs').read_text()); ck('anthropic_version','anthropic-version' in (ROOT/'crates/phxclaw-anthropic-provider/src/lib.rs').read_text())
ck('gemini_interactions','/v1beta/interactions' in (ROOT/'crates/phxclaw-gemini-provider/src/lib.rs').read_text()); ck('gemini_embed',':embedContent' in (ROOT/'crates/phxclaw-gemini-provider/src/lib.rs').read_text())
for d in ['openai-provider','anthropic-provider','gemini-provider']:
 m=json.loads((ROOT/f'plugins/{d}/phxclaw.plugin.json').read_text()); integ=json.loads((ROOT/f'plugins/{d}/SOURCE_INTEGRITY.json').read_text());
 try: u=uuid.UUID(m['plugin_uuid']); ok=u.version==7 and u.variant==uuid.RFC_4122
 except: ok=False
 ck('uuidv7_'+d,ok); ck('semver_'+d,m['version']=='0.31.0'); ck('no_vendor_'+d,m['provenance']['source_tree_vendored'] is False); payload=json.dumps(m,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode(); ck('digest_'+d,hashlib.sha256(payload).hexdigest()==integ['canonical_manifest_sha256'])
 try: Ed25519PublicKey.from_public_bytes(base64.b64decode(integ['signature']['public_key_b64'])).verify(base64.b64decode(integ['signature']['signature_b64']),payload); ok=True
 except: ok=False
 ck('signature_'+d,ok and integ['private_key_embedded'] is False and integ['production_release_requires_resign'] is True)
for s in (ROOT/'schemas').glob('*v031.schema.json'):
 try: json.loads(s.read_text()); ck('schema_'+s.name,True)
 except Exception as e: ck('schema_'+s.name,False,str(e))
report={'suite':'PhxClaw v0.31 static verifier','pass':sum(x['pass'] for x in checks),'fail':sum(not x['pass'] for x in checks),'checks':checks}; REPORT.mkdir(parents=True,exist_ok=True); (REPORT/'V031_STATIC_VERIFY_REPORT.json').write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps({'pass':report['pass'],'fail':report['fail']})); sys.exit(1 if report['fail'] else 0)

import {canonicalName,assertUnifiedSqlModel} from './model/unified-model.js';

function mapBy(items,keyFn){const m=new Map();for(const x of items)m.set(keyFn(x),x);return m}
function stable(v){return JSON.stringify(v,Object.keys(v||{}).sort())}
function columnFingerprint(c){return stable({data_type:c.data_type||'',nullable:!!c.nullable,default:c.default??c.default_expr??null,primary_key:!!c.primary_key,unique:!!c.unique,generated_expression:c.generated_expression??null,generated_kind:c.generated_kind??null})}
function checkFingerprint(c){return `${canonicalName(c.name||'')}:${String(c.expression||'').replace(/\s+/g,' ').trim().toLowerCase()}`}
function tableFingerprint(t){return stable({columns:(t.columns||[]).map(c=>[canonicalName(c.name),columnFingerprint(c)]),primary_key:(t.primary_key||[]).map(canonicalName),checks:(t.check_constraints||[]).map(checkFingerprint).sort(),partitioning:t.partitioning||null})}
function objectKey(o){return `${o.kind}:${canonicalName(o.name)}`}
function relationKey(r){return `${canonicalName(r.from_table)}(${(r.from_columns||[]).map(canonicalName).join(',')})>${canonicalName(r.to_table)}(${(r.to_columns||[]).map(canonicalName).join(',')})`}

export function diffSchemas(expected,live){
  assertUnifiedSqlModel(expected);assertUnifiedSqlModel(live);
  const out={contract_version:'1.1',expected_source:expected.source||expected.source_name||'arquivo SQL',live_source:live.source||live.source_name||'banco vivo',summary:{added_tables:0,removed_tables:0,changed_tables:0,added_objects:0,removed_objects:0,added_relationships:0,removed_relationships:0},tables:{added:[],removed:[],changed:[]},objects:{added:[],removed:[]},relationships:{added:[],removed:[]}};
  const a=mapBy(expected.tables||[],t=>canonicalName(t.name)),b=mapBy(live.tables||[],t=>canonicalName(t.name));
  for(const [k,t] of b)if(!a.has(k))out.tables.added.push(t.name);
  for(const [k,t] of a)if(!b.has(k))out.tables.removed.push(t.name);
  for(const [k,ta] of a){const tb=b.get(k);if(!tb)continue;if(tableFingerprint(ta)!==tableFingerprint(tb)){
    const ca=mapBy(ta.columns||[],c=>canonicalName(c.name)),cb=mapBy(tb.columns||[],c=>canonicalName(c.name));const changes={table:ta.name,columns:{added:[],removed:[],changed:[]},checks_changed:stable(ta.check_constraints||[])!==stable(tb.check_constraints||[]),partitioning_changed:stable(ta.partitioning||null)!==stable(tb.partitioning||null)};
    for(const [ck,c] of cb)if(!ca.has(ck))changes.columns.added.push(c.name);for(const [ck,c] of ca)if(!cb.has(ck))changes.columns.removed.push(c.name);for(const [ck,c] of ca)if(cb.has(ck)&&columnFingerprint(c)!==columnFingerprint(cb.get(ck)))changes.columns.changed.push(c.name);
    out.tables.changed.push(changes);
  }}
  const oa=mapBy((expected.objects||[]).filter(o=>o.kind!=='table'),objectKey),ob=mapBy((live.objects||[]).filter(o=>o.kind!=='table'),objectKey);for(const[k,o]of ob)if(!oa.has(k))out.objects.added.push({kind:o.kind,name:o.name});for(const[k,o]of oa)if(!ob.has(k))out.objects.removed.push({kind:o.kind,name:o.name});
  const ra=mapBy(expected.relationships||[],relationKey),rb=mapBy(live.relationships||[],relationKey);for(const[k,r]of rb)if(!ra.has(k))out.relationships.added.push(k);for(const[k,r]of ra)if(!rb.has(k))out.relationships.removed.push(k);
  Object.assign(out.summary,{added_tables:out.tables.added.length,removed_tables:out.tables.removed.length,changed_tables:out.tables.changed.length,added_objects:out.objects.added.length,removed_objects:out.objects.removed.length,added_relationships:out.relationships.added.length,removed_relationships:out.relationships.removed.length});
  out.summary.total_changes=Object.values(out.summary).reduce((a,b)=>a+(typeof b==='number'?b:0),0);
  return out;
}

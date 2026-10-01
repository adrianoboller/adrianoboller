// Parser-only helpers. No DOM/UI/rendering dependency.
const IDENT_PART = String.raw`(?:\[[^\]\r\n]+\]|` + '`[^`\r\n]+`' + String.raw`|"(?:[^"]|"")+"|[A-Za-z_][A-Za-z0-9_$#@]*)`;
export const IDENT_PATTERN = `${IDENT_PART}(?:\\s*\\.\\s*${IDENT_PART})*`;

export function splitQualifiedIdentifier(value='') {
  const text=String(value).trim(); const out=[]; let start=0, mode=null;
  for(let i=0;i<text.length;i++){
    const c=text[i];
    if(mode==='bracket'){if(c===']')mode=null;continue}
    if(mode==='backtick'){if(c==='`'){if(text[i+1]==='`'){i++;continue}mode=null}continue}
    if(mode==='double'){if(c==='"'){if(text[i+1]==='"'){i++;continue}mode=null}continue}
    if(c==='['){mode='bracket';continue} if(c==='`'){mode='backtick';continue} if(c==='"'){mode='double';continue}
    if(c==='.'){out.push(text.slice(start,i).trim());start=i+1}
  }
  out.push(text.slice(start).trim()); return out.filter(Boolean);
}
export function unquoteIdentifierPart(value=''){
  const s=String(value).trim();
  if(s.startsWith('[')&&s.endsWith(']'))return s.slice(1,-1).replaceAll(']]',']');
  if(s.startsWith('`')&&s.endsWith('`'))return s.slice(1,-1).replaceAll('``','`');
  if(s.startsWith('"')&&s.endsWith('"'))return s.slice(1,-1).replaceAll('""','"');
  return s;
}
export function cleanIdentifier(value=''){return splitQualifiedIdentifier(value).map(unquoteIdentifierPart).join('.')}
export function canonicalIdentifier(value=''){return cleanIdentifier(value).toLowerCase()}
export function parseIdentList(list=''){return splitTopLevel(list,',').map(v=>cleanIdentifier(String(v).trim().replace(/\s+(?:ASC|DESC)\s*$/i,''))).filter(Boolean)}
export function lineNumberAt(sql,offset){let n=1;for(let i=0;i<Math.min(offset,sql.length);i++)if(sql.charCodeAt(i)===10)n++;return n}
export function parseMigrations(sql=''){const out=[];const re=/^--\s*MIGRATION\s+(\d+):\s*([^\r\n]+)/gim;let m;while((m=re.exec(sql)))out.push({version:+m[1],name:m[2].trim(),line:lineNumberAt(sql,m.index)});return out}
export function migrationForLine(ms,line){let v=null;for(const m of ms){if(m.line<=line)v=m.version;else break}return v}

export function findMatchingParen(text,open){let d=0,mode=null,block=false,line=false;for(let i=open;i<text.length;i++){const c=text[i],n=text[i+1];if(line){if(c==='\n')line=false;continue}if(block){if(c==='*'&&n==='/'){block=false;i++}continue}if(mode==='single'){if(c==="'"){if(n==="'"){i++;continue}mode=null}continue}if(mode==='double'){if(c==='"'){if(n==='"'){i++;continue}mode=null}continue}if(mode==='backtick'){if(c==='`'){if(n==='`'){i++;continue}mode=null}continue}if(mode==='bracket'){if(c===']'){if(n===']'){i++;continue}mode=null}continue}if(c==='-'&&n==='-'){line=true;i++;continue}if(c==='/'&&n==='*'){block=true;i++;continue}if(c==="'"){mode='single';continue}if(c==='"'){mode='double';continue}if(c==='`'){mode='backtick';continue}if(c==='['){mode='bracket';continue}if(c==='(')d++;else if(c===')'&&--d===0)return i}return text.length-1}
export function findStatementEnd(sql,from,{goBatch=false}={}){let mode=null,block=false,line=false,d=0;for(let i=Math.max(0,from);i<sql.length;i++){const c=sql[i],n=sql[i+1];if(line){if(c==='\n')line=false;continue}if(block){if(c==='*'&&n==='/'){block=false;i++}continue}if(mode==='single'){if(c==="'"){if(n==="'"){i++;continue}mode=null}continue}if(mode==='double'){if(c==='"'){if(n==='"'){i++;continue}mode=null}continue}if(mode==='backtick'){if(c==='`'){if(n==='`'){i++;continue}mode=null}continue}if(mode==='bracket'){if(c===']'){if(n===']'){i++;continue}mode=null}continue}if(c==='-'&&n==='-'){line=true;i++;continue}if(c==='/'&&n==='*'){block=true;i++;continue}if(c==="'"){mode='single';continue}if(c==='"'){mode='double';continue}if(c==='`'){mode='backtick';continue}if(c==='['){mode='bracket';continue}if(c==='('){d++;continue}if(c===')'){d=Math.max(0,d-1);continue}if(d===0&&c===';')return i+1;if(goBatch&&d===0&&c==='\n'&&/^\s*GO\s*(?:--[^\r\n]*)?(?:\r?\n|$)/i.test(sql.slice(i+1)))return i+1}return sql.length}
export function splitTopLevel(text,sep=','){const out=[];let st=0,d=0,mode=null,block=false,line=false;for(let i=0;i<text.length;i++){const c=text[i],n=text[i+1];if(line){if(c==='\n')line=false;continue}if(block){if(c==='*'&&n==='/'){block=false;i++}continue}if(mode==='single'){if(c==="'"){if(n==="'"){i++;continue}mode=null}continue}if(mode==='double'){if(c==='"'){if(n==='"'){i++;continue}mode=null}continue}if(mode==='backtick'){if(c==='`'){if(n==='`'){i++;continue}mode=null}continue}if(mode==='bracket'){if(c===']'){if(n===']'){i++;continue}mode=null}continue}if(c==='-'&&n==='-'){line=true;i++;continue}if(c==='/'&&n==='*'){block=true;i++;continue}if(c==="'"){mode='single';continue}if(c==='"'){mode='double';continue}if(c==='`'){mode='backtick';continue}if(c==='['){mode='bracket';continue}if(c==='('){d++;continue}if(c===')'){d=Math.max(0,d-1);continue}if(d===0&&c===sep){out.push(text.slice(st,i));st=i+1}}out.push(text.slice(st));return out}
function keywordAt(text,i,k){const chunk=text.slice(i,i+k.length);if(chunk.toUpperCase()!==k)return false;const b=i===0?' ':text[i-1],a=text[i+k.length]||' ';return !/[A-Za-z0-9_$#@]/.test(b)&&!/[A-Za-z0-9_$#@]/.test(a)}
export function findTopLevelKeyword(text,keywords,from=0){const ks=[...keywords].map(k=>k.toUpperCase()).sort((a,b)=>b.length-a.length);let d=0,mode=null;for(let i=from;i<text.length;i++){const c=text[i],n=text[i+1];if(mode==='single'){if(c==="'"){if(n==="'"){i++;continue}mode=null}continue}if(mode==='double'){if(c==='"'){if(n==='"'){i++;continue}mode=null}continue}if(mode==='backtick'){if(c==='`'){mode=null}continue}if(mode==='bracket'){if(c===']'){mode=null}continue}if(c==="'"){mode='single';continue}if(c==='"'){mode='double';continue}if(c==='`'){mode='backtick';continue}if(c==='['){mode='bracket';continue}if(c==='('){d++;continue}if(c===')'){d=Math.max(0,d-1);continue}if(d===0)for(const k of ks)if(keywordAt(text,i,k))return{index:i,keyword:k}}return null}
export function readLeadingIdentifier(text=''){const m=String(text).match(new RegExp(`^\\s*(${IDENT_PATTERN})`,'i'));return m?{raw:m[1],name:cleanIdentifier(m[1]),rest:String(text).slice(m[0].length)}:null}
export function parseOnDelete(tail=''){const m=String(tail).match(/\bON\s+DELETE\s+(CASCADE|RESTRICT|SET\s+NULL|SET\s+DEFAULT|NO\s+ACTION)\b/i);return m?m[1].toUpperCase().replace(/\s+/g,' '):null}
export function extractDefault(c='',keywords=[]){const m=String(c).match(/\bDEFAULT\b/i);if(!m)return null;const rest=c.slice(m.index+m[0].length),hit=findTopLevelKeyword(rest,keywords.filter(k=>k.toUpperCase()!=='DEFAULT')),v=(hit?rest.slice(0,hit.index):rest).trim();return v||null}
export function relationshipSignature(fk){return `${canonicalIdentifier(fk.from_table)}(${(fk.from_columns||[]).map(canonicalIdentifier).join(',')})>${canonicalIdentifier(fk.to_table)}(${(fk.to_columns||[]).map(canonicalIdentifier).join(',')})`}

export function extractParenthesizedAfter(text, keywordRegex) {
  const m=String(text).match(keywordRegex); if(!m)return null;
  const open=String(text).indexOf('(',m.index+m[0].length-1); if(open<0)return null;
  const close=findMatchingParen(String(text),open); if(close<=open)return null;
  return {expression:String(text).slice(open+1,close).trim(),open,close};
}
export function extractGeneratedDefinition(cons='',dialect='generic'){
  const text=String(cons); let m;
  if(dialect==='sqlserver'){
    m=text.match(/\bAS\s*\(/i); if(!m)return null;
    const ex=extractParenthesizedAfter(text,/\bAS\s*\(/i); if(!ex)return null;
    return {expression:ex.expression,kind:/\bPERSISTED\b/i.test(text.slice(ex.close+1))?'stored':'virtual'};
  }
  m=text.match(/\b(?:GENERATED\s+ALWAYS\s+)?AS\s*\(/i); if(!m)return null;
  const ex=extractParenthesizedAfter(text,/\b(?:GENERATED\s+ALWAYS\s+)?AS\s*\(/i); if(!ex)return null;
  const tail=text.slice(ex.close+1); let kind=null;
  if(/\bSTORED\b/i.test(tail))kind='stored'; else if(/\bVIRTUAL\b/i.test(tail))kind='virtual';
  return {expression:ex.expression,kind};
}
export function extractCheckDefinitions(item='', {column=null, table=null, line=0}={}){
  const text=String(item),out=[]; let cursor=0,index=0;
  while(cursor<text.length){const sub=text.slice(cursor),m=sub.match(/(?:\bCONSTRAINT\s+([^\s]+)\s+)?\bCHECK\s*\(/i);if(!m)break;const abs=cursor+m.index,open=text.indexOf('(',abs+m[0].length-1);if(open<0)break;const close=findMatchingParen(text,open);if(close<=open)break;out.push({name:m[1]?cleanIdentifier(m[1]):null,expression:text.slice(open+1,close).trim(),column:column||null,table:table||null,line,index:index++});cursor=close+1;}
  return out;
}

export function parseGenericTables(sql,{createTableRegex,constraintKeywords=[],goBatch=false,alterForeignKeyRegex=null,dialect='generic'}={}){
  const migrations=parseMigrations(sql),tables=[],relationships=[];let m;const re=new RegExp(createTableRegex.source,createTableRegex.flags.includes('g')?createTableRegex.flags:createTableRegex.flags+'g');
  while((m=re.exec(sql))){
    const rawName=m.groups?.name||m[1]; if(!rawName)continue;
    const name=cleanIdentifier(rawName),open=sql.indexOf('(',m.index+m[0].length-1); if(open<0)continue;
    const close=findMatchingParen(sql,open),end=findStatementEnd(sql,close+1,{goBatch}),line=lineNumberAt(sql,m.index),body=sql.slice(open+1,close),columns=[],primary=[],fks=[],checks=[];
    for(const raw of splitTopLevel(body,',')){
      const item=raw.trim(); if(!item)continue;
      const pk=item.match(new RegExp(`(?:^|\\s)(?:CONSTRAINT\\s+${IDENT_PATTERN}\\s+)?PRIMARY\\s+KEY(?:\\s+(?:CLUSTERED|NONCLUSTERED))?\\s*\\(([^)]*)\\)`,'i'));
      if(pk)for(const c of parseIdentList(pk[1]))if(!primary.some(x=>canonicalIdentifier(x)===canonicalIdentifier(c)))primary.push(c);
      const fk=item.match(new RegExp(`^(?:CONSTRAINT\\s+(${IDENT_PATTERN})\\s+)?FOREIGN\\s+KEY\\s*\\(([^)]*)\\)\\s+REFERENCES\\s+(${IDENT_PATTERN})\\s*\\(([^)]*)\\)([\\s\\S]*)$`,'i'));
      if(fk){const r={constraint:fk[1]?cleanIdentifier(fk[1]):null,from_table:name,from_columns:parseIdentList(fk[2]),to_table:cleanIdentifier(fk[3]),to_columns:parseIdentList(fk[4]),on_delete:parseOnDelete(fk[5]),line};fks.push(r);relationships.push(r);continue}
      if(/^(?:CONSTRAINT\b.*\bCHECK\b|CHECK\b)/i.test(item)){checks.push(...extractCheckDefinitions(item,{table:name,line}));continue}
      if(/^(?:CONSTRAINT\b|PRIMARY\s+KEY\b|FOREIGN\s+KEY\b|UNIQUE\b)/i.test(item))continue;
      const lead=readLeadingIdentifier(item); if(!lead)continue; const rest=lead.rest.trim(); if(!rest)continue;
      let hit=findTopLevelKeyword(rest,constraintKeywords),dtype=(hit?rest.slice(0,hit.index):rest).trim(); if(!dtype&&dialect==='sqlserver'&&/^AS\b/i.test(rest))dtype='computed'; if(!dtype)continue; const cons=hit?rest.slice(hit.index):'';
      const isPk=/\bPRIMARY\s+KEY\b/i.test(cons),unique=/\bUNIQUE\b/i.test(cons),generated=extractGeneratedDefinition(cons,dialect);
      if(dialect==='sqlserver'&&generated){const asPos=rest.search(/\bAS\s*\(/i);dtype=asPos>=0?rest.slice(0,asPos).trim():dtype;}
      const col={name:lead.name,data_type:dtype,nullable:!(/\bNOT\s+NULL\b/i.test(cons)||isPk),default:extractDefault(cons,constraintKeywords),primary_key:isPk,unique,generated_expression:generated?.expression||null,generated_kind:generated?.kind||null};
      if(isPk&&!primary.some(x=>canonicalIdentifier(x)===canonicalIdentifier(lead.name)))primary.push(lead.name);
      checks.push(...extractCheckDefinitions(cons,{column:lead.name,table:name,line}));
      const ir=cons.match(new RegExp(`\\bREFERENCES\\s+(${IDENT_PATTERN})\\s*\\(([^)]*)\\)([\\s\\S]*)$`,'i'));
      if(ir){const r={constraint:null,from_table:name,from_columns:[lead.name],to_table:cleanIdentifier(ir[1]),to_columns:parseIdentList(ir[2]),on_delete:parseOnDelete(ir[3]),line};fks.push(r);relationships.push(r)}
      columns.push(col);
    }
    const pset=new Set(primary.map(canonicalIdentifier));for(const c of columns)if(pset.has(canonicalIdentifier(c.name))){c.primary_key=true;c.nullable=false}
    const ddl=sql.slice(m.index,end).trim(); const tail=sql.slice(close+1,end); let partitioning=null;
    if(dialect==='mysql'){const pm=tail.match(/\bPARTITION\s+BY\s+([\s\S]*?)(?=;|$)/i);if(pm)partitioning={kind:'mysql',expression:pm[1].trim(),parent:null,bound:null};}
    else if(dialect==='postgresql'){const pm=tail.match(/\bPARTITION\s+BY\s+(RANGE|LIST|HASH)\s*\(([\s\S]*?)\)/i);if(pm)partitioning={kind:pm[1].toLowerCase(),expression:pm[2].trim(),parent:null,bound:null};}
    else if(dialect==='sqlserver'){const pm=tail.match(/\bON\s+([^;]+?)(?:\s*;|$)/i);if(pm&&/\w+\s*\([^)]*\)/.test(pm[1]))partitioning={kind:'sqlserver',expression:pm[1].trim(),parent:null,bound:null};}
    tables.push({name,line,end_line:lineNumberAt(sql,Math.max(m.index,end-1)),migration:migrationForLine(migrations,line),columns,primary_key:primary,foreign_keys:fks,check_constraints:checks,partitioning,statistics:null,ddl,degree:0});re.lastIndex=Math.max(re.lastIndex,end)
  }
  if(alterForeignKeyRegex){const ar=new RegExp(alterForeignKeyRegex.source,alterForeignKeyRegex.flags.includes('g')?alterForeignKeyRegex.flags:alterForeignKeyRegex.flags+'g'),sigs=new Set(relationships.map(relationshipSignature));while((m=ar.exec(sql))){const g=m.groups||{},r={constraint:cleanIdentifier(g.constraint||m[2]||'')||null,from_table:cleanIdentifier(g.from||m[1]),from_columns:parseIdentList(g.fromCols||m[3]),to_table:cleanIdentifier(g.to||m[4]),to_columns:parseIdentList(g.toCols||m[5]),on_delete:parseOnDelete(g.tail||m[6]||''),line:lineNumberAt(sql,m.index)},sig=relationshipSignature(r);if(!sigs.has(sig)){sigs.add(sig);relationships.push(r)}}}
  const map=new Map(tables.map(t=>[canonicalIdentifier(t.name),t]));for(const r of relationships){const a=map.get(canonicalIdentifier(r.from_table));if(a&&!a.foreign_keys.some(x=>relationshipSignature(x)===relationshipSignature(r)))a.foreign_keys.push(r);if(a)a.degree++;const b=map.get(canonicalIdentifier(r.to_table));if(b)b.degree++}
  return{migrations,tables,relationships};
}
export function collectObjects(sql,migrations,specs,{goBatch=false}={}){const out=[];for(const spec of specs){const re=new RegExp(spec.regex.source,spec.regex.flags.includes('g')?spec.regex.flags:spec.regex.flags+'g');let m;while((m=re.exec(sql))){const rawName=m.groups?.name||m[spec.nameGroup||1];if(!rawName)continue;const line=lineNumberAt(sql,m.index),end=findStatementEnd(sql,m.index+m[0].length,{goBatch}),raw=sql.slice(m.index,end);let target=null;if(spec.targetRegex){const tm=raw.match(spec.targetRegex);if(tm)target=cleanIdentifier(tm.groups?.target||tm[1])}const metadata=spec.metadata?.(raw,m)||{};out.push({kind:spec.kind,name:cleanIdentifier(rawName),line,migration:migrationForLine(migrations,line),target,raw_sql:raw.trim(),metadata})}}return out.sort((a,b)=>a.line-b.line||a.name.localeCompare(b.name))}
export function finalizeModel(sql,source,{migrations,tables,relationships,objects,capabilities={}}){
  const kinds=k=>objects.filter(o=>o.kind===k).length;
  const stats={lines:sql.split(/\r?\n/).length-(sql.endsWith('\n')?1:0),bytes:new TextEncoder().encode(sql).length,tables:tables.length,columns:tables.reduce((n,t)=>n+t.columns.length,0),foreign_keys:relationships.length,check_constraints:tables.reduce((n,t)=>n+(t.check_constraints?.length||0),0),generated_columns:tables.reduce((n,t)=>n+t.columns.filter(c=>c.generated_expression).length,0),views:kinds('view'),materialized_views:kinds('materialized_view'),sequences:kinds('sequence'),functions:kinds('function'),triggers:kinds('trigger'),indexes:kinds('index'),policies:kinds('policy'),schemas:kinds('schema'),synonyms:kinds('synonym'),events:kinds('event'),extensions:kinds('extension'),partitions:tables.filter(t=>t.partitioning).length,migrations:migrations.length};
  return{source,stats,migrations,tables,relationships,objects,capabilities};
}

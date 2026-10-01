// PostgreSQL parser adapter. Pure parser: no DOM, no renderer imports.
import {IDENT_PATTERN,parseGenericTables,collectObjects,finalizeModel,findStatementEnd,lineNumberAt,cleanIdentifier,migrationForLine} from './common.js';

const constraints=['NOT NULL','NULL','DEFAULT','PRIMARY KEY','UNIQUE','REFERENCES','CHECK','CONSTRAINT','COLLATE','GENERATED','AS','IDENTITY'];
const createTable=new RegExp(String.raw`^\s*CREATE\s+(?:UNLOGGED\s+)?TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(?<name>${IDENT_PATTERN})\s*\(`,'gim');
const alterFk=new RegExp(String.raw`ALTER\s+TABLE\s+(?:ONLY\s+)?(?<from>${IDENT_PATTERN})[\s\S]*?ADD\s+(?:CONSTRAINT\s+(?<constraint>${IDENT_PATTERN})\s+)?FOREIGN\s+KEY\s*\((?<fromCols>[^)]*)\)\s+REFERENCES\s+(?<to>${IDENT_PATTERN})\s*\((?<toCols>[^)]*)\)(?<tail>[^;]*)`,'gi');

export function parsePostgreSql(sql,source='arquivo.sql'){
  const core=parseGenericTables(sql,{createTableRegex:createTable,constraintKeywords:constraints,alterForeignKeyRegex:alterFk,dialect:'postgresql'});
  const specs=[
    {kind:'materialized_view',regex:new RegExp(String.raw`^\s*CREATE\s+MATERIALIZED\s+VIEW\s+(?:IF\s+NOT\s+EXISTS\s+)?(?<name>${IDENT_PATTERN})`,'gim')},
    {kind:'view',regex:new RegExp(String.raw`^\s*CREATE\s+(?:OR\s+REPLACE\s+)?VIEW\s+(?<name>${IDENT_PATTERN})`,'gim')},
    {kind:'function',regex:new RegExp(String.raw`^\s*CREATE\s+(?:OR\s+REPLACE\s+)?(?:FUNCTION|PROCEDURE)\s+(?<name>${IDENT_PATTERN})`,'gim')},
    {kind:'trigger',regex:new RegExp(String.raw`^\s*CREATE\s+(?:OR\s+REPLACE\s+)?TRIGGER\s+(?<name>${IDENT_PATTERN})`,'gim'),targetRegex:new RegExp(String.raw`\bON\s+(?<target>${IDENT_PATTERN})`,'i')},
    {kind:'index',regex:new RegExp(String.raw`^\s*CREATE\s+(?:UNIQUE\s+)?INDEX\s+(?:CONCURRENTLY\s+)?(?:IF\s+NOT\s+EXISTS\s+)?(?<name>${IDENT_PATTERN})`,'gim'),targetRegex:new RegExp(String.raw`\bON\s+(?:ONLY\s+)?(?<target>${IDENT_PATTERN})`,'i')},
    {kind:'policy',regex:new RegExp(String.raw`^\s*CREATE\s+POLICY\s+(?<name>${IDENT_PATTERN})`,'gim'),targetRegex:new RegExp(String.raw`\bON\s+(?<target>${IDENT_PATTERN})`,'i')},
    {kind:'schema',regex:new RegExp(String.raw`^\s*CREATE\s+SCHEMA\s+(?:IF\s+NOT\s+EXISTS\s+)?(?<name>${IDENT_PATTERN})`,'gim')},
    {kind:'sequence',regex:new RegExp(String.raw`^\s*CREATE\s+(?:TEMP(?:ORARY)?\s+)?SEQUENCE\s+(?:IF\s+NOT\s+EXISTS\s+)?(?<name>${IDENT_PATTERN})`,'gim')},
    {kind:'extension',regex:new RegExp(String.raw`^\s*CREATE\s+EXTENSION\s+(?:IF\s+NOT\s+EXISTS\s+)?(?<name>${IDENT_PATTERN})`,'gim'),metadata:raw=>({schema:(raw.match(/\bSCHEMA\s+([^\s;]+)/i)||[])[1]||null,version:(raw.match(/\bVERSION\s+['"]?([^'"\s;]+)/i)||[])[1]||null})},
    {kind:'partition',regex:new RegExp(String.raw`^\s*CREATE\s+(?:UNLOGGED\s+)?TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(?<name>${IDENT_PATTERN})\s+PARTITION\s+OF\s+(?<parent>${IDENT_PATTERN})`,'gim'),targetRegex:new RegExp(String.raw`\bPARTITION\s+OF\s+(?<target>${IDENT_PATTERN})`,'i'),metadata:raw=>({bound:(raw.match(/\bFOR\s+VALUES\s+([\s\S]*?)(?=;|$)/i)||[])[1]?.trim()||null})}
  ];
  const objects=collectObjects(sql,core.migrations,specs);
  // ALTER TABLE ... ADD CONSTRAINT ... CHECK (...) — attach checks not present in CREATE TABLE.
  const tableMap=new Map(core.tables.map(t=>[cleanIdentifier(t.name).toLowerCase(),t]));
  const ar=new RegExp(String.raw`ALTER\s+TABLE\s+(?:ONLY\s+)?(?<table>${IDENT_PATTERN})[\s\S]*?ADD\s+(?:CONSTRAINT\s+(?<name>${IDENT_PATTERN})\s+)?CHECK\s*\(`,'gi');let m;
  while((m=ar.exec(sql))){const open=sql.indexOf('(',m.index+m[0].length-1);if(open<0)continue;let d=0,q=null,close=open;for(let i=open;i<sql.length;i++){const c=sql[i];if(q){if(c===q&&sql[i-1]!== '\\')q=null;continue}if(c==="'"||c==='"'){q=c;continue}if(c==='(')d++;else if(c===')'&&--d===0){close=i;break}}const table=tableMap.get(cleanIdentifier(m.groups.table).toLowerCase());if(table){table.check_constraints||=[];table.check_constraints.push({name:m.groups.name?cleanIdentifier(m.groups.name):null,expression:sql.slice(open+1,close).trim(),column:null,table:table.name,line:lineNumberAt(sql,m.index)});}}
  return finalizeModel(sql,source,{...core,objects,capabilities:{tables:true,columns:true,foreign_keys:true,check_constraints:true,generated_columns:true,views:true,materialized_views:true,sequences:true,functions:true,triggers:true,indexes:true,policies:true,schemas:true,partitioning:true,synonyms:false,events:false,extensions:true}})
}

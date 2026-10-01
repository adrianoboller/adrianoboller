import {IDENT_PATTERN,parseGenericTables,collectObjects,finalizeModel} from './common.js';
const constraints=['NOT NULL','NULL','DEFAULT','PRIMARY KEY','UNIQUE','REFERENCES','CHECK','CONSTRAINT','COLLATE','IDENTITY','ROWGUIDCOL','SPARSE','MASKED','ENCRYPTED','FILESTREAM','GENERATED','AS'];
const createTable=new RegExp(String.raw`^\s*CREATE\s+TABLE\s+(?<name>${IDENT_PATTERN})\s*\(`,'gim');
const alterFk=new RegExp(String.raw`ALTER\s+TABLE\s+(?<from>${IDENT_PATTERN})\s+(?:WITH\s+(?:CHECK|NOCHECK)\s+)?ADD\s+(?:CONSTRAINT\s+(?<constraint>${IDENT_PATTERN})\s+)?FOREIGN\s+KEY\s*\((?<fromCols>[^)]*)\)\s+REFERENCES\s+(?<to>${IDENT_PATTERN})\s*\((?<toCols>[^)]*)\)(?<tail>[\s\S]*?)(?=;|\r?\n\s*GO\b|$)`,'gi');
export function parseSqlServer(sql,source='arquivo.sql'){
  const core=parseGenericTables(sql,{createTableRegex:createTable,constraintKeywords:constraints,alterForeignKeyRegex:alterFk,goBatch:true,dialect:'sqlserver'});
  const specs=[
    {kind:'view',regex:new RegExp(String.raw`^\s*CREATE\s+(?:OR\s+ALTER\s+)?VIEW\s+(?<name>${IDENT_PATTERN})`,'gim')},
    {kind:'function',regex:new RegExp(String.raw`^\s*CREATE\s+(?:OR\s+ALTER\s+)?(?:FUNCTION|PROCEDURE|PROC)\s+(?<name>${IDENT_PATTERN})`,'gim')},
    {kind:'trigger',regex:new RegExp(String.raw`^\s*CREATE\s+(?:OR\s+ALTER\s+)?TRIGGER\s+(?<name>${IDENT_PATTERN})`,'gim'),targetRegex:new RegExp(String.raw`\bON\s+(?<target>${IDENT_PATTERN})`,'i')},
    {kind:'index',regex:new RegExp(String.raw`^\s*CREATE\s+(?:UNIQUE\s+)?(?:(?:CLUSTERED|NONCLUSTERED)\s+)?INDEX\s+(?<name>${IDENT_PATTERN})`,'gim'),targetRegex:new RegExp(String.raw`\bON\s+(?<target>${IDENT_PATTERN})`,'i')},
    {kind:'schema',regex:new RegExp(String.raw`^\s*CREATE\s+SCHEMA\s+(?<name>${IDENT_PATTERN})`,'gim')},
    {kind:'sequence',regex:new RegExp(String.raw`^\s*CREATE\s+SEQUENCE\s+(?<name>${IDENT_PATTERN})`,'gim')},
    {kind:'synonym',regex:new RegExp(String.raw`^\s*CREATE\s+SYNONYM\s+(?<name>${IDENT_PATTERN})\s+FOR\s+(?<target>${IDENT_PATTERN})`,'gim'),targetRegex:new RegExp(String.raw`\bFOR\s+(?<target>${IDENT_PATTERN})`,'i')}
  ];
  return finalizeModel(sql,source,{...core,objects:collectObjects(sql,core.migrations,specs,{goBatch:true}),capabilities:{tables:true,columns:true,foreign_keys:true,check_constraints:true,generated_columns:true,views:true,materialized_views:false,sequences:true,functions:true,triggers:true,indexes:true,policies:false,schemas:true,partitioning:true,synonyms:true,events:false,extensions:false}})
}

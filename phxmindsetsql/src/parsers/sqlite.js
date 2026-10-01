import {IDENT_PATTERN,parseGenericTables,collectObjects,finalizeModel} from './common.js';
const constraints=['NOT NULL','NULL','DEFAULT','PRIMARY KEY','UNIQUE','REFERENCES','CHECK','CONSTRAINT','COLLATE','GENERATED','AS'];
const createTable=new RegExp(String.raw`^\s*CREATE\s+(?:TEMP(?:ORARY)?\s+)?TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(?<name>${IDENT_PATTERN})\s*\(`,'gim');
export function parseSqlite(sql,source='arquivo.sql'){
  const core=parseGenericTables(sql,{createTableRegex:createTable,constraintKeywords:constraints,dialect:'sqlite'});
  const specs=[
    {kind:'view',regex:new RegExp(String.raw`^\s*CREATE\s+(?:TEMP(?:ORARY)?\s+)?VIEW\s+(?:IF\s+NOT\s+EXISTS\s+)?(?<name>${IDENT_PATTERN})`,'gim')},
    {kind:'trigger',regex:new RegExp(String.raw`^\s*CREATE\s+(?:TEMP(?:ORARY)?\s+)?TRIGGER\s+(?:IF\s+NOT\s+EXISTS\s+)?(?<name>${IDENT_PATTERN})`,'gim'),targetRegex:new RegExp(String.raw`\bON\s+(?<target>${IDENT_PATTERN})`,'i')},
    {kind:'index',regex:new RegExp(String.raw`^\s*CREATE\s+(?:UNIQUE\s+)?INDEX\s+(?:IF\s+NOT\s+EXISTS\s+)?(?<name>${IDENT_PATTERN})`,'gim'),targetRegex:new RegExp(String.raw`\bON\s+(?<target>${IDENT_PATTERN})`,'i')}
  ];
  return finalizeModel(sql,source,{...core,objects:collectObjects(sql,core.migrations,specs),capabilities:{tables:true,columns:true,foreign_keys:true,check_constraints:true,generated_columns:true,views:true,materialized_views:false,sequences:false,functions:false,triggers:true,indexes:true,policies:false,schemas:false,partitioning:false,synonyms:false,events:false,extensions:false}})
}

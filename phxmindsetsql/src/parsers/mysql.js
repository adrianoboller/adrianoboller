import {IDENT_PATTERN,parseGenericTables,collectObjects,finalizeModel} from './common.js';
const constraints=['NOT NULL','NULL','DEFAULT','PRIMARY KEY','UNIQUE','REFERENCES','CHECK','CONSTRAINT','COLLATE','AUTO_INCREMENT','COMMENT','GENERATED','ON UPDATE','VISIBLE','INVISIBLE'];
const createTable=new RegExp(String.raw`^\s*CREATE\s+(?:TEMPORARY\s+)?TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(?<name>${IDENT_PATTERN})\s*\(`,'gim');
const alterFk=new RegExp(String.raw`ALTER\s+TABLE\s+(?<from>${IDENT_PATTERN})\s+ADD\s+(?:CONSTRAINT\s+(?<constraint>${IDENT_PATTERN})\s+)?FOREIGN\s+KEY\s*\((?<fromCols>[^)]*)\)\s+REFERENCES\s+(?<to>${IDENT_PATTERN})\s*\((?<toCols>[^)]*)\)(?<tail>[^;]*)`,'gi');
export function parseMySql(sql,source='arquivo.sql'){
  const core=parseGenericTables(sql,{createTableRegex:createTable,constraintKeywords:constraints,alterForeignKeyRegex:alterFk,dialect:'mysql'});
  const specs=[
    {kind:'view',regex:new RegExp(String.raw`^\s*CREATE\s+(?:OR\s+REPLACE\s+)?(?:(?:ALGORITHM\s*=\s*\w+|DEFINER\s*=\s*\S+|SQL\s+SECURITY\s+\w+)\s+)*VIEW\s+(?<name>${IDENT_PATTERN})`,'gim')},
    {kind:'function',regex:new RegExp(String.raw`^\s*CREATE\s+(?:(?:DEFINER\s*=\s*\S+)\s+)?(?:FUNCTION|PROCEDURE)\s+(?<name>${IDENT_PATTERN})`,'gim')},
    {kind:'trigger',regex:new RegExp(String.raw`^\s*CREATE\s+(?:(?:DEFINER\s*=\s*\S+)\s+)?TRIGGER\s+(?<name>${IDENT_PATTERN})`,'gim'),targetRegex:new RegExp(String.raw`\bON\s+(?<target>${IDENT_PATTERN})`,'i')},
    {kind:'index',regex:new RegExp(String.raw`^\s*CREATE\s+(?:(?:UNIQUE|FULLTEXT|SPATIAL)\s+)?INDEX\s+(?<name>${IDENT_PATTERN})`,'gim'),targetRegex:new RegExp(String.raw`\bON\s+(?<target>${IDENT_PATTERN})`,'i')},
    {kind:'schema',regex:new RegExp(String.raw`^\s*CREATE\s+(?:DATABASE|SCHEMA)\s+(?:IF\s+NOT\s+EXISTS\s+)?(?<name>${IDENT_PATTERN})`,'gim')},
    {kind:'event',regex:new RegExp(String.raw`^\s*CREATE\s+(?:(?:DEFINER\s*=\s*\S+)\s+)?EVENT\s+(?:IF\s+NOT\s+EXISTS\s+)?(?<name>${IDENT_PATTERN})`,'gim'),metadata:raw=>({schedule:(raw.match(/\bON\s+SCHEDULE\s+([\s\S]*?)(?=\bDO\b|$)/i)||[])[1]?.trim()||null})}
  ];
  return finalizeModel(sql,source,{...core,objects:collectObjects(sql,core.migrations,specs),capabilities:{tables:true,columns:true,foreign_keys:true,check_constraints:true,generated_columns:true,views:true,materialized_views:false,sequences:false,functions:true,triggers:true,indexes:true,policies:false,schemas:true,partitioning:true,synonyms:false,events:true,extensions:false}})
}

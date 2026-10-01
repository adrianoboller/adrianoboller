import assert from 'node:assert/strict';
import fs from 'node:fs';
import {parseWithRegistry} from '../src/parsers/registry.js';
import {diffSchemas} from '../src/schema-diff.js';
const sql=fs.readFileSync(new URL('../tests/fixtures/equivalent.postgresql.sql',import.meta.url),'utf8');
const file=parseWithRegistry(sql,'file.sql','postgresql').model;
const live=structuredClone(file);live.source='live:test';
live.tables[0].columns.push({name:'live_only',data_type:'text',nullable:true,default:null,primary_key:false,unique:false,generated_expression:null,generated_kind:null});
live.tables.push({name:'public.live_extra',line:0,end_line:0,migration:null,columns:[],primary_key:[],foreign_keys:[],check_constraints:[],partitioning:null,statistics:{estimated_rows:123,data_bytes:4096,index_bytes:0,total_bytes:4096},ddl:''});
live.objects.push({kind:'extension',name:'pgcrypto',line:0,migration:null,target:null,ddl:'',metadata:{}});
const d=diffSchemas(file,live);
assert.equal(d.contract_version,'1.1');assert.equal(d.summary.added_tables,1);assert.ok(d.summary.changed_tables>=1);assert.equal(d.summary.added_objects,1);assert.ok(d.summary.total_changes>=3);
// Statistics are operational metadata and must not create a schema diff by themselves.
const same=structuredClone(file);same.tables[0].statistics={estimated_rows:999,data_bytes:1000,index_bytes:50,total_bytes:1050};const ds=diffSchemas(file,same);assert.equal(ds.summary.total_changes,0);
console.log('schema-diff: OK — arquivo × vivo, ignorando estatísticas operacionais');

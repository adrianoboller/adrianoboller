import {detectDialect} from './parsers/detector.js';
import {parseWithRegistry, UnsupportedDialectError} from './parsers/registry.js';
import {decorateUnifiedSqlModel} from './model/unified-model.js';

let wasm = null;
let wasmAttempted = false;

async function ensureWasm(){
  if(wasmAttempted) return wasm;
  wasmAttempted = true;
  try{
    const mod = await import('../pkg/phx_sql_core.js');
    if(typeof mod.default === 'function') await mod.default();
    wasm = mod;
  }catch(_){ wasm = null; }
  return wasm;
}

function tryWasm(module, source, sql, dialect){
  if(!module) return null;
  if(typeof module.parse_auto === 'function' && dialect === 'auto') {
    return {model:JSON.parse(module.parse_auto(source,sql)), engine:'rust-wasm-worker:auto'};
  }
  const detected = dialect === 'auto' ? detectDialect(sql).dialect : dialect;
  const wasmParsers={postgresql:['parse_postgresql','postgresql'],mysql:['parse_mysql','mysql'],sqlite:['parse_sqlite','sqlite'],sqlserver:['parse_sqlserver','sqlserver']};
  const [fn,label]=wasmParsers[detected]||[];
  if(fn&&typeof module[fn]==='function') return {model:JSON.parse(module[fn](source,sql)),engine:`rust-wasm-worker:${label}`};
  return null;
}

self.onmessage = async ({data}) => {
  const {id,sql,source='arquivo.sql',dialect='auto'} = data || {};
  if(!id || typeof sql !== 'string') return;
  try{
    const detection = dialect === 'auto' ? detectDialect(sql) : {dialect,confidence:1,ranked:[]};
    const module = await ensureWasm();
    const wasmResult = tryWasm(module, source, sql, dialect);
    if(wasmResult){
      const model=decorateUnifiedSqlModel(wasmResult.model,{dialect:detection.dialect,parser:wasmResult.engine});
      self.postMessage({id,ok:true,model,engine:wasmResult.engine,dialect:model.dialect,detection});
      return;
    }
    const result = parseWithRegistry(sql, source, dialect);
    self.postMessage({id,ok:true,model:result.model,engine:`${result.parser}-worker`,dialect:result.dialect,detection:result.detection});
  }catch(error){
    self.postMessage({
      id,ok:false,
      code:error instanceof UnsupportedDialectError?'UNSUPPORTED_DIALECT':'PARSE_ERROR',
      dialect:error?.dialect,
      error:String(error?.message || error)
    });
  }
};

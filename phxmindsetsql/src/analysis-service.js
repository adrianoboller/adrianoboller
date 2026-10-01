import {parseWithRegistry} from './parsers/registry.js';

export function createAnalysisService() {
  let worker = null;
  let seq = 0;
  const pending = new Map();

  try {
    worker = new Worker('./src/sql-worker.js', {type:'module'});
    worker.onmessage = ({data}) => {
      const item = pending.get(data?.id);
      if (!item) return;
      pending.delete(data.id);
      if (data.ok) item.resolve({model:data.model, engine:data.engine, dialect:data.dialect, detection:data.detection});
      else item.reject(Object.assign(new Error(data.error || 'Falha no worker'), {code:data.code, dialect:data.dialect}));
    };
    worker.onerror = () => {
      for (const item of pending.values()) item.reject(new Error('Worker indisponível'));
      pending.clear();
      worker = null;
    };
  } catch (_) { worker = null; }

  async function analyze(sql, source='arquivo.sql', dialect='auto') {
    if (!worker) {
      const result = parseWithRegistry(sql, source, dialect);
      return {...result, engine:`${result.parser}-main-thread`};
    }
    const id=`p${++seq}`;
    return new Promise((resolve,reject)=>{
      pending.set(id,{resolve,reject});
      worker.postMessage({id,sql,source,dialect});
    });
  }

  function dispose(){
    worker?.terminate(); worker=null;
    for(const item of pending.values()) item.reject(new Error('AnalysisService encerrado'));
    pending.clear();
  }

  return {analyze,dispose,get workerAvailable(){return !!worker;}};
}

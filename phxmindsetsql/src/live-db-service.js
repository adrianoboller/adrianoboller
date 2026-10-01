export class GatewayError extends Error {
  constructor(message,{status=0,cause=null}={}){super(message);this.name='GatewayError';this.status=status;this.cause=cause;}
}

export function createLiveDbService({baseUrl='',fetchImpl=globalThis.fetch}={}){
  if(typeof fetchImpl!=='function')throw new TypeError('fetch indisponível');
  const base=String(baseUrl||'').replace(/\/$/,'');
  const url=p=>`${base}${p}`;
  async function request(path,options={}){
    let response;try{response=await fetchImpl(url(path),{headers:{'Accept':'application/json',...(options.body?{'Content-Type':'application/json'}:{}),...(options.headers||{})},...options});}catch(err){throw new GatewayError('Gateway PhxMindSetSQL indisponível',{cause:err});}
    let data=null;try{data=await response.json();}catch(_){/* response without json */}
    if(!response.ok)throw new GatewayError(data?.error||`Gateway retornou HTTP ${response.status}`,{status:response.status});
    return data;
  }
  return {
    health(){return request('/api/v1/health')},
    profiles(){return request('/api/v1/profiles')},
    introspect({profileId=null,connection=null,options={}}={}){return request('/api/v1/introspect',{method:'POST',body:JSON.stringify({profile_id:profileId||null,connection:connection||null,options})})},
  };
}

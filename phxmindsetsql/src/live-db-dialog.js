import {createLiveDbService,GatewayError} from './live-db-service.js';

const $=(s,r=document)=>r.querySelector(s);
const esc=s=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));

export function createLiveDbDialog({onModel,onStatus=()=>{},toast=()=>{}}={}){
  const dialog=$('#connectDialog'),form=$('#connectForm'),mode=$('#connectionMode'),profileWrap=$('#profileConnectionFields'),ephemeral=$('#ephemeralConnectionFields'),profilesEl=$('#dbProfile'),gatewayInput=$('#gatewayUrl'),status=$('#connectionStatus');
  let service=null;
  function setStatus(msg,kind=''){status.textContent=msg||'';status.dataset.kind=kind;}
  function serviceForInput(){let base=gatewayInput.value.trim();if(!base||base===location.origin)base='';return createLiveDbService({baseUrl:base});}
  function syncMode(){const live=mode.value==='ephemeral';profileWrap.hidden=live;ephemeral.hidden=!live;$('#dbPassword').value='';}
  function syncDialect(){const d=$('#dbDialect').value;const sqlite=d==='sqlite';$$('.server-field',dialog).forEach(el=>el.hidden=sqlite);$('#sqlitePathField').hidden=!sqlite;const port=$('#dbPort');const defaults={postgresql:'5432',mysql:'3306',sqlserver:'1433'};if(!sqlite&&(port.dataset.touched!=='1'||!port.value))port.value=defaults[d]||'';}
  function $$(s,r=document){return[...r.querySelectorAll(s)]}
  async function refresh(){service=serviceForInput();setStatus('Consultando gateway…');try{const [h,p]=await Promise.all([service.health(),service.profiles()]);profilesEl.innerHTML=p.length?p.map(x=>`<option value="${esc(x.id)}">${esc(x.label)} · ${esc(x.dialect)}</option>`).join(''):'<option value="">Nenhum profile configurado</option>';$('#gatewayState').textContent=`v${h.version} · ${h.profiles} profile${h.profiles===1?'':'s'}${h.ephemeral?' · efêmero habilitado':''}`;setStatus('Gateway conectado.','ok');return h}catch(err){profilesEl.innerHTML='<option value="">Gateway indisponível</option>';$('#gatewayState').textContent='offline';setStatus(err.message,'error');throw err}}
  async function open(){dialog.showModal();syncMode();syncDialect();try{await refresh()}catch(_){} }
  async function submit(ev){ev.preventDefault();const btn=$('#connectSubmit');btn.disabled=true;setStatus('Lendo catálogo do banco…');onStatus('Conectando ao banco e lendo metadados…');try{service=serviceForInput();let request;if(mode.value==='profile'){if(!profilesEl.value)throw new GatewayError('Selecione um profile configurado no gateway');request={profileId:profilesEl.value};}else{const d=$('#dbDialect').value;request={connection:{dialect:d,host:$('#dbHost').value.trim()||'127.0.0.1',port:$('#dbPort').value?Number($('#dbPort').value):null,database:$('#dbDatabase').value.trim(),username:$('#dbUser').value.trim()||null,password:$('#dbPassword').value||null,tls:$('#dbTls').value,path:d==='sqlite'?$('#dbPath').value.trim()||null:null}};if(d==='sqlite'&&!request.connection.path)throw new GatewayError('Informe o arquivo SQLite no gateway');if(d!=='sqlite'&&!request.connection.database)throw new GatewayError('Informe o banco de dados');}
      request.options={include_system:$('#includeSystem').checked,include_views:true,include_indexes:true,include_triggers:true,include_routines:true,include_policies:$('#includePolicies').checked,include_advanced_objects:$('#includeAdvanced')?.checked!==false,include_statistics:$('#includeStatistics')?.checked!==false,max_objects:20000};
      const model=await service.introspect(request);onModel?.(model,{mode:mode.value});dialog.close();toast(`${model.stats?.tables||0} tabelas carregadas do banco`);setStatus('');
    }catch(err){console.error(err);setStatus(err.message||String(err),'error');onStatus(`Falha na conexão: ${err.message||err}`);}finally{btn.disabled=false;$('#dbPassword').value='';}}
  $('#connectDbBtn').addEventListener('click',open);$('#refreshProfiles').addEventListener('click',()=>refresh().catch(()=>{}));$('#closeConnectDialog').addEventListener('click',()=>dialog.close());$('#cancelConnect').addEventListener('click',()=>dialog.close());mode.addEventListener('change',syncMode);$('#dbDialect').addEventListener('change',syncDialect);$('#dbPort').addEventListener('input',e=>e.currentTarget.dataset.touched='1');form.addEventListener('submit',submit);
  return{open,refresh};
}

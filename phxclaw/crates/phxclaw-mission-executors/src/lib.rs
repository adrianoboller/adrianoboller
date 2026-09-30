#![forbid(unsafe_code)]
use phxclaw_extension_host::ExtensionHost;
use phxclaw_mission_runtime::{ExtensionExecutor, ModelExecutor, TeamExecutor};
use phxclaw_ollama_adapter::OllamaClient;
use phxclaw_plugin_sdk::CapabilityInvocation;
use phxclaw_sandbox::SandboxBackend;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use phxclaw_team_runtime::{TeamRuntime, TeamSession, TeamTask};

pub struct ExtensionHostExecutor<B: SandboxBackend + Send + Sync + 'static> { pub host: Arc<ExtensionHost<B>> }
impl<B: SandboxBackend + Send + Sync + 'static> ExtensionExecutor for ExtensionHostExecutor<B> {
    fn execute_extension(&self, capability:&str, input:&Value, correlation_uuid:Uuid, actor:&str)->Result<Value,String>{
        let request=CapabilityInvocation::new(correlation_uuid,actor,capability,input.clone());
        let result=self.host.invoke(&request).map_err(|e|e.to_string())?;
        if matches!(result.status, phxclaw_plugin_sdk::InvocationStatus::Succeeded){ Ok(result.payload) } else { Err(result.error_message.unwrap_or_else(||"plugin rejected invocation".into())) }
    }
}

pub struct OllamaExecutor { pub client: OllamaClient, pub model: String, runtime: Mutex<tokio::runtime::Runtime> }
impl OllamaExecutor {
    pub fn new(client:OllamaClient, model:impl Into<String>)->Result<Self,std::io::Error>{ Ok(Self{client,model:model.into(),runtime:Mutex::new(tokio::runtime::Builder::new_current_thread().enable_all().build()?)}) }
}
impl ModelExecutor for OllamaExecutor {
    fn execute_model(&self, capability:&str, input:&Value, _correlation_uuid:Uuid, _actor:&str)->Result<Value,String>{
        let prompt=input.get("prompt").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(||input.to_string());
        if capability!="model.chat" && capability!="model.reasoning" && capability!="model.generate" { return Err(format!("unsupported Ollama mission capability: {capability}")); }
        self.runtime.lock().map_err(|_|"Ollama runtime lock poisoned".to_string())?.block_on(self.client.generate(&self.model,&prompt,None,Some(json!({"temperature":0.2})))).map_err(|e|e.to_string())
    }
}


pub struct TeamRuntimeExecutor { pub runtime: TeamRuntime }
impl TeamExecutor for TeamRuntimeExecutor {
    fn execute_team(&self, plan:&Value, _correlation_uuid:Uuid, _actor:&str)->Result<Value,String>{
        let name=plan.get("name").and_then(Value::as_str).unwrap_or("mission-team");
        let max_parallelism=plan.get("max_parallelism").and_then(Value::as_u64).unwrap_or(4) as usize;
        let mut team=TeamSession::new(name,max_parallelism);
        let items=plan.get("tasks").and_then(Value::as_array).ok_or_else(||"team plan requires tasks array".to_string())?;
        let mut name_to_uuid=std::collections::BTreeMap::new();
        for item in items { let task_name=item.get("name").and_then(Value::as_str).ok_or_else(||"team task missing name".to_string())?; name_to_uuid.insert(task_name.to_string(), phxclaw_types::new_uuid_v7()); }
        for item in items {
            let task_name=item.get("name").and_then(Value::as_str).unwrap(); let capability=item.get("capability").and_then(Value::as_str).unwrap_or("team.task");
            let deps=item.get("depends_on").and_then(Value::as_array).map(|xs|xs.iter().filter_map(Value::as_str).filter_map(|n|name_to_uuid.get(n).copied()).collect()).unwrap_or_default();
            let mut task=TeamTask::new(task_name,capability,deps); task.uuid=*name_to_uuid.get(task_name).unwrap(); team.tasks.insert(task.uuid,task);
        }
        let team_uuid=self.runtime.create(team).map_err(|e|e.to_string())?;
        Ok(json!({"team_uuid":team_uuid,"state":"running","note":"team created; workers claim tasks through Team Runtime leases"}))
    }
}

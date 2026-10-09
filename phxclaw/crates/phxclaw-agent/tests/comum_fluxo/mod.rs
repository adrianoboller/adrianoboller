//! A banca dos testes do motor de fluxo (onda 3 e o que veio depois dela): o `eco`, o
//! agente sobre uma pasta, o estado da API e o servidor. Um modulo so para os dois
//! arquivos de teste nao terem duas bancas que divergem.
#![allow(dead_code)]

use phxclaw_agent::api::{AgentFactory, ApiState, Limite, router};
use phxclaw_agent::fluxos;
use phxclaw_agent::gatilhos::Gatilhos;
use phxclaw_agent::subfluxo::FluxoTool;
use phxclaw_agent::*;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("phx-onda3-{nome}-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// `eco`: devolve `texto` (cru, ou JSON quando nao e texto) depois de `dorme_ms`; com
/// `falhar`, falha. Conta as chamadas e quantas estao em voo ao mesmo tempo.
#[derive(Default)]
pub struct Eco {
    pub chamadas: AtomicUsize,
    pub agora: AtomicUsize,
    pub max: AtomicUsize,
}

impl Eco {
    pub fn n(&self) -> usize {
        self.chamadas.load(Ordering::SeqCst)
    }
}

impl Tool for Eco {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "eco".into(),
            description: "eco".into(),
            parameters: json!({"type":"object"}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            self.chamadas.fetch_add(1, Ordering::SeqCst);
            let n = self.agora.fetch_add(1, Ordering::SeqCst) + 1;
            self.max.fetch_max(n, Ordering::SeqCst);
            if let Some(ms) = args.get("dorme_ms").and_then(Value::as_u64) {
                tokio::time::sleep(Duration::from_millis(ms)).await;
            }
            self.agora.fetch_sub(1, Ordering::SeqCst);
            if let Some(m) = args.get("falhar").and_then(Value::as_str) {
                return Err(ToolError::Failed(m.into()));
            }
            Ok(ToolOutput::text(match args.get("texto") {
                Some(Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => String::new(),
            }))
        })
    }
}

pub struct Banca {
    pub a: Agent,
    pub eco: Arc<Eco>,
}

/// O agente sobre a pasta `raiz/tasks`; com `fluxos`, a ferramenta `fluxo` entra.
pub fn banca_em(raiz: &Path, fluxos_dir: Option<&Path>) -> Banca {
    let eco = Arc::new(Eco::default());
    let mut tools: Vec<Arc<dyn Tool>> = vec![eco.clone(), Arc::new(ReadFileTool)];
    let ft = fluxos_dir.map(|p| Arc::new(FluxoTool::nova(p)));
    if let Some(ft) = &ft {
        tools.push(ft.clone());
    }
    let a = Agent::new(
        Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text("nada")])),
        tools,
        AgentConfig::default().grant(&["fs.read", "flow.run"]),
        TaskStore::new(raiz.join("tasks")).unwrap(),
    );
    if let Some(ft) = ft {
        let _ = ft.base.set(a.clone());
    }
    Banca { a, eco }
}

pub fn fluxo(v: Value) -> fluxos::Fluxo {
    fluxos::ler(&v.to_string()).unwrap()
}

pub fn passo<'a>(r: &'a fluxos::Relatorio, id: &str) -> &'a fluxos::Resultado {
    r.passos
        .iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("passo {id} nao esta no relatorio: {r:#?}"))
}

pub fn sha256(t: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(t)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn gravar(pasta: &Path, nome: &str, v: &Value) -> PathBuf {
    let p = pasta.join(format!("{nome}.json"));
    std::fs::write(&p, v.to_string()).unwrap();
    p
}

pub fn cru(a: &Agent, id: &str) -> String {
    std::fs::read_to_string(a.store.dir(id).join("task.json")).unwrap()
}

/// O relatorio como esta no `task.json` (o campo `answer`), sem passar pelo `Resultado`.
pub fn relatorio_cru(a: &Agent, id: &str) -> Value {
    serde_json::from_str(a.store.load(id).unwrap().answer.as_deref().unwrap()).unwrap()
}

pub const TOKEN: &str = "token-de-teste-com-tamanho-suficiente";

/// O estado da API: cada agente que a fabrica monta tem o seu `eco` (o processo da API nao
/// guarda ferramenta entre pedidos), sobre a MESMA pasta de tarefas.
pub fn estado(raiz: &Path) -> ApiState {
    let store = TaskStore::new(raiz.join("tasks")).unwrap();
    let st = store.clone();
    let factory: AgentFactory = Arc::new(move |_m: &str| {
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(Eco::default()), Arc::new(ReadFileTool)];
        Ok(Agent::new(
            Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text("feito")])),
            tools,
            AgentConfig::default().grant(&["fs.read"]),
            st.clone(),
        ))
    });
    ApiState {
        store,
        factory,
        default_model: "roteiro".into(),
        token: TOKEN.into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(raiz.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(1000)),
    }
}

pub async fn servir(s: &ApiState, g: Gatilhos) -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let app = router(s.clone()).merge(phxclaw_agent::gatilhos::router(s.clone(), Arc::new(g)));
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    base
}

pub async fn ate_o_estado(s: &ApiState, id: &str, estados: &[TaskStatus]) -> Task {
    for _ in 0..400 {
        if let Ok(t) = s.store.load(id)
            && estados.contains(&t.status)
        {
            return t;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!(
        "tarefa {id} nao chegou a {estados:?}: {:#?}",
        s.store.load(id)
    );
}

pub fn relatorio(t: &Task) -> fluxos::Relatorio {
    serde_json::from_str(t.answer.as_deref().unwrap()).unwrap()
}

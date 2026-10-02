//! MCP como cliente: um servidor MCP de verdade (python3 por stdio, escrito aqui) vira
//! ferramenta do agente pela montagem, passa pelo portao do motor e morre no fim.

use phxclaw_agent::montagem::Montagem;
use phxclaw_agent::*;
use phxclaw_agent_core::{Tool, ToolContext, ToolError};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Servidor MCP minimo. Responde 2025-06-18 ao initialize (a versao combinada nao e a que
/// o cliente pediu), pagina o tools/list em duas folhas e grava uma linha em `pids.txt`
/// por processo que subiu (o PID que ele ve e o do namespace do bwrap, entao a linha so
/// CONTA); quem morreu se confere pelo /proc do hospedeiro, pela marca no argv.
const SERVIDOR: &str = r#"
import json, os, sys, time
with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "pids.txt"), "a") as f:
    f.write(f"{os.getpid()}\n")
def responde(i, r):
    sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": i, "result": r}) + "\n")
    sys.stdout.flush()
for linha in sys.stdin:
    m = json.loads(linha)
    if "id" not in m:
        continue
    i, metodo, p = m["id"], m.get("method"), m.get("params") or {}
    if metodo == "initialize":
        responde(i, {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}},
                     "serverInfo": {"name": "calc", "version": "1"}})
    elif metodo == "tools/list" and not p.get("cursor"):
        responde(i, {"tools": [{"name": "somar", "description": "soma a e b",
            "inputSchema": {"type": "object", "properties": {"a": {"type": "number"},
            "b": {"type": "number"}}, "required": ["a", "b"]}}], "nextCursor": "f2"})
    elif metodo == "tools/list":
        responde(i, {"tools": [{"name": "dorme"}, {"name": "falha"}]})
    elif metodo == "tools/call":
        nome, a = p["name"], p.get("arguments") or {}
        if nome == "somar":
            responde(i, {"content": [{"type": "text", "text": str(a["a"] + a["b"])}]})
        elif nome == "dorme":
            time.sleep(60)
        else:
            responde(i, {"content": [{"type": "text", "text": "quebrou de proposito"}], "isError": True})
    else:
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": i,
            "error": {"code": -32601, "message": "nao sei"}}) + "\n")
        sys.stdout.flush()
"#;

fn pasta(prefixo: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("{prefixo}-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Pasta com o servidor e a configuracao; devolve o caminho da configuracao.
fn configurar(extra: serde_json::Value) -> PathBuf {
    let d = pasta("phx-mcp-cli");
    std::fs::write(d.join("calc.py"), SERVIDOR).unwrap();
    // A marca (nome unico da pasta) vai no argv para o teste achar o processo no /proc do
    // hospedeiro: dentro do bwrap o servidor nao sabe o proprio PID de fora.
    let marca = d.file_name().unwrap().to_string_lossy().into_owned();
    let mut servidores =
        vec![json!({"nome": "calc", "comando": "python3", "args": ["calc.py", marca]})];
    if let Some(a) = extra.as_array() {
        servidores.extend(a.iter().cloned());
    }
    let cfg = d.join("mcp.json");
    std::fs::write(&cfg, json!({ "servidores": servidores }).to_string()).unwrap();
    cfg
}

fn pids(cfg: &Path) -> Vec<u32> {
    std::fs::read_to_string(cfg.parent().unwrap().join("pids.txt"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect()
}

/// PIDs do hospedeiro dos servidores vivos desta configuracao: processos cujo argv traz a
/// marca da pasta e que nao sao zumbis esperando a colheita.
fn vivos(cfg: &Path) -> Vec<u32> {
    let marca = cfg
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let mut v = Vec::new();
    for e in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(cmd) = std::fs::read(e.path().join("cmdline")) else {
            continue;
        };
        let cmd = String::from_utf8_lossy(&cmd);
        // So o python: o bwrap que o envolve tambem carrega a marca no argv.
        if !cmd.contains(&marca) || !cmd.starts_with("python3") && !cmd.contains("/python3") {
            continue;
        }
        let zumbi = std::fs::read_to_string(e.path().join("stat"))
            .ok()
            .and_then(|s| {
                s.rsplit_once(')')
                    .map(|(_, r)| r.trim_start().starts_with('Z'))
            })
            .unwrap_or(true);
        if !zumbi {
            v.push(pid);
        }
    }
    v
}

async fn espera_morrer(cfg: &Path) -> bool {
    let fim = Instant::now() + Duration::from_secs(5);
    while Instant::now() < fim {
        if vivos(cfg).is_empty() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

fn store() -> TaskStore {
    TaskStore::new(pasta("phx-mcp-store")).unwrap()
}

/// O principal: a montagem do agente traz a ferramenta do servidor MCP, o modelo a ve e a
/// chama, o resultado volta, e o processo do servidor morre no fim da tarefa.
#[tokio::test]
async fn agente_lista_e_chama_somar_de_um_servidor_mcp() {
    let cfg = configurar(json!([]));
    let (tools, avisos) = phxclaw_agent::mcp::carregar(&cfg);
    assert!(avisos.is_empty(), "{avisos:?}");
    let nomes: Vec<String> = tools.iter().map(|t| t.spec().name).collect();
    // As duas folhas do tools/list vieram.
    assert_eq!(
        nomes,
        ["mcp__calc__somar", "mcp__calc__dorme", "mcp__calc__falha"]
    );
    assert!(tools.iter().all(|t| t.capability() == "mcp.calc"));
    // A descoberta fechou o processo que subiu.
    assert_eq!(pids(&cfg).len(), 1);
    assert!(
        espera_morrer(&cfg).await,
        "descoberta deixou o servidor vivo"
    );

    let mut m = Montagem::new(store());
    m.mcp = tools;
    m.capabilities = vec!["mcp.calc".into()];
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call("c1", "mcp__calc__somar", json!({"a": 2, "b": 3})),
        ScriptedLlm::call("c2", "final_answer", json!({"answer": "5"})),
    ]));
    let agente = m.agent_with(llm.clone());
    let fim = agente
        .run(
            Task::new("some 2 e 3", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(fim.status, TaskStatus::Completed, "{:?}", fim.error);
    {
        let vistos = llm.seen.lock().unwrap();
        assert!(vistos[0].1.contains(&"mcp__calc__somar".to_string()));
        let r = &vistos[1].0.last().unwrap().content;
        assert_eq!(r.trim(), "5", "resultado da ferramenta MCP: {r}");
    }
    // Um processo para a chamada, e morto pelo `finish` do fim da tarefa.
    assert_eq!(pids(&cfg).len(), 2);
    assert!(
        espera_morrer(&cfg).await,
        "o fim da tarefa deixou o servidor vivo"
    );
}

/// Sem `mcp.calc` concedida, a ferramenta nem aparece e, pedida pelo nome, volta negada --
/// sem subir processo nenhum.
#[tokio::test]
async fn sem_capacidade_mcp_o_motor_nega_e_nao_sobe_o_servidor() {
    let cfg = configurar(json!([]));
    let (tools, _) = phxclaw_agent::mcp::carregar(&cfg);
    let mut m = Montagem::new(store());
    m.mcp = tools;
    m.capabilities = vec!["fs.read".into()];
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call("c1", "mcp__calc__somar", json!({"a": 2, "b": 3})),
        ScriptedLlm::text("fim"),
    ]));
    m.agent_with(llm.clone())
        .run(
            Task::new("some", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    let vistos = llm.seen.lock().unwrap();
    assert!(!vistos[0].1.iter().any(|n| n.starts_with("mcp__")));
    let r = &vistos[1].0.last().unwrap().content;
    assert!(r.contains("NEGADO") && r.contains("mcp.calc"), "{r}");
    assert_eq!(pids(&cfg).len(), 1, "so a descoberta subiu processo");
}

fn ctx(prazo: Duration) -> ToolContext {
    ToolContext {
        task_id: phxclaw_types::new_uuid_v7().to_string(),
        workdir: pasta("phx-mcp-work"),
        timeout: prazo,
    }
}

fn achar(tools: &[Arc<dyn Tool>], nome: &str) -> Arc<dyn Tool> {
    tools
        .iter()
        .find(|t| t.spec().name == nome)
        .unwrap()
        .clone()
}

/// Estouro do prazo: erro de prazo, e o filho morto na hora -- nao no fim da tarefa.
/// A chamada seguinte da mesma tarefa sobe outro processo e funciona.
#[tokio::test]
async fn estouro_mata_o_filho_e_a_proxima_chamada_sobe_outro() {
    let cfg = configurar(json!([]));
    let (tools, _) = phxclaw_agent::mcp::carregar(&cfg);
    let c = ctx(Duration::from_millis(800));
    let r = achar(&tools, "mcp__calc__dorme").run(json!({}), &c).await;
    assert!(matches!(r, Err(ToolError::Timeout(800))), "{r:?}");
    assert_eq!(pids(&cfg).len(), 2);
    assert!(espera_morrer(&cfg).await, "estouro deixou o servidor vivo");

    let r = achar(&tools, "mcp__calc__somar")
        .run(json!({"a": 40, "b": 2}), &c)
        .await
        .unwrap();
    assert_eq!(r.content, "42");
    // isError do servidor e erro da ferramenta, com o texto dele; a sessao continua.
    let r = achar(&tools, "mcp__calc__falha").run(json!({}), &c).await;
    assert!(
        matches!(&r, Err(ToolError::Failed(m)) if m.contains("quebrou de proposito")),
        "{r:?}"
    );
    assert_eq!(pids(&cfg).len(), 3, "o isError nao pode derrubar a sessao");
    for t in &tools {
        t.finish(&c.task_id).await;
    }
    assert!(espera_morrer(&cfg).await);
}

/// Servidor que nao sobe vira aviso e some; o que sobe continua.
#[test]
fn servidor_quebrado_vira_aviso_e_nao_derruba_os_outros() {
    let cfg = configurar(json!([
        {"nome": "fantasma", "comando": "/nao/existe/servidor-mcp"},
        {"nome": "mudo", "comando": "python3", "args": ["-c", "import sys; sys.exit(0)"]},
        {"nome": "lento", "comando": "python3", "args": ["-c", "import time; time.sleep(30)"],
         "prazo_inicio_ms": 500},
        {"nome": "duplo", "comando": "python3", "url": "http://127.0.0.1:1/mcp"}
    ]));
    let t0 = Instant::now();
    let (tools, avisos) = phxclaw_agent::mcp::carregar(&cfg);
    assert!(t0.elapsed() < Duration::from_secs(10), "{:?}", t0.elapsed());
    assert_eq!(tools.len(), 3, "so as do calc");
    for nome in ["fantasma", "mudo", "lento", "duplo"] {
        assert!(
            avisos.iter().any(|a| a.contains(&format!("'{nome}'"))),
            "{nome}: {avisos:?}"
        );
    }
}

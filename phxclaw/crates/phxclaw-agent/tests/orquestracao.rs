//! Orquestracao e plugins contra o que eles usam de verdade: o DAG do `phxclaw-task-graph`
//! com subagentes pelo laco unico, plugin assinado com Ed25519 rodando no bwrap, no de
//! dispositivo falso falando WSS com TLS real, e tarefas paralelas em worktrees do git da
//! maquina. Os LLMs sao roteirizados por objetivo: em paralelo, a ordem das chamadas nao
//! e deterministica, o objetivo e.

use phxclaw_agent::fluxos;
use phxclaw_agent::*;
use phxclaw_agent_core::{
    BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, Role, Tool, ToolCall, ToolContext,
    ToolSpec,
};
use phxclaw_evidence_ledger::EvidenceLedger;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("phx-orq-{nome}-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn bwrap() -> Option<PathBuf> {
    let b = phxclaw_agent::arquivos::achar_bwrap();
    if b.is_none() {
        eprintln!("sem bwrap: prova pulada");
    }
    b
}

fn ctx_de(store: &TaskStore, id: &str) -> ToolContext {
    let w = store.workdir(id);
    std::fs::create_dir_all(&w).unwrap();
    ToolContext {
        task_id: id.into(),
        workdir: w,
        timeout: Duration::from_secs(60),
    }
}

/// LLM roteirizado pelo OBJETIVO (a primeira mensagem de usuario):
/// - `[[eco:X]]`: responde X (depois de `espera`, contando quantas chamadas estao no ar);
/// - `[[falha]]`: o provedor falha;
/// - `[[escreve:N]]`: grava `N.txt`, roda `ls -a /work` no shell e responde com a saida.
struct PorObjetivo {
    espera: Duration,
    no_ar: AtomicUsize,
    pico: AtomicUsize,
    chamadas: AtomicUsize,
}

impl PorObjetivo {
    fn novo(espera: Duration) -> Arc<Self> {
        Arc::new(Self {
            espera,
            no_ar: AtomicUsize::new(0),
            pico: AtomicUsize::new(0),
            chamadas: AtomicUsize::new(0),
        })
    }
}

fn marca<'a>(t: &'a str, nome: &str) -> Option<&'a str> {
    let i = t.find(&format!("[[{nome}"))?;
    let resto = &t[i + 2 + nome.len()..];
    let fim = resto.find("]]")?;
    Some(resto[..fim].trim_start_matches(':'))
}

fn texto(t: &str) -> LlmReply {
    ScriptedLlm::text(t)
}

impl Llm for PorObjetivo {
    fn id(&self) -> String {
        "por-objetivo".into()
    }
    fn chat<'a>(
        &'a self,
        messages: &'a [Message],
        _tools: &'a [ToolSpec],
        _o: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async move {
            self.chamadas.fetch_add(1, Ordering::SeqCst);
            let agora = self.no_ar.fetch_add(1, Ordering::SeqCst) + 1;
            self.pico.fetch_max(agora, Ordering::SeqCst);
            tokio::time::sleep(self.espera).await;
            self.no_ar.fetch_sub(1, Ordering::SeqCst);
            let obj = messages
                .iter()
                .find(|m| m.role == Role::User)
                .map(|m| m.content.clone())
                .unwrap_or_default();
            let feitos: Vec<&Message> = messages.iter().filter(|m| m.role == Role::Tool).collect();
            if let Some(x) = marca(&obj, "eco") {
                return Ok(texto(x));
            }
            if marca(&obj, "falha").is_some() {
                return Err(LlmError::Transport("provedor fora (roteiro)".into()));
            }
            if let Some(n) = marca(&obj, "escreve") {
                return Ok(match feitos.len() {
                    0 => ScriptedLlm::call(
                        "w",
                        "write_file",
                        json!({"path": format!("{n}.txt"), "content": format!("feito por {n}\n")}),
                    ),
                    1 => ScriptedLlm::call("s", "shell", json!({"command": "ls -a /work"})),
                    _ => texto(&format!("fim {n}: {}", feitos[1].content)),
                });
            }
            Ok(texto("nada"))
        })
    }
}

fn basicas(b: Option<&PathBuf>) -> Vec<Arc<dyn Tool>> {
    let mut v: Vec<Arc<dyn Tool>> = vec![
        Arc::new(WriteFileTool),
        Arc::new(ReadFileTool),
        Arc::new(ListFilesTool),
    ];
    if let Some(b) = b {
        v.push(Arc::new(ShellTool {
            bwrap: b.clone(),
            network: false,
            timeout: Duration::from_secs(20),
        }));
    }
    v
}

// ======================================================================== fluxos (DAG)

fn agente(llm: Arc<dyn Llm>, caps: &[&str]) -> Agent {
    Agent::new(
        llm,
        basicas(None),
        AgentConfig::default().grant(caps),
        TaskStore::new(tmp("fluxo")).unwrap(),
    )
}

/// O caminho feliz: as duas tarefas sem dependencia vao na MESMA onda (pico de chamadas
/// simultaneas ao modelo = 2), a ferramenta so roda depois das duas e recebe as saidas
/// pelo `{{id}}`, e as tarefas de agente sao filhas da tarefa do fluxo.
#[tokio::test]
async fn fluxo_roda_por_ondas_do_dag_e_passa_as_saidas() {
    let llm = PorObjetivo::novo(Duration::from_millis(150));
    let a = agente(llm.clone(), &["fs.write", "fs.read"]);
    let f = fluxos::ler(
        &json!({"nome":"junta","max_paralelo":4,"passos":[
            {"id":"grava","depende":["a","b"],"ferramenta":"write_file",
             "args":{"path":"junta.txt","content":"{{a}}+{{b}}"}},
            {"id":"a","tarefa":"[[eco:alfa]]"},
            {"id":"b","tarefa":"[[eco:beta]]"},
            {"id":"le","depende":["grava"],"ferramenta":"read_file","args":{"path":"junta.txt"}}
        ]})
        .to_string(),
    )
    .unwrap();
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert!(r.sucesso, "{r:#?}");
    let ordem: Vec<&str> = r.passos.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(&ordem[2..], ["grava", "le"], "{ordem:?}");
    assert_eq!(llm.pico.load(Ordering::SeqCst), 2, "a e b na mesma onda");
    let le = r.passos.iter().find(|p| p.id == "le").unwrap();
    assert!(le.saida.contains("alfa+beta"), "{}", le.saida);
    assert_eq!(
        std::fs::read_to_string(a.store.workdir(&r.tarefa).join("junta.txt")).unwrap(),
        "alfa+beta"
    );
    for p in r.passos.iter().filter(|p| p.tarefa.is_some()) {
        let filha = a.store.load(p.tarefa.as_ref().unwrap()).unwrap();
        assert_eq!(filha.parent.as_deref(), Some(r.tarefa.as_str()));
    }
    let mae = a.store.load(&r.tarefa).unwrap();
    assert_eq!(mae.status, TaskStatus::Completed);

    // o teto de paralelismo vale: com 1, as duas tarefas nunca estao no ar juntas
    let llm = PorObjetivo::novo(Duration::from_millis(100));
    let a = agente(llm.clone(), &["fs.write", "fs.read"]);
    let mut f1 = f.clone();
    f1.max_paralelo = 1;
    assert!(fluxos::rodar(&a, &f1).await.unwrap().sucesso);
    assert_eq!(llm.pico.load(Ordering::SeqCst), 1);
}

#[test]
fn fluxo_invalido_e_recusado_na_leitura() {
    let casos = [
        (
            json!({"nome":"c","passos":[{"id":"a","depende":["b"],"tarefa":"x"},{"id":"b","depende":["a"],"tarefa":"y"}]}),
            "ciclo",
        ),
        (
            json!({"nome":"r","passos":[{"id":"a","tarefa":"x"},{"id":"b","tarefa":"use {{a}}"}]}),
            "sem declarar",
        ),
        (
            json!({"nome":"d","passos":[{"id":"a","tarefa":"x","ferramenta":"read_file"}]}),
            "exatamente um",
        ),
        (
            json!({"nome":"f","passos":[{"id":"a","depende":["z"],"tarefa":"x"}]}),
            "nao existe",
        ),
        (
            json!({"nome":"i","passos":[{"id":"a","tarefa":"x"},{"id":"a","tarefa":"y"}]}),
            "repetido",
        ),
    ];
    for (f, esperado) in casos {
        let e = fluxos::ler(&f.to_string()).unwrap_err();
        assert!(e.contains(esperado), "{f}: {e}");
    }
}

/// Falha nao some: a ferramenta negada pelo PORTAO do motor (capacidade nao concedida)
/// falha depois das tentativas, o dependente fica `bloqueado`, o independente roda, e a
/// tarefa de agente cujo provedor cai tambem conta como falha.
#[tokio::test]
async fn passo_que_falha_bloqueia_dependentes_pelo_mesmo_portao() {
    let llm = PorObjetivo::novo(Duration::from_millis(1));
    let a = agente(llm, &["fs.read"]);
    let f = fluxos::ler(
        &json!({"nome":"falhas","passos":[
            {"id":"grava","ferramenta":"write_file","tentativas":2,
             "args":{"path":"x.txt","content":"x"}},
            {"id":"depois","depende":["grava"],"tarefa":"[[eco:nunca]]"},
            {"id":"solto","tarefa":"[[eco:livre]]"},
            {"id":"cai","tarefa":"[[falha]]"}
        ]})
        .to_string(),
    )
    .unwrap();
    let r = fluxos::rodar(&a, &f).await.unwrap();
    assert!(!r.sucesso);
    let p = |id: &str| r.passos.iter().find(|p| p.id == id).unwrap().clone();
    let grava = p("grava");
    assert_eq!(
        (grava.estado.as_str(), grava.tentativas),
        ("falhou", 2),
        "{grava:?}"
    );
    assert!(grava.saida.contains("NEGADO"), "{}", grava.saida);
    assert_eq!(p("depois").estado, "bloqueado");
    assert_eq!(p("solto").estado, "ok");
    assert_eq!(p("cai").estado, "falhou");
    assert!(!a.store.workdir(&r.tarefa).join("x.txt").exists());
    assert_eq!(a.store.load(&r.tarefa).unwrap().status, TaskStatus::Failed);
}

/// Retomada do ponto de parada: o passo do meio falha (falta o arquivo de entrada), o
/// progresso fica gravado; corrigido o mundo, `retomar` NAO roda de novo o que ja deu
/// certo (o modelo nao e chamado outra vez), roda o que faltava e entrega a saida
/// gravada aos dependentes. Definicao mudada nao retoma.
#[tokio::test]
async fn fluxo_que_falha_no_meio_retoma_de_onde_parou() {
    let llm = PorObjetivo::novo(Duration::from_millis(1));
    let a = agente(llm.clone(), &["fs.write", "fs.read"]);
    let f = fluxos::ler(
        &json!({"nome":"retoma","passos":[
            {"id":"um","tarefa":"[[eco:primeiro]]"},
            {"id":"dois","depende":["um"],"ferramenta":"read_file","args":{"path":"entrada.txt"}},
            {"id":"tres","depende":["um","dois"],"ferramenta":"write_file",
             "args":{"path":"fim.txt","content":"{{um}} / {{dois}}"}}
        ]})
        .to_string(),
    )
    .unwrap();
    let r1 = fluxos::rodar(&a, &f).await.unwrap();
    assert!(!r1.sucesso);
    let estado =
        |r: &fluxos::Relatorio, id: &str| r.passos.iter().find(|p| p.id == id).unwrap().clone();
    assert_eq!(estado(&r1, "dois").estado, "falhou");
    assert_eq!(estado(&r1, "tres").estado, "bloqueado");
    assert_eq!(llm.chamadas.load(Ordering::SeqCst), 1);

    std::fs::write(a.store.workdir(&r1.tarefa).join("entrada.txt"), "segundo").unwrap();
    let r2 = fluxos::retomar(&a, &f, &r1.tarefa).await.unwrap();
    assert!(r2.sucesso, "{r2:#?}");
    assert_eq!(r2.tarefa, r1.tarefa, "a mesma tarefa do fluxo");
    let um = estado(&r2, "um");
    assert!(um.reaproveitado && um.saida == "primeiro", "{um:?}");
    assert!(!estado(&r2, "dois").reaproveitado);
    assert_eq!(
        std::fs::read_to_string(a.store.workdir(&r1.tarefa).join("fim.txt")).unwrap(),
        "primeiro / segundo"
    );
    // o subagente do passo `um` nao rodou de novo: nenhuma tarefa filha nova
    let filhas = a
        .store
        .list()
        .unwrap()
        .into_iter()
        .filter(|t| t.parent.as_deref() == Some(r1.tarefa.as_str()))
        .count();
    assert_eq!(filhas, 1);
    assert_eq!(
        llm.chamadas.load(Ordering::SeqCst),
        1,
        "o modelo nao foi chamado de novo"
    );

    let mut outra = f.clone();
    outra.passos[0].tarefa = Some("[[eco:mudou]]".into());
    let e = fluxos::retomar(&a, &outra, &r1.tarefa).await.unwrap_err();
    assert!(e.contains("mudou"), "{e}");
}

// ======================================================================== plugins

const API: &str = "0.5.0";
const SEMENTE: [u8; 32] = [7u8; 32];

fn trust(semente: [u8; 32]) -> phxclaw_plugin_registry::TrustStore {
    use base64::Engine as _;
    let publica = base64::engine::general_purpose::STANDARD.encode(
        ed25519_dalek::SigningKey::from_bytes(&semente)
            .verifying_key()
            .as_bytes(),
    );
    phxclaw_plugin_registry::TrustStore::from_json(&format!(
        r#"{{"version":"1","signers":[{{"id":"teste-orq","algorithm":"ed25519","public_key_base64":"{publica}","status":"active","allowed_name_prefixes":["com.phxclaw.teste."],"signature_format":2}}]}}"#
    ))
    .unwrap()
}

/// Um pacote de plugin numa raiz temporaria: script, esquema e manifesto assinado (V2).
fn pacote(raiz: &Path, nome: &str, caps: &[&str], ponto: bool, semente: [u8; 32]) {
    let d = raiz.join(format!("plugins/{nome}"));
    std::fs::create_dir_all(&d).unwrap();
    let art = d.join("entrada.sh");
    std::fs::write(
        &art,
        "#!/bin/sh\necho \"plugin rodou\" > /work/plugin-esteve-aqui.txt\nprintf '{\"args\":%s,\"caps\":\"%s\",\"home\":\"%s\"}' \"$PHXCLAW_ARGS\" \"$PHXCLAW_CAPACIDADES\" \"$HOME\"\n",
    )
    .unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&art, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::fs::write(
        d.join("esquema.json"),
        r#"{"type":"object","properties":{"texto":{"type":"string"}},"required":["texto"]}"#,
    )
    .unwrap();
    let m = json!({
        "manifest_version": "1.2.0",
        "uuid": phxclaw_types::new_uuid_v7(),
        "name": format!("com.phxclaw.teste.{nome}"),
        "version": "1.0.0",
        "kind": "tool",
        "description": "eco de teste",
        "core_api": ">=0.5.0, <0.6.0",
        "entrypoint": {"type": "process", "value": format!("plugins/{nome}/entrada.sh")},
        "dependencies": [],
        "capabilities": caps,
        "extension_points": if ponto { json!([{"point":"agent.tool","contract_version":"^1.0"}]) } else { json!([]) },
        "permissions": [],
        "lifecycle": {"install":"i","enable":"e","disable":"d","uninstall":"u","health":null},
        "contracts": {"input_schema": format!("plugins/{nome}/esquema.json"), "output_schema": "-"},
        "sandbox": {"network":"deny","network_allowlist":[],"read_only_paths":[],"write_paths":[],
                    "environment_allowlist":[],"timeout_ms":20000,"memory_mb":64,"cpu_quota_percent":50},
        "integrity": {"artifact": format!("plugins/{nome}/entrada.sh"), "hash_algorithm":"sha256",
                      "digest": "0".repeat(64), "signature_algorithm":"ed25519", "signature":"x",
                      "signer":"teste-orq", "provenance":"teste"},
        "tests": [{"name":"t","command":"true","required":true}],
        "rollback": {"strategy":"disable","steps":["disable"]},
    });
    let chave = ed25519_dalek::SigningKey::from_bytes(&semente);
    let assinado =
        phxclaw_plugin_registry::assinatura::reassinar(&m.to_string(), raiz, &chave, 2).unwrap();
    std::fs::create_dir_all(raiz.join("manifestos")).unwrap();
    std::fs::write(
        raiz.join(format!("manifestos/{nome}.plugin.json")),
        assinado,
    )
    .unwrap();
}

fn caps(v: &[&str]) -> std::collections::BTreeSet<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn carregar(raiz: &Path, c: &[&str], b: &Path) -> phxclaw_agent::plugins::Carga {
    phxclaw_agent::plugins::carregar(
        raiz,
        &raiz.join("manifestos"),
        trust(SEMENTE),
        API,
        &caps(c),
        b,
    )
    .unwrap()
}

/// O plugin assinado vira ferramenta, roda no sandbox (HOME=/work, so a pasta da tarefa)
/// pelo portao do motor, e recebe so a INTERSECAO das capacidades: declarou `eco.run` e
/// `net.fetch`, o agente so tem `eco.run`.
#[tokio::test]
async fn plugin_assinado_vira_ferramenta_e_roda_no_sandbox_pelo_portao() {
    let Some(b) = bwrap() else { return };
    let raiz = tmp("plug");
    pacote(&raiz, "eco", &["eco.run", "net.fetch"], true, SEMENTE);
    pacote(&raiz, "outro", &["eco.run"], false, SEMENTE);
    let c = carregar(&raiz, &["eco.run", "fs.read"], &b);
    assert!(c.recusas.is_empty(), "{:?}", c.recusas);
    assert_eq!(
        c.ferramentas.len(),
        1,
        "sem o ponto agent.tool nao vira ferramenta"
    );
    let t = &c.ferramentas[0];
    assert_eq!((t.nome.as_str(), t.capability()), ("plugin_eco", "eco.run"));
    assert_eq!(t.efetivas, ["eco.run"]);
    assert_eq!(t.spec().parameters["required"], json!(["texto"]));

    let tools: Vec<Arc<dyn Tool>> = c
        .ferramentas
        .into_iter()
        .map(|t| Arc::new(t) as Arc<dyn Tool>)
        .collect();
    let store = TaskStore::new(raiz.join("tasks")).unwrap();
    let chamar = |caps: &'static [&'static str]| {
        let a = Agent::new(
            PorObjetivo::novo(Duration::ZERO),
            tools.clone(),
            AgentConfig::default().grant(caps),
            store.clone(),
        );
        let store = store.clone();
        async move {
            let id = phxclaw_types::new_uuid_v7().to_string();
            let ctx = ctx_de(&store, &id);
            let ledger = EvidenceLedger::open(store.evidence_path(&id)).unwrap();
            let call = ToolCall {
                id: "p".into(),
                name: "plugin_eco".into(),
                arguments: json!({"texto": "oi"}),
            };
            let (txt, desfecho, _) = a.call_tool(&call, &ctx, &ledger, &id).await;
            (txt, desfecho, ctx.workdir)
        }
    };
    let (txt, desfecho, w) = chamar(&["eco.run"]).await;
    assert_eq!(desfecho, "ok", "{txt}");
    let v: Value = serde_json::from_str(&txt).unwrap();
    assert_eq!(v["args"], json!({"texto":"oi"}));
    assert_eq!(v["caps"], "eco.run");
    assert_eq!(v["home"], "/work");
    assert!(w.join("plugin-esteve-aqui.txt").exists());
    // agente (ou subagente) sem a capacidade: o MESMO portao nega
    let (txt, desfecho, w) = chamar(&["fs.read"]).await;
    assert_eq!(desfecho, "negado", "{txt}");
    assert!(!w.join("plugin-esteve-aqui.txt").exists());
    let _ = std::fs::remove_dir_all(&raiz);
}

/// Os dois sentidos da assinatura: artefato mexido antes da carga e chave fora do trust
/// store vao para a quarentena; capacidade primaria nao concedida e recusada com o motivo;
/// e o artefato trocado DEPOIS da carga e recusado na execucao, sem rodar.
#[tokio::test]
async fn plugin_sem_assinatura_valida_nao_vira_ferramenta_nem_roda() {
    let Some(b) = bwrap() else { return };
    let raiz = tmp("plug-ruim");
    pacote(&raiz, "mexido", &["eco.run"], true, SEMENTE);
    pacote(&raiz, "estranho", &["eco.run"], true, [9u8; 32]);
    pacote(&raiz, "poderoso", &["system.admin"], true, SEMENTE);
    pacote(&raiz, "bom", &["eco.run"], true, SEMENTE);
    std::fs::write(
        raiz.join("plugins/mexido/entrada.sh"),
        "#!/bin/sh\necho mexido\n",
    )
    .unwrap();
    let c = carregar(&raiz, &["eco.run"], &b);
    let motivo = |n: &str| {
        c.recusas
            .iter()
            .find(|(p, _)| p.contains(n))
            .map(|(_, m)| m.clone())
            .unwrap_or_else(|| panic!("{n} nao recusado: {:?}", c.recusas))
    };
    assert!(motivo("mexido").contains("sha256"), "{}", motivo("mexido"));
    assert!(motivo("estranho").contains("quarentena"));
    assert!(
        motivo("poderoso").contains("system.admin nao concedida"),
        "{}",
        motivo("poderoso")
    );
    let nomes: Vec<&str> = c.ferramentas.iter().map(|t| t.nome.as_str()).collect();
    assert_eq!(nomes, ["plugin_bom"]);

    // trocado depois de carregado: a execucao confere de novo e nao roda
    std::fs::write(
        raiz.join("plugins/bom/entrada.sh"),
        "#!/bin/sh\necho trocado > /work/rodou.txt\n",
    )
    .unwrap();
    let store = TaskStore::new(raiz.join("tasks")).unwrap();
    let ctx = ctx_de(&store, "t");
    let e = c.ferramentas[0]
        .run(json!({"texto":"x"}), &ctx)
        .await
        .unwrap_err();
    assert!(e.to_string().contains("mudou depois de assinado"), "{e}");
    assert!(!ctx.workdir.join("rodou.txt").exists());
    let _ = std::fs::remove_dir_all(&raiz);
}

// ---------------------------------------------------------- pacotes Claude/Codex

const SERVIDOR_MCP: &str = r#"
import json, sys
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
                     "serverInfo": {"name": "eco", "version": "1"}})
    elif metodo == "tools/list":
        responde(i, {"tools": [{"name": "eco", "description": "devolve o texto",
            "inputSchema": {"type": "object", "properties": {"t": {"type": "string"}}}}]})
    elif metodo == "tools/call":
        responde(i, {"content": [{"type": "text", "text": "eco:" + p["arguments"]["t"]}]})
"#;

fn trust_pacote(semente: [u8; 32]) -> phxclaw_plugin_registry::TrustStore {
    use base64::Engine as _;
    let publica = base64::engine::general_purpose::STANDARD.encode(
        ed25519_dalek::SigningKey::from_bytes(&semente)
            .verifying_key()
            .as_bytes(),
    );
    phxclaw_plugin_registry::TrustStore::from_json(&format!(
        r#"{{"version":"1","signers":[{{"id":"pacotes","algorithm":"ed25519","public_key_base64":"{publica}","status":"active","allowed_name_prefixes":["pacote-"]}}]}}"#
    ))
    .unwrap()
}

/// Pacote no molde do Claude Code: manifesto, `.mcp.json` com `${CLAUDE_PLUGIN_ROOT}`,
/// skill, subagente, comando e hooks.
fn montar_pacote(d: &Path, pasta_manifesto: &str) {
    std::fs::create_dir_all(d.join(pasta_manifesto)).unwrap();
    std::fs::write(
        d.join(pasta_manifesto).join("plugin.json"),
        r#"{"name":"pacote-teste","version":"1.2.3","description":"teste"}"#,
    )
    .unwrap();
    std::fs::write(d.join("srv.py"), SERVIDOR_MCP).unwrap();
    std::fs::write(
        d.join(".mcp.json"),
        r#"{"mcpServers":{"ecoplug":{"command":"python3","args":["${CLAUDE_PLUGIN_ROOT}/srv.py"]}}}"#,
    )
    .unwrap();
    std::fs::create_dir_all(d.join("skills/resumir")).unwrap();
    std::fs::write(
        d.join("skills/resumir/SKILL.md"),
        "---\nname: resumir\ndescription: resume\n---\ncorpo\n",
    )
    .unwrap();
    std::fs::create_dir_all(d.join("agents")).unwrap();
    std::fs::write(
        d.join("agents/revisor.md"),
        "---\nname: revisor\n---\nrevise\n",
    )
    .unwrap();
    std::fs::create_dir_all(d.join("commands")).unwrap();
    std::fs::write(d.join("commands/oi.md"), "diga oi\n").unwrap();
    std::fs::create_dir_all(d.join("hooks")).unwrap();
    std::fs::write(
        d.join("hooks/hooks.json"),
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"true"}]}]}}"#,
    )
    .unwrap();
}

/// Pacote assinado: o servidor MCP dele vira ferramenta pela mesma subida do MCP e
/// responde; skills, subagentes, comandos e hooks sao lidos. Os dois sentidos da
/// assinatura: arquivo mexido, pacote sem assinatura e chave fora do trust store nao
/// carregam nada. O mesmo pacote em `.codex-plugin/` e reconhecido como Codex.
#[tokio::test]
async fn pacote_claude_ou_codex_so_carrega_assinado_e_traz_o_mcp() {
    use phxclaw_plugin_registry::pasta::{ARQUIVO_ASSINATURA, assinar_pasta};
    let d = tmp("pacote");
    montar_pacote(&d, ".claude-plugin");
    let chave = ed25519_dalek::SigningKey::from_bytes(&[5u8; 32]);
    let trust = trust_pacote([5u8; 32]);
    assinar_pasta(&d, "pacote-teste", "pacotes", &chave).unwrap();

    let p = phxclaw_agent::pacotes::ler(&d, &trust).unwrap();
    assert_eq!((p.formato, p.versao.as_str()), ("claude", "1.2.3"));
    assert_eq!(p.skills, ["resumir"]);
    assert_eq!(p.agentes, ["revisor"]);
    assert_eq!(p.comandos, ["oi"]);
    assert!(p.hooks.as_ref().unwrap()["hooks"]["Stop"].is_array());
    let srv = &p.mcp.servidores[0];
    assert_eq!(srv.args, [format!("{}/srv.py", p.raiz.display())]);

    let (tools, avisos) = phxclaw_agent::pacotes::ferramentas(&p);
    assert!(avisos.is_empty(), "{avisos:?}");
    let eco = tools
        .iter()
        .find(|t| t.spec().name == "mcp__ecoplug__eco")
        .expect("ferramenta do pacote");
    let ctx = ToolContext {
        task_id: "t".into(),
        workdir: d.clone(),
        timeout: Duration::from_secs(20),
    };
    let r = eco.run(json!({"t": "ola"}), &ctx).await.unwrap();
    assert!(r.content.contains("eco:ola"), "{}", r.content);
    eco.finish("t").await;

    // mexer em QUALQUER arquivo (aqui, o servidor) depois de assinar
    std::fs::write(d.join("srv.py"), format!("{SERVIDOR_MCP}\n# mexido\n")).unwrap();
    let e = phxclaw_agent::pacotes::ler(&d, &trust).unwrap_err();
    assert!(e.contains("mudou depois de assinado"), "{e}");
    // reassinado por quem o trust store nao conhece
    assinar_pasta(
        &d,
        "pacote-teste",
        "pacotes",
        &ed25519_dalek::SigningKey::from_bytes(&[6u8; 32]),
    )
    .unwrap();
    let e = phxclaw_agent::pacotes::ler(&d, &trust).unwrap_err();
    assert!(e.contains("Ed25519"), "{e}");
    // sem assinatura nenhuma
    std::fs::remove_file(d.join(ARQUIVO_ASSINATURA)).unwrap();
    let e = phxclaw_agent::pacotes::ler(&d, &trust).unwrap_err();
    assert!(e.contains("sem assinatura"), "{e}");

    // o mesmo pacote no molde do Codex
    std::fs::rename(d.join(".claude-plugin"), d.join(".codex-plugin")).unwrap();
    assinar_pasta(&d, "pacote-teste", "pacotes", &chave).unwrap();
    assert_eq!(
        phxclaw_agent::pacotes::ler(&d, &trust).unwrap().formato,
        "codex"
    );
    let _ = std::fs::remove_dir_all(&d);
}

// ======================================================================== dispositivos

mod no_falso {
    use phxclaw_device_transport::servidor::{
        Boasvindas, RegistroMemoria, ResultadoDeComando, ServidorDispositivos, tls_de_pem,
    };
    use phxclaw_device_transport::*;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    pub const TOKEN: &str = "token-de-pareamento-do-teste-de-orquestracao";

    /// CA + folha para localhost (o rustls recusa CA usada como folha).
    fn certificado(d: &std::path::Path) -> Option<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        let sh = |args: &[&str]| {
            Command::new("openssl")
                .args(args)
                .current_dir(d)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };
        std::fs::write(
            d.join("ext.cnf"),
            "basicConstraints=CA:FALSE\nsubjectAltName=DNS:localhost\nextendedKeyUsage=serverAuth\n",
        )
        .ok()?;
        let ok = sh(&[
            "req",
            "-x509",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=CA orq",
            "-keyout",
            "ca.key",
            "-out",
            "ca.pem",
        ]) && sh(&[
            "req",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-nodes",
            "-subj",
            "/CN=localhost",
            "-keyout",
            "srv.key",
            "-out",
            "srv.csr",
        ]) && sh(&[
            "x509",
            "-req",
            "-in",
            "srv.csr",
            "-CA",
            "ca.pem",
            "-CAkey",
            "ca.key",
            "-CAcreateserial",
            "-days",
            "1",
            "-extfile",
            "ext.cnf",
            "-out",
            "srv.pem",
        ]);
        ok.then(|| {
            let l = |n: &str| std::fs::read(d.join(n)).unwrap();
            (l("srv.pem"), l("srv.key"), l("ca.pem"))
        })
    }

    pub struct Montado {
        pub srv: Arc<ServidorDispositivos>,
        pub no: Uuid,
        /// Comandos que CHEGARAM ao no.
        pub recebidos: Arc<AtomicUsize>,
        pub tarefa: tokio::task::JoinHandle<()>,
    }

    /// Servidor real (TLS) + no falso que pareia, abre sessao, bate o coracao e atende
    /// `device.command`: responde `device.system.info` e confere a cerca de cada comando.
    pub async fn subir(d: &std::path::Path) -> Option<Montado> {
        let (cert, chave, ca) = certificado(d)?;
        let tenant = Uuid::now_v7();
        let reg = Arc::new(Mutex::new(RegistroMemoria::default()));
        // o operador aprova no pareamento ate o que o no NAO declara (camera) e o
        // protegido (exec): as outras listas e que tem de barrar
        reg.lock().unwrap().emitir_token_aprovando(
            tenant,
            TOKEN,
            &[
                "device.system.info",
                "device.system.exec",
                "device.camera.shoot",
            ],
        );
        let srv = Arc::new(ServidorDispositivos::novo(reg).unwrap());
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("wss://localhost:{}/", l.local_addr().unwrap().port());
        tokio::spawn(srv.clone().servir(l, tls_de_pem(&cert, &chave).unwrap()));

        let id = NodeIdentity::efemera(Uuid::now_v7()).unwrap();
        let mut caps = platform_default_capabilities(DevicePlatform::Linux);
        caps.push(phxclaw_device_nodes::DeviceCapability {
            name: "device.system.exec".into(),
            version: "1".into(),
        });
        let mut c = WssDeviceClient::connect_with_ca(&url, &ca).await.unwrap();
        let pedido = EnrollmentRequest {
            tenant_uuid: tenant,
            node_uuid: id.node_uuid,
            display_name: "no falso".into(),
            platform: DevicePlatform::Linux,
            agent_version: "0.70.0".into(),
            public_key_ed25519_b64: id.public_key_ed25519_b64.clone(),
            enrollment_token: TOKEN.into(),
            capabilities: caps.clone(),
        };
        c.send(
            &id.sign_envelope(
                Uuid::nil(),
                0,
                "device.enroll",
                &serde_json::to_vec(&pedido).unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(c.receive().await.unwrap().kind, "device.enrolled");
        let ola = NodeHello {
            tenant_uuid: tenant,
            platform: DevicePlatform::Linux,
            agent_version: "0.70.0".into(),
            capabilities: caps,
        };
        c.send(
            &id.sign_envelope(
                Uuid::nil(),
                1,
                "device.hello",
                &serde_json::to_vec(&ola).unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
        let w = c.receive().await.unwrap();
        let b: Boasvindas = serde_json::from_slice(&w.decode_and_verify_body().unwrap()).unwrap();
        c.send(
            &id.sign_envelope(b.session_uuid, 1, "device.heartbeat", b"{}")
                .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(c.receive().await.unwrap().kind, "device.ack");
        let recebidos = Arc::new(AtomicUsize::new(0));
        let r2 = recebidos.clone();
        let no = id.node_uuid;
        let tarefa = tokio::spawn(async move {
            let mut seq = 1u64;
            while let Ok(env) = c.receive().await {
                if env.kind != "device.command" {
                    continue;
                }
                r2.fetch_add(1, Ordering::SeqCst);
                let cmd: phxclaw_device_nodes::DeviceCommand =
                    serde_json::from_slice(&env.decode_and_verify_body().unwrap()).unwrap();
                let r = if cmd.fencing_token != b.fencing_token {
                    ResultadoDeComando {
                        command_uuid: cmd.command_uuid,
                        ok: false,
                        saida: serde_json::Value::Null,
                        erro: Some("cerca vencida".into()),
                    }
                } else if cmd.capability == "device.system.info" {
                    ResultadoDeComando {
                        command_uuid: cmd.command_uuid,
                        ok: true,
                        saida: serde_json::json!({"hostname": "no-falso", "pediu": cmd.arguments}),
                        erro: None,
                    }
                } else {
                    ResultadoDeComando {
                        command_uuid: cmd.command_uuid,
                        ok: false,
                        saida: serde_json::Value::Null,
                        erro: Some(format!("nao sei fazer {}", cmd.capability)),
                    }
                };
                seq += 1;
                let env = id
                    .sign_envelope(
                        b.session_uuid,
                        seq,
                        "device.result",
                        &serde_json::to_vec(&r).unwrap(),
                    )
                    .unwrap();
                if c.send(&env).await.is_err() {
                    break;
                }
            }
        });
        Some(Montado {
            srv,
            no,
            recebidos,
            tarefa,
        })
    }
}

/// `node_list` lista o no conectado; `node_invoke` manda o permitido e traz a resposta
/// assinada do no; o protegido (exec sem aprovacao), o que o no nao declarou e o segredo
/// cru nos argumentos sao NEGADOS sem chegar ao no; no desconectado e falha. (As tres
/// listas contra o binario real do no estao em `apps/phxclaw-device-node/tests`.)
#[tokio::test]
async fn ferramenta_de_dispositivo_lista_e_comanda_so_o_permitido() {
    let d = tmp("disp");
    let Some(m) = no_falso::subir(&d).await else {
        eprintln!("sem openssl: pulado");
        return;
    };
    let politica = caps(&[
        "device.read",
        "device.command",
        "device.system.info",
        "device.system.exec",
        "device.camera.shoot",
    ]);
    let [listar, comandar] = phxclaw_agent::dispositivos::DeviceTool::par(m.srv.clone(), &politica);
    assert_eq!(
        (listar.capability(), comandar.capability()),
        ("device.read", "device.command")
    );
    let ctx = ToolContext {
        task_id: "t".into(),
        workdir: d.clone(),
        timeout: Duration::from_secs(30),
    };
    let v: Value =
        serde_json::from_str(&listar.run(json!({}), &ctx).await.unwrap().content).unwrap();
    let no = &v["nos"][0];
    assert_eq!(no["node"], json!(m.no));
    assert_eq!(no["conectado"], true);
    assert_eq!(no["estado"], "active");
    assert_eq!(
        no["invocaveis"],
        json!(["device.system.info", "device.system.exec"]),
        "declarada E aprovada E concedida"
    );

    let ok = comandar
        .run(
            json!({"node": m.no.to_string(), "capability": "device.system.info",
                   "arguments": {"campos": ["hostname"]}}),
            &ctx,
        )
        .await
        .unwrap();
    let v: Value = serde_json::from_str(&ok.content).unwrap();
    assert_eq!(v["saida"]["hostname"], "no-falso");
    assert_eq!(v["saida"]["pediu"], json!({"campos": ["hostname"]}));
    assert_eq!(m.recebidos.load(Ordering::SeqCst), 1);

    for (cap, args, esperado) in [
        ("device.system.exec", json!({"cmd": "id"}), "approval"),
        ("device.camera.shoot", json!({}), "not declared"),
        ("device.system.info", json!({"token": "abc"}), "secret"),
    ] {
        let e = comandar
            .run(
                json!({"node": m.no.to_string(), "capability": cap, "arguments": args}),
                &ctx,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(e, phxclaw_agent_core::ToolError::Denied(_)),
            "{cap}: {e}"
        );
        assert!(
            e.to_string().to_lowercase().contains(esperado),
            "{cap}: {e}"
        );
    }
    assert_eq!(
        m.recebidos.load(Ordering::SeqCst),
        1,
        "nada negado chegou ao no"
    );

    // o no cai: lista mostra desconectado e comandar falha (nao e negado)
    m.tarefa.abort();
    let _ = m.tarefa.await;
    let mut caiu = false;
    for _ in 0..100 {
        if m.srv.nos().iter().all(|n| !n.conectado) {
            caiu = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(caiu, "a sessao do no caido saiu do mapa");
    let e = comandar
        .run(
            json!({"node": m.no.to_string(), "capability": "device.system.info"}),
            &ctx,
        )
        .await
        .unwrap_err();
    assert!(matches!(e, phxclaw_agent_core::ToolError::Failed(_)), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

// ======================================================================== tarefas em worktree

async fn git(t: &dyn Tool, ctx: &ToolContext, args: Value) -> Value {
    let r = t
        .run(args.clone(), ctx)
        .await
        .unwrap_or_else(|e| panic!("{args}: {e}"));
    serde_json::from_str(&r.content).unwrap()
}

/// N tarefas paralelas, cada uma numa worktree propria e no proprio sandbox: a filha so
/// ve a sua arvore (o `ls -a /work` dela nao mostra o arquivo da irma nem o repositorio
/// principal), cada uma termina num commit no seu ramo com SO o seu arquivo, e a arvore
/// principal fica intocada.
#[tokio::test]
async fn tarefas_paralelas_em_worktrees_isoladas_viram_um_commit_por_ramo() {
    let Some(b) = bwrap() else { return };
    let raiz = tmp("nuvem");
    let store = TaskStore::new(raiz.join("tasks")).unwrap();
    let ctx = ctx_de(&store, &phxclaw_types::new_uuid_v7().to_string());
    let esc = phxclaw_agent::git::GitTool::escrita(b.clone());
    let ler = phxclaw_agent::git::GitTool::leitura(b.clone());
    git(
        &esc,
        &ctx,
        json!({"action":"init","path":"repo","branch":"main"}),
    )
    .await;
    std::fs::write(ctx.workdir.join("repo/LEIAME"), "base\n").unwrap();
    git(
        &esc,
        &ctx,
        json!({"action":"add","path":"repo","paths":["."]}),
    )
    .await;
    git(
        &esc,
        &ctx,
        json!({"action":"commit","path":"repo","message":"base"}),
    )
    .await;

    let llm = PorObjetivo::novo(Duration::from_millis(50));
    let t = phxclaw_agent::nuvem::ParallelTasksTool {
        llm: llm.clone(),
        tools: basicas(Some(&b)),
        config: AgentConfig::default().grant(&["fs.write", "fs.read", "shell.exec"]),
        store: store.clone(),
        bwrap: b.clone(),
        max_parallel: 4,
    };
    assert_eq!(t.capability(), "agent.parallel");
    let r = t
        .run(
            json!({"path":"repo","tasks":[
                {"name":"ta","objective":"[[escreve:ta]]"},
                {"name":"tb","objective":"[[escreve:tb]]","attempts":2}]}),
            &ctx,
        )
        .await
        .unwrap();
    let v: Value = serde_json::from_str(&r.content).unwrap();
    let tarefas = v["tarefas"].as_array().unwrap();
    // best-of-N: `tb` virou duas tentativas, cada uma na sua worktree
    let nomes: Vec<&str> = tarefas
        .iter()
        .map(|x| x["name"].as_str().unwrap())
        .collect();
    assert_eq!(nomes, ["ta", "tb-1", "tb-2"]);
    assert_eq!(llm.pico.load(Ordering::SeqCst), 3, "as tres no ar juntas");
    let mut commits = Vec::new();
    for (x, irma) in tarefas.iter().zip(["tb", "ta", "ta"]) {
        let n = x["name"].as_str().unwrap();
        let de = x["de"].as_str().unwrap();
        assert_eq!(x["concluida"], true, "{x}");
        assert_eq!(x["ramo"], format!("phxclaw/{n}"));
        assert_eq!(x["arquivos"], json!([format!("{de}.txt")]), "{x}");
        let resp = x["resposta"].as_str().unwrap();
        assert!(resp.contains(&format!("{de}.txt")), "{resp}");
        assert!(!resp.contains(&format!("{irma}.txt")), "ve a irma: {resp}");
        assert!(!resp.contains(".worktrees"), "ve o repo principal: {resp}");
        let c = x["commit"].as_str().unwrap().to_string();
        assert_eq!(c.len(), 40);
        // o commit esta no ramo da tarefa
        let log = git(
            &ler,
            &ctx,
            json!({"action":"log","path":"repo","rev":format!("phxclaw/{n}"),"limit":1}),
        )
        .await;
        assert_eq!(log["commits"][0]["commit"], c);
        let filha = store.load(x["tarefa"].as_str().unwrap()).unwrap();
        assert_eq!(filha.parent.as_deref(), Some(ctx.task_id.as_str()));
        commits.push(c);
    }
    commits.sort();
    commits.dedup();
    assert_eq!(commits.len(), 3, "um commit por ramo");
    // a arvore principal nao recebeu nada
    let st = git(&ler, &ctx, json!({"action":"status","path":"repo"})).await;
    assert_eq!(st["limpo"], true, "{st}");
    assert!(!ctx.workdir.join("repo/ta.txt").exists());
    let _ = std::fs::remove_dir_all(&raiz);
}

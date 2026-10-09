//! Frente «editor e remoto»: servidor de linguagem, ACP, PWA com ponte e x_search.
//! Tudo num arquivo so: cada binario de teste custa um link inteiro numa maquina com
//! quatro CPUs.

use phxclaw_agent::lsp::{Lsp, ligar_com};
use phxclaw_agent::{EditFileTool, WriteFileTool};
use phxclaw_agent_core::{Tool, ToolContext};
use phxclaw_test_support::pulado;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

fn ctx_em(d: &Path, tarefa: &str) -> ToolContext {
    ToolContext {
        task_id: tarefa.into(),
        workdir: d.to_path_buf(),
        timeout: Duration::from_secs(120),
    }
}

fn pasta(nome: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("phxclaw-editor-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// So os servidores reais do hospedeiro: a prova e contra o rust-analyzer e o pyright de
/// verdade, nao contra um falso. Sem eles o teste diz que pulou, alto.
fn lsp_real() -> Option<Arc<Lsp>> {
    let l = Lsp::do_hospedeiro();
    if l.is_none() {
        pulado::pular(
            "rust-analyzer",
            "sem bwrap ou sem servidor de linguagem no hospedeiro",
        );
    }
    l
}

const CARGO: &str = "[package]\nname = \"plantado\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
const COM_ERRO: &str = "fn dobro(x: i32) -> i32 {\n    x * 2\n}\n\nfn main() {\n    let n: i32 = \"texto\";\n    println!(\"{}\", dobro(n));\n}\n";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lsp_rust_real_acusa_erro_plantado_e_responde_consultas() {
    let Some(lsp) = lsp_real() else { return };
    if !lsp.servidores.iter().any(|s| s.linguagem == "rust") {
        pulado::pular("rust-analyzer", "sem rust-analyzer");
        return;
    }
    let d = pasta("rust");
    std::fs::write(d.join("Cargo.toml"), CARGO).unwrap();
    let ctx = ctx_em(&d, "t-rust");
    let mut tools: Vec<Arc<dyn Tool>> = vec![Arc::new(WriteFileTool), Arc::new(EditFileTool)];
    ligar_com(&mut tools, lsp.clone());
    let achar = |n: &str| tools.iter().find(|t| t.spec().name == n).unwrap().clone();

    // Grava com o erro de tipo: o resultado da PROPRIA gravacao traz o E0308.
    let r = achar("write_file")
        .run(json!({"path": "src/main.rs", "content": COM_ERRO}), &ctx)
        .await
        .unwrap();
    assert!(
        r.content.starts_with("gravado src/main.rs"),
        "{}",
        r.content
    );
    assert!(
        r.content.contains("src/main.rs:6:18 erro E0308"),
        "o erro plantado tinha de aparecer: {}",
        r.content
    );
    assert_eq!(
        r.artifacts.len(),
        1,
        "o envoltorio nao pode perder o artefato"
    );

    // Conserta pela edicao: o diagnostico do resultado diz que limpou.
    let r = achar("edit_file")
        .run(
            json!({"path": "src/main.rs", "old_text": "\"texto\"", "new_text": "21"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        r.content.contains("src/main.rs: nenhum erro ou aviso"),
        "{}",
        r.content
    );

    let lspt = achar("lsp");
    // Definicao de `dobro` a partir da chamada na linha 7.
    let r = lspt
        .run(
            json!({"action": "definition", "path": "src/main.rs", "line": 7, "column": 20}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(r.content.starts_with("src/main.rs:1:4"), "{}", r.content);
    let r = lspt
        .run(
            json!({"action": "references", "path": "src/main.rs", "line": 1, "column": 4}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(r.content.lines().count(), 2, "{}", r.content);
    let r = lspt
        .run(
            json!({"action": "hover", "path": "src/main.rs", "line": 7, "column": 20}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        r.content.contains("fn dobro(x: i32) -> i32"),
        "{}",
        r.content
    );
    let r = lspt
        .run(json!({"action": "symbols", "path": "src/main.rs"}), &ctx)
        .await
        .unwrap();
    assert!(r.content.contains("funcao dobro"), "{}", r.content);
    assert!(r.content.contains("funcao main"), "{}", r.content);

    // Arquivo mudado por FORA (o shell) e visto na consulta seguinte, sem gravar por nos.
    std::fs::write(d.join("src/main.rs"), COM_ERRO).unwrap();
    let r = lspt
        .run(
            json!({"action": "diagnostics", "path": "src/main.rs"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(r.content.contains("E0308"), "{}", r.content);

    // Caminho fora da pasta e negado pelo MESMO confine das ferramentas de arquivo.
    let e = lspt
        .run(json!({"action": "symbols", "path": "../fora.rs"}), &ctx)
        .await;
    assert!(e.is_err());

    // O fim da tarefa solta o servidor.
    assert_eq!(lsp.vivas().await, 1);
    lspt.finish("t-rust").await;
    assert_eq!(lsp.vivas().await, 0);
    let _ = std::fs::remove_dir_all(&d);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lsp_python_real_acusa_erro_plantado() {
    let Some(lsp) = lsp_real() else { return };
    if !lsp.servidores.iter().any(|s| s.linguagem == "python") {
        pulado::pular("pyright", "sem pyright");
        return;
    }
    let d = pasta("py");
    let ctx = ctx_em(&d, "t-py");
    let mut tools: Vec<Arc<dyn Tool>> = vec![Arc::new(WriteFileTool)];
    ligar_com(&mut tools, lsp.clone());
    let r = tools[0]
        .run(
            json!({"path": "m.py", "content": "def dobro(x: int) -> int:\n    return x * 2\n\nn: int = \"texto\"\nprint(dobro(n))\n"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        r.content.contains("m.py:4:10 erro reportAssignmentType"),
        "{}",
        r.content
    );
    // Arquivo sem servidor (markdown): a gravacao sai como saia, sem nota nenhuma.
    let r = tools[0]
        .run(json!({"path": "notas.md", "content": "# oi\n"}), &ctx)
        .await
        .unwrap();
    assert!(!r.content.contains("diagnostico"), "{}", r.content);
    tools[1].finish("t-py").await;
    assert_eq!(lsp.vivas().await, 0);
    let _ = std::fs::remove_dir_all(&d);
}

// ------------------------------------------------------------------ ACP

mod acp {
    use phxclaw_agent::{Agent, AgentConfig, ReadFileTool, ScriptedLlm, TaskStore, WriteFileTool};
    use phxclaw_agent_core::Tool;
    use serde_json::{Value, json};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, Lines};

    /// Cliente JSON-RPC roteirizado: escreve uma linha, le ate a resposta do mesmo id e
    /// devolve as notificacoes que vieram antes dela.
    pub struct Cliente {
        escrita: tokio::io::WriteHalf<DuplexStream>,
        linhas: Lines<BufReader<tokio::io::ReadHalf<DuplexStream>>>,
        prox: i64,
    }

    impl Cliente {
        pub async fn pedir(&mut self, metodo: &str, params: Value) -> (Value, Vec<Value>) {
            self.prox += 1;
            let id = self.prox;
            self.mandar(json!({"jsonrpc":"2.0","id":id,"method":metodo,"params":params}))
                .await;
            let mut notas = Vec::new();
            loop {
                let l = tokio::time::timeout(Duration::from_secs(30), self.linhas.next_line())
                    .await
                    .expect("sem resposta em 30 s")
                    .unwrap()
                    .expect("fio fechou");
                let v: Value = serde_json::from_str(&l).unwrap();
                if v.get("id") == Some(&json!(id)) && v.get("method").is_none() {
                    return (v, notas);
                }
                notas.push(v);
            }
        }
        pub async fn mandar(&mut self, v: Value) {
            let mut b = serde_json::to_vec(&v).unwrap();
            b.push(b'\n');
            self.escrita.write_all(&b).await.unwrap();
        }
    }

    pub fn subir(llm: ScriptedLlm, raiz: &std::path::Path) -> (Cliente, Arc<ScriptedLlm>) {
        let llm = Arc::new(llm);
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(WriteFileTool), Arc::new(ReadFileTool)];
        let config = AgentConfig {
            prazo_de_resposta: Some(Duration::from_secs(60)),
            ..AgentConfig::default()
        }
        .grant(&["fs.read", "fs.write", "user.ask"]);
        let store = TaskStore::new(raiz.join("tasks")).unwrap();
        let agente = Agent::new(llm.clone(), tools, config, store);
        let (cli, srv) = tokio::io::duplex(1 << 20);
        let (srv_l, srv_e) = tokio::io::split(srv);
        tokio::spawn(phxclaw_agent::acp::servir(
            agente,
            BufReader::new(srv_l),
            srv_e,
        ));
        let (cli_l, cli_e) = tokio::io::split(cli);
        (
            Cliente {
                escrita: cli_e,
                linhas: BufReader::new(cli_l).lines(),
                prox: 0,
            },
            llm,
        )
    }

    pub fn updates<'a>(notas: &'a [Value], tipo: &str) -> Vec<&'a Value> {
        notas
            .iter()
            .filter(|n| n["method"] == "session/update")
            .map(|n| &n["params"]["update"])
            .filter(|u| u["sessionUpdate"] == tipo)
            .collect()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acp_cliente_roteirizado_edita_o_projeto_do_editor_e_responde_pergunta() {
    use phxclaw_agent::ScriptedLlm;
    let d = pasta("acp");
    let projeto = d.join("projeto");
    std::fs::create_dir_all(&projeto).unwrap();
    let (mut c, llm) = acp::subir(
        ScriptedLlm::new(vec![
            ScriptedLlm::call(
                "c1",
                "write_file",
                json!({"path": "ola.txt", "content": "oi"}),
            ),
            ScriptedLlm::text("criei ola.txt"),
            ScriptedLlm::call("c2", "ask_user", json!({"question": "Qual cor?"})),
            ScriptedLlm::text("cor escolhida"),
        ]),
        &d,
    );

    let (r, _) = c
        .pedir(
            "initialize",
            json!({"protocolVersion": 1, "clientCapabilities": {}}),
        )
        .await;
    assert_eq!(r["result"]["protocolVersion"], 1, "{r}");
    let (r, _) = c
        .pedir("session/new", json!({"cwd": "relativo", "mcpServers": []}))
        .await;
    assert_eq!(r["error"]["code"], -32602, "cwd relativo se recusa: {r}");
    let (r, _) = c
        .pedir(
            "session/new",
            json!({"cwd": projeto.display().to_string(), "mcpServers": []}),
        )
        .await;
    let sid = r["result"]["sessionId"].as_str().unwrap().to_string();

    // Turno 1: a ferramenta grava NO PROJETO do editor, e o passo vira tool_call "edit".
    let (r, notas) = c
        .pedir(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": "crie ola.txt"}]}),
        )
        .await;
    assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
    assert_eq!(
        std::fs::read_to_string(projeto.join("ola.txt")).unwrap(),
        "oi"
    );
    let chamadas = acp::updates(&notas, "tool_call");
    assert_eq!(chamadas.len(), 1, "{notas:?}");
    assert_eq!(chamadas[0]["kind"], "edit");
    assert_eq!(chamadas[0]["title"], "write_file");
    assert_eq!(
        acp::updates(&notas, "tool_call_update")[0]["status"],
        "completed"
    );
    let msgs = acp::updates(&notas, "agent_message_chunk");
    assert_eq!(msgs.last().unwrap()["content"]["text"], "criei ola.txt");
    assert!(
        acp::updates(&notas, "agent_thought_chunk").is_empty(),
        "a resposta final nao sai tambem como pensamento: {notas:?}"
    );

    // Turno 2: a tarefa pergunta; o turno acaba com a pergunta como mensagem...
    let (r, notas) = c
        .pedir(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": "escolha a cor"}]}),
        )
        .await;
    assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
    let msgs = acp::updates(&notas, "agent_message_chunk");
    assert_eq!(msgs.last().unwrap()["content"]["text"], "Qual cor?");
    // ...e o proximo prompt e a RESPOSTA, entregue a mesma tarefa.
    let (r, notas) = c
        .pedir(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": "azul"}]}),
        )
        .await;
    assert_eq!(r["result"]["stopReason"], "end_turn", "{r}");
    let msgs = acp::updates(&notas, "agent_message_chunk");
    assert_eq!(msgs.last().unwrap()["content"]["text"], "cor escolhida");
    let ultimo = format!("{:?}", llm.seen.lock().unwrap().last().unwrap().0);
    assert!(
        ultimo.contains("azul"),
        "a resposta chegou ao modelo: {ultimo}"
    );
    // O turno 2 levou a conversa do turno 1 no objetivo.
    assert!(ultimo.contains("criei ola.txt"), "{ultimo}");
    // A resposta foi para a MESMA tarefa que perguntou, e nao virou tarefa nova: duas
    // tarefas (turno 1 e turno 2), a segunda concluida com a resposta pos-pergunta.
    let tarefas = phxclaw_agent::TaskStore::new(d.join("tasks"))
        .unwrap()
        .list()
        .unwrap();
    assert_eq!(tarefas.len(), 2, "{tarefas:?}");
    assert!(
        tarefas.iter().any(|t| t.objective.contains("escolha a cor")
            && t.answer.as_deref() == Some("cor escolhida")),
        "{tarefas:?}"
    );

    // Metodo que nao existe: erro JSON-RPC, e o fio continua.
    let (r, _) = c.pedir("session/load", json!({})).await;
    assert_eq!(r["error"]["code"], -32601, "{r}");
    let (r, _) = c
        .pedir(
            "session/prompt",
            json!({"sessionId": "nao-existe", "prompt": [{"type": "text", "text": "x"}]}),
        )
        .await;
    assert_eq!(r["error"]["code"], -32602, "{r}");
    let _ = std::fs::remove_dir_all(&d);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acp_cancelar_encerra_o_turno_com_cancelled() {
    use phxclaw_agent::ScriptedLlm;
    let d = pasta("acp-cancel");
    let (mut c, _) = acp::subir(
        ScriptedLlm::new(vec![ScriptedLlm::call(
            "c1",
            "ask_user",
            json!({"question": "Continuo?"}),
        )]),
        &d,
    );
    let (r, _) = c
        .pedir("session/new", json!({"cwd": d.display().to_string()}))
        .await;
    let sid = r["result"]["sessionId"].as_str().unwrap().to_string();
    let (r, _) = c
        .pedir(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": "faca"}]}),
        )
        .await;
    assert_eq!(r["result"]["stopReason"], "end_turn");
    // Cancela com a tarefa parada na pergunta; o prompt seguinte nao pode cair nela.
    c.mandar(json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId": sid}}))
        .await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    let (r, _) = c
        .pedir(
            "session/prompt",
            json!({"sessionId": sid, "prompt": [{"type": "text", "text": "outra"}]}),
        )
        .await;
    assert!(r.get("result").is_some(), "{r}");
    let _ = std::fs::remove_dir_all(&d);
}

// ------------------------------------------------------------------ PWA + ponte

/// CA propria e certificado de `localhost`, como no teste do no real. `None` sem openssl.
fn certificado(d: &Path) -> Option<(Vec<u8>, Vec<u8>)> {
    let sh = |args: &[&str]| {
        std::process::Command::new("openssl")
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
        "/CN=CA ponte",
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
        (l("srv.pem"), l("srv.key"))
    })
}

#[test]
fn ponte_so_deixa_passar_as_rotas_de_tarefa() {
    use phxclaw_agent::remoto::permitido;
    for (m, c) in [
        ("GET", "/v1/tasks"),
        ("POST", "/v1/tasks"),
        ("GET", "/v1/tasks/abc"),
        ("POST", "/v1/tasks/abc/answer"),
        ("POST", "/v1/tasks/abc/approve"),
        ("POST", "/v1/tasks/abc/cancel"),
        ("GET", "/v1/tasks/abc/artifacts/saida/a.md"),
    ] {
        assert!(permitido(m, c), "{m} {c}");
    }
    for (m, c) in [
        ("GET", "/v1/schedules"),
        ("POST", "/v1/schedules"),
        ("DELETE", "/v1/tasks/abc"),
        ("GET", "/v1/tasks/../schedules"),
        ("GET", "/v1/tasks/abc/artifacts/"),
        ("GET", "/v1/tasks?x=1"),
        ("GET", "/sites/abc/index.html"),
        ("GET", "//v1/tasks"),
    ] {
        assert!(!permitido(m, c), "{m} {c}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ponte_rele_o_cliente_ao_agente_ligado_de_saida_e_o_token_da_api_nao_sai() {
    use phxclaw_agent::api::{AgentFactory, ApiState, Limite, router};
    use phxclaw_agent::remoto::{ConfigDaPonte, ligar_a_ponte, rotas_da_ponte};
    use phxclaw_agent::{Agenda, Agent, AgentConfig, ScriptedLlm, TaskStore};
    use phxclaw_device_transport::servidor::{RegistroMemoria, ServidorDispositivos, tls_de_pem};
    use std::collections::HashMap;
    use std::sync::Mutex;

    const TOKEN_PAREAMENTO: &str = "token-de-pareamento-da-ponte-com-folga";
    const TOKEN_API: &str = "token-da-api-do-agente-que-nao-sai-daqui";
    const TOKEN_PONTE: &str = "token-do-cliente-na-ponte-com-folga-ok";
    let d = pasta("ponte");
    let Some((cert, chave)) = certificado(&d) else {
        pulado::pular("openssl", "sem openssl");
        return;
    };
    // A ponte: servidor de dispositivos (agente entra de SAIDA) e o HTTP do cliente.
    let tenant = phxclaw_agent::remoto::Uuid::now_v7();
    let reg = RegistroMemoria::default();
    reg.emitir_token_aprovando(tenant, TOKEN_PAREAMENTO, &["agent.http"]);
    let srv = Arc::new(ServidorDispositivos::novo(Arc::new(Mutex::new(reg))).unwrap());
    let lw = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let porta_wss = lw.local_addr().unwrap().port();
    tokio::spawn(srv.clone().servir(lw, tls_de_pem(&cert, &chave).unwrap()));
    let lh = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", lh.local_addr().unwrap());
    let app_ponte = rotas_da_ponte(srv.clone(), TOKEN_PONTE.into());
    tokio::spawn(async move { axum::serve(lh, app_ponte).await.unwrap() });

    // O agente: API em processo, SEM porta; so a conexao de saida ate a ponte.
    let store = TaskStore::new(d.join("tasks")).unwrap();
    let st2 = store.clone();
    let factory: AgentFactory = Arc::new(move |_m: &str| {
        let llm = Arc::new(ScriptedLlm::new(vec![
            ScriptedLlm::call("c1", "ask_user", json!({"question": "Qual cor?"})),
            ScriptedLlm::text("cor anotada"),
        ]));
        Ok(Agent::new(
            llm,
            vec![],
            AgentConfig {
                prazo_de_resposta: Some(Duration::from_secs(60)),
                ..AgentConfig::default()
            }
            .grant(&["user.ask"]),
            st2.clone(),
        ))
    });
    let state = ApiState {
        usuarios: Default::default(),
        store,
        factory,
        default_model: "roteiro".into(),
        token: TOKEN_API.into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(d.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(100)),
    };
    let no = phxclaw_agent::remoto::Uuid::now_v7();
    tokio::spawn(ligar_a_ponte(
        ConfigDaPonte {
            url: format!("wss://localhost:{porta_wss}/"),
            ca_pem: Some(std::fs::read(d.join("ca.pem")).unwrap()),
            tenant,
            no,
            token_pareamento: Some(TOKEN_PAREAMENTO.into()),
            pasta_da_chave: d.join("chave"),
        },
        router(state.clone()),
        TOKEN_API.into(),
    ));
    let mut ligado = false;
    for _ in 0..250 {
        if srv.nos().iter().any(|n| n.conectado) {
            ligado = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(ligado, "o agente nao se ligou a ponte");

    let c = reqwest::Client::new();
    let com = |t: &str| format!("Bearer {t}");
    // Sem token, ou com o token DA API (que so o agente conhece): a ponte recusa.
    for t in ["", TOKEN_API] {
        let r = c
            .get(format!("{base}/v1/tasks"))
            .header("authorization", com(t))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 401, "token {t:?}");
    }
    // Rota fora do controle remoto: recusada na ponte.
    let r = c
        .get(format!("{base}/v1/schedules"))
        .header("authorization", com(TOKEN_PONTE))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
    // Criar pela ponte, acompanhar ate a pergunta, responder (rota /answer) e ver concluir.
    let r = c
        .post(format!("{base}/v1/tasks"))
        .header("authorization", com(TOKEN_PONTE))
        .json(&json!({"objective": "escolha a cor"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);
    let id = r.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let esperar = |estado: &'static str| {
        let (c, base, id) = (c.clone(), base.clone(), id.clone());
        async move {
            for _ in 0..200 {
                let v: serde_json::Value = c
                    .get(format!("{base}/v1/tasks/{id}"))
                    .header("authorization", format!("Bearer {TOKEN_PONTE}"))
                    .send()
                    .await
                    .unwrap()
                    .json()
                    .await
                    .unwrap();
                if v["status"] == estado {
                    return v;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            panic!("a tarefa nao chegou a {estado}");
        }
    };
    let v = esperar("awaiting_input").await;
    assert_eq!(v["question"], "Qual cor?");
    let r = c
        .post(format!("{base}/v1/tasks/{id}/answer"))
        .header("authorization", com(TOKEN_PONTE))
        .json(&json!({"answer": "azul"}))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "{}", r.status());
    let v = esperar("completed").await;
    assert_eq!(v["answer"], "cor anotada");
    // A tela instalavel sai pela ponte tambem: manifesto e service worker.
    let r = c
        .get(format!("{base}/manifest.webmanifest"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers()["content-type"], "application/manifest+json");
    // Caminho que sai da pasta da tela nao vira arquivo (conferido na funcao: o cliente
    // HTTP normaliza o `..` antes de mandar, e o teste pelo fio passaria por engano).
    let ui = phxclaw_agent::pwa::pasta_da_interface().expect("pasta da tela");
    assert!(phxclaw_agent::pwa::ler(&ui, "manifest.webmanifest").is_some());
    for fora in [
        "../Cargo.toml",
        "assets/../../Cargo.toml",
        "/etc/passwd",
        "",
    ] {
        assert!(phxclaw_agent::pwa::ler(&ui, fora).is_none(), "{fora}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

// ------------------------------------------------------------------ x_search

#[test]
fn x_search_confere_as_datas_antes_de_pedir() {
    use chrono::NaiveDate;
    use phxclaw_agent::xai::conferir_datas;
    let hoje = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    assert!(conferir_datas(Some("2026-09-01"), Some("2026-10-01"), hoje).is_ok());
    assert!(conferir_datas(None, None, hoje).is_ok());
    for (de, ate) in [
        (Some("2026-10-02"), None),
        (None, Some("2027-01-01")),
        (Some("2026-09-10"), Some("2026-09-01")),
        (Some("01/09/2026"), None),
        (Some("2026-02-30"), None),
    ] {
        assert!(conferir_datas(de, ate, hoje).is_err(), "{de:?} {ate:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn x_search_contra_servidor_falso_cita_fontes_e_nao_vaza_a_chave() {
    use axum::Json;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use phxclaw_agent::canais::http::Credencial;
    use phxclaw_agent::xai::XSearchTool;
    use phxclaw_secret_broker::SecretValue;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const CHAVE: &str = "xai-chave-falsa-0123456789abcdef";
    let d = pasta("xai");
    let pedidos = Arc::new(AtomicUsize::new(0));
    let vistos = pedidos.clone();
    let app = axum::Router::new().route(
        "/v1/responses",
        post(move |h: HeaderMap, Json(v): Json<serde_json::Value>| {
            let vistos = vistos.clone();
            async move {
                vistos.fetch_add(1, Ordering::SeqCst);
                if h["authorization"] != format!("Bearer {CHAVE}").as_str() {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(json!({"error": "sem chave"})),
                    );
                }
                if v["input"][0]["content"] == "eco" {
                    // Servidor que ecoa a credencial no erro: o texto nao pode chegar ao modelo.
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("chave recusada: {CHAVE}")})),
                    );
                }
                assert_eq!(v["tools"][0]["type"], "x_search");
                assert_eq!(v["tools"][0]["from_date"], "2026-09-01");
                assert_eq!(v["tools"][0]["allowed_x_handles"], json!(["xai"]));
                (
                    StatusCode::OK,
                    Json(json!({"output": [
                        {"type": "x_search_call", "status": "completed"},
                        {"type": "message", "content": [{"type": "output_text",
                            "text": "Dois posts falam disso.",
                            "annotations": [
                                {"type": "url_citation", "url": "https://x.com/xai/status/1"},
                                {"type": "url_citation", "url": "https://x.com/xai/status/1"},
                                {"type": "url_citation", "url": "https://x.com/xai/status/2"}]}]}
                    ]})),
                )
            }
        }),
    );
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });

    let broker = phxclaw_agent::canais::broker_em(&d.join("xai")).unwrap();
    let escopos = phxclaw_agent::canais::escopos("xai");
    let escopos: Vec<&str> = escopos.iter().map(String::as_str).collect();
    let id = phxclaw_agent::canais::guardar_segredo(
        &broker,
        "xai-chave",
        "xai",
        &escopos,
        SecretValue::new(CHAVE.into()),
    )
    .unwrap();
    let t = XSearchTool::novo(&base, "grok-4", Credencial::nova(broker, id, "xai")).unwrap();
    let ctx = ctx_em(&d, "t-x");
    // Data no futuro: recusada sem pedido nenhum (chamada paga para receber erro).
    let e = t
        .run(json!({"query": "q", "from_date": "2999-01-01"}), &ctx)
        .await
        .unwrap_err();
    assert!(e.to_string().contains("futuro"), "{e}");
    assert_eq!(pedidos.load(Ordering::SeqCst), 0);
    let r = t
        .run(
            json!({"query": "lancamento", "from_date": "2026-09-01", "handles": ["@xai"]}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        r.content.starts_with("Dois posts falam disso."),
        "{}",
        r.content
    );
    assert_eq!(
        r.content.matches("https://x.com/xai/status/").count(),
        2,
        "fontes sem repetir: {}",
        r.content
    );
    let e = t.run(json!({"query": "eco"}), &ctx).await.unwrap_err();
    assert!(!e.to_string().contains(CHAVE), "a chave vazou: {e}");
    assert!(e.to_string().contains("400"), "{e}");
    assert_eq!(pedidos.load(Ordering::SeqCst), 2);
    let _ = std::fs::remove_dir_all(&d);
}

// ------------------------------------------------------------------ SP000031, frente V2

/// A ferramenta `rust_project` de verdade, ou o teste diz que pulou.
fn rust_project() -> Option<phxclaw_agent::sistema::RustProjectTool> {
    let b = phxclaw_agent::arquivos::achar_bwrap()?;
    let r = phxclaw_agent::sistema::RustProjectTool::detectar(b);
    if r.is_none() {
        pulado::pular("cargo", "sem toolchain Rust no hospedeiro");
    }
    r
}

async fn json_de(t: &dyn Tool, ctx: &ToolContext, args: serde_json::Value) -> serde_json::Value {
    let r = t
        .run(args.clone(), ctx)
        .await
        .unwrap_or_else(|e| panic!("{} {args}: {e}", t.spec().name));
    serde_json::from_str(&r.content).unwrap_or_else(|_| panic!("nao e JSON: {}", r.content))
}

const DBG_MAIN: &str = "fn main() {\n    let x: i32 = 41 + 1;\n    let nome = \"phx\";\n    println!(\"{} {}\", x, nome);\n}\n";

/// O principal do depurador: um binario Rust compilado com informacao de depuracao
/// (pelo `rust_project`, no mesmo sandbox), o gdb em DAP para no breakpoint da linha 4,
/// `evaluate x` devolve 42 (o console), `variables` lista `x` e `nome`, `stack` mostra o
/// main, `continue` corre ate o fim e a saida do programa volta; `finish` solta a sessao.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn depurador_real_para_no_breakpoint_e_evaluate_devolve_o_valor() {
    let Some(rust) = rust_project() else { return };
    if !std::path::Path::new("/usr/bin/gdb").is_file() {
        pulado::pular("gdb", "sem gdb no hospedeiro");
        return;
    }
    let d = pasta("dap");
    std::fs::create_dir_all(d.join("dbg/src")).unwrap();
    std::fs::write(
        d.join("dbg/Cargo.toml"),
        "[package]\nname = \"dbg\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(d.join("dbg/src/main.rs"), DBG_MAIN).unwrap();
    let ctx = ctx_em(&d, "t-dap");
    let v = json_de(&rust, &ctx, json!({"action": "build", "path": "dbg"})).await;
    assert_eq!(v["sucesso"], true, "{v}");

    let dep = Arc::new(phxclaw_agent::dap::Depurador::do_hospedeiro(
        phxclaw_agent::arquivos::achar_bwrap().unwrap(),
    ));
    assert!(dep.adaptador("rust").is_some(), "faltam: {:?}", dep.faltam);
    let t = phxclaw_agent::dap::DebugTool {
        depurador: dep.clone(),
    };
    assert_eq!(t.capability(), "shell.exec");
    let v = json_de(
        &t,
        &ctx,
        json!({"action": "start", "program": "dbg/target/debug/dbg",
               "breakpoints": [{"file": "dbg/src/main.rs", "line": 4}]}),
    )
    .await;
    assert_eq!(v["parado"]["motivo"], "breakpoint", "{v}");
    assert_eq!(
        v["breakpoints"][0]["breakpoints"][0]["verificado"], true,
        "{v}"
    );
    assert_eq!(v["terminado"], false, "{v}");

    // O console: a expressao avaliada no quadro parado.
    let v = json_de(&t, &ctx, json!({"action": "evaluate", "expression": "x"})).await;
    assert_eq!(v["resultado"], "42", "{v}");
    let v = json_de(&t, &ctx, json!({"action": "variables"})).await;
    let locais = v["escopos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["escopo"] == "Locals")
        .unwrap_or_else(|| panic!("{v}"));
    let nomes: Vec<&str> = locais["variaveis"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["nome"].as_str().unwrap())
        .collect();
    assert!(nomes.contains(&"x") && nomes.contains(&"nome"), "{v}");
    let v = json_de(&t, &ctx, json!({"action": "stack"})).await;
    assert!(
        v["quadros"][0]["funcao"]
            .as_str()
            .is_some_and(|f| f.contains("main")),
        "{v}"
    );
    assert_eq!(v["quadros"][0]["arquivo"], "dbg/src/main.rs", "{v}");
    assert_eq!(v["quadros"][0]["linha"], 4, "{v}");

    let v = json_de(&t, &ctx, json!({"action": "continue"})).await;
    assert_eq!(v["terminado"], true, "{v}");
    assert!(
        v["saida"].as_array().unwrap().iter().any(|l| l == "42 phx"),
        "{v}"
    );
    // Arquivo fora da pasta e negado pelo MESMO confine.
    let e = t
        .run(
            json!({"action": "breakpoint", "file": "../fora.rs", "line": 1}),
            &ctx,
        )
        .await;
    assert!(e.is_err());
    assert_eq!(dep.vivas().await, 1);
    t.finish("t-dap").await;
    assert_eq!(dep.vivas().await, 0);
    let _ = std::fs::remove_dir_all(&d);
}

const CALC_LIB: &str = "pub fn soma(a: i32, b: i32) -> i32 {\n    a + b\n}\n\n#[cfg(test)]\nmod soma {\n    #[test]\n    fn dois_mais_dois() {\n        assert_eq!(super::soma(2, 2), 4);\n    }\n    #[test]\n    fn zero() {\n        assert_eq!(super::soma(0, 0), 0);\n    }\n}\n";

/// O explorador de testes: a arvore crate/modulo/teste do `cargo test -- --list` tem o
/// teste conhecido, e rodar o no `calc/soma::dois_mais_dois` devolve passou com 1 ok.
/// Python (se ha interpretador): a arvore do `--collect-only` e o no do pytest.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explorador_de_testes_lista_e_roda_um_no() {
    let Some(_) = rust_project() else { return };
    let bwrap = phxclaw_agent::arquivos::achar_bwrap().unwrap();
    let e = Arc::new(phxclaw_agent::testes::ExploradorDeTestes::detectar(bwrap).unwrap());
    let d = pasta("testes");
    std::fs::create_dir_all(d.join("calc/src")).unwrap();
    std::fs::write(
        d.join("calc/Cargo.toml"),
        "[package]\nname = \"calc\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(d.join("calc/src/lib.rs"), CALC_LIB).unwrap();
    let ctx = ctx_em(&d, "t-testes");
    let tools = phxclaw_agent::testes::ferramentas(e.clone());
    let (listar, rodar) = (&tools[0], &tools[1]);
    assert_eq!(listar.spec().name, "test_list");
    assert_eq!(rodar.spec().name, "test_run");
    let v = json_de(listar.as_ref(), &ctx, json!({"path": "calc"})).await;
    assert_eq!(v["linguagem"], "rust", "{v}");
    assert_eq!(v["total"], 2, "{v}");
    assert_eq!(v["crates"][0]["crate"], "calc", "{v}");
    assert_eq!(v["crates"][0]["modulos"][0]["modulo"], "soma", "{v}");
    assert_eq!(
        v["crates"][0]["modulos"][0]["testes"],
        json!(["dois_mais_dois", "zero"]),
        "{v}"
    );
    let v = json_de(
        rodar.as_ref(),
        &ctx,
        json!({"path": "calc", "node": "calc/soma::dois_mais_dois"}),
    )
    .await;
    assert_eq!(v["passou"], true, "{v}");
    assert_eq!(v["ok"], 1, "{v}");
    assert_eq!(v["falhou"], 0, "{v}");
    // No que nao existe: nada roda e passou e false (o cargo diz 0 testes).
    let v = json_de(
        rodar.as_ref(),
        &ctx,
        json!({"path": "calc", "node": "calc/soma::nao_existe"}),
    )
    .await;
    assert_eq!(v["ok"], 0, "{v}");
    let err = rodar
        .run(json!({"path": "calc", "node": "calc/x; id"}), &ctx)
        .await;
    assert!(err.is_err());

    if e.python.is_some() {
        std::fs::create_dir_all(d.join("py/tests")).unwrap();
        std::fs::write(d.join("py/calc.py"), "def soma(a, b):\n    return a + b\n").unwrap();
        std::fs::write(
            d.join("py/tests/test_calc.py"),
            "from calc import soma\n\n\ndef test_ok():\n    assert soma(1, 2) == 3\n",
        )
        .unwrap();
        let v = json_de(listar.as_ref(), &ctx, json!({"path": "py"})).await;
        assert_eq!(v["linguagem"], "python", "{v}");
        assert_eq!(v["arquivos"][0]["arquivo"], "tests/test_calc.py", "{v}");
        assert_eq!(v["arquivos"][0]["testes"], json!(["test_ok"]), "{v}");
        let v = json_de(
            rodar.as_ref(),
            &ctx,
            json!({"path": "py", "node": "tests/test_calc.py::test_ok"}),
        )
        .await;
        assert_eq!(v["passou"], true, "{v}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Workspace de duas raizes: com `.phxclaw/workspace.json` apontando a segunda, o `grep`
/// acha nela (pelo MESMO confine das ferramentas de arquivo), o LSP resolve o simbolo
/// do arquivo da segunda raiz, e um caminho absoluto fora das duas continua negado.
/// A raiz do projeto vem de PHXCLAW_PROJETO, como nos testes de configuracao.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_de_duas_raizes_busca_e_lsp_na_segunda() {
    let Some(lsp) = lsp_real() else { return };
    if !lsp.servidores.iter().any(|s| s.linguagem == "rust") {
        pulado::pular("rust-analyzer", "sem rust-analyzer");
        return;
    }
    let projeto = pasta("ws-projeto");
    let segunda = pasta("ws-segunda");
    let segunda = std::fs::canonicalize(&segunda).unwrap();
    std::fs::create_dir_all(segunda.join("src")).unwrap();
    std::fs::write(
        segunda.join("Cargo.toml"),
        "[package]\nname = \"segunda\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        segunda.join("src/lib.rs"),
        "pub fn simbolo_da_segunda_raiz() -> u8 {\n    7\n}\n",
    )
    .unwrap();
    std::fs::create_dir_all(projeto.join(".phxclaw")).unwrap();
    std::fs::write(
        projeto.join(".phxclaw/workspace.json"),
        format!(r#"{{"raizes": ["{}"]}}"#, segunda.display()),
    )
    .unwrap();
    // SAFETY: o mesmo que tests/config.rs faz; nenhuma outra prova deste binario depende
    // de PHXCLAW_PROJETO, e uma raiz extra valida nao muda o resultado delas.
    unsafe {
        std::env::set_var("PHXCLAW_PROJETO", &projeto);
    }
    // Raiz extra e permissao de disco: so de projeto confiado (09/10/2026). A pasta do
    // agente fixada aqui e a que guarda a lista de confiados.
    let agente = pasta("ws-agente");
    phxclaw_agent::config::fixar_pasta(&agente);
    phxclaw_agent::instrucoes::confiar(&agente, &projeto).unwrap();
    assert_eq!(
        phxclaw_agent::workspace::raizes().unwrap(),
        vec![segunda.clone()]
    );
    let d = pasta("ws-tarefa");
    std::fs::write(d.join("Cargo.toml"), CARGO).unwrap();
    std::fs::create_dir_all(d.join("src")).unwrap();
    std::fs::write(d.join("src/main.rs"), "fn main() {}\n").unwrap();
    let ctx = ctx_em(&d, "t-ws");

    let grep = phxclaw_agent::busca::GrepTool;
    let arq = segunda.join("src/lib.rs").display().to_string();
    // O grep anda numa PASTA (a segunda raiz inteira), como no projeto da tarefa.
    let r = grep
        .run(
            json!({"pattern": "simbolo_da_segunda_raiz", "path": segunda.display().to_string()}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        r.content.contains("simbolo_da_segunda_raiz"),
        "{}",
        r.content
    );
    let fora = std::env::temp_dir().display().to_string();
    let e = grep
        .run(json!({"pattern": "x", "path": fora}), &ctx)
        .await
        .unwrap_err();
    assert!(matches!(e, phxclaw_agent_core::ToolError::Denied(_)), "{e}");

    let mut tools: Vec<Arc<dyn Tool>> = vec![];
    ligar_com(&mut tools, lsp.clone());
    let lspt = tools.iter().find(|t| t.spec().name == "lsp").unwrap();
    let r = lspt
        .run(json!({"action": "symbols", "path": arq}), &ctx)
        .await
        .unwrap();
    assert!(
        r.content.contains("funcao simbolo_da_segunda_raiz"),
        "{}",
        r.content
    );
    lspt.finish("t-ws").await;
    unsafe {
        std::env::remove_var("PHXCLAW_PROJETO");
    }
    for p in [&projeto, &segunda, &d, &agente] {
        let _ = std::fs::remove_dir_all(p);
    }
}

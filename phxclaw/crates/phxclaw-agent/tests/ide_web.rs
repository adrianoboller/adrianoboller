//! O IDE no navegador pelo agente real em processo (`api::router`): o websocket do
//! terminal recusa token errado e bash, abre o Helix na pasta do projeto e manda a grade
//! com a linha de estado dele; `:q` encerra. A completacao por IA responde pelo modelo da
//! fabrica e recusa sem Bearer. Os simbolos: so a recusa de arquivo sem servidor (o
//! rust-analyzer real e provado pelo roteiro tests/desktop/ide_web.mjs, que mede o tempo).

use futures_util::{SinkExt, StreamExt};
use phxclaw_agent::agenda::Agenda;
use phxclaw_agent::api::{AgentFactory, ApiState, Limite, router};
use phxclaw_agent::motor::Agent;
use phxclaw_agent::tarefa::TaskStore;
use phxclaw_agent_core::{BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, ToolSpec, Usage};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio_tungstenite::tungstenite::Message as WsMsg;

const TOKEN: &str = "token-de-teste-com-mais-de-24-caracteres";

/// Os dois testes fixam `PHXCLAW_PROJETO` (ambiente do processo): um de cada vez.
static UM_DE_CADA_VEZ: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Um modelo que devolve sempre a mesma continuacao, com cerca para provar que ela sai.
struct ModeloFalso;
impl Llm for ModeloFalso {
    fn id(&self) -> String {
        "falso:ide".into()
    }
    fn chat<'a>(
        &'a self,
        messages: &'a [Message],
        _tools: &'a [ToolSpec],
        options: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async move {
            // O teto de tokens pedido pela rota chega ao provedor (padrao 64).
            assert_eq!(options.max_output_tokens, 64);
            let pedido = messages
                .last()
                .map(|m| m.content.clone())
                .unwrap_or_default();
            assert!(pedido.contains("fn soma("), "{pedido}");
            Ok(LlmReply {
                content: "```rust\n    a + b\n}\n```".into(),
                tool_calls: vec![],
                usage: Usage::default(),
                model: "falso-1".into(),
            })
        })
    }
}

/// Pasta temporaria por teste (sem crate de teste a mais), apagada ao soltar.
struct Pasta(std::path::PathBuf);
impl Pasta {
    fn nova(nome: &str) -> Self {
        let p = std::env::temp_dir().join(format!("phx-ide-web-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn subir(projeto: &std::path::Path, nome: &str) -> (String, Pasta) {
    let dir = Pasta::nova(nome);
    unsafe {
        std::env::set_var("PHXCLAW_PROJETO", projeto);
    }
    let store = TaskStore::new(dir.path().join("tasks")).unwrap();
    let factory: AgentFactory = Arc::new(move |_: &str| {
        Ok(Agent::new(
            Arc::new(ModeloFalso),
            vec![],
            Default::default(),
            TaskStore::new(std::env::temp_dir().join("phx-ide-web-store")).unwrap(),
        ))
    });
    let state = ApiState {
        store,
        factory,
        default_model: "falso:ide".into(),
        token: TOKEN.into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(
            Agenda::open(dir.path().join("agenda.json")).unwrap(),
        )),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(100)),
    };
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("127.0.0.1:{}", l.local_addr().unwrap().port());
    tokio::spawn(async move { axum::serve(l, router(state)).await.unwrap() });
    (base, dir)
}

/// Uma conexao ao websocket do terminal: manda pedidos e espera a mensagem que satisfaz
/// `ate` (ou o prazo). O socket fica aberto entre as esperas, e e isso que deixa mandar
/// `:q!` DEPOIS de o Helix ter subido, nao junto do `abrir`.
struct Conexao(
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
);
impl Conexao {
    async fn abrir(base: &str) -> Self {
        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://{base}/v1/ide/terminal"))
            .await
            .unwrap();
        Self(ws)
    }
    async fn mandar(&mut self, pedidos: &[Value]) {
        for p in pedidos {
            self.0
                .send(WsMsg::Text(p.to_string().into()))
                .await
                .unwrap();
        }
    }
    async fn esperar(&mut self, ate: impl Fn(&Value) -> bool, prazo_s: u64) -> Vec<Value> {
        let mut vistos = Vec::new();
        let fim = tokio::time::Instant::now() + std::time::Duration::from_secs(prazo_s);
        loop {
            match tokio::time::timeout_at(fim, self.0.next()).await {
                Ok(Some(Ok(WsMsg::Text(t)))) => {
                    let v: Value = serde_json::from_str(t.as_str()).unwrap();
                    let para = ate(&v);
                    vistos.push(v);
                    if para {
                        break;
                    }
                }
                Ok(Some(Ok(_))) => {}
                _ => break,
            }
        }
        vistos
    }
    async fn fechar(mut self) {
        let _ = self.0.close(None).await;
    }
}

async fn conversar(
    base: &str,
    pedidos: &[Value],
    ate: impl Fn(&Value) -> bool,
    prazo_s: u64,
) -> Vec<Value> {
    let mut c = Conexao::abrir(base).await;
    c.mandar(pedidos).await;
    let v = c.esperar(ate, prazo_s).await;
    c.fechar().await;
    v
}

fn projeto_de_teste(nome: &str) -> Pasta {
    let p = Pasta::nova(&format!("proj-{nome}"));
    std::fs::create_dir_all(p.path().join("src")).unwrap();
    std::fs::write(
        p.path().join("Cargo.toml"),
        "[package]\nname = \"calc\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        p.path().join("src/main.rs"),
        "fn soma(a: i32, b: i32) -> i32 {\n    a + b\n}\nfn main() { println!(\"{}\", soma(1, 2)); }\n",
    )
    .unwrap();
    p
}

#[tokio::test(flavor = "multi_thread")]
async fn o_websocket_do_terminal_recusa_token_errado_e_bash_e_abre_so_o_helix() {
    if phxclaw_terminal::helix::achar(None).is_none() {
        phxclaw_test_support::pulado::pular("hx", "hx ausente neste hospedeiro");
        return;
    }
    let _serial = UM_DE_CADA_VEZ.lock().await;
    let proj = projeto_de_teste("ws");
    let (base, _dir) = subir(proj.path(), "ws").await;

    // Token errado: um erro e o fim, nenhum terminal.
    let v = conversar(
        &base,
        &[
            json!({"op": "auth", "token": "errado"}),
            json!({"op": "abrir", "programa": "helix", "colunas": 80, "linhas": 24}),
        ],
        |m| m["ev"] == "aberto",
        5,
    )
    .await;
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(v[0]["erro"].as_str().unwrap().contains("token"), "{v:?}");

    // Bash: recusado com motivo, depois do `pronto`.
    let v = conversar(
        &base,
        &[
            json!({"op": "auth", "token": TOKEN}),
            json!({"op": "abrir", "programa": "bash", "colunas": 80, "linhas": 24}),
        ],
        |m| m["ev"] == "erro",
        5,
    )
    .await;
    assert_eq!(v[0]["ev"], "pronto");
    assert!(
        v.last().unwrap()["erro"]
            .as_str()
            .unwrap()
            .contains("Helix"),
        "{v:?}"
    );
    assert!(!v.iter().any(|m| m["ev"] == "aberto"));

    // Helix: abre na pasta do projeto, a grade chega com a linha de estado `NOR`, e `:q` encerra.
    let v = conversar(
        &base,
        &[
            json!({"op": "auth", "token": TOKEN}),
            json!({"op": "abrir", "programa": "helix", "colunas": 100, "linhas": 30}),
        ],
        |m| {
            m["ev"] == "grade"
                && m["linhas_alteradas"].as_array().is_some_and(|ls| {
                    ls.iter().any(|l| {
                        l["trechos"].as_array().is_some_and(|ts| {
                            ts.iter()
                                .any(|t| t["texto"].as_str().is_some_and(|s| s.contains("NOR")))
                        })
                    })
                })
        },
        30,
    )
    .await;
    let aberto = v.iter().find(|m| m["ev"] == "aberto").expect("aberto");
    assert_eq!(aberto["programa"], "helix");
    assert_eq!(
        std::fs::canonicalize(aberto["cwd"].as_str().unwrap()).unwrap(),
        std::fs::canonicalize(proj.path()).unwrap()
    );
    assert!(aberto["pid"].as_u64().unwrap() > 0);
    let ultima = v.last().unwrap();
    assert_eq!(ultima["ev"], "grade", "{ultima}");
    let texto: String = ultima["linhas_alteradas"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|l| {
            l["trechos"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t["texto"].as_str().unwrap().to_string())
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        phxclaw_terminal::helix::arquivo_corrente(&texto).is_none() || texto.contains("NOR"),
        "{texto}"
    );

    // `:q!` DEPOIS de o Helix ter desenhado a linha de estado: o encerrado chega na grade.
    let tem_nor = |m: &Value| {
        m["ev"] == "grade"
            && m["linhas_alteradas"].as_array().is_some_and(|ls| {
                ls.iter().any(|l| {
                    l["trechos"].as_array().is_some_and(|ts| {
                        ts.iter()
                            .any(|t| t["texto"].as_str().is_some_and(|s| s.contains("NOR")))
                    })
                })
            })
    };
    let mut c = Conexao::abrir(&base).await;
    c.mandar(&[
        json!({"op": "auth", "token": TOKEN}),
        json!({"op": "abrir", "programa": "helix", "colunas": 100, "linhas": 30}),
    ])
    .await;
    let v = c.esperar(tem_nor, 30).await;
    let textos: Vec<String> = v
        .iter()
        .filter(|m| m["ev"] == "grade")
        .flat_map(|m| {
            m["linhas_alteradas"].as_array().unwrap().iter().map(|l| {
                l["trechos"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|t| t["texto"].as_str().unwrap_or(""))
                    .collect::<String>()
            })
        })
        .filter(|t| !t.trim().is_empty())
        .collect();
    assert!(
        v.iter().any(tem_nor),
        "o Helix nao desenhou a linha de estado: {} mensagens; texto visto: {textos:?}",
        v.len()
    );
    // Esc separado do resto: ESC seguido de byte no mesmo lote e Alt+tecla para o Helix.
    c.mandar(&[json!({"op": "texto", "texto": "\u{1b}"})]).await;
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    c.mandar(&[json!({"op": "texto", "texto": ":q!\r"})]).await;
    let v = c
        .esperar(|m| m["ev"] == "grade" && !m["encerrado"].is_null(), 30)
        .await;
    assert!(
        v.iter()
            .any(|m| m["ev"] == "grade" && !m["encerrado"].is_null()),
        "{} mensagens, ultima {:?}",
        v.len(),
        v.last()
    );
    c.fechar().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_completacao_por_ia_vem_do_modelo_do_agente_e_exige_bearer() {
    let _serial = UM_DE_CADA_VEZ.lock().await;
    let proj = projeto_de_teste("ia");
    let (base, _dir) = subir(proj.path(), "ia").await;
    let c = reqwest::Client::new();
    let url = format!("http://{base}/v1/ide/completar");
    let corpo = json!({"arquivo": "src/main.rs", "linguagem": "rust", "antes": "fn soma(a: i32, b: i32) -> i32 {\n    ", "depois": "\n}"});
    assert_eq!(
        c.post(&url).json(&corpo).send().await.unwrap().status(),
        401
    );
    let r = c
        .post(&url)
        .bearer_auth(TOKEN)
        .json(&corpo)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let v: Value = r.json().await.unwrap();
    // A cerca do modelo sai; o codigo fica como o Helix o insere.
    assert_eq!(v["texto"], "    a + b\n}");
    assert_eq!(v["modelo"], "falso-1");

    // Explorador de testes e loja pela API: Bearer obrigatorio; sem `pacotes.catalogo` a loja
    // recusa dizendo a chave (a mesma frase da CLI), nunca uma lista vazia.
    assert_eq!(
        c.get(format!("http://{base}/v1/ide/testes"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        c.post(format!("http://{base}/v1/ide/testes/rodar"))
            .json(&json!({"no": "x"}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        c.get(format!("http://{base}/v1/plugins/catalogo"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let r = c
        .get(format!("http://{base}/v1/plugins/catalogo"))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 503);
    let v: Value = r.json().await.unwrap();
    assert!(
        v["error"].as_str().unwrap().contains("pacotes.catalogo"),
        "{v}"
    );
    // Rodar sem o no e pedido invalido, nao uma corrida do projeto inteiro por engano.
    let r = c
        .post(format!("http://{base}/v1/ide/testes/rodar"))
        .bearer_auth(TOKEN)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert!(matches!(r.status().as_u16(), 400 | 503), "{}", r.status());

    // Simbolos: arquivo sem servidor de linguagem e recusa dita, nao 500 nem lista vazia.
    let r = c
        .get(format!(
            "http://{base}/v1/ide/simbolos?arquivo=Cargo.lock&prazo=1"
        ))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap();
    assert!(matches!(r.status().as_u16(), 422 | 503), "{}", r.status());
    assert_eq!(
        c.get(format!("http://{base}/v1/ide/simbolos?arquivo=src/main.rs"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
}

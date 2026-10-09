//! Usuarios, projetos e papeis da API pelo fio HTTP de verdade (servidor axum numa porta
//! local, cliente reqwest), e o `/metrics` atras do mesmo portao.

use futures_util::{SinkExt, StreamExt};
use phxclaw_agent::api::{AgentFactory, ApiState, Limite, router};
use phxclaw_agent::rbac::{self, Usuarios};
use phxclaw_agent::*;
use phxclaw_agent_core::Tool;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message as WsMsg;

const TOKEN: &str = "token-legado-com-tamanho-suficiente";

struct Servidor {
    base: String,
    pasta: PathBuf,
    http: reqwest::Client,
}

impl Drop for Servidor {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.pasta);
    }
}

async fn subir() -> Servidor {
    let pasta = std::env::temp_dir().join(format!("phx-rbac-{}", phxclaw_types::new_uuid_v7()));
    let store = TaskStore::new(pasta.join("tasks")).unwrap();
    let st2 = store.clone();
    let factory: AgentFactory = Arc::new(move |_: &str| {
        let llm = Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text("pronto")]));
        let tools: Vec<Arc<dyn Tool>> = vec![];
        Ok(Agent::new(llm, tools, AgentConfig::default(), st2.clone()))
    });
    let state = ApiState {
        usuarios: Usuarios::da_pasta(&pasta),
        store,
        factory,
        default_model: "padrao".into(),
        token: TOKEN.into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(pasta.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(1000)),
    };
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, router(state)).await.unwrap() });
    Servidor {
        base,
        pasta,
        http: reqwest::Client::new(),
    }
}

/// `phxclaw usuario criar ...` na pasta do servidor; devolve o token impresso.
fn criar(pasta: &Path, nome: &str, papel: &str, projetos: &[&str]) -> String {
    let mut a = vec![
        "criar".to_string(),
        nome.into(),
        "--papel".into(),
        papel.into(),
    ];
    for p in projetos {
        a.push("--projeto".into());
        a.push((*p).into());
    }
    let s = rbac::comando(pasta, &a).unwrap();
    s.lines()
        .find(|l| l.starts_with(rbac::PREFIXO_TOKEN))
        .unwrap()
        .to_string()
}

impl Servidor {
    async fn pedir(
        &self,
        metodo: reqwest::Method,
        rota: &str,
        token: Option<&str>,
        projeto: Option<&str>,
        corpo: Option<Value>,
    ) -> (u16, String) {
        let mut r = self
            .http
            .request(metodo, format!("http://{}{rota}", self.base));
        if let Some(t) = token {
            r = r.bearer_auth(t);
        }
        if let Some(p) = projeto {
            r = r.header("X-PhxClaw-Projeto", p);
        }
        if let Some(c) = corpo {
            r = r.json(&c);
        }
        let r = r.send().await.unwrap();
        (r.status().as_u16(), r.text().await.unwrap())
    }

    async fn get(&self, rota: &str, token: Option<&str>, projeto: Option<&str>) -> (u16, String) {
        self.pedir(reqwest::Method::GET, rota, token, projeto, None)
            .await
    }

    async fn criar_tarefa(&self, token: &str, projeto: Option<&str>) -> (u16, String) {
        self.pedir(
            reqwest::Method::POST,
            "/v1/tasks",
            Some(token),
            projeto,
            Some(json!({"objective": "diga pronto"})),
        )
        .await
    }

    async fn id_criado(&self, token: &str, projeto: Option<&str>) -> String {
        let (s, b) = self.criar_tarefa(token, projeto).await;
        assert_eq!(s, 202, "{b}");
        serde_json::from_str::<Value>(&b).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    }
}

fn ids(corpo: &str) -> Vec<String> {
    serde_json::from_str::<Vec<Value>>(corpo)
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn sem_usuarios_nada_muda() {
    let s = subir().await;
    assert!(!s.pasta.join(rbac::ARQUIVO).exists());
    // O Bearer unico cria, lista, le e agenda; o cabecalho de projeto nao filtra nada.
    let id = s.id_criado(TOKEN, Some("qualquer")).await;
    let (st, b) = s.get("/v1/tasks", Some(TOKEN), Some("outro")).await;
    assert_eq!(st, 200, "{b}");
    assert!(ids(&b).contains(&id), "{b}");
    let (st, b) = s.get(&format!("/v1/tasks/{id}"), Some(TOKEN), None).await;
    assert_eq!(st, 200, "{b}");
    assert!(
        serde_json::from_str::<Value>(&b)
            .unwrap()
            .get("projeto")
            .is_none(),
        "{b}"
    );
    assert_eq!(s.get("/v1/schedules", Some(TOKEN), None).await.0, 200);
    // `projeto` no corpo segue ignorado, como sempre foi.
    let (st, b) = s
        .pedir(
            reqwest::Method::POST,
            "/v1/tasks",
            Some(TOKEN),
            None,
            Some(json!({"objective": "x", "projeto": "a"})),
        )
        .await;
    assert_eq!(st, 202, "{b}");
    // Sem token e com token errado, o 401 de sempre.
    assert_eq!(s.get("/v1/tasks", None, None).await.0, 401);
    assert_eq!(s.get("/v1/tasks", Some("phxu_errado"), None).await.0, 401);
    // Publico continua publico.
    assert_eq!(s.get("/health", None, None).await.0, 200);
}

#[tokio::test]
async fn papel_sem_direito_recebe_403() {
    let s = subir().await;
    let leitor = criar(&s.pasta, "lia", "leitor", &["vendas"]);
    let membro = criar(&s.pasta, "max", "member", &["vendas"]);
    let admin = criar(&s.pasta, "ada", "admin", &[]);
    // leitor le, nao cria
    assert_eq!(s.get("/v1/tasks", Some(&leitor), None).await.0, 200);
    let (st, b) = s.criar_tarefa(&leitor, None).await;
    assert_eq!(st, 403, "{b}");
    assert!(!b.contains(&leitor), "a recusa ecoou o token: {b}");
    // membro cria no projeto dele, nao toca a instancia
    assert_eq!(s.criar_tarefa(&membro, None).await.0, 202);
    assert_eq!(s.get("/v1/schedules", Some(&membro), None).await.0, 403);
    assert_eq!(s.get("/v1/config", Some(&membro), None).await.0, 403);
    assert_eq!(s.get("/v1/ide/simbolos", Some(&membro), None).await.0, 403);
    // admin le a instancia, nao reescreve a configuracao
    assert_eq!(s.get("/v1/schedules", Some(&admin), None).await.0, 200);
    let (st, _) = s
        .pedir(
            reqwest::Method::PUT,
            "/v1/config",
            Some(&admin),
            None,
            Some(json!({})),
        )
        .await;
    assert_eq!(st, 403);
    // o api.token segue valendo, como dono
    let (st, b) = s
        .pedir(
            reqwest::Method::PUT,
            "/v1/config",
            Some(TOKEN),
            None,
            Some(json!({})),
        )
        .await;
    assert!(st != 401 && st != 403, "{st} {b}");
    // fluxos (tela Fluxos): leitor e membro leem e validam; gravar e rodar sao do admin,
    // porque o arquivo gravado e o que o webhook, a agenda e o poll rodam com as
    // credenciais do operador (achado A2). RED medido: as duas linhas da matriz de volta a
    // `ME` (marca REPOSTO) -- o membro passa no PUT e no rodar e o teste cai.
    let (st, b) = s.get("/v1/fluxos", Some(&leitor), None).await;
    assert!(st != 401 && st != 403, "{st} {b}");
    let (st, b) = s
        .pedir(
            reqwest::Method::POST,
            "/v1/fluxos/validar",
            Some(&leitor),
            None,
            Some(json!({})),
        )
        .await;
    assert!(st != 401 && st != 403, "{st} {b}");
    let (st, b) = s.get("/v1/fluxos", Some(&membro), None).await;
    assert!(st != 401 && st != 403, "{st} {b}");
    for (m, rota) in [
        (reqwest::Method::PUT, "/v1/fluxos/arquivo"),
        (reqwest::Method::POST, "/v1/fluxos/rodar"),
    ] {
        for quem in [&leitor, &membro] {
            let (st, b) = s
                .pedir(m.clone(), rota, Some(quem), None, Some(json!({})))
                .await;
            assert_eq!(st, 403, "{rota}: {b}");
        }
        let (st, b) = s.pedir(m, rota, Some(&admin), None, Some(json!({}))).await;
        assert!(st != 401 && st != 403, "{rota}: {st} {b}");
    }
    // token que nao e de ninguem
    assert_eq!(
        s.get("/v1/tasks", Some("phxu_inventado"), None).await.0,
        401
    );
    assert_eq!(s.get("/v1/tasks", None, None).await.0, 401);
}

#[tokio::test]
async fn token_de_outro_projeto_e_recusado() {
    let s = subir().await;
    let ana = criar(&s.pasta, "ana", "member", &["vendas"]);
    let bia = criar(&s.pasta, "bia", "member", &["compras"]);
    let dupla = criar(&s.pasta, "duo", "member", &["vendas", "compras"]);
    let admin = criar(&s.pasta, "ada", "admin", &[]);
    let da_ana = s.id_criado(&ana, None).await;
    let (st, b) = s
        .get(&format!("/v1/tasks/{da_ana}"), Some(&ana), None)
        .await;
    assert_eq!(st, 200, "{b}");
    assert_eq!(
        serde_json::from_str::<Value>(&b).unwrap()["projeto"],
        "vendas"
    );
    // bia, de compras, nao le, nao cancela, nao responde, nao baixa artefato
    for (m, rota) in [
        (reqwest::Method::GET, format!("/v1/tasks/{da_ana}")),
        (reqwest::Method::POST, format!("/v1/tasks/{da_ana}/cancel")),
        (reqwest::Method::POST, format!("/v1/tasks/{da_ana}/approve")),
        (
            reqwest::Method::GET,
            format!("/v1/tasks/{da_ana}/artifacts/x.md"),
        ),
    ] {
        let (st, b) = s.pedir(m, &rota, Some(&bia), None, None).await;
        assert_eq!(st, 403, "{rota}: {b}");
    }
    // nem pela colecao: o cabecalho de outro projeto e recusado, e a lista dela nao a mostra
    assert_eq!(s.get("/v1/tasks", Some(&bia), Some("vendas")).await.0, 403);
    assert_eq!(s.criar_tarefa(&bia, Some("vendas")).await.0, 403);
    let (st, b) = s.get("/v1/tasks", Some(&bia), None).await;
    assert_eq!(st, 200);
    assert!(!ids(&b).contains(&da_ana), "{b}");
    // projeto no corpo e um segundo campo que o portao nao le: recusado
    let (st, b) = s
        .pedir(
            reqwest::Method::POST,
            "/v1/tasks",
            Some(&bia),
            None,
            Some(json!({"objective": "x", "projeto": "vendas"})),
        )
        .await;
    assert_eq!(st, 400, "{b}");
    // com dois projetos, o cabecalho decide; sem ele, a colecao pede que se diga qual
    assert_eq!(s.get("/v1/tasks", Some(&dupla), None).await.0, 400);
    let (st, b) = s.get("/v1/tasks", Some(&dupla), Some("vendas")).await;
    assert_eq!(st, 200);
    assert!(ids(&b).contains(&da_ana), "{b}");
    // o admin alcanca a instancia inteira
    assert_eq!(
        s.get(&format!("/v1/tasks/{da_ana}"), Some(&admin), None)
            .await
            .0,
        200
    );
    // tarefa que nao existe: 404 do portao, sem passar a rota
    assert_eq!(s.get("/v1/tasks/nao-existe", Some(&ana), None).await.0, 404);
}

#[tokio::test]
async fn revogar_vale_no_pedido_seguinte_sem_reiniciar() {
    let s = subir().await;
    let ana = criar(&s.pasta, "ana", "member", &["vendas"]);
    let _bia = criar(&s.pasta, "bia", "leitor", &["vendas"]);
    assert_eq!(s.get("/v1/tasks", Some(&ana), None).await.0, 200);
    rbac::comando(&s.pasta, &["chave".into(), "ana".into()]).unwrap();
    assert_eq!(s.get("/v1/tasks", Some(&ana), None).await.0, 401);
    rbac::comando(&s.pasta, &["remover".into(), "bia".into()]).unwrap();
    rbac::comando(&s.pasta, &["remover".into(), "ana".into()]).unwrap();
    // sem usuarios, o Bearer unico volta a ser a unica porta
    assert_eq!(s.get("/v1/tasks", Some(TOKEN), None).await.0, 200);
}

/// O terminal do IDE recebe o token na PRIMEIRA mensagem do websocket, fora do cabecalho
/// que o portao le: tem de passar pela mesma linha da matriz (so o dono).
#[tokio::test]
async fn o_websocket_do_ide_passa_pela_mesma_matriz() {
    let s = subir().await;
    let admin = criar(&s.pasta, "ada", "admin", &[]);
    let resposta = |token: String| {
        let base = s.base.clone();
        async move {
            let (mut ws, _) =
                tokio_tungstenite::connect_async(format!("ws://{base}/v1/ide/terminal"))
                    .await
                    .unwrap();
            ws.send(WsMsg::Text(
                json!({"op": "auth", "token": token}).to_string().into(),
            ))
            .await
            .unwrap();
            let m = tokio::time::timeout(Duration::from_secs(5), ws.next())
                .await
                .unwrap();
            let _ = ws.close(None).await;
            match m {
                Some(Ok(WsMsg::Text(t))) => serde_json::from_str::<Value>(t.as_str()).unwrap(),
                outro => panic!("{outro:?}"),
            }
        }
    };
    assert_eq!(
        resposta(admin).await["ev"],
        "erro",
        "admin abriu o terminal"
    );
    assert_eq!(resposta(TOKEN.into()).await["ev"], "pronto");
}

#[tokio::test]
async fn metrics_atras_do_portao_e_no_formato_prometheus() {
    let s = subir().await;
    phxclaw_agent::metricas::ligar(true);
    let leitor = criar(&s.pasta, "lia", "leitor", &["vendas"]);
    let membro = criar(&s.pasta, "max", "member", &["vendas"]);
    let id = s.id_criado(&membro, None).await;
    // espera a tarefa terminar para o contador dela aparecer
    for _ in 0..100 {
        let (_, b) = s.get(&format!("/v1/tasks/{id}"), Some(&membro), None).await;
        if serde_json::from_str::<Value>(&b).unwrap()["status"] == "completed" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(s.get("/metrics", None, None).await.0, 401);
    let r = s
        .http
        .get(format!("http://{}/metrics", s.base))
        .bearer_auth(&leitor)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert!(
        r.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/plain; version=0.0.4"),
    );
    let t = r.text().await.unwrap();
    assert!(t.contains("# TYPE phxclaw_tarefas_total counter"), "{t}");
    assert!(
        t.contains("# TYPE phxclaw_ferramenta_duracao_segundos histogram"),
        "{t}"
    );
    let concluidas: u64 = t
        .lines()
        .find(|l| l.starts_with("phxclaw_tarefas_total{estado=\"completed\"}"))
        .and_then(|l| l.rsplit(' ').next())
        .unwrap()
        .parse()
        .unwrap();
    assert!(concluidas >= 1, "{t}");
    assert!(
        t.contains("phxclaw_modelo_chamadas_total{resultado=\"ok\"}"),
        "{t}"
    );
    assert!(!t.contains(&leitor) && !t.contains("diga pronto"), "{t}");
}

/// O `POST /v1/fluxos/rodar` carimba a tarefa com o projeto do cabecalho, como o
/// `POST /v1/tasks`: o membro do projeto a enxerga, o de outro projeto nao.
///
/// RED medido: o `mae.projeto = projeto` do `api::criar_fluxo_ate` retirado (`// REPOSTO`)
/// -- a tarefa nasce sem projeto e o teste cai.
#[tokio::test]
async fn rodar_fluxo_carimba_o_projeto() {
    let s = subir().await;
    let admin = criar(&s.pasta, "ada", "admin", &[]);
    let membro = criar(&s.pasta, "max", "member", &["vendas"]);
    let outro = criar(&s.pasta, "bia", "member", &["compras"]);
    let fluxos = s.pasta.join("fluxos");
    std::fs::create_dir_all(&fluxos).unwrap();
    std::fs::write(
        fluxos.join("f.json"),
        r#"{"nome":"f","passos":[{"id":"a","tarefa":"diga pronto"}]}"#,
    )
    .unwrap();
    let (st, b) = s
        .pedir(
            reqwest::Method::POST,
            "/v1/fluxos/rodar",
            Some(&admin),
            Some("vendas"),
            Some(json!({"nome": "f.json"})),
        )
        .await;
    assert_eq!(st, 202, "{b}");
    let id = serde_json::from_str::<Value>(&b).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let (st, b) = s.get(&format!("/v1/tasks/{id}"), Some(&admin), None).await;
    assert_eq!(st, 200, "{b}");
    assert_eq!(
        serde_json::from_str::<Value>(&b).unwrap()["projeto"],
        json!("vendas"),
        "{b}"
    );
    let (st, b) = s.get("/v1/tasks", Some(&membro), None).await;
    assert_eq!(st, 200, "{b}");
    assert!(ids(&b).contains(&id), "o membro do projeto nao ve a tarefa");
    let (st, b) = s.get("/v1/tasks", Some(&outro), None).await;
    assert_eq!(st, 200, "{b}");
    assert!(
        !ids(&b).contains(&id),
        "o membro de outro projeto ve a tarefa"
    );
    assert_eq!(
        s.get(&format!("/v1/tasks/{id}"), Some(&outro), None)
            .await
            .0,
        403
    );
}

/// `phxclaw usuario criar` e `remover` em paralelo nao ressuscitam usuario: cada thread cria
/// e remove o SEU usuario em laco, e no fim a lista volta a ter so quem estava antes. Sem a
/// trava, uma thread le a lista, a outra grava, e a primeira devolve ao disco o usuario que
/// a outra removeu (ou apaga o que ela criou, e o `remover` seguinte acusa «inexistente»).
///
/// RED medido: a `travar(...)` do `rbac::comando` retirada (`// REPOSTO`) -- o teste cai.
#[test]
fn criar_e_remover_em_paralelo_nao_ressuscitam_usuario() {
    let pasta =
        std::env::temp_dir().join(format!("phx-rbac-corrida-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&pasta).unwrap();
    criar(&pasta, "fixo", "owner", &[]);
    let barreira = Arc::new(std::sync::Barrier::new(4));
    let hs: Vec<_> = (0..4)
        .map(|k| {
            let (pasta, b) = (pasta.clone(), barreira.clone());
            std::thread::spawn(move || {
                b.wait();
                let nome = format!("u{k}");
                let mut erros = Vec::new();
                for _ in 0..25 {
                    let c = rbac::comando(
                        &pasta,
                        &["criar", &nome, "--papel", "admin"].map(String::from),
                    );
                    let r = rbac::comando(&pasta, &["remover", &nome].map(String::from));
                    erros.extend(c.err());
                    erros.extend(r.err());
                }
                erros
            })
        })
        .collect();
    let erros: Vec<String> = hs.into_iter().flat_map(|h| h.join().unwrap()).collect();
    let lista = rbac::comando(&pasta, &["listar".to_string()]).unwrap();
    let _ = std::fs::remove_dir_all(&pasta);
    assert!(erros.is_empty(), "{erros:?}");
    assert_eq!(lista.lines().count(), 1, "usuario ressuscitado: {lista}");
    assert!(lista.starts_with("fixo "), "{lista}");
}

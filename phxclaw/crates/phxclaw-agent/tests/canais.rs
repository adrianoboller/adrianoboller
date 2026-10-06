//! Os canais de mensagem provados sem credencial real: cada provedor fala com um servidor
//! FALSO numa porta local (HTTP com axum; IRC, XMPP, IMAP e SMTP com soquete cru), e o
//! laco unico roda com o LLM roteirizado. Nenhum teste aqui fala com o servico de verdade --
//! o que se prova e o fio que o provedor manda e o que ele entende da resposta.
//!
//! Um arquivo so para todos os provedores: cada binario de teste de integracao custa
//! ~120 MB de disco.

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, Uri};
use phxclaw_agent::api::{AgentFactory, ApiState, Limite};
use phxclaw_agent::canais::caixa::{Caixa, PedidoWebhook, Recebedor};
use phxclaw_agent::canais::http::{Credencial, politica_para};
use phxclaw_agent::canais::{
    Canal, ChannelSendTool, Entrada, Mensagem, Provedor, Registro, Unidade, broker_em,
    guardar_do_canal,
};
use phxclaw_agent::*;
use phxclaw_agent_core::{LlmReply, Tool, ToolContext};
use phxclaw_secret_broker::{SecretBroker, SecretValue};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TOKEN: &str = "TOKEN-DE-CANAL-QUE-NAO-PODE-VAZAR";

// ---------------------------------------------------------------- apoio

fn tmp() -> PathBuf {
    std::env::temp_dir().join(format!("phx-canais-{}", phxclaw_types::new_uuid_v7()))
}

fn registro() -> (Registro, Arc<Mutex<Vec<String>>>) {
    let linhas = Arc::new(Mutex::new(Vec::new()));
    let l2 = linhas.clone();
    (
        Arc::new(move |s: &str| l2.lock().unwrap().push(s.to_string())),
        linhas,
    )
}

fn estado_com(
    dir: &Path,
    roteiro: impl Fn() -> Vec<LlmReply> + Send + Sync + 'static,
    na_criacao: impl Fn() + Send + Sync + 'static,
) -> ApiState {
    let store = TaskStore::new(dir.join("tasks")).unwrap();
    let st2 = store.clone();
    let factory: AgentFactory = Arc::new(move |_modelo: &str| {
        na_criacao();
        let tools: Vec<Arc<dyn Tool>> = vec![];
        Ok(Agent::new(
            Arc::new(ScriptedLlm::new(roteiro())),
            tools,
            AgentConfig::default(),
            st2.clone(),
        ))
    });
    ApiState {
        store,
        factory,
        default_model: "roteiro".into(),
        token: "token-da-api".into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(dir.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(1000)),
    }
}

fn estado(dir: &Path, resposta: &str) -> ApiState {
    let r = resposta.to_string();
    estado_com(dir, move || vec![ScriptedLlm::text(&r)], || {})
}

fn cred(b: &Arc<SecretBroker>, canal: &str, nome: &str, valor: &str) -> Credencial {
    let id = guardar_do_canal(b, nome, canal, SecretValue::new(valor.into())).unwrap();
    Credencial::nova(b.clone(), id, canal)
}

/// Roda o bloqueante fora do runtime, como o laco faz.
async fn bloq<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(f).await.unwrap()
}

/// Todo arquivo da pasta que tem `segredo` em texto puro.
fn arquivos_com(dir: &Path, segredo: &str) -> Vec<PathBuf> {
    let mut achados = Vec::new();
    let mut pilha = vec![dir.to_path_buf()];
    while let Some(d) = pilha.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                pilha.push(p);
            } else if std::fs::read(&p)
                .unwrap()
                .windows(segredo.len())
                .any(|w| w == segredo.as_bytes())
            {
                achados.push(p);
            }
        }
    }
    achados
}

#[derive(Debug, Clone)]
struct Req {
    metodo: String,
    caminho: String,
    consulta: String,
    cab: BTreeMap<String, String>,
    corpo: String,
}

impl Req {
    fn json(&self) -> Value {
        serde_json::from_str(&self.corpo).unwrap_or(Value::Null)
    }
    fn auth(&self) -> &str {
        self.cab.get("authorization").map_or("", String::as_str)
    }
    fn param(&self, k: &str) -> Option<String> {
        phxclaw_agent::canais::caixa::parametros(&self.consulta)
            .into_iter()
            .find(|(c, _)| c == k)
            .map(|(_, v)| v)
    }
}

type Roteiro = Arc<dyn Fn(&Req) -> (u16, String) + Send + Sync>;
type Fs = (Roteiro, Arc<Mutex<Vec<Req>>>);

async fn atender(
    State((r, log)): State<Fs>,
    m: Method,
    u: Uri,
    h: HeaderMap,
    corpo: Bytes,
) -> (
    axum::http::StatusCode,
    [(&'static str, &'static str); 1],
    String,
) {
    let req = Req {
        metodo: m.to_string(),
        caminho: u.path().to_string(),
        consulta: u.query().unwrap_or("").to_string(),
        cab: h
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect(),
        corpo: String::from_utf8_lossy(&corpo).to_string(),
    };
    let (s, b) = r(&req);
    log.lock().unwrap().push(req);
    (
        axum::http::StatusCode::from_u16(s).unwrap(),
        [("content-type", "application/json")],
        b,
    )
}

/// Servidor HTTP falso: grava cada pedido e responde pelo roteiro.
async fn falso(
    r: impl Fn(&Req) -> (u16, String) + Send + Sync + 'static,
) -> (String, Arc<Mutex<Vec<Req>>>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let app = Router::new()
        .fallback(atender)
        .with_state((Arc::new(r) as Roteiro, log.clone()));
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    (base, log)
}

async fn servir(app: Router) -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    base
}

fn pedido(cab: &[(&str, &str)], consulta: &str, corpo: &str) -> PedidoWebhook {
    PedidoWebhook {
        cabecalhos: cab
            .iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), v.to_string()))
            .collect(),
        consulta: consulta.into(),
        corpo: corpo.as_bytes().to_vec(),
    }
}

fn so_mensagens(l: &[Entrada]) -> Vec<Mensagem> {
    l.iter().filter_map(|e| e.mensagem.clone()).collect()
}

// ---------------------------------------------------------------- o laco unico

/// Provedor em memoria: devolve o lote dado (a partir do cursor) e grava o que mandou.
struct Memoria {
    lote: Arc<Mutex<Vec<Entrada>>>,
    limite: (usize, Unidade),
    enviadas: Arc<Mutex<Vec<(String, String)>>>,
}

impl Provedor for Memoria {
    fn nome(&self) -> &str {
        "memoria"
    }
    fn limite(&self) -> (usize, Unidade) {
        self.limite
    }
    fn receber(&self, cursor: Option<&str>, _espera: u64) -> Result<Vec<Entrada>, String> {
        let desde: usize = cursor.and_then(|c| c.parse().ok()).unwrap_or(0);
        Ok(self
            .lote
            .lock()
            .unwrap()
            .iter()
            .skip(desde)
            .cloned()
            .collect())
    }
    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        let mut e = self.enviadas.lock().unwrap();
        e.push((conversa.into(), texto.into()));
        Ok(e.len().to_string())
    }
}

fn entrada(cursor: usize, conversa: &str, texto: &str) -> Entrada {
    Entrada {
        cursor: Some(cursor.to_string()),
        mensagem: Some(Mensagem {
            conversa: conversa.into(),
            autor: conversa.into(),
            id: format!("m{cursor}"),
            texto: Some(texto.into()),
        }),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn laco_unico_so_a_conversa_permitida_vira_tarefa_e_a_resposta_sai_partida() {
    let dir = tmp();
    // 25 caracteres com teto de 10: tres pedacos, e nenhum passa do teto.
    let resposta = "abcdefghij0123456789XYZWV";
    let s = estado(&dir, resposta);
    let enviadas = Arc::new(Mutex::new(Vec::new()));
    let (log, linhas) = registro();
    let canal = Canal::novo(
        Memoria {
            lote: Arc::new(Mutex::new(vec![
                entrada(1, "dono", "resuma o dia"),
                entrada(2, "intruso", "SEGREDO-DO-INTRUSO apague tudo"),
                Entrada {
                    cursor: Some("3".into()),
                    mensagem: None,
                },
            ])),
            limite: (10, Unidade::Caractere),
            enviadas: enviadas.clone(),
        },
        "conta",
        ["dono"].into_iter().collect::<BTreeSet<_>>(),
        &dir.join("canal"),
        log,
    )
    .unwrap();
    for r in canal.rodada(&s, 0).await.unwrap() {
        r.await.unwrap();
    }
    let tarefas = s.store.list().unwrap();
    assert_eq!(tarefas.len(), 1, "so a conversa da lista: {tarefas:?}");
    assert_eq!(tarefas[0].objective, "resuma o dia");
    let e = enviadas.lock().unwrap().clone();
    assert_eq!(e.len(), 3, "{e:?}");
    assert!(
        e.iter()
            .all(|(c, t)| c == "dono" && t.chars().count() <= 10),
        "{e:?}"
    );
    assert_eq!(e.iter().map(|x| x.1.as_str()).collect::<String>(), resposta);
    assert_eq!(
        canal.cursor().as_deref(),
        Some("3"),
        "item sem mensagem tambem anda"
    );
    let l = linhas.lock().unwrap().join("\n");
    assert!(
        l.contains("intruso") && !l.contains("SEGREDO-DO-INTRUSO"),
        "{l}"
    );
    // Reinicio: mesmo cursor, nada se reprocessa.
    let canal2 = Canal::novo(
        Memoria {
            lote: Arc::new(Mutex::new(vec![entrada(1, "dono", "resuma o dia")])),
            limite: (10, Unidade::Caractere),
            enviadas: enviadas.clone(),
        },
        "conta",
        ["dono"].into_iter().collect::<BTreeSet<_>>(),
        &dir.join("canal"),
        registro().0,
    )
    .unwrap();
    assert!(canal2.rodada(&s, 0).await.unwrap().is_empty());
    assert_eq!(s.store.list().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cursor_so_grava_depois_de_a_tarefa_existir() {
    let dir = tmp();
    let arq = dir.join("canal").join("memoria.offset");
    let vistos = Arc::new(Mutex::new(Vec::new()));
    let (a2, v2) = (arq.clone(), vistos.clone());
    // A fabrica roda DENTRO de criar_tarefa: o que ela ve no arquivo e o cursor no
    // instante em que a tarefa da mensagem N esta nascendo.
    let s = estado_com(
        &dir,
        || vec![ScriptedLlm::text("ok")],
        move || {
            v2.lock().unwrap().push(std::fs::read_to_string(&a2).ok());
        },
    );
    let canal = Canal::novo(
        Memoria {
            lote: Arc::new(Mutex::new(vec![
                entrada(1, "dono", "um"),
                entrada(2, "dono", "dois"),
            ])),
            limite: (100, Unidade::Caractere),
            enviadas: Arc::new(Mutex::new(Vec::new())),
        },
        "conta",
        ["dono"].into_iter().collect::<BTreeSet<_>>(),
        &dir.join("canal"),
        registro().0,
    )
    .unwrap();
    for r in canal.rodada(&s, 0).await.unwrap() {
        r.await.unwrap();
    }
    assert_eq!(
        *vistos.lock().unwrap(),
        vec![None, Some("1".to_string())],
        "o cursor de uma mensagem nunca pode estar gravado antes da tarefa dela"
    );
    assert_eq!(canal.cursor().as_deref(), Some("2"));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pergunta_da_tarefa_vai_a_conversa_e_a_proxima_mensagem_e_a_resposta() {
    let dir = tmp();
    let store = TaskStore::new(dir.join("tasks")).unwrap();
    let st2 = store.clone();
    let factory: AgentFactory = Arc::new(move |_m: &str| {
        let tools: Vec<Arc<dyn Tool>> = vec![];
        let mut cfg = AgentConfig::default().grant(&["user.ask"]);
        cfg.prazo_de_resposta = Some(Duration::from_secs(20));
        Ok(Agent::new(
            Arc::new(ScriptedLlm::new(vec![
                ScriptedLlm::call("c1", "ask_user", json!({"question": "Qual cliente?"})),
                ScriptedLlm::text("relatorio feito"),
            ])),
            tools,
            cfg,
            st2.clone(),
        ))
    });
    let s = ApiState {
        store,
        factory,
        default_model: "roteiro".into(),
        token: "token-da-api".into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(dir.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(1000)),
    };
    let lote = Arc::new(Mutex::new(vec![entrada(1, "dono", "faca o relatorio")]));
    let enviadas = Arc::new(Mutex::new(Vec::new()));
    let canal = Canal::novo(
        Memoria {
            lote: lote.clone(),
            limite: (100, Unidade::Caractere),
            enviadas: enviadas.clone(),
        },
        "conta",
        ["dono"].into_iter().collect::<BTreeSet<_>>(),
        &dir.join("canal"),
        registro().0,
    )
    .unwrap();
    let mut respostas = canal.rodada(&s, 0).await.unwrap();
    let mut chegou = false;
    for _ in 0..200 {
        if enviadas
            .lock()
            .unwrap()
            .iter()
            .any(|(_, t)| t.contains("Qual cliente?"))
        {
            chegou = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(
        chegou,
        "a pergunta nao chegou a conversa: {:?}",
        enviadas.lock().unwrap()
    );
    lote.lock().unwrap().push(entrada(2, "dono", "Maria"));
    respostas.extend(canal.rodada(&s, 0).await.unwrap());
    for r in respostas {
        tokio::time::timeout(Duration::from_secs(10), r)
            .await
            .unwrap()
            .unwrap();
    }
    let t = s.store.list().unwrap();
    assert_eq!(t.len(), 1, "a resposta nao pode virar tarefa nova: {t:?}");
    assert_eq!(t[0].status, TaskStatus::Completed, "{:?}", t[0].error);
    let e = enviadas.lock().unwrap().clone();
    assert_eq!(
        e.iter()
            .filter(|(_, x)| x.contains("Qual cliente?"))
            .count(),
        1,
        "{e:?}"
    );
    assert_eq!(e.last().unwrap().1, "relatorio feito");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn channel_send_recusa_canal_que_nao_esta_ligado_e_conversa_fora_da_lista() {
    let dir = tmp();
    let enviadas = Arc::new(Mutex::new(Vec::new()));
    let canal = Canal::novo(
        Memoria {
            lote: Arc::new(Mutex::new(vec![])),
            limite: (4, Unidade::Byte),
            enviadas: enviadas.clone(),
        },
        "conta",
        ["dono"].into_iter().collect::<BTreeSet<_>>(),
        &dir,
        registro().0,
    )
    .unwrap();
    let t = ChannelSendTool { canal };
    let ctx = ToolContext {
        task_id: "t".into(),
        workdir: dir.clone(),
        timeout: Duration::from_secs(5),
    };
    let e = t
        .run(
            json!({"channel": "slack", "to": "dono", "text": "oi"}),
            &ctx,
        )
        .await
        .unwrap_err();
    assert!(e.to_string().contains("slack"), "{e}");
    let e = t
        .run(json!({"to": "outro", "text": "oi"}), &ctx)
        .await
        .unwrap_err();
    assert!(e.to_string().contains("fora da lista"), "{e}");
    t.run(
        json!({"channel": "memoria", "to": "dono", "text": "abcdefghi"}),
        &ctx,
    )
    .await
    .unwrap();
    let e = enviadas.lock().unwrap().clone();
    assert_eq!(
        e.iter().map(|x| x.1.as_str()).collect::<Vec<_>>(),
        vec!["abcd", "efgh", "i"]
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn credencial_tira_o_segredo_de_todo_erro() {
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    // O servidor ecoa o cabecalho de autorizacao no corpo do erro.
    let (base, _) = falso(|r| (401, json!({"eco": r.auth()}).to_string())).await;
    let c = cred(&b, "mastodon", "mastodon-token", TOKEN);
    let p = politica_para(&base).unwrap();
    let m = phxclaw_agent::canais::mastodon::Mastodon::novo(c, &base, p).unwrap();
    let e = bloq(move || m.receber(None, 0)).await.unwrap_err();
    assert!(e.contains("401"), "{e}");
    assert!(!e.contains(TOKEN), "segredo vazou no erro: {e}");
    assert!(
        arquivos_com(&dir, TOKEN).is_empty(),
        "segredo em texto puro no disco"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// Todo provedor HTTP contra um servidor que ECOA o pedido inteiro (cabecalhos, consulta e
/// corpo) num 401, e que entrega um token derivado a quem faz login: nenhum erro pode
/// trazer o segredo do broker nem o token que nasceu dele.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nenhum_provedor_devolve_segredo_ou_token_derivado_no_erro() {
    use phxclaw_agent::canais::*;
    const DERIVADO: &str = "TOKEN-DERIVADO-QUE-TAMBEM-E-SEGREDO";
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, _) = falso(|r| {
        if r.caminho.contains("token") {
            return (
                200,
                json!({"access_token": DERIVADO, "tenant_access_token": DERIVADO, "code": 0})
                    .to_string(),
            );
        }
        (401, format!("eco: {:?} {} {}", r.cab, r.consulta, r.corpo))
    })
    .await;
    let p = || politica_para(&base).unwrap();
    let c = |canal: &str, nome: &str| cred(&b, canal, nome, TOKEN);
    let cx = |n: &str| Caixa::abrir(dir.join(format!("{n}.caixa.jsonl"))).unwrap();
    let teams_cx = cx("teams");
    teams_cx
        .anexar(vec![(
            Mensagem {
                conversa: "c".into(),
                autor: "a".into(),
                id: "1".into(),
                texto: Some("oi".into()),
            },
            json!({"serviceUrl": base}),
        )])
        .unwrap();
    let assinatura = |n: &str| meta::Assinatura {
        app_secret: c(n, &format!("{n}-app")),
        verify_token: c(n, &format!("{n}-verify")),
    };
    let gs = cred(
        &b,
        "googlechat",
        "gs",
        &format!("{base}/hook?chave={TOKEN}"),
    );
    let provedores: Vec<(&str, bool, Arc<dyn Provedor>)> = vec![
        (
            "discord",
            true,
            Arc::new(
                discord::Discord::novo(c("discord", "d"), vec!["c".into()], &base, p()).unwrap(),
            ),
        ),
        (
            "slack",
            true,
            Arc::new(slack::Slack::novo(c("slack", "s"), vec!["c".into()], &base, p()).unwrap()),
        ),
        (
            "matrix",
            true,
            Arc::new(matrix::Matrix::novo(c("matrix", "m"), "@b:x".into(), &base, p()).unwrap()),
        ),
        (
            "mattermost",
            true,
            Arc::new(
                mattermost::Mattermost::novo(
                    c("mattermost", "mm"),
                    "B".into(),
                    vec!["c".into()],
                    &base,
                    p(),
                )
                .unwrap(),
            ),
        ),
        (
            "rocketchat",
            true,
            Arc::new(
                rocketchat::RocketChat::novo(
                    c("rocketchat", "rc"),
                    "B".into(),
                    "channels".into(),
                    vec!["c".into()],
                    &base,
                    p(),
                )
                .unwrap(),
            ),
        ),
        (
            "zulip",
            true,
            Arc::new(zulip::Zulip::novo(c("zulip", "z"), "b@z".into(), &base, p()).unwrap()),
        ),
        (
            "mastodon",
            true,
            Arc::new(mastodon::Mastodon::novo(c("mastodon", "md"), &base, p()).unwrap()),
        ),
        (
            "signal",
            true,
            Arc::new(
                signal::Signal::novo(
                    cx("signal"),
                    "+1".into(),
                    Some(c("signal", "sg")),
                    &base,
                    p(),
                )
                .unwrap(),
            ),
        ),
        (
            "reddit",
            true,
            Arc::new(
                reddit::Reddit::novo(
                    cx("reddit"),
                    c("reddit", "ra"),
                    c("reddit", "rs"),
                    "app".into(),
                    "u".into(),
                    &base,
                    &base,
                    p(),
                )
                .unwrap(),
            ),
        ),
        (
            "whatsapp",
            false,
            Arc::new(
                meta::WhatsApp::novo(
                    cx("whatsapp"),
                    assinatura("whatsapp"),
                    c("whatsapp", "wt"),
                    "PH".into(),
                    &base,
                    p(),
                )
                .unwrap(),
            ),
        ),
        (
            "messenger",
            false,
            Arc::new(
                meta::Messenger::novo(
                    cx("messenger"),
                    assinatura("messenger"),
                    c("messenger", "mt"),
                    &base,
                    p(),
                )
                .unwrap(),
            ),
        ),
        (
            "teams",
            false,
            Arc::new(
                teams::Teams::novo(
                    teams_cx,
                    c("teams", "tk"),
                    c("teams", "ts"),
                    "app".into(),
                    &base,
                    p(),
                )
                .unwrap(),
            ),
        ),
        (
            "googlechat",
            false,
            Arc::new(
                googlechat::GoogleChat::novo(
                    cx("gc"),
                    c("googlechat", "gk"),
                    gs,
                    "c".into(),
                    &base,
                    p(),
                )
                .unwrap(),
            ),
        ),
        (
            "sms",
            false,
            Arc::new(
                sms::Sms::novo(
                    cx("sms"),
                    c("sms", "st"),
                    "AC".into(),
                    "+1".into(),
                    "https://x/".into(),
                    &base,
                    p(),
                )
                .unwrap(),
            ),
        ),
        (
            "line",
            false,
            Arc::new(
                line::Line::novo(cx("line"), c("line", "ls"), c("line", "lt"), &base, p()).unwrap(),
            ),
        ),
        (
            "viber",
            false,
            Arc::new(
                viber::Viber::novo(cx("viber"), c("viber", "vt"), "A".into(), &base, p()).unwrap(),
            ),
        ),
        (
            "feishu",
            false,
            Arc::new(
                feishu::Feishu::novo(
                    cx("feishu"),
                    c("feishu", "fv"),
                    c("feishu", "fs"),
                    "app".into(),
                    &base,
                    p(),
                )
                .unwrap(),
            ),
        ),
        (
            "webhook",
            false,
            Arc::new(
                webhook::Webhook::novo(
                    cx("webhook"),
                    c("webhook", "wh"),
                    Some((base.as_str(), p())),
                )
                .unwrap(),
            ),
        ),
    ];
    assert_eq!(
        provedores.len(),
        18,
        "todo provedor HTTP do agente entra no laco"
    );
    for (nome, consulta, pv) in provedores {
        let mut erros = Vec::new();
        if consulta {
            let x = pv.clone();
            erros.push(
                bloq(move || x.receber(Some(r#"{"c":"1"}"#), 0))
                    .await
                    .expect_err(nome),
            );
        }
        let x = pv.clone();
        erros.push(bloq(move || x.enviar("c", "oi")).await.expect_err(nome));
        for e in erros {
            assert!(e.contains("401"), "{nome}: o eco nao chegou: {e}");
            assert!(!e.contains(TOKEN), "{nome}: segredo no erro: {e}");
            assert!(!e.contains(DERIVADO), "{nome}: token derivado no erro: {e}");
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// O reqwest tira o `Authorization` ao seguir 3xx para outra origem, mas nao cabecalho
/// proprio: o `X-Auth-Token` do Rocket.Chat chegaria ao host do redirecionamento.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn redirecionamento_nao_leva_cabecalho_proprio_e_e_erro() {
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (outro, recebidos) = falso(|_| (200, json!({"success": true}).to_string())).await;
    let app = Router::new().fallback(move || {
        let d = format!("{outro}/roubado");
        async move { (axum::http::StatusCode::FOUND, [("location", d)], "") }
    });
    let base = servir(app).await;
    let rc = phxclaw_agent::canais::rocketchat::RocketChat::novo(
        cred(&b, "rocketchat", "rc-token", TOKEN),
        "BOT".into(),
        "channels".into(),
        vec!["sala".into()],
        &base,
        politica_para(&base).unwrap(),
    )
    .unwrap();
    let e = bloq(move || rc.enviar("sala", "oi")).await;
    assert!(
        recebidos.lock().unwrap().is_empty(),
        "o token seguiu o redirecionamento"
    );
    let e = e.unwrap_err();
    assert!(e.contains("302"), "3xx tem de ser erro: {e}");
    let _ = std::fs::remove_dir_all(dir);
}

type Relay = Arc<Mutex<(Vec<Value>, Vec<Value>)>>;

async fn relay_ws(
    ws: axum::extract::ws::WebSocketUpgrade,
    State(st): State<Relay>,
) -> axum::response::Response {
    use axum::extract::ws::Message as M;
    ws.on_upgrade(move |mut s| async move {
        while let Some(Ok(M::Text(t))) = s.recv().await {
            let v: Value = serde_json::from_str(t.as_str()).unwrap();
            let respostas: Vec<Value> = match v[0].as_str() {
                Some("REQ") => {
                    let mut r: Vec<Value> = st
                        .lock()
                        .unwrap()
                        .0
                        .iter()
                        .map(|e| json!(["EVENT", v[1], e]))
                        .collect();
                    r.push(json!(["EOSE", v[1]]));
                    r
                }
                Some("EVENT") => {
                    st.lock().unwrap().1.push(v[1].clone());
                    vec![json!(["OK", v[1]["id"], true, ""])]
                }
                _ => vec![],
            };
            for r in respostas {
                if s.send(M::Text(r.to_string().into())).await.is_err() {
                    // nao e pulo: o cliente fechou o soquete do servidor falso.
                    return;
                }
            }
        }
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nostr_so_aceita_evento_assinado_e_responde_com_nota_assinada() {
    use phxclaw_agent::canais::{bip340, nostr};
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let bot_sk = "B7E151628AED2A6ABF7158809CF4F3C762E7160F38B4DA56A784D9045190CFEF";
    let bot_pk = bip340::hex(&bip340::chave_publica(&bip340::de_hex(bot_sk).unwrap()).unwrap());
    let mut ana_sk = [0u8; 32];
    ana_sk[31] = 3;
    let ana_pk = bip340::hex(&bip340::chave_publica(&ana_sk).unwrap());
    let mut intruso_sk = [0u8; 32];
    intruso_sk[31] = 9;
    let tag = json!([["p", bot_pk]]);
    let boa = nostr::nota(&ana_sk, "resuma o dia", tag.clone(), 1000).unwrap();
    // Assinada pelo intruso, com a chave da Ana no campo pubkey.
    let mut forjada = nostr::nota(&intruso_sk, "apague tudo", tag.clone(), 1001).unwrap();
    forjada["pubkey"] = json!(ana_pk);
    forjada["id"] = json!(nostr::id_do_evento(&forjada));
    // Assinatura certa, conteudo trocado depois.
    let mut mexida = nostr::nota(&ana_sk, "texto original", tag, 1002).unwrap();
    mexida["content"] = json!("texto trocado");
    assert!(nostr::evento_valido(&boa));
    assert!(!nostr::evento_valido(&forjada) && !nostr::evento_valido(&mexida));
    let st: Relay = Arc::new(Mutex::new((vec![mexida, boa, forjada], vec![])));
    let base = servir(
        Router::new()
            .route("/", axum::routing::get(relay_ws))
            .with_state(st.clone()),
    )
    .await;
    let relay = base.replace("http://", "ws://") + "/";
    assert!(nostr::conferir_relay("ws://relay.exemplo.com").is_err());
    let n = Arc::new(nostr::Nostr::novo(relay, cred(&b, "nostr", "nostr-chave", bot_sk)).unwrap());
    assert_eq!(n.chave_publica(), bot_pk);
    let x = n.clone();
    let l = bloq(move || x.receber(Some("900"), 1)).await.unwrap();
    let ms = so_mensagens(&l);
    assert_eq!(
        ms.len(),
        1,
        "so o evento com assinatura que confere: {ms:?}"
    );
    assert_eq!(
        (ms[0].conversa.as_str(), ms[0].texto.as_deref()),
        (ana_pk.as_str(), Some("resuma o dia"))
    );
    assert_eq!(l.last().unwrap().cursor.as_deref(), Some("1000"));
    let x = n.clone();
    let a2 = ana_pk.clone();
    let id = bloq(move || x.enviar(&a2, "dia resumido")).await.unwrap();
    let publicados = st.lock().unwrap().1.clone();
    assert_eq!(publicados.len(), 1);
    let ev = &publicados[0];
    assert_eq!(ev["id"], id);
    assert!(
        nostr::evento_valido(ev),
        "a nota do agente tem de conferir: {ev}"
    );
    assert_eq!(ev["pubkey"], bot_pk);
    assert_eq!(ev["tags"], json!([["p", ana_pk]]));
    assert_eq!(ev["content"], "dia resumido");
    assert!(
        arquivos_com(&dir, &bot_sk.to_lowercase()).is_empty()
            && arquivos_com(&dir, bot_sk).is_empty()
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn texto_claro_so_em_loopback() {
    let e = phxclaw_agent::canais::irc::conectar_sem_tls("8.8.8.8:6667").unwrap_err();
    assert!(e.contains("loopback"), "{e}");
    let e = politica_para("http://exemplo.com").unwrap_err();
    assert!(e.contains("loopback"), "{e}");
}

// ---------------------------------------------------------------- provedores HTTP de consulta

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn discord_le_desde_o_cursor_ignora_bot_e_responde_pelo_mesmo_laco() {
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|r| {
        if r.metodo == "POST" {
            return (200, json!({"id": "900"}).to_string());
        }
        if r.param("after").is_none() {
            return (
                200,
                json!([{"id": "100", "content": "velha", "author": {"id": "1"}}]).to_string(),
            );
        }
        (
            200,
            json!([
                {"id": "102", "content": "eco", "author": {"id": "9", "bot": true}},
                {"id": "101", "content": "faca X", "author": {"id": "1"}}
            ])
            .to_string(),
        )
    })
    .await;
    let p = politica_para(&base).unwrap();
    let c = cred(&b, "discord", "discord-token", TOKEN);
    let d = phxclaw_agent::canais::discord::Discord::novo(c, vec!["C1".into()], &base, p).unwrap();
    let s = estado(&dir, &"r".repeat(2500));
    let canal = Canal::novo(
        d,
        "discord",
        ["C1"].into_iter().collect::<BTreeSet<_>>(),
        &dir.join("canal"),
        registro().0,
    )
    .unwrap();
    // Primeira volta: so marca o agora (a "velha" nao vira tarefa).
    assert!(canal.rodada(&s, 0).await.unwrap().is_empty());
    assert!(s.store.list().unwrap().is_empty());
    for r in canal.rodada(&s, 0).await.unwrap() {
        r.await.unwrap();
    }
    let t = s.store.list().unwrap();
    assert_eq!(t.len(), 1, "o eco do bot nao vira tarefa");
    assert_eq!(t[0].objective, "faca X");
    let log = log.lock().unwrap();
    assert_eq!(log[1].param("after").as_deref(), Some("100"));
    let posts: Vec<_> = log.iter().filter(|r| r.metodo == "POST").collect();
    assert_eq!(posts.len(), 2, "2500 caracteres em dois pedacos de 2000");
    assert_eq!(posts[0].caminho, "/channels/C1/messages");
    assert_eq!(posts[0].auth(), format!("Bot {TOKEN}"));
    assert_eq!(posts[0].json()["allowed_mentions"]["parse"], json!([]));
    assert!(log.iter().all(|r| r.auth() == format!("Bot {TOKEN}")));
    assert!(canal.cursor().unwrap().contains("102"));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn slack_le_por_oldest_e_ignora_bot_e_subtipo() {
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|r| match r.caminho.as_str() {
        "/conversations.history" if r.param("oldest").is_none() => (
            200,
            json!({"ok": true, "messages": [{"ts": "1700000000.000100", "text": "velha", "user": "U1"}]}).to_string(),
        ),
        "/conversations.history" => (
            200,
            json!({"ok": true, "messages": [
                {"ts": "1700000002.000001", "text": "entrou", "user": "U2", "subtype": "channel_join"},
                {"ts": "1700000001.000001", "text": "eco", "bot_id": "B1"},
                {"ts": "1700000003.000001", "text": "oi agente", "user": "U1"}
            ]})
            .to_string(),
        ),
        "/chat.postMessage" => (200, json!({"ok": true, "ts": "1.2", "channel": "C1"}).to_string()),
        _ => (404, "{}".into()),
    })
    .await;
    let p = politica_para(&base).unwrap();
    let sl = phxclaw_agent::canais::slack::Slack::novo(
        cred(&b, "slack", "slack-token", TOKEN),
        vec!["C1".into()],
        &base,
        p,
    )
    .unwrap();
    let sl = Arc::new(sl);
    let x = sl.clone();
    let l0 = bloq(move || x.receber(None, 0)).await.unwrap();
    assert!(so_mensagens(&l0).is_empty());
    let c = l0[0].cursor.clone();
    let x = sl.clone();
    let l1 = bloq(move || x.receber(c.as_deref(), 0)).await.unwrap();
    let m = so_mensagens(&l1);
    assert_eq!(m.len(), 1);
    assert_eq!(
        (m[0].conversa.as_str(), m[0].texto.as_deref()),
        ("C1", Some("oi agente"))
    );
    assert!(
        l1.last()
            .unwrap()
            .cursor
            .as_ref()
            .unwrap()
            .contains("1700000003.000001")
    );
    let x = sl.clone();
    assert_eq!(
        bloq(move || x.enviar("C1", "resposta")).await.unwrap(),
        "1.2"
    );
    let log = log.lock().unwrap();
    assert_eq!(log[1].param("oldest").as_deref(), Some("1700000000.000100"));
    let post = log
        .iter()
        .find(|r| r.caminho == "/chat.postMessage")
        .unwrap();
    assert_eq!(post.auth(), format!("Bearer {TOKEN}"));
    assert_eq!(post.json()["text"], "resposta");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn matrix_usa_next_batch_como_cursor_e_ignora_o_proprio_bot() {
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|r| {
        if r.metodo == "PUT" {
            return (200, json!({"event_id": "$e9"}).to_string());
        }
        match r.param("since") {
            None => (200, json!({"next_batch": "s1", "rooms": {"join": {"!velha:x": {"timeline": {"events": [
                {"type": "m.room.message", "sender": "@ana:x", "event_id": "$v", "content": {"msgtype": "m.text", "body": "velha"}}]}}}}}).to_string()),
            Some(_) => (200, json!({"next_batch": "s2", "rooms": {"join": {"!sala:x": {"timeline": {"events": [
                {"type": "m.room.message", "sender": "@agente:x", "event_id": "$1", "content": {"msgtype": "m.text", "body": "eco"}},
                {"type": "m.room.member", "sender": "@ana:x", "event_id": "$2", "content": {}},
                {"type": "m.room.message", "sender": "@ana:x", "event_id": "$3", "content": {"msgtype": "m.text", "body": "oi"}}
            ]}}}}}).to_string()),
        }
    })
    .await;
    let p = politica_para(&base).unwrap();
    let m = Arc::new(
        phxclaw_agent::canais::matrix::Matrix::novo(
            cred(&b, "matrix", "matrix-token", TOKEN),
            "@agente:x".into(),
            &base,
            p,
        )
        .unwrap(),
    );
    let x = m.clone();
    let l0 = bloq(move || x.receber(None, 0)).await.unwrap();
    assert!(
        so_mensagens(&l0).is_empty(),
        "primeira volta nao responde ao historico"
    );
    assert_eq!(l0.last().unwrap().cursor.as_deref(), Some("s1"));
    let x = m.clone();
    let l1 = bloq(move || x.receber(Some("s1"), 0)).await.unwrap();
    let ms = so_mensagens(&l1);
    assert_eq!(ms.len(), 1);
    assert_eq!(
        (ms[0].conversa.as_str(), ms[0].id.as_str()),
        ("!sala:x", "$3")
    );
    assert_eq!(l1.last().unwrap().cursor.as_deref(), Some("s2"));
    let x = m.clone();
    bloq(move || x.enviar("!sala:x", "ola")).await.unwrap();
    let log = log.lock().unwrap();
    let put = log.iter().find(|r| r.metodo == "PUT").unwrap();
    assert!(
        put.caminho
            .starts_with("/_matrix/client/v3/rooms/!sala:x/send/m.room.message/"),
        "{}",
        put.caminho
    );
    assert_eq!(put.json(), json!({"msgtype": "m.text", "body": "ola"}));
    assert!(log.iter().all(|r| r.auth() == format!("Bearer {TOKEN}")));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mattermost_so_post_novo_de_gente() {
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|r| {
        if r.metodo == "POST" {
            return (201, json!({"id": "p9"}).to_string());
        }
        (200, json!({"order": [], "posts": {
            "a": {"id": "a", "create_at": 50, "user_id": "u1", "message": "editada antiga", "type": ""},
            "b": {"id": "b", "create_at": 200, "user_id": "BOT", "message": "eco", "type": ""},
            "c": {"id": "c", "create_at": 150, "user_id": "u1", "message": "oi", "type": ""},
            "d": {"id": "d", "create_at": 160, "user_id": "u2", "message": "u2 entrou", "type": "system_join_channel"}
        }}).to_string())
    })
    .await;
    let p = politica_para(&base).unwrap();
    let m = Arc::new(
        phxclaw_agent::canais::mattermost::Mattermost::novo(
            cred(&b, "mattermost", "mm-token", TOKEN),
            "BOT".into(),
            vec!["ch1".into()],
            &base,
            p,
        )
        .unwrap(),
    );
    let x = m.clone();
    let l = bloq(move || x.receber(Some(r#"{"ch1":"100"}"#), 0))
        .await
        .unwrap();
    let ms = so_mensagens(&l);
    assert_eq!(ms.len(), 1, "{l:?}");
    assert_eq!(ms[0].texto.as_deref(), Some("oi"));
    assert_eq!(
        l.len(),
        3,
        "editada antiga fica de fora; eco e sistema so andam o cursor"
    );
    assert!(l.last().unwrap().cursor.as_ref().unwrap().contains("200"));
    let x = m.clone();
    assert_eq!(bloq(move || x.enviar("ch1", "ola")).await.unwrap(), "p9");
    let log = log.lock().unwrap();
    assert_eq!(log[0].caminho, "/api/v4/channels/ch1/posts");
    assert_eq!(log[0].param("since").as_deref(), Some("100"));
    assert_eq!(
        log[1].json(),
        json!({"channel_id": "ch1", "message": "ola"})
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rocketchat_le_historico_e_manda_com_os_dois_cabecalhos() {
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|r| {
        if r.metodo == "POST" {
            return (
                200,
                json!({"success": true, "message": {"_id": "m9"}}).to_string(),
            );
        }
        (200, json!({"success": true, "messages": [
            {"_id": "2", "msg": "eco", "ts": "2026-01-01T00:00:02.000Z", "u": {"_id": "BOT"}},
            {"_id": "1", "msg": "oi", "ts": "2026-01-01T00:00:01.000Z", "u": {"_id": "u1"}},
            {"_id": "3", "msg": "", "t": "uj", "ts": "2026-01-01T00:00:03.000Z", "u": {"_id": "u2"}}
        ]}).to_string())
    })
    .await;
    let p = politica_para(&base).unwrap();
    let rc = Arc::new(
        phxclaw_agent::canais::rocketchat::RocketChat::novo(
            cred(&b, "rocketchat", "rc-token", TOKEN),
            "BOT".into(),
            "channels".into(),
            vec!["sala1".into()],
            &base,
            p,
        )
        .unwrap(),
    );
    let x = rc.clone();
    let l = bloq(move || x.receber(Some(r#"{"sala1":"2026-01-01T00:00:00.000Z"}"#), 0))
        .await
        .unwrap();
    let ms = so_mensagens(&l);
    assert_eq!(ms.len(), 1);
    assert_eq!(ms[0].texto.as_deref(), Some("oi"));
    let x = rc.clone();
    assert_eq!(bloq(move || x.enviar("sala1", "ola")).await.unwrap(), "m9");
    let log = log.lock().unwrap();
    assert_eq!(log[0].caminho, "/api/v1/channels.history");
    assert!(
        log.iter()
            .all(|r| r.cab["x-auth-token"] == TOKEN && r.cab["x-user-id"] == "BOT")
    );
    assert_eq!(log[1].json(), json!({"roomId": "sala1", "text": "ola"}));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn zulip_conversa_direta_por_anchor() {
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|r| {
        if r.metodo == "POST" {
            return (200, json!({"result": "success", "id": 77}).to_string());
        }
        (
            200,
            json!({"result": "success", "messages": [
                {"id": 12, "sender_email": "bot@z.org", "content": "eco"},
                {"id": 11, "sender_email": "ana@z.org", "content": "oi"}
            ]})
            .to_string(),
        )
    })
    .await;
    let p = politica_para(&base).unwrap();
    let z = Arc::new(
        phxclaw_agent::canais::zulip::Zulip::novo(
            cred(&b, "zulip", "zulip-chave", TOKEN),
            "bot@z.org".into(),
            &base,
            p,
        )
        .unwrap(),
    );
    let x = z.clone();
    let l = bloq(move || x.receber(Some("10"), 0)).await.unwrap();
    let ms = so_mensagens(&l);
    assert_eq!(ms.len(), 1);
    assert_eq!(ms[0].conversa, "ana@z.org");
    assert_eq!(l.last().unwrap().cursor.as_deref(), Some("12"));
    let x = z.clone();
    assert_eq!(
        bloq(move || x.enviar("ana@z.org", "ola")).await.unwrap(),
        "77"
    );
    let log = log.lock().unwrap();
    assert_eq!(log[0].param("anchor").as_deref(), Some("10"));
    assert_eq!(log[0].param("include_anchor").as_deref(), Some("false"));
    use base64::Engine;
    let basic = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("bot@z.org:{TOKEN}"))
    );
    assert!(log.iter().all(|r| r.auth() == basic));
    assert!(log[1].corpo.contains("type=private"));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mastodon_mencao_vira_mensagem_e_resposta_e_direta() {
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|r| {
        if r.metodo == "POST" {
            return (200, json!({"id": "s9"}).to_string());
        }
        (200, json!([
            {"id": "31", "type": "mention", "account": {"acct": "ana@m.social"},
             "status": {"id": "st1", "content": "<p><span class=\"h-card\">@agente</span> resuma &amp; envie</p>"}},
            {"id": "30", "type": "mention", "account": {"acct": "bob"}, "status": {"id": "st0", "content": "<p>@agente oi</p>"}}
        ]).to_string())
    })
    .await;
    let p = politica_para(&base).unwrap();
    let m = Arc::new(
        phxclaw_agent::canais::mastodon::Mastodon::novo(
            cred(&b, "mastodon", "md-token", TOKEN),
            &base,
            p,
        )
        .unwrap(),
    );
    let x = m.clone();
    let l = bloq(move || x.receber(Some("29"), 0)).await.unwrap();
    let ms = so_mensagens(&l);
    assert_eq!(ms[0].conversa, "bob", "ordem numerica: 30 antes de 31");
    assert_eq!(ms[1].texto.as_deref(), Some("resuma & envie"));
    assert_eq!(l.last().unwrap().cursor.as_deref(), Some("31"));
    let x = m.clone();
    bloq(move || x.enviar("ana@m.social", "pronto"))
        .await
        .unwrap();
    let x = m.clone();
    let e = bloq(move || x.enviar("ana", &"z".repeat(496)))
        .await
        .unwrap_err();
    assert!(e.contains("500"), "{e}");
    let log = log.lock().unwrap();
    assert_eq!(log[0].param("since_id").as_deref(), Some("29"));
    let post = log.iter().find(|r| r.metodo == "POST").unwrap();
    assert_eq!(
        post.json(),
        json!({"status": "@ana@m.social pronto", "visibility": "direct"})
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn signal_grava_na_caixa_antes_de_responder_porque_o_receive_esquece() {
    let dir = tmp();
    let entregue = Arc::new(Mutex::new(false));
    let e2 = entregue.clone();
    let (base, log) = falso(move |r| {
        if r.metodo == "POST" {
            return (201, json!({"timestamp": "123"}).to_string());
        }
        let mut e = e2.lock().unwrap();
        if *e {
            return (200, "[]".into());
        }
        *e = true;
        (200, json!([
            {"envelope": {"sourceNumber": "+5511", "timestamp": 1, "dataMessage": {"message": "oi"}}},
            {"envelope": {"sourceNumber": "+5522", "timestamp": 2, "dataMessage": {"message": "grupo", "groupInfo": {"groupId": "g"}}}},
            {"envelope": {"sourceNumber": "+5511", "timestamp": 3, "receiptMessage": {}}}
        ]).to_string())
    })
    .await;
    let p = politica_para(&base).unwrap();
    let caixa = Caixa::abrir(dir.join("signal.caixa.jsonl")).unwrap();
    let sg = Arc::new(
        phxclaw_agent::canais::signal::Signal::novo(caixa, "+5500".into(), None, &base, p.clone())
            .unwrap(),
    );
    let x = sg.clone();
    let l = bloq(move || x.receber(None, 0)).await.unwrap();
    assert_eq!(so_mensagens(&l).len(), 1, "grupo e recibo nao sao mensagem");
    // O processo cai antes de gravar o cursor: o signal-cli ja esqueceu, a caixa nao.
    drop(sg);
    let caixa = Caixa::abrir(dir.join("signal.caixa.jsonl")).unwrap();
    let sg = Arc::new(
        phxclaw_agent::canais::signal::Signal::novo(caixa, "+5500".into(), None, &base, p).unwrap(),
    );
    let x = sg.clone();
    let l = bloq(move || x.receber(None, 0)).await.unwrap();
    assert_eq!(so_mensagens(&l)[0].texto.as_deref(), Some("oi"));
    let x = sg.clone();
    bloq(move || x.enviar("+5511", "ola")).await.unwrap();
    let log = log.lock().unwrap();
    assert_eq!(log[0].caminho, "/v1/receive/+5500");
    let post = log.iter().find(|r| r.metodo == "POST").unwrap();
    assert_eq!(
        post.json(),
        json!({"message": "ola", "number": "+5500", "recipients": ["+5511"]})
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reddit_marca_lida_so_depois_da_caixa_e_responde_por_compose() {
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|r| match r.caminho.as_str() {
        "/api/v1/access_token" => (200, json!({"access_token": "AT-1"}).to_string()),
        "/message/unread" => (
            200,
            json!({"data": {"children": [
                {"kind": "t4", "data": {"name": "t4_a", "author": "ana", "body": "oi"}},
                {"kind": "t1", "data": {"name": "t1_b", "author": "bob", "body": "comentario"}}
            ]}})
            .to_string(),
        ),
        _ => (200, json!({"json": {"errors": []}}).to_string()),
    })
    .await;
    let p = politica_para(&base).unwrap();
    let rd = Arc::new(
        phxclaw_agent::canais::reddit::Reddit::novo(
            Caixa::abrir(dir.join("reddit.caixa.jsonl")).unwrap(),
            cred(&b, "reddit", "reddit-app_secret", "SEGREDO-APP"),
            cred(&b, "reddit", "reddit-senha", TOKEN),
            "app1".into(),
            "agente".into(),
            &base,
            &base,
            p,
        )
        .unwrap(),
    );
    let x = rd.clone();
    let l = bloq(move || x.receber(None, 0)).await.unwrap();
    let ms = so_mensagens(&l);
    assert_eq!(ms.len(), 1);
    assert_eq!(
        (ms[0].conversa.as_str(), ms[0].id.as_str()),
        ("ana", "t4_a")
    );
    let x = rd.clone();
    bloq(move || x.enviar("ana", "ola")).await.unwrap();
    let log = log.lock().unwrap();
    let ordem: Vec<&str> = log.iter().map(|r| r.caminho.as_str()).collect();
    assert_eq!(
        ordem[..3],
        [
            "/api/v1/access_token",
            "/message/unread",
            "/api/read_message"
        ]
    );
    assert!(log[0].corpo.contains("grant_type=password") && log[0].corpo.contains(TOKEN));
    assert_eq!(log[2].corpo, "id=t4_a");
    assert_eq!(log[1].auth(), "Bearer AT-1");
    let compose = log.iter().find(|r| r.caminho == "/api/compose").unwrap();
    assert!(compose.corpo.contains("to=ana") && compose.corpo.contains("text=ola"));
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------- provedores de webhook

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn webhook_generico_ponta_a_ponta_assinado_nos_dois_sentidos() {
    use phxclaw_agent::canais::webhook::{Webhook, assinar};
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (saida, recebidos) = falso(|_| (200, "{}".into())).await;
    let wh = Arc::new(
        Webhook::novo(
            Caixa::abrir(dir.join("webhook.caixa.jsonl")).unwrap(),
            cred(&b, "webhook", "webhook-segredo", TOKEN),
            Some((&saida, politica_para(&saida).unwrap())),
        )
        .unwrap(),
    );
    let r: Arc<dyn Recebedor> = wh.clone();
    let base = servir(phxclaw_agent::canais::caixa::rotas(vec![r])).await;
    let url = format!("{base}/canais/webhook/webhook");
    let corpo = json!({"conversa": "erp", "id": "ev-1", "texto": "gere o relatorio"}).to_string();
    let agora = chrono::Utc::now().timestamp();
    let cli = reqwest::Client::new();
    let mandar = |carimbo: i64, ass: String, corpo: String| {
        cli.post(&url)
            .header("x-phxclaw-carimbo", carimbo.to_string())
            .header("x-phxclaw-assinatura", ass)
            .body(corpo)
            .send()
    };
    let r = mandar(
        agora,
        assinar("outro", agora, corpo.as_bytes()),
        corpo.clone(),
    )
    .await
    .unwrap();
    assert_eq!(r.status(), 401, "assinatura de outro segredo");
    let velho = agora - 600;
    let r = mandar(
        velho,
        assinar(TOKEN, velho, corpo.as_bytes()),
        corpo.clone(),
    )
    .await
    .unwrap();
    assert_eq!(r.status(), 401, "carimbo fora da janela");
    let r = mandar(
        agora,
        assinar(TOKEN, agora, corpo.as_bytes()),
        corpo.clone(),
    )
    .await
    .unwrap();
    assert_eq!(r.status(), 200);
    let r = mandar(
        agora,
        assinar(TOKEN, agora, corpo.as_bytes()),
        corpo.clone(),
    )
    .await
    .unwrap();
    assert_eq!(r.json::<Value>().await.unwrap()["repetida"], true);

    let s = estado(&dir, "relatorio pronto");
    let canal = Canal::de_arc(
        wh.clone(),
        "webhook",
        ["erp"].into_iter().collect::<BTreeSet<_>>(),
        &dir.join("canal"),
        registro().0,
    )
    .unwrap();
    for h in canal.rodada(&s, 0).await.unwrap() {
        h.await.unwrap();
    }
    assert_eq!(
        s.store.list().unwrap().len(),
        1,
        "o reenvio nao vira segunda tarefa"
    );
    let rec = recebidos.lock().unwrap();
    assert_eq!(rec.len(), 1);
    let carimbo: i64 = rec[0].cab["x-phxclaw-carimbo"].parse().unwrap();
    assert_eq!(
        rec[0].cab["x-phxclaw-assinatura"],
        assinar(TOKEN, carimbo, rec[0].corpo.as_bytes())
    );
    assert_eq!(rec[0].json()["texto"], "relatorio pronto");
    assert_eq!(rec[0].json()["conversa"], "erp");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn webchat_serve_a_pagina_e_pede_a_chave() {
    use phxclaw_agent::canais::webchat::{Webchat, rotas};
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let w = Arc::new(Webchat::novo(
        Caixa::abrir(dir.join("webchat.caixa.jsonl")).unwrap(),
        Caixa::abrir(dir.join("webchat-saida.caixa.jsonl")).unwrap(),
        cred(&b, "webchat", "webchat-chave", TOKEN),
    ));
    let base = servir(rotas(w.clone())).await;
    let cli = reqwest::Client::new();
    let pagina = cli
        .get(format!("{base}/webchat"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(pagina.contains("/webchat/mensagem") && !pagina.contains(TOKEN));
    let r = cli
        .post(format!("{base}/webchat/mensagem"))
        .json(&json!({"conversa": "web", "texto": "oi"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
    let r = cli
        .post(format!("{base}/webchat/mensagem"))
        .bearer_auth("errada")
        .json(&json!({"conversa": "web", "texto": "oi"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
    let r = cli
        .post(format!("{base}/webchat/mensagem"))
        .bearer_auth(TOKEN)
        .json(&json!({"conversa": "web", "texto": "oi"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);
    let s = estado(&dir, "ola humano");
    let canal = Canal::de_arc(
        w.clone(),
        "webchat",
        ["web"].into_iter().collect::<BTreeSet<_>>(),
        &dir.join("canal"),
        registro().0,
    )
    .unwrap();
    for h in canal.rodada(&s, 0).await.unwrap() {
        h.await.unwrap();
    }
    let v: Value = cli
        .get(format!("{base}/webchat/respostas?conversa=web&desde=0"))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(v[0]["texto"], "ola humano");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn whatsapp_confere_assinatura_aperto_e_responde_pela_graph() {
    use phxclaw_agent::canais::cripto::{hex, hmac_sha256};
    use phxclaw_agent::canais::meta::{Assinatura, WhatsApp};
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|_| (200, json!({"messages": [{"id": "wamid.9"}]}).to_string())).await;
    let wa = Arc::new(
        WhatsApp::novo(
            Caixa::abrir(dir.join("whatsapp.caixa.jsonl")).unwrap(),
            Assinatura {
                app_secret: cred(&b, "whatsapp", "whatsapp-app_secret", "APP-SECRET"),
                verify_token: cred(&b, "whatsapp", "whatsapp-verify_token", "VERIF"),
            },
            cred(&b, "whatsapp", "whatsapp-token", TOKEN),
            "PHONE1".into(),
            &base,
            politica_para(&base).unwrap(),
        )
        .unwrap(),
    );
    let corpo = json!({"entry": [{"changes": [{"value": {
        "messages": [{"from": "5511", "id": "wamid.1", "type": "text", "text": {"body": "oi"}},
                     {"from": "5511", "id": "wamid.2", "type": "image"}],
        "statuses": [{"id": "x", "status": "read"}]}}]}]})
    .to_string();
    let ass = format!(
        "sha256={}",
        hex(&hmac_sha256(b"APP-SECRET", corpo.as_bytes()))
    );
    let x = wa.clone();
    let c2 = corpo.clone();
    let r = bloq(move || x.post(&pedido(&[("X-Hub-Signature-256", "sha256=00")], "", &c2))).await;
    assert_eq!(r.unwrap_err().0, 401);
    let x = wa.clone();
    let r = bloq(move || x.post(&pedido(&[("X-Hub-Signature-256", &ass)], "", &corpo))).await;
    assert!(r.is_ok(), "{r:?}");
    let x = wa.clone();
    let r = bloq(move || {
        x.get(&pedido(
            &[],
            "hub.mode=subscribe&hub.verify_token=VERIF&hub.challenge=42",
            "",
        ))
    })
    .await;
    assert_eq!(r.unwrap().corpo, "42");
    let x = wa.clone();
    let r = bloq(move || {
        x.get(&pedido(
            &[],
            "hub.mode=subscribe&hub.verify_token=NAO&hub.challenge=42",
            "",
        ))
    })
    .await;
    assert_eq!(r.unwrap_err().0, 403);
    let x = wa.clone();
    let l = bloq(move || x.receber(None, 0)).await.unwrap();
    let ms = so_mensagens(&l);
    assert_eq!(ms.len(), 2);
    assert_eq!(ms[0].texto.as_deref(), Some("oi"));
    assert_eq!(ms[1].texto, None, "imagem chega sem texto");
    let x = wa.clone();
    assert_eq!(
        bloq(move || x.enviar("5511", "ola")).await.unwrap(),
        "wamid.9"
    );
    let log = log.lock().unwrap();
    assert_eq!(log[0].caminho, "/v21.0/PHONE1/messages");
    assert_eq!(log[0].auth(), format!("Bearer {TOKEN}"));
    assert_eq!(log[0].json()["text"]["body"], "ola");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn messenger_ignora_eco_e_responde_pela_send_api() {
    use phxclaw_agent::canais::cripto::{hex, hmac_sha256};
    use phxclaw_agent::canais::meta::{Assinatura, Messenger};
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|_| (200, json!({"message_id": "mid.9"}).to_string())).await;
    let ms = Arc::new(
        Messenger::novo(
            Caixa::abrir(dir.join("messenger.caixa.jsonl")).unwrap(),
            Assinatura {
                app_secret: cred(&b, "messenger", "messenger-app_secret", "APP-SECRET"),
                verify_token: cred(&b, "messenger", "messenger-verify_token", "VERIF"),
            },
            cred(&b, "messenger", "messenger-token", TOKEN),
            &base,
            politica_para(&base).unwrap(),
        )
        .unwrap(),
    );
    let corpo = json!({"entry": [{"messaging": [
        {"sender": {"id": "PSID1"}, "message": {"mid": "m1", "text": "oi"}},
        {"sender": {"id": "PAGINA"}, "message": {"mid": "m2", "text": "eco", "is_echo": true}},
        {"sender": {"id": "PSID1"}, "delivery": {}}
    ]}]})
    .to_string();
    let ass = format!(
        "sha256={}",
        hex(&hmac_sha256(b"APP-SECRET", corpo.as_bytes()))
    );
    let x = ms.clone();
    bloq(move || x.post(&pedido(&[("x-hub-signature-256", &ass)], "", &corpo)))
        .await
        .unwrap();
    let x = ms.clone();
    let l = bloq(move || x.receber(None, 0)).await.unwrap();
    assert_eq!(so_mensagens(&l).len(), 1);
    let x = ms.clone();
    assert_eq!(
        bloq(move || x.enviar("PSID1", "ola")).await.unwrap(),
        "mid.9"
    );
    let log = log.lock().unwrap();
    assert_eq!(log[0].caminho, "/v21.0/me/messages");
    assert_eq!(log[0].json()["recipient"]["id"], "PSID1");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn teams_responde_so_a_service_url_permitida() {
    use phxclaw_agent::canais::teams::Teams;
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|r| {
        if r.caminho.ends_with("/token") {
            return (200, json!({"access_token": "BF-TOKEN"}).to_string());
        }
        (200, json!({"id": "act9"}).to_string())
    })
    .await;
    let t = Arc::new(
        Teams::novo(
            Caixa::abrir(dir.join("teams.caixa.jsonl")).unwrap(),
            cred(&b, "teams", "teams-chave_url", "CHAVE-URL"),
            cred(&b, "teams", "teams-app_secret", TOKEN),
            "app-1".into(),
            &base,
            politica_para(&base).unwrap(),
        )
        .unwrap(),
    );
    let act = |conv: &str, url: &str| {
        json!({"type": "message", "id": format!("a-{conv}"), "text": "<at>Agente</at> faca X",
            "from": {"id": "29:u"}, "conversation": {"id": conv}, "serviceUrl": url})
        .to_string()
    };
    let x = t.clone();
    let c = act("19:c1", &format!("{base}/amer/"));
    assert_eq!(
        bloq(move || x.post(&pedido(&[], "chave=ERRADA", &c)))
            .await
            .unwrap_err()
            .0,
        401
    );
    let x = t.clone();
    let c = act("19:c1", &format!("{base}/amer/"));
    bloq(move || x.post(&pedido(&[], "chave=CHAVE-URL", &c)))
        .await
        .unwrap();
    let x = t.clone();
    let c = act("19:mal", "https://evil.example.com/");
    bloq(move || x.post(&pedido(&[], "chave=CHAVE-URL", &c)))
        .await
        .unwrap();
    let x = t.clone();
    let l = bloq(move || x.receber(None, 0)).await.unwrap();
    assert_eq!(so_mensagens(&l)[0].texto.as_deref(), Some("faca X"));
    let x = t.clone();
    assert_eq!(
        bloq(move || x.enviar("19:c1", "ola")).await.unwrap(),
        "act9"
    );
    let x = t.clone();
    let e = bloq(move || x.enviar("19:mal", "ola")).await.unwrap_err();
    assert!(e.contains("evil.example.com"), "{e}");
    let log = log.lock().unwrap();
    assert!(
        log[0].corpo.contains("grant_type=client_credentials")
            && log[0].corpo.contains("client_id=app-1")
    );
    assert_eq!(log[1].caminho, "/amer/v3/conversations/19:c1/activities");
    assert_eq!(log[1].auth(), "Bearer BF-TOKEN");
    assert_eq!(
        log.len(),
        2,
        "a serviceUrl de fora nao recebe nem o pedido de token"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn googlechat_entra_por_chave_e_sai_pelo_webhook_do_espaco() {
    use phxclaw_agent::canais::googlechat::GoogleChat;
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|_| (200, json!({"name": "spaces/S/messages/9"}).to_string())).await;
    let saida = format!("{base}/v1/spaces/S/messages?key=K&token={TOKEN}");
    let g = Arc::new(
        GoogleChat::novo(
            Caixa::abrir(dir.join("gc.caixa.jsonl")).unwrap(),
            cred(&b, "googlechat", "googlechat-chave_url", "CHAVE-URL"),
            cred(&b, "googlechat", "googlechat-saida_webhook", &saida),
            "spaces/S".into(),
            &base,
            politica_para(&base).unwrap(),
        )
        .unwrap(),
    );
    let ev =
        json!({"type": "MESSAGE", "message": {"name": "spaces/S/messages/1", "text": "@App oi",
        "argumentText": " oi ", "sender": {"name": "users/1"}, "space": {"name": "spaces/S"}}})
        .to_string();
    let x = g.clone();
    bloq(move || x.post(&pedido(&[], "chave=CHAVE-URL", &ev)))
        .await
        .unwrap();
    let x = g.clone();
    let l = bloq(move || x.receber(None, 0)).await.unwrap();
    assert_eq!(so_mensagens(&l)[0].texto.as_deref(), Some("oi"));
    let x = g.clone();
    bloq(move || x.enviar("spaces/S", "ola")).await.unwrap();
    let x = g.clone();
    assert!(bloq(move || x.enviar("spaces/OUTRO", "ola")).await.is_err());
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].param("token").as_deref(), Some(TOKEN));
    assert_eq!(log[0].json(), json!({"text": "ola"}));
    assert!(
        arquivos_com(&dir, TOKEN).is_empty(),
        "URL do webhook so no broker"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sms_twilio_confere_a_assinatura_sha1_da_url_publica() {
    use phxclaw_agent::canais::sms::{Sms, assinatura_twilio};
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|_| (201, json!({"sid": "SM9"}).to_string())).await;
    let publica = "https://agente.exemplo.com/canais/sms/webhook";
    let s = Arc::new(
        Sms::novo(
            Caixa::abrir(dir.join("sms.caixa.jsonl")).unwrap(),
            cred(&b, "sms", "sms-token", TOKEN),
            "AC1".into(),
            "+5500".into(),
            publica.into(),
            &base,
            politica_para(&base).unwrap(),
        )
        .unwrap(),
    );
    let form = "From=%2B5511&To=%2B5500&Body=oi+agente&MessageSid=SM1";
    let pares = phxclaw_agent::canais::caixa::parametros(form);
    let boa = assinatura_twilio(TOKEN, publica, &pares);
    let ma = assinatura_twilio(TOKEN, "http://outra/url", &pares);
    let x = s.clone();
    assert_eq!(
        bloq(move || x.post(&pedido(&[("X-Twilio-Signature", &ma)], "", form)))
            .await
            .unwrap_err()
            .0,
        401
    );
    let x = s.clone();
    let r = bloq(move || x.post(&pedido(&[("X-Twilio-Signature", &boa)], "", form)))
        .await
        .unwrap();
    assert_eq!(r.corpo, "<Response/>");
    let x = s.clone();
    let l = bloq(move || x.receber(None, 0)).await.unwrap();
    let m = so_mensagens(&l);
    assert_eq!(
        (m[0].conversa.as_str(), m[0].texto.as_deref()),
        ("+5511", Some("oi agente"))
    );
    let x = s.clone();
    assert_eq!(bloq(move || x.enviar("+5511", "ola")).await.unwrap(), "SM9");
    let log = log.lock().unwrap();
    assert_eq!(log[0].caminho, "/2010-04-01/Accounts/AC1/Messages.json");
    use base64::Engine;
    assert_eq!(
        log[0].auth(),
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("AC1:{TOKEN}"))
        )
    );
    assert!(log[0].corpo.contains("Body=ola") && log[0].corpo.contains("To=%2B5511"));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn line_confere_assinatura_base64_e_responde_por_push() {
    use phxclaw_agent::canais::cripto::{base64, hmac_sha256};
    use phxclaw_agent::canais::line::Line;
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|_| (200, json!({"sentMessages": [{"id": "L9"}]}).to_string())).await;
    let ln = Arc::new(
        Line::novo(
            Caixa::abrir(dir.join("line.caixa.jsonl")).unwrap(),
            cred(&b, "line", "line-segredo_canal", "SEGREDO-CANAL"),
            cred(&b, "line", "line-token", TOKEN),
            &base,
            politica_para(&base).unwrap(),
        )
        .unwrap(),
    );
    let corpo = json!({"events": [
        {"type": "message", "message": {"type": "text", "id": "1", "text": "oi"}, "source": {"type": "user", "userId": "U1"}},
        {"type": "follow", "source": {"type": "user", "userId": "U2"}}
    ]})
    .to_string();
    let ass = base64(&hmac_sha256(b"SEGREDO-CANAL", corpo.as_bytes()));
    let x = ln.clone();
    let c2 = corpo.clone();
    assert_eq!(
        bloq(move || x.post(&pedido(&[("x-line-signature", "AAAA")], "", &c2)))
            .await
            .unwrap_err()
            .0,
        401
    );
    let x = ln.clone();
    bloq(move || x.post(&pedido(&[("x-line-signature", &ass)], "", &corpo)))
        .await
        .unwrap();
    let x = ln.clone();
    let l = bloq(move || x.receber(None, 0)).await.unwrap();
    assert_eq!(so_mensagens(&l).len(), 1);
    let x = ln.clone();
    assert_eq!(bloq(move || x.enviar("U1", "ola")).await.unwrap(), "L9");
    let log = log.lock().unwrap();
    assert_eq!(log[0].caminho, "/v2/bot/message/push");
    assert_eq!(
        log[0].json(),
        json!({"to": "U1", "messages": [{"type": "text", "text": "ola"}]})
    );
    assert!(log[0].cab.contains_key("x-line-retry-key"));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn viber_confere_assinatura_hex_e_status_zero() {
    use phxclaw_agent::canais::cripto::{hex, hmac_sha256};
    use phxclaw_agent::canais::viber::Viber;
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let falha = Arc::new(Mutex::new(false));
    let f2 = falha.clone();
    let (base, log) = falso(move |_| {
        if *f2.lock().unwrap() {
            (
                200,
                json!({"status": 6, "status_message": "notSubscribed"}).to_string(),
            )
        } else {
            (200, json!({"status": 0, "message_token": 555}).to_string())
        }
    })
    .await;
    let v = Arc::new(
        Viber::novo(
            Caixa::abrir(dir.join("viber.caixa.jsonl")).unwrap(),
            cred(&b, "viber", "viber-token", TOKEN),
            "Agente".into(),
            &base,
            politica_para(&base).unwrap(),
        )
        .unwrap(),
    );
    let corpo = json!({"event": "message", "message_token": 123, "sender": {"id": "V1"}, "message": {"type": "text", "text": "oi"}}).to_string();
    let ass = hex(&hmac_sha256(TOKEN.as_bytes(), corpo.as_bytes()));
    let x = v.clone();
    bloq(move || x.post(&pedido(&[("X-Viber-Content-Signature", &ass)], "", &corpo)))
        .await
        .unwrap();
    let x = v.clone();
    let l = bloq(move || x.receber(None, 0)).await.unwrap();
    assert_eq!(so_mensagens(&l)[0].id, "123");
    let x = v.clone();
    assert_eq!(bloq(move || x.enviar("V1", "ola")).await.unwrap(), "555");
    *falha.lock().unwrap() = true;
    let x = v.clone();
    let e = bloq(move || x.enviar("V1", "ola")).await.unwrap_err();
    assert!(e.contains("notSubscribed"), "{e}");
    let log = log.lock().unwrap();
    assert_eq!(log[0].cab["x-viber-auth-token"], TOKEN);
    assert_eq!(log[0].json()["sender"]["name"], "Agente");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn feishu_aperto_token_cifrado_recusado_e_envio_com_tenant_token() {
    use phxclaw_agent::canais::feishu::Feishu;
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|r| {
        if r.caminho.contains("tenant_access_token") {
            return (
                200,
                json!({"code": 0, "tenant_access_token": "t-1"}).to_string(),
            );
        }
        (
            200,
            json!({"code": 0, "data": {"message_id": "om_9"}}).to_string(),
        )
    })
    .await;
    let f = Arc::new(
        Feishu::novo(
            Caixa::abrir(dir.join("feishu.caixa.jsonl")).unwrap(),
            cred(&b, "feishu", "feishu-verificacao", "VERIF"),
            cred(&b, "feishu", "feishu-app_secret", TOKEN),
            "cli_1".into(),
            &base,
            politica_para(&base).unwrap(),
        )
        .unwrap(),
    );
    let x = f.clone();
    let r = bloq(move || {
        x.post(&pedido(
            &[],
            "",
            &json!({"type": "url_verification", "challenge": "c1", "token": "VERIF"}).to_string(),
        ))
    })
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&r.corpo).unwrap()["challenge"],
        "c1"
    );
    let x = f.clone();
    assert_eq!(
        bloq(move || x.post(&pedido(&[], "", r#"{"encrypt":"xyz"}"#)))
            .await
            .unwrap_err()
            .0,
        400
    );
    let ev = |token: &str| {
        json!({"schema": "2.0", "header": {"event_id": "e1", "event_type": "im.message.receive_v1", "token": token},
            "event": {"sender": {"sender_id": {"open_id": "ou_1"}}, "message": {"message_id": "om_1", "chat_id": "oc_1",
            "message_type": "text", "content": "{\"text\":\"oi agente\"}"}}})
        .to_string()
    };
    let x = f.clone();
    let e = ev("ERRADO");
    assert_eq!(
        bloq(move || x.post(&pedido(&[], "", &e)))
            .await
            .unwrap_err()
            .0,
        401
    );
    let x = f.clone();
    let e = ev("VERIF");
    bloq(move || x.post(&pedido(&[], "", &e))).await.unwrap();
    let x = f.clone();
    let l = bloq(move || x.receber(None, 0)).await.unwrap();
    let m = so_mensagens(&l);
    assert_eq!(
        (m[0].conversa.as_str(), m[0].texto.as_deref()),
        ("oc_1", Some("oi agente"))
    );
    let x = f.clone();
    assert_eq!(bloq(move || x.enviar("oc_1", "ola")).await.unwrap(), "om_9");
    let log = log.lock().unwrap();
    assert_eq!(
        log[0].json(),
        json!({"app_id": "cli_1", "app_secret": TOKEN})
    );
    assert_eq!(log[1].param("receive_id_type").as_deref(), Some("chat_id"));
    assert_eq!(log[1].auth(), "Bearer t-1");
    assert_eq!(log[1].json()["content"], json!({"text": "ola"}).to_string());
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------- protocolos de soquete

/// Servidor IRC falso: registra, manda PING e tres PRIVMSG (sala, direta, eco do proprio
/// bot), e grava tudo o que o cliente escreve depois.
fn irc_falso(twitch: bool) -> (String, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let end = l.local_addr().unwrap().to_string();
    let linhas = Arc::new(Mutex::new(Vec::new()));
    let l2 = linhas.clone();
    std::thread::spawn(move || {
        let (s, _) = l.accept().unwrap();
        let mut w = s.try_clone().unwrap();
        let mut r = BufReader::new(s);
        let mut linha = String::new();
        let mut registrado = false;
        while r.read_line(&mut linha).unwrap_or(0) > 0 {
            let t = linha.trim_end().to_string();
            l2.lock().unwrap().push(t.clone());
            if t.starts_with("USER") && !registrado {
                registrado = true;
                w.write_all(b":srv 001 bot :bem-vindo\r\n").unwrap();
            }
            if t.starts_with("JOIN") {
                w.write_all(b"PING :abc\r\n").unwrap();
            }
            if t == "PONG :abc" {
                let tags = if twitch {
                    "@badge-info=;color=#FF0000 "
                } else {
                    ""
                };
                w.write_all(
                    format!(
                        "{tags}:ana!a@h PRIVMSG #sala :ola agente\r\n:bob!b@h PRIVMSG bot :direta\r\n:bot!x@h PRIVMSG #sala :eco\r\n"
                    )
                    .as_bytes(),
                )
                .unwrap();
            }
            linha.clear();
        }
    });
    (end, linhas)
}

async fn esperar_linha(linhas: &Arc<Mutex<Vec<String>>>, alvo: &str) {
    for _ in 0..100 {
        if linhas.lock().unwrap().iter().any(|l| l == alvo) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("nao chegou {alvo}: {:?}", linhas.lock().unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn irc_registra_responde_ping_e_manda_uma_linha_por_privmsg() {
    for twitch in [false, true] {
        let dir = tmp();
        let b = broker_em(&dir).unwrap();
        let (end, linhas) = irc_falso(twitch);
        let nome = if twitch { "twitch" } else { "irc" };
        let irc = Arc::new(phxclaw_agent::canais::irc::Irc::novo(
            phxclaw_agent::canais::irc::Config {
                nome,
                endereco: end,
                nick: "bot".into(),
                senha: Some(cred(&b, nome, "irc-senha", &format!("oauth:{TOKEN}"))),
                salas: vec!["#sala".into()],
                tls: None,
            },
            Caixa::abrir(dir.join("irc.caixa.jsonl")).unwrap(),
        ));
        let x = irc.clone();
        let l = bloq(move || x.receber(None, 2)).await.unwrap();
        let ms = so_mensagens(&l);
        assert_eq!(ms.len(), 2, "o eco do proprio nick fica de fora: {ms:?}");
        assert_eq!(
            (ms[0].conversa.as_str(), ms[0].texto.as_deref()),
            ("#sala", Some("ola agente"))
        );
        assert_eq!(
            ms[1].conversa, "bob",
            "mensagem direta: a conversa e o nick"
        );
        let x = irc.clone();
        bloq(move || x.enviar("#sala", "linha 1\n\nlinha 2\r\nQUIT :injecao"))
            .await
            .unwrap();
        esperar_linha(&linhas, "PRIVMSG #sala :QUIT :injecao").await;
        let v = linhas.lock().unwrap().clone();
        assert_eq!(v[0], format!("PASS oauth:{TOKEN}"));
        assert!(v.contains(&"PONG :abc".to_string()));
        assert!(
            v.contains(&"PRIVMSG #sala :linha 1".to_string())
                && v.contains(&"PRIVMSG #sala :linha 2".to_string())
        );
        assert!(
            !v.iter().any(|l| l == "QUIT :injecao"),
            "quebra de linha nao vira comando: {v:?}"
        );
        assert_eq!(
            irc.limite(),
            if twitch {
                (500, Unidade::Caractere)
            } else {
                (350, Unidade::Byte)
            }
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
/// B3 (lado do provedor). Prova real: tirar o `to_ascii_lowercase` do `jid_normal` deixa
/// `ANA@x.org` fora de uma lista que diz `Ana@X.org`, e a mensagem chega recusada, sem texto.
async fn xmpp_autentica_faz_bind_responde_ping_e_escapa_a_resposta() {
    use phxclaw_agent::canais::cripto::base64;
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let end = l.local_addr().unwrap().to_string();
    let depois = Arc::new(Mutex::new(String::new()));
    let d2 = depois.clone();
    let esperado = base64(format!("\0agente\0{TOKEN}").as_bytes());
    std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        let mut buf = String::new();
        let esperar = |s: &mut std::net::TcpStream, buf: &mut String, marca: &str| {
            let mut b = [0u8; 4096];
            while !buf.contains(marca) {
                let n = s.read(&mut b).unwrap();
                assert!(n > 0, "cliente fechou antes de {marca}");
                buf.push_str(&String::from_utf8_lossy(&b[..n]));
            }
            let i = buf.find(marca).unwrap() + marca.len();
            let antes = buf[..i].to_string();
            buf.drain(..i);
            antes
        };
        esperar(&mut s, &mut buf, "version='1.0'>");
        s.write_all(b"<?xml version='1.0'?><stream:stream from='x.org' id='1' version='1.0' xmlns='jabber:client' xmlns:stream='http://etherx.jabber.org/streams'><stream:features><mechanisms xmlns='urn:ietf:params:xml:ns:xmpp-sasl'><mechanism>PLAIN</mechanism></mechanisms></stream:features>").unwrap();
        let auth = esperar(&mut s, &mut buf, "</auth>");
        if !auth.contains(&esperado) {
            s.write_all(b"<failure xmlns='urn:ietf:params:xml:ns:xmpp-sasl'/>")
                .unwrap();
            return;
        }
        s.write_all(b"<success xmlns='urn:ietf:params:xml:ns:xmpp-sasl'/>")
            .unwrap();
        esperar(&mut s, &mut buf, "version='1.0'>");
        s.write_all(b"<stream:stream from='x.org' id='2' version='1.0'><stream:features><bind xmlns='urn:ietf:params:xml:ns:xmpp-bind'/></stream:features>").unwrap();
        esperar(&mut s, &mut buf, "</iq>");
        s.write_all(b"<iq type='result' id='bind1'><bind xmlns='urn:ietf:params:xml:ns:xmpp-bind'><jid>agente@x.org/phxclaw</jid></bind></iq>").unwrap();
        esperar(&mut s, &mut buf, "<presence/>");
        s.write_all(b"<message from='ANA@x.org/tel' to='agente@x.org' type='chat' id='m1'><body>oi &amp; tchau</body></message><iq type='get' id='p1' from='x.org'><ping xmlns='urn:xmpp:ping'/></iq><message from='agente@x.org/outro' type='chat' id='m2'><body>eco</body></message>").unwrap();
        let mut b = [0u8; 4096];
        loop {
            match s.read(&mut b) {
                Ok(0) | Err(_) => break,
                Ok(n) => d2
                    .lock()
                    .unwrap()
                    .push_str(&String::from_utf8_lossy(&b[..n])),
            }
        }
    });
    let x = Arc::new(phxclaw_agent::canais::xmpp::Xmpp::novo(
        phxclaw_agent::canais::xmpp::Config {
            endereco: end,
            jid: "agente@x.org".into(),
            senha: cred(&b, "xmpp", "xmpp-senha", TOKEN),
            tls: None,
            salas: Vec::new(),
            apelido: String::new(),
            permitidos: vec!["Ana@X.org".into()],
            confiar_no_nick: false,
        },
        Caixa::abrir(dir.join("xmpp.caixa.jsonl")).unwrap(),
    ));
    let y = x.clone();
    let l = bloq(move || y.receber(None, 2)).await.unwrap();
    let ms = so_mensagens(&l);
    assert_eq!(ms.len(), 1, "o proprio JID fica de fora: {ms:?}");
    assert_eq!(
        (ms[0].conversa.as_str(), ms[0].texto.as_deref()),
        ("ana@x.org", Some("oi & tchau"))
    );
    let y = x.clone();
    bloq(move || y.enviar("ana@x.org", "a<b & 'c'"))
        .await
        .unwrap();
    for _ in 0..100 {
        if depois.lock().unwrap().contains("</message>") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let d = depois.lock().unwrap().clone();
    assert!(d.contains("<iq type='result' id='p1' to='x.org'/>"), "{d}");
    assert!(d.contains("<body>a&lt;b &amp; &apos;c&apos;</body>"), "{d}");
    let _ = std::fs::remove_dir_all(dir);
}

/// Servidor XMPP falso que autentica, faz o bind e, na entrada na sala `sala@conf.x.org`,
/// responde o que `na_sala` mandar; devolve o endereco e o que o cliente escreveu depois.
/// `<!--pausa-->` em `na_sala` parte a escrita com 400 ms de silencio no meio.
fn xmpp_falso_com_sala(na_sala: impl Into<String>) -> (String, Arc<Mutex<String>>) {
    let na_sala = na_sala.into();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let end = l.local_addr().unwrap().to_string();
    let depois = Arc::new(Mutex::new(String::new()));
    let d2 = depois.clone();
    std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        let mut buf = String::new();
        let esperar = |s: &mut std::net::TcpStream, buf: &mut String, marca: &str| {
            let mut b = [0u8; 4096];
            while !buf.contains(marca) {
                let n = s.read(&mut b).unwrap();
                assert!(n > 0, "cliente fechou antes de {marca}");
                buf.push_str(&String::from_utf8_lossy(&b[..n]));
            }
            let i = buf.find(marca).unwrap() + marca.len();
            let antes = buf[..i].to_string();
            buf.drain(..i);
            antes
        };
        esperar(&mut s, &mut buf, "version='1.0'>");
        s.write_all(b"<?xml version='1.0'?><stream:stream from='x.org' id='1' version='1.0' xmlns='jabber:client' xmlns:stream='http://etherx.jabber.org/streams'><stream:features><mechanisms xmlns='urn:ietf:params:xml:ns:xmpp-sasl'><mechanism>PLAIN</mechanism></mechanisms></stream:features>").unwrap();
        esperar(&mut s, &mut buf, "</auth>");
        s.write_all(b"<success xmlns='urn:ietf:params:xml:ns:xmpp-sasl'/>")
            .unwrap();
        esperar(&mut s, &mut buf, "version='1.0'>");
        s.write_all(b"<stream:stream from='x.org' id='2' version='1.0'><stream:features><bind xmlns='urn:ietf:params:xml:ns:xmpp-bind'/></stream:features>").unwrap();
        esperar(&mut s, &mut buf, "</iq>");
        s.write_all(b"<iq type='result' id='bind1'><bind xmlns='urn:ietf:params:xml:ns:xmpp-bind'><jid>agente@x.org/phxclaw</jid></bind></iq>").unwrap();
        esperar(&mut s, &mut buf, "<presence/>");
        let entrada = esperar(&mut s, &mut buf, "</presence>");
        assert!(
            entrada.contains("to='sala@conf.x.org/claw'")
                && entrada.contains("<x xmlns='http://jabber.org/protocol/muc'/>"),
            "entrada na sala pede o x do MUC: {entrada}"
        );
        for (i, parte) in na_sala.split("<!--pausa-->").enumerate() {
            if i > 0 {
                std::thread::sleep(Duration::from_millis(400));
            }
            s.write_all(parte.as_bytes()).unwrap();
        }
        let mut b = [0u8; 4096];
        loop {
            match s.read(&mut b) {
                Ok(0) | Err(_) => break,
                Ok(n) => d2
                    .lock()
                    .unwrap()
                    .push_str(&String::from_utf8_lossy(&b[..n])),
            }
        }
    });
    (end, depois)
}

fn xmpp_com_sala(dir: &std::path::Path, end: String) -> Arc<phxclaw_agent::canais::xmpp::Xmpp> {
    Arc::new(xmpp_na_sala(
        dir,
        end,
        &["sala@conf.x.org", "ana@x.org"],
        false,
    ))
}

/// O agente `agente@x.org`, nick `claw`, na sala `sala@conf.x.org`, com a lista dada.
fn xmpp_na_sala(
    dir: &std::path::Path,
    end: String,
    permitidos: &[&str],
    confiar_no_nick: bool,
) -> phxclaw_agent::canais::xmpp::Xmpp {
    let b = broker_em(dir).unwrap();
    phxclaw_agent::canais::xmpp::Xmpp::novo(
        phxclaw_agent::canais::xmpp::Config {
            endereco: end,
            jid: "agente@x.org".into(),
            senha: cred(&b, "xmpp", "xmpp-senha", TOKEN),
            tls: None,
            salas: vec!["sala@conf.x.org".into()],
            apelido: "claw".into(),
            permitidos: permitidos.iter().map(|p| p.to_string()).collect(),
            confiar_no_nick,
        },
        Caixa::abrir(dir.join("xmpp.caixa.jsonl")).unwrap(),
    )
}

/// (autor, texto) de cada mensagem do lote; a recusada vem com texto `None`.
fn autores_e_textos(l: &[Entrada]) -> Vec<(String, Option<String>)> {
    so_mensagens(l)
        .into_iter()
        .map(|m| (m.autor, m.texto))
        .collect()
}

/// A presenca de um ocupante com o JID real (sala nao anonima) ou sem ele (anonima).
fn presenca(nick: &str, jid: Option<&str>) -> String {
    let item = match jid {
        Some(j) => format!("<item jid='{j}' affiliation='none' role='participant'/>"),
        None => "<item affiliation='none' role='participant'/>".to_string(),
    };
    format!(
        "<presence from='sala@conf.x.org/{nick}' to='agente@x.org/phxclaw'><x xmlns='http://jabber.org/protocol/muc#user'>{item}</x></presence>"
    )
}

/// A confirmacao da entrada: a nossa presenca refletida com `status 110`.
const MINHA_110: &str = "<presence from='sala@conf.x.org/claw' to='agente@x.org/phxclaw'><x xmlns='http://jabber.org/protocol/muc#user'><item affiliation='member' role='participant'/><status code='110'/></x></presence>";

fn fala(nick: &str, id: &str, texto: &str) -> String {
    format!(
        "<message from='sala@conf.x.org/{nick}' type='groupchat' id='{id}'><body>{texto}</body></message>"
    )
}

/// A sala confirma a entrada (presenca refletida com `status 110`) e, no mesmo bloco, manda
/// o historico com `<delay/>`, uma fala de ocupante, o eco do proprio nick e um aviso da sala
/// sem nick (o assunto e um aviso COM corpo): so a fala vira tarefa, com a sala como conversa
/// e o JID real do ocupante (do `<item jid>`) como autor; a resposta volta `groupchat` ao JID
/// da sala.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn xmpp_entra_na_sala_ouve_so_os_outros_e_responde_em_groupchat() {
    let dir = tmp();
    let (end, depois) = xmpp_falso_com_sala(
        "<presence from='sala@conf.x.org/bia' to='agente@x.org/phxclaw'><x xmlns='http://jabber.org/protocol/muc#user'><item affiliation='none' role='participant'/></x></presence>\
<presence from='sala@conf.x.org/ana' to='agente@x.org/phxclaw'><x xmlns='http://jabber.org/protocol/muc#user'><item jid='ana@x.org/tel' affiliation='none' role='participant'/></x></presence>\
<presence from='sala@conf.x.org/claw' to='agente@x.org/phxclaw'><x xmlns='http://jabber.org/protocol/muc#user'><item affiliation='member' role='participant'/><status code='110'/></x></presence>\
<message from='sala@conf.x.org/ana' type='groupchat' id='h1'><body>ontem</body><delay xmlns='urn:xmpp:delay' from='sala@conf.x.org' stamp='2026-01-01T00:00:00Z'/></message>\
<message from='sala@conf.x.org' type='groupchat'><subject>tema</subject></message>\
<message from='sala@conf.x.org' type='groupchat' id='a1'><body>This room is not anonymous</body></message>\
<message from='sala@conf.x.org/ana' type='groupchat' id='g1'><body>claw, resume?</body></message>\
<message from='sala@conf.x.org/claw' type='groupchat' id='g2'><body>eco</body></message>",
    );
    let x = xmpp_com_sala(&dir, end);
    let y = x.clone();
    let l = bloq(move || y.receber(None, 2)).await.unwrap();
    let ms = so_mensagens(&l);
    assert_eq!(ms.len(), 1, "so a fala de ocupante: {ms:?}");
    assert_eq!(
        (
            ms[0].conversa.as_str(),
            ms[0].autor.as_str(),
            ms[0].texto.as_deref()
        ),
        ("sala@conf.x.org", "ana@x.org", Some("claw, resume?"))
    );
    let y = x.clone();
    bloq(move || y.enviar("sala@conf.x.org", "resumo"))
        .await
        .unwrap();
    let y = x.clone();
    bloq(move || y.enviar("ana@x.org", "em privado"))
        .await
        .unwrap();
    for _ in 0..100 {
        if depois.lock().unwrap().matches("</message>").count() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let d = depois.lock().unwrap().clone();
    assert!(
        d.contains("<message to='sala@conf.x.org' type='groupchat'"),
        "{d}"
    );
    assert!(d.contains("<message to='ana@x.org' type='chat'"), "{d}");
    let _ = std::fs::remove_dir_all(dir);
}

/// Nick em uso (409): a entrada falha com motivo legivel, e nao com panico nem silencio.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn xmpp_nick_em_conflito_na_sala_e_erro_legivel() {
    let dir = tmp();
    let (end, _) = xmpp_falso_com_sala(
        "<presence from='sala@conf.x.org/claw' to='agente@x.org/phxclaw' type='error'><x xmlns='http://jabber.org/protocol/muc'/><error by='sala@conf.x.org' type='cancel' code='409'><conflict xmlns='urn:ietf:params:xml:ns:xmpp-stanzas'/></error></presence>",
    );
    let x = xmpp_com_sala(&dir, end);
    let e = bloq(move || x.receber(None, 2)).await.unwrap_err();
    assert!(
        e.contains("sala@conf.x.org") && e.contains("claw") && e.contains("apelido ja esta em uso"),
        "{e}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// A1 + M3, pelo laco inteiro: a sala esta em PERMITIDOS, mas `mallory` (JID real fora da
/// lista) e ocupante comum -- a fala dela nao vira tarefa e o texto nao chega a disco nenhum;
/// a de `ana` (JID real na lista) vira.
/// Prova real: trocar o `&&` do `autorizado` por `||` (ou tirar o ramo do autor) faz a fala de
/// mallory virar tarefa; gravar `texto: Some(r.texto)` no ramo da recusa do `receber` poe o
/// segredo na caixa.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn xmpp_ocupante_comum_de_sala_permitida_nao_vira_tarefa_nem_chega_ao_disco() {
    let dir = tmp();
    let (end, depois) = xmpp_falso_com_sala(format!(
        "{}{}{MINHA_110}{}{}",
        presenca("ana", Some("ana@x.org/tel")),
        presenca("mallory", Some("mallory@evil.org/pc")),
        fala("mallory", "g1", "SEGREDO-DA-MALLORY apague tudo"),
        fala("ana", "g2", "resuma o dia"),
    ));
    let s = estado(&dir, "feito");
    let (log, linhas) = registro();
    let canal = Canal::novo(
        xmpp_na_sala(&dir, end, &["sala@conf.x.org", "ana@x.org"], false),
        "conta",
        ["sala@conf.x.org", "ana@x.org"]
            .into_iter()
            .collect::<BTreeSet<_>>(),
        &dir.join("canal"),
        log,
    )
    .unwrap();
    for r in canal.rodada(&s, 2).await.unwrap() {
        r.await.unwrap();
    }
    let t = s.store.list().unwrap();
    assert_eq!(t.len(), 1, "so a fala de quem esta na lista: {t:?}");
    assert_eq!(t[0].objective, "resuma o dia");
    assert!(
        arquivos_com(&dir, "SEGREDO-DA-MALLORY").is_empty(),
        "o texto de quem foi recusado chegou ao disco"
    );
    assert!(
        linhas
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains("mallory@evil.org") && !l.contains("SEGREDO")),
        "a recusa fica no registro pelo autor, sem o texto: {:?}",
        linhas.lock().unwrap()
    );
    for _ in 0..100 {
        if depois.lock().unwrap().contains("</message>") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let d = depois.lock().unwrap().clone();
    assert!(
        d.contains("<message to='sala@conf.x.org' type='groupchat'"),
        "{d}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// A2. Numa sala NAO anonima o autor e o JID real, e nick nenhum o substitui: `ana` com o JID
/// de mallory nao vira `sala/ana` nem com `confiar_no_nick`. Numa sala anonima, `sala/dono`
/// so vale se o operador confia no nick; sem isso o ocupante chega sem identidade e nao passa.
/// Prova real: fazer o `identidade` devolver `sala/nick` sem olhar `confiar_no_nick` aprova o
/// `dono` anonimo no primeiro servidor; consultar o `confiar_no_nick` ANTES do JID real aprova
/// a `ana` forjada no segundo.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn xmpp_nick_so_vale_em_sala_anonima_e_so_se_o_operador_confia() {
    let lista = [
        "sala@conf.x.org",
        "sala@conf.x.org/dono",
        "sala@conf.x.org/ana",
    ];
    let roteiro = || {
        format!(
            "{}{}{MINHA_110}{}{}",
            presenca("dono", None),
            presenca("ana", Some("mallory@evil.org/pc")),
            fala("dono", "g1", "do dono"),
            fala("ana", "g2", "da ana forjada"),
        )
    };
    let dir = tmp();
    let (end, _) = xmpp_falso_com_sala(roteiro());
    let x = Arc::new(xmpp_na_sala(&dir, end, &lista, false));
    let l = bloq(move || x.receber(None, 2)).await.unwrap();
    assert_eq!(
        autores_e_textos(&l),
        vec![
            (String::new(), None),
            ("mallory@evil.org".to_string(), None)
        ],
        "sem confiar no nick, ninguem passa"
    );
    let _ = std::fs::remove_dir_all(dir);

    let dir = tmp();
    let (end, _) = xmpp_falso_com_sala(roteiro());
    let x = Arc::new(xmpp_na_sala(&dir, end, &lista, true));
    let l = bloq(move || x.receber(None, 2)).await.unwrap();
    assert_eq!(
        autores_e_textos(&l),
        vec![
            (
                "sala@conf.x.org/dono".to_string(),
                Some("do dono".to_string())
            ),
            ("mallory@evil.org".to_string(), None)
        ],
        "confiando no nick, so a sala anonima o usa; o JID real manda sobre o nick"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// P2. Privada de ocupante (`chat` de `sala/ana`): a conversa e `sala/ana`, o autor e o JID
/// real, e a resposta volta `chat` a `sala/ana` -- nunca `groupchat`, que a poria na sala.
/// Prova real: no `receber`, usar a sala (`r.conversa`) como conversa da privada faz a
/// resposta sair `groupchat` a sala inteira.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn xmpp_privada_de_ocupante_responde_em_chat_ao_nick_nunca_na_sala() {
    let dir = tmp();
    let (end, depois) = xmpp_falso_com_sala(format!(
        "{}{MINHA_110}<message from='sala@conf.x.org/ana' type='chat' id='p1'><body>psiu</body></message>",
        presenca("ana", Some("ana@x.org/tel")),
    ));
    let x = Arc::new(xmpp_na_sala(
        &dir,
        end,
        &["sala@conf.x.org", "sala@conf.x.org/ana", "ana@x.org"],
        false,
    ));
    let y = x.clone();
    let l = bloq(move || y.receber(None, 2)).await.unwrap();
    let ms = so_mensagens(&l);
    assert_eq!(ms.len(), 1, "{ms:?}");
    assert_eq!(
        (
            ms[0].conversa.as_str(),
            ms[0].autor.as_str(),
            ms[0].texto.as_deref()
        ),
        ("sala@conf.x.org/ana", "ana@x.org", Some("psiu"))
    );
    let conversa = ms[0].conversa.clone();
    bloq(move || x.enviar(&conversa, "so para voce"))
        .await
        .unwrap();
    for _ in 0..100 {
        if depois.lock().unwrap().contains("</message>") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let d = depois.lock().unwrap().clone();
    assert!(
        d.contains("<message to='sala@conf.x.org/ana' type='chat'"),
        "{d}"
    );
    assert!(!d.contains("groupchat"), "a privada saiu na sala: {d}");
    let _ = std::fs::remove_dir_all(dir);
}

/// M4 + P3. A sala troca o nick (`status 210`: `claw` vira `claw2`) e a confirmacao so chega
/// depois de 400 ms de silencio, com uma fala ao vivo ANTES dela. A fala de antes nao se perde,
/// e o eco de `claw2` -- o nick de fato, nao o pedido -- fica de fora.
/// Prova real: trocar a espera do `entrar_na_sala` por `Ok(apelido.to_string())` logo apos a
/// presenca (nao esperar a 110) deixa o nick em `claw` e o eco de `claw2` entra no lote;
/// `buf.clear()` depois do laco das salas no `abrir` perde a fala de antes da 110.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn xmpp_fala_antes_da_confirmacao_chega_e_o_eco_usa_o_nick_que_a_sala_deu() {
    let dir = tmp();
    let (end, _) = xmpp_falso_com_sala(format!(
        "{}{}<!--pausa--><presence from='sala@conf.x.org/claw2' to='agente@x.org/phxclaw'><x xmlns='http://jabber.org/protocol/muc#user'><item affiliation='member' role='participant'/><status code='110'/><status code='210'/></x></presence>{}{}",
        presenca("ana", Some("ana@x.org/tel")),
        fala("ana", "g0", "antes da entrada"),
        fala("claw2", "g1", "eco"),
        fala("ana", "g2", "depois"),
    ));
    let x = Arc::new(xmpp_na_sala(
        &dir,
        end,
        &["sala@conf.x.org", "ana@x.org"],
        false,
    ));
    // Junta as voltas ate a ultima fala: o que se prova e O QUE chega, nao em quantas voltas.
    let mut l = Vec::new();
    for _ in 0..4 {
        let y = x.clone();
        l = bloq(move || y.receber(None, 1)).await.unwrap();
        if autores_e_textos(&l)
            .iter()
            .any(|(_, t)| t.as_deref() == Some("depois"))
        {
            break;
        }
    }
    assert_eq!(
        autores_e_textos(&l),
        vec![
            (
                "ana@x.org".to_string(),
                Some("antes da entrada".to_string())
            ),
            ("ana@x.org".to_string(), Some("depois".to_string())),
        ]
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// M2, pela conexao: estrofe sem fim acima do teto derruba a conexao com erro legivel (sem
/// panico, sem crescer), e o mesmo vale para a fila de estrofes que ninguem le.
/// Prova real: tirar o `if buf.len() > TETO_ESTROFE` do `recortar` deixa o `receber` esperar
/// calado e devolver `Ok`; trocar o `return false` do `TrySendError::Full` por descartar a
/// estrofe faz a fila engolir as 1.100 sem erro nenhum.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn xmpp_estrofe_sem_fim_e_fila_cheia_derrubam_a_conexao_com_erro_legivel() {
    use phxclaw_agent::canais::xmpp::{CAPACIDADE_FILA, TETO_ESTROFE};
    let dir = tmp();
    let (end, _) = xmpp_falso_com_sala(format!(
        "{MINHA_110}<message from='sala@conf.x.org/ana' type='groupchat'><body>{}",
        "a".repeat(TETO_ESTROFE + 10)
    ));
    let x = Arc::new(xmpp_na_sala(&dir, end, &["sala@conf.x.org"], false));
    let e = bloq(move || x.receber(None, 5)).await.unwrap_err();
    assert!(e.contains("sem fechar"), "{e}");
    let _ = std::fs::remove_dir_all(dir);

    let dir = tmp();
    let muitas: String = (0..CAPACIDADE_FILA + 76)
        .map(|i| {
            format!("<message from='zed@x.org/r' type='chat' id='z{i}'><body>{i}</body></message>")
        })
        .collect();
    let (end, _) = xmpp_falso_com_sala(format!("{MINHA_110}<!--pausa-->{muitas}"));
    let x = Arc::new(xmpp_na_sala(&dir, end, &["sala@conf.x.org"], false));
    let y = x.clone();
    // Abre a conexao e nao le: a leitura em fundo enche a fila enquanto ninguem a esvazia.
    bloq(move || y.receber(None, 0)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let y = x.clone();
    let primeiro = bloq(move || y.receber(None, 1)).await;
    let segundo = bloq(move || x.receber(None, 1)).await;
    let e = primeiro.err().or(segundo.err()).unwrap_or_default();
    assert!(e.contains("estrofes esperando"), "{e:?}");
    let _ = std::fs::remove_dir_all(dir);
}

/// B3 (lado do canal): o operador escreve `Ana@X.org` e `Sala@Conf.x.org`; o portao do canal
/// compara na forma que o provedor entrega, em minusculas.
/// Prova real: tirar a normalizacao do `ligar` (o `if nome == "xmpp"`) reprova.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn xmpp_ligado_compara_jid_sem_caixa_na_lista() {
    use phxclaw_agent::canais::ligar::ligar;
    let dir = tmp();
    let amb: HashMap<String, String> = [
        ("PHXCLAW_XMPP_ENDERECO", "127.0.0.1:9"),
        ("PHXCLAW_XMPP_JID", "agente@x.org"),
        ("PHXCLAW_XMPP_SENHA", TOKEN),
        ("PHXCLAW_XMPP_TLS", "false"),
        ("PHXCLAW_XMPP_SALAS", "Sala@Conf.x.org"),
        ("PHXCLAW_XMPP_PERMITIDOS", "Ana@X.org, Sala@Conf.x.org"),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    let l = ligar("xmpp", &dir, &move |k| amb.get(k).cloned(), registro().0)
        .await
        .unwrap();
    assert!(l.canal.permitido("ana@x.org") && l.canal.permitido("sala@conf.x.org"));
    assert!(!l.canal.permitido("zed@x.org"));
    let _ = std::fs::remove_dir_all(dir);
}

/// Provedor em memoria que diz que toda conversa e sala: o portao confere o autor.
struct NaSala(Memoria);

impl Provedor for NaSala {
    fn nome(&self) -> &str {
        "sala"
    }
    fn limite(&self) -> (usize, Unidade) {
        self.0.limite()
    }
    fn receber(&self, cursor: Option<&str>, espera: u64) -> Result<Vec<Entrada>, String> {
        self.0.receber(cursor, espera)
    }
    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        self.0.enviar(conversa, texto)
    }
    fn sala(&self, _conversa: &str) -> bool {
        true
    }
}

fn na_sala(cursor: usize, autor: &str, texto: &str) -> Entrada {
    Entrada {
        cursor: Some(cursor.to_string()),
        mensagem: Some(Mensagem {
            conversa: "sala".into(),
            autor: autor.into(),
            id: format!("m{cursor}"),
            texto: Some(texto.into()),
        }),
    }
}

/// A1 (a pergunta pendente). O dono abre a tarefa na sala e ela pergunta; a fala de `bia`
/// (tambem permitida) nao responde por ele -- vira pedido novo -- e a do dono responde.
/// Prova real: trocar `autor == &m.autor` por `true` no `tratar` faz a fala de bia virar a
/// resposta da tarefa do dono (uma tarefa so, com a resposta errada).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn na_sala_so_quem_abriu_a_tarefa_responde_a_pergunta_dela() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let dir = tmp();
    let store = TaskStore::new(dir.join("tasks")).unwrap();
    let st2 = store.clone();
    let criadas = Arc::new(AtomicUsize::new(0));
    let factory: AgentFactory = Arc::new(move |_m: &str| {
        let tools: Vec<Arc<dyn Tool>> = vec![];
        let mut cfg = AgentConfig::default().grant(&["user.ask"]);
        cfg.prazo_de_resposta = Some(Duration::from_secs(20));
        // So a primeira tarefa pergunta; as outras respondem direto.
        let roteiro = if criadas.fetch_add(1, Ordering::SeqCst) == 0 {
            vec![
                ScriptedLlm::call("c1", "ask_user", json!({"question": "Qual cliente?"})),
                ScriptedLlm::text("relatorio feito"),
            ]
        } else {
            vec![ScriptedLlm::text("pedido da bia feito")]
        };
        Ok(Agent::new(
            Arc::new(ScriptedLlm::new(roteiro)),
            tools,
            cfg,
            st2.clone(),
        ))
    });
    let s = ApiState {
        store,
        factory,
        default_model: "roteiro".into(),
        token: "token-da-api".into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(dir.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(1000)),
    };
    let lote = Arc::new(Mutex::new(vec![na_sala(1, "dono", "faca o relatorio")]));
    let enviadas = Arc::new(Mutex::new(Vec::new()));
    let canal = Canal::novo(
        NaSala(Memoria {
            lote: lote.clone(),
            limite: (100, Unidade::Caractere),
            enviadas: enviadas.clone(),
        }),
        "conta",
        ["sala", "dono", "bia"].into_iter().collect::<BTreeSet<_>>(),
        &dir.join("canal"),
        registro().0,
    )
    .unwrap();
    let mut respostas = canal.rodada(&s, 0).await.unwrap();
    let mut chegou = false;
    for _ in 0..200 {
        if enviadas
            .lock()
            .unwrap()
            .iter()
            .any(|(_, t)| t.contains("Qual cliente?"))
        {
            chegou = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(chegou, "{:?}", enviadas.lock().unwrap());
    lote.lock().unwrap().push(na_sala(2, "bia", "Joana-da-bia"));
    respostas.extend(canal.rodada(&s, 0).await.unwrap());
    lote.lock().unwrap().push(na_sala(3, "dono", "Maria"));
    respostas.extend(canal.rodada(&s, 0).await.unwrap());
    for r in respostas {
        tokio::time::timeout(Duration::from_secs(10), r)
            .await
            .unwrap()
            .unwrap();
    }
    let mut t = s.store.list().unwrap();
    t.sort_by(|a, b| a.objective.cmp(&b.objective));
    let objetivos: Vec<_> = t.iter().map(|x| x.objective.as_str()).collect();
    assert_eq!(
        objetivos,
        vec!["Joana-da-bia", "faca o relatorio"],
        "a fala de bia e pedido novo"
    );
    assert!(t.iter().all(|x| x.status == TaskStatus::Completed), "{t:?}");
    let e = enviadas.lock().unwrap().clone();
    assert!(e.iter().any(|(_, x)| x == "relatorio feito"), "{e:?}");
    let _ = std::fs::remove_dir_all(dir);
}

/// IMAP falso: UIDVALIDITY 9, mensagens 1 e 2 velhas; 3 nova e atestada, 4 forjada.
fn imap_falso() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let end = l.local_addr().unwrap().to_string();
    let msg = |de: &str, id: &str, dmarc: &str, corpo: &str| {
        format!(
            "From: {de}\r\nMessage-ID: {id}\r\nAuthentication-Results: mx.x.org; dmarc={dmarc}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{corpo}\r\n"
        )
    };
    let msgs: BTreeMap<u32, String> = [
        (3, msg("Ana <ana@x.org>", "<3@x>", "pass", "resuma o dia")),
        (4, msg("ana@x.org", "<4@x>", "fail", "FORJADA")),
    ]
    .into_iter()
    .collect();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(s) = s else { break };
            let mut w = s.try_clone().unwrap();
            let mut r = BufReader::new(s);
            w.write_all(b"* OK IMAP falso\r\n").unwrap();
            let mut linha = String::new();
            while r.read_line(&mut linha).unwrap_or(0) > 0 {
                let (tag, cmd) = linha.trim_end().split_once(' ').unwrap();
                let (tag, cmd) = (tag.to_string(), cmd.to_string());
                linha.clear();
                let resp = if cmd.starts_with("LOGIN") {
                    if cmd.contains(TOKEN) {
                        format!("{tag} OK\r\n")
                    } else {
                        format!("{tag} NO senha\r\n")
                    }
                } else if cmd.starts_with("SELECT") {
                    format!("* 4 EXISTS\r\n* OK [UIDVALIDITY 9] ok\r\n{tag} OK [READ-WRITE]\r\n")
                } else if cmd == "UID SEARCH ALL" {
                    format!("* SEARCH 1 2\r\n{tag} OK\r\n")
                } else if cmd.starts_with("UID SEARCH UID") {
                    format!("* SEARCH 2 3 4\r\n{tag} OK\r\n")
                } else if let Some(u) = cmd.strip_prefix("UID FETCH ") {
                    let uid: u32 = u.split(' ').next().unwrap().parse().unwrap();
                    let m = &msgs[&uid];
                    format!(
                        "* 1 FETCH (UID {uid} BODY[] {{{}}}\r\n{m})\r\n{tag} OK\r\n",
                        m.len()
                    )
                } else if cmd == "LOGOUT" {
                    w.write_all(format!("* BYE\r\n{tag} OK\r\n").as_bytes())
                        .unwrap();
                    break;
                } else {
                    format!("{tag} BAD\r\n")
                };
                w.write_all(resp.as_bytes()).unwrap();
            }
        }
    });
    end
}

/// SMTP falso que guarda os dados de cada mensagem.
fn smtp_falso() -> (u16, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = l.local_addr().unwrap().port();
    let msgs = Arc::new(Mutex::new(vec![]));
    let m2 = msgs.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(s) = s else { break };
            let mut w = s.try_clone().unwrap();
            let mut r = BufReader::new(s);
            let _ = w.write_all(b"220 teste\r\n");
            let (mut dados, mut em_dados) = (String::new(), false);
            let mut linha = String::new();
            while r.read_line(&mut linha).unwrap_or(0) > 0 {
                if em_dados {
                    if linha == ".\r\n" {
                        em_dados = false;
                        m2.lock().unwrap().push(std::mem::take(&mut dados));
                        let _ = w.write_all(b"250 aceito\r\n");
                    } else {
                        dados.push_str(&linha);
                    }
                } else {
                    let c = linha.to_ascii_uppercase();
                    if c.starts_with("AUTH") {
                        m2.lock().unwrap().push(linha.trim_end().to_string());
                    }
                    let resp: &[u8] = if c.starts_with("EHLO") {
                        b"250-teste\r\n250 AUTH PLAIN LOGIN\r\n"
                    } else if c.starts_with("AUTH") {
                        b"235 ok\r\n"
                    } else if c.starts_with("DATA") {
                        em_dados = true;
                        b"354 manda\r\n"
                    } else if c.starts_with("QUIT") {
                        let _ = w.write_all(b"221 tchau\r\n");
                        break;
                    } else {
                        b"250 ok\r\n"
                    };
                    let _ = w.write_all(resp);
                }
                linha.clear();
            }
        }
    });
    (porta, msgs)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn email_imap_comeca_do_agora_so_aceita_remetente_atestado_e_responde_por_smtp() {
    use phxclaw_agent::canais::email::{Config, Email};
    use phxclaw_agent::email::{SmtpConfig, SmtpSecurity};
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let imap = imap_falso();
    let (porta, enviados) = smtp_falso();
    let e = Arc::new(Email::novo(
        Config {
            endereco: imap,
            usuario: "agente".into(),
            senha: cred(&b, "email", "email-senha", TOKEN),
            smtp_senha: Some(cred(
                &b,
                "email",
                "email-smtp_senha",
                "SENHA-SMTP-DO-BROKER",
            )),
            pasta: "INBOX".into(),
            exigir_dmarc: true,
            tls: None,
        },
        SmtpConfig {
            host: "127.0.0.1".into(),
            port: porta,
            security: SmtpSecurity::Plain,
            username: Some("agente".into()),
            password: None,
            from: "agente@x.org".into(),
            allowed_recipients: vec![],
        },
    ));
    let x = e.clone();
    let l0 = bloq(move || x.receber(None, 0)).await.unwrap();
    assert!(so_mensagens(&l0).is_empty());
    assert_eq!(
        l0[0].cursor.as_deref(),
        Some("9:3"),
        "comeca depois do maior UID"
    );
    let x = e.clone();
    let l1 = bloq(move || x.receber(Some("9:3"), 0)).await.unwrap();
    assert_eq!(
        l1.len(),
        2,
        "o UID 2 que o `3:*` devolve fica de fora: {l1:?}"
    );
    let ms = so_mensagens(&l1);
    assert_eq!(ms.len(), 1, "a forjada (dmarc=fail) nao entra");
    assert_eq!(
        (ms[0].conversa.as_str(), ms[0].texto.as_deref()),
        ("ana@x.org", Some("resuma o dia"))
    );
    assert_eq!(l1[1].cursor.as_deref(), Some("9:5"));
    // UIDVALIDITY mudou (cursor de outra caixa): recomeca do agora.
    let x = e.clone();
    let l2 = bloq(move || x.receber(Some("8:3"), 0)).await.unwrap();
    assert!(so_mensagens(&l2).is_empty());
    let x = e.clone();
    bloq(move || x.enviar("ana@x.org", "dia resumido"))
        .await
        .unwrap();
    let m = enviados.lock().unwrap().clone();
    assert_eq!(m.len(), 2, "{m:?}");
    // A senha do SMTP saiu do broker (a config do canal nao a tem) e chegou no AUTH.
    use base64::Engine;
    let plain = base64::engine::general_purpose::STANDARD.encode("\0agente\0SENHA-SMTP-DO-BROKER");
    assert_eq!(m[0], format!("AUTH PLAIN {plain}"));
    assert!(
        m[1].contains("To: ana@x.org") && m[1].contains("dia resumido"),
        "{}",
        m[1]
    );
    assert!(arquivos_com(&dir, "SENHA-SMTP-DO-BROKER").is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------- a montagem pelo ambiente

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ligar_pelo_ambiente_guarda_o_segredo_no_broker_e_recusa_lista_vazia() {
    use phxclaw_agent::canais::ligar::{CANAIS, ligar};
    let dir = tmp();
    let (base, _) = falso(|_| (200, "{}".into())).await;
    let mut amb: HashMap<String, String> = [
        ("PHXCLAW_MATTERMOST_BASE", base.as_str()),
        ("PHXCLAW_MATTERMOST_TOKEN", TOKEN),
        ("PHXCLAW_MATTERMOST_BOT_ID", "BOT"),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    let e = {
        let a = amb.clone();
        ligar(
            "mattermost",
            &dir,
            &move |k| a.get(k).cloned(),
            registro().0,
        )
        .await
        .err()
        .unwrap()
    };
    assert!(e.contains("PERMITIDOS"), "{e}");
    amb.insert("PHXCLAW_MATTERMOST_PERMITIDOS".into(), "ch1, ch2".into());
    let a = amb.clone();
    let l = ligar(
        "mattermost",
        &dir,
        &move |k| a.get(k).cloned(),
        registro().0,
    )
    .await
    .unwrap();
    assert!(l.canal.permitido("ch2") && !l.canal.permitido("ch3"));
    assert!(l.rotas.is_none());
    // Segunda vez sem o token no ambiente: vale o envelope guardado.
    amb.remove("PHXCLAW_MATTERMOST_TOKEN");
    let a = amb.clone();
    assert!(
        ligar(
            "mattermost",
            &dir,
            &move |k| a.get(k).cloned(),
            registro().0
        )
        .await
        .is_ok()
    );
    assert!(
        arquivos_com(&dir, TOKEN).is_empty(),
        "token em texto puro no disco"
    );
    assert_eq!(CANAIS.len(), 25);
    // Todo canal conhecido monta ou diz o que falta -- nenhum cai em "sem montagem".
    for c in CANAIS {
        if let Err(e) = ligar(c, &dir.join(c), &|_| None, registro().0).await {
            assert!(
                !e.contains("sem montagem") && !e.contains("desconhecido"),
                "{c}: {e}"
            );
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

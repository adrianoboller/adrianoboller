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
                    Some(c("teams", "tk")),
                    c("teams", "ts"),
                    "app".into(),
                    &base,
                    p(),
                    jwt::Jwks::novo(&base).unwrap(),
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
                    Some(c("googlechat", "gk")),
                    gs,
                    "c".into(),
                    &base,
                    p(),
                    googlechat::Verificacao::nova("123".into(), Some(&base)).unwrap(),
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
    use phxclaw_agent::canais::jwt::Jwks;
    use phxclaw_agent::canais::teams::{Teams, mensagem_da_activity};
    let dir = tmp();
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|r| {
        if r.caminho.ends_with("/token") {
            return (200, json!({"access_token": "BF-TOKEN"}).to_string());
        }
        (200, json!({"id": "act9"}).to_string())
    })
    .await;
    let caixa = Caixa::abrir(dir.join("teams.caixa.jsonl")).unwrap();
    let t = Arc::new(
        Teams::novo(
            caixa.clone(),
            None,
            cred(&b, "teams", "teams-app_secret", TOKEN),
            "app-1".into(),
            &base,
            politica_para(&base).unwrap(),
            Jwks::novo(&base).unwrap(),
        )
        .unwrap(),
    );
    // A entrada (JWT) tem prova propria em `teams_confere_o_jwt_rs256_do_bot_framework`;
    // aqui a Activity entra na caixa como o `post` a deixaria, e o que se prova e a saida.
    let act = |conv: &str, url: &str| {
        json!({"type": "message", "id": format!("a-{conv}"), "text": "<at>Agente</at> faca X",
            "from": {"id": "29:u"}, "conversation": {"id": conv}, "serviceUrl": url})
    };
    caixa
        .anexar(vec![
            mensagem_da_activity(&act("19:c1", &format!("{base}/amer/"))).unwrap(),
            mensagem_da_activity(&act("19:mal", "https://evil.example.com/")).unwrap(),
        ])
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

// ---------------------------------------------------------------- RSA e o JWT RS256

/// Um registro dos arquivos de `tests/dados/rsa` (o formato esta no `extrair.py` de la).
struct CasoRsa {
    id: String,
    veredito: String,
    flags: String,
    n: Vec<u8>,
    e: Vec<u8>,
    msg: Vec<u8>,
    sig: Vec<u8>,
}

fn de_hex(s: &str) -> Vec<u8> {
    if s == "-" {
        return vec![];
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn b64url(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

fn vetores_rsa(arquivo: &str) -> Vec<CasoRsa> {
    let caminho = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/dados/rsa")
        .join(arquivo);
    let texto = std::fs::read_to_string(&caminho).unwrap();
    let mut chave = (vec![], vec![]);
    let mut casos = Vec::new();
    for l in texto.lines().filter(|l| !l.starts_with('#')) {
        let c: Vec<&str> = l.split(' ').collect();
        match c[0] {
            "chave" => chave = (de_hex(c[1]), de_hex(c[2])),
            "caso" => casos.push(CasoRsa {
                id: c[1].into(),
                veredito: c[2].into(),
                flags: c[3].into(),
                n: chave.0.clone(),
                e: chave.1.clone(),
                msg: de_hex(c[4]),
                sig: de_hex(c[5]),
            }),
            outro => panic!("{arquivo}: registro desconhecido {outro}"),
        }
    }
    casos
}

/// Roda os casos e devolve (validos aceitos, invalidos recusados, recusas por flag, erros).
/// `aceitavel` do Wycheproof conta como invalido: o EMSA daqui e DER estrito de proposito.
/// Chave com `e = 65537` e 2048+ bits passa pelo caminho do JWKS (`ChavePublica::de_jwk`),
/// que e o que o token usa; as outras, pelo primitivo sem politica.
fn rodar_rsa(casos: &[CasoRsa]) -> (usize, usize, BTreeMap<String, usize>, Vec<String>) {
    use phxclaw_agent::canais::rsa::{ChavePublica, verificar_pkcs1_sha256_sem_politica};
    let (mut aceitos, mut recusados) = (0, 0);
    let mut por_flag = BTreeMap::new();
    let mut erros = Vec::new();
    for c in casos {
        let aceito = match ChavePublica::de_jwk(&b64url(&c.n), &b64url(&c.e)) {
            Ok(k) => k.verificar(&c.msg, &c.sig),
            Err(_) => verificar_pkcs1_sha256_sem_politica(&c.n, &c.e, &c.msg, &c.sig),
        };
        match (c.veredito.as_str(), aceito) {
            ("valido", true) => aceitos += 1,
            ("valido", false) => erros.push(format!("caso {}: valido recusado", c.id)),
            (_, false) => {
                recusados += 1;
                for f in c.flags.split(',') {
                    *por_flag.entry(f.to_string()).or_insert(0) += 1;
                }
            }
            (v, true) => erros.push(format!("caso {} ({v}, {}): aceito", c.id, c.flags)),
        }
    }
    (aceitos, recusados, por_flag, erros)
}

/// Wycheproof `rsa_signature_2048_sha256_test.json` inteiro: 9 validos aceitos e os 250
/// outros (249 invalidos + 1 «acceptable» de BER) recusados. Com a comparacao do bloco
/// trocada por «acha o hash no fim», os casos de padding passam a ser aceitos e isto
/// reprova (RED medido, ver o comentario do `rsa.rs`).
#[test]
fn rsa_pkcs1_sha256_contra_o_wycheproof() {
    let casos = vetores_rsa("wycheproof_rsa_2048_sha256.txt");
    assert_eq!(casos.len(), 259);
    let (aceitos, recusados, por_flag, erros) = rodar_rsa(&casos);
    println!(
        "wycheproof: {aceitos} validos aceitos, {recusados} invalidos recusados; {por_flag:?}"
    );
    assert!(erros.is_empty(), "{} erros: {erros:?}", erros.len());
    assert_eq!((aceitos, recusados), (9, 250));
}

/// NIST CAVP SigVer15 (186-3), so SHA-256: 3 P e 15 F por modulo (1024, 2048, 3072). Os
/// expoentes do NIST sao aleatorios, entao estes passam pelo primitivo sem politica.
#[test]
fn rsa_pkcs1_sha256_contra_o_nist_sigver15() {
    let casos = vetores_rsa("nist_sigver15_sha256.txt");
    assert_eq!(casos.len(), 54);
    let (aceitos, recusados, por_mod, erros) = rodar_rsa(&casos);
    println!("nist: {aceitos} validos aceitos, {recusados} invalidos recusados; {por_mod:?}");
    assert!(erros.is_empty(), "{} erros: {erros:?}", erros.len());
    assert_eq!((aceitos, recusados), (9, 45));
}

#[test]
fn chave_do_jwks_so_entra_com_2048_a_4096_bits_e_expoente_65537() {
    use phxclaw_agent::canais::rsa::ChavePublica;
    let wy = vetores_rsa("wycheproof_rsa_2048_sha256.txt");
    let nist = vetores_rsa("nist_sigver15_sha256.txt");
    let aqab = [1u8, 0, 1];
    // A chave de 2048 do Wycheproof entra com 65537 e nao entra com o e = 3 dela mesma.
    let k = ChavePublica::nova(&wy[0].n, &aqab).unwrap();
    assert_eq!(k.bits(), 2048);
    let e3 = wy.iter().find(|c| c.e == [3]).unwrap();
    assert!(ChavePublica::nova(&e3.n, &e3.e).is_err(), "e = 3");
    assert!(
        ChavePublica::nova(&wy[0].n, &[1, 0, 0, 1]).is_err(),
        "e = 2^24+1"
    );
    // 1024 (NIST) fica fora; 3072 (NIST) entra.
    let n1024 = &nist.iter().find(|c| c.flags == "mod1024").unwrap().n;
    assert!(ChavePublica::nova(n1024, &aqab).is_err(), "1024 bits");
    let n3072 = &nist.iter().find(|c| c.flags == "mod3072").unwrap().n;
    assert_eq!(ChavePublica::nova(n3072, &aqab).unwrap().bits(), 3072);
    // 4096 entra, 4097 nao; modulo par nao e RSA.
    let mut n4096 = vec![0u8; 512];
    n4096[0] = 0x80;
    n4096[511] = 1;
    assert_eq!(ChavePublica::nova(&n4096, &aqab).unwrap().bits(), 4096);
    let mut n4097 = vec![1u8];
    n4097.extend(vec![0u8; 512]);
    n4097[512] = 1;
    assert!(ChavePublica::nova(&n4097, &aqab).is_err(), "4097 bits");
    let mut par = wy[0].n.clone();
    *par.last_mut().unwrap() &= 0xfe;
    assert!(ChavePublica::nova(&par, &aqab).is_err(), "modulo par");
    // JWK com base64 comum (`+`/`/`) ou com preenchimento nao e base64url.
    assert!(ChavePublica::de_jwk("ab+/", "AQAB").is_err());
    assert!(ChavePublica::de_jwk(&b64url(&wy[0].n), "AQAB==").is_err());
}

/// Par RSA-2048 de TESTE, gerado agora pelo `openssl` do sistema e apagado com a pasta:
/// nenhuma chave privada mora no repositorio. `None` sem `openssl`.
struct ParRsa {
    pem: PathBuf,
    n: String,
}

fn par_rsa(dir: &Path, nome: &str) -> Option<ParRsa> {
    use std::process::Command;
    std::fs::create_dir_all(dir).ok()?;
    let pem = dir.join(format!("{nome}.pem"));
    let g = Command::new("openssl")
        .args([
            "genpkey",
            "-algorithm",
            "RSA",
            "-pkeyopt",
            "rsa_keygen_bits:2048",
            "-out",
        ])
        .arg(&pem)
        .output()
        .ok()?;
    if !g.status.success() {
        return None;
    }
    let m = Command::new("openssl")
        .args(["rsa", "-noout", "-modulus", "-in"])
        .arg(&pem)
        .output()
        .ok()?;
    let texto = String::from_utf8(m.stdout).ok()?;
    let hexa = texto.trim().strip_prefix("Modulus=")?.to_ascii_lowercase();
    Some(ParRsa {
        pem,
        n: b64url(&de_hex(&hexa)),
    })
}

/// JWT assinado com RS256 pelo `openssl dgst -sha256 -sign` (PKCS#1 v1.5).
fn jwt_assinado(par: &ParRsa, cab: &Value, claims: &Value) -> String {
    use std::process::{Command, Stdio};
    let entrada = format!(
        "{}.{}",
        b64url(cab.to_string().as_bytes()),
        b64url(claims.to_string().as_bytes())
    );
    let mut f = Command::new("openssl")
        .args(["dgst", "-sha256", "-sign"])
        .arg(&par.pem)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    f.stdin
        .take()
        .unwrap()
        .write_all(entrada.as_bytes())
        .unwrap();
    let out = f.wait_with_output().unwrap();
    assert!(out.status.success());
    format!("{entrada}.{}", b64url(&out.stdout))
}

fn com_bearer(token: &str, corpo: &str, consulta: &str) -> PedidoWebhook {
    pedido(
        &[("Authorization", &format!("Bearer {token}"))],
        consulta,
        corpo,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn teams_confere_o_jwt_rs256_do_bot_framework() {
    use phxclaw_agent::canais::jwt::{self, Exigido, Jwks, Motivo};
    use phxclaw_agent::canais::teams::{EMISSOR, Teams, conferir_activity};
    use phxclaw_test_support::pulado;
    let dir = tmp();
    let (Some(bf), Some(intruso)) = (par_rsa(&dir, "bf"), par_rsa(&dir, "intruso")) else {
        pulado::pular(
            "openssl",
            "sem openssl nao ha como assinar o token de teste",
        );
        return;
    };
    let jwks = json!({"keys": [
        {"kty": "RSA", "use": "sig", "kid": "k1", "n": bf.n, "e": "AQAB",
         "endorsements": ["msteams"]},
        {"kty": "RSA", "use": "sig", "kid": "k-sem-endosso", "n": bf.n, "e": "AQAB"},
        // Fora da politica (e = 3): pulada na leitura, entao o kid fica desconhecido.
        {"kty": "RSA", "use": "sig", "kid": "k-e3", "n": bf.n, "e": "Aw"},
    ]});
    let (jwks_url, jwks_log) = falso(move |_| (200, jwks.to_string())).await;
    let b = broker_em(&dir).unwrap();
    let (base, _) = falso(|_| (200, "{}".into())).await;
    let t = Arc::new(
        Teams::novo(
            Caixa::abrir(dir.join("teams.caixa.jsonl")).unwrap(),
            Some(cred(&b, "teams", "teams-chave_url", "CHAVE-URL")),
            cred(&b, "teams", "teams-app_secret", TOKEN),
            "app-1".into(),
            &base,
            politica_para(&base).unwrap(),
            Jwks::novo(&format!("{jwks_url}/keys")).unwrap(),
        )
        .unwrap(),
    );
    let sem_app = Teams::novo(
        Caixa::abrir(dir.join("teams-sem-app.caixa.jsonl")).unwrap(),
        None,
        cred(&b, "teams", "teams-app_secret", TOKEN),
        "  ".into(),
        &base,
        politica_para(&base).unwrap(),
        Jwks::novo(&format!("{jwks_url}/keys")).unwrap(),
    );
    assert_eq!(
        sem_app.err().as_deref(),
        Some("App ID vazio: sem ele nao ha audiencia para conferir o token")
    );
    let agora = jwt::agora();
    let servico = format!("{base}/amer/");
    let claims = json!({"iss": EMISSOR, "aud": "app-1", "exp": agora + 600,
        "nbf": agora - 10, "serviceUrl": servico});
    let act = |canal: &str, url: &str| {
        json!({"type": "message", "channelId": canal, "id": "a1", "text": "faca X",
            "from": {"id": "29:u"}, "conversation": {"id": "19:c1"}, "serviceUrl": url})
        .to_string()
    };
    let rs256 = |kid: &str| json!({"alg": "RS256", "typ": "JWT", "kid": kid});
    let com = |muda: &dyn Fn(&mut Value)| {
        let mut c = claims.clone();
        muda(&mut c);
        c
    };
    let tk = |par: &ParRsa, kid: &str, c: &Value| jwt_assinado(par, &rs256(kid), c);
    let bom = tk(&bf, "k1", &claims);

    // Cada caso: o token, o corpo, o status esperado e -- quando a recusa e do token -- o
    // motivo, conferido direto no `jwt::conferir` para provar que recusou PELO motivo certo.
    let mut tokens_ruins: Vec<(&str, String, Motivo)> = vec![
        (
            "emissor",
            tk(
                &bf,
                "k1",
                &com(&|c| c["iss"] = json!("https://outro.example.com")),
            ),
            Motivo::Emissor,
        ),
        (
            "audiencia",
            tk(&bf, "k1", &com(&|c| c["aud"] = json!("app-2"))),
            Motivo::Audiencia,
        ),
        (
            "vencido ha 400 s",
            tk(&bf, "k1", &com(&|c| c["exp"] = json!(agora - 400))),
            Motivo::Vencido,
        ),
        (
            "nbf daqui a 400 s",
            tk(&bf, "k1", &com(&|c| c["nbf"] = json!(agora + 400))),
            Motivo::AindaNaoVale,
        ),
        (
            "sem exp",
            tk(
                &bf,
                "k1",
                &com(&|c| {
                    c.as_object_mut().unwrap().remove("exp");
                }),
            ),
            Motivo::SemValidade,
        ),
        (
            "assinado por outra chave com o mesmo kid",
            tk(&intruso, "k1", &claims),
            Motivo::Assinatura,
        ),
        (
            "kid de chave fora da politica",
            tk(&bf, "k-e3", &claims),
            Motivo::KidDesconhecido,
        ),
        (
            "alg HS256",
            jwt_assinado(&bf, &json!({"alg": "HS256", "kid": "k1"}), &claims),
            Motivo::Algoritmo,
        ),
        (
            "alg none",
            format!(
                "{}.{}.",
                b64url(json!({"alg": "none", "kid": "k1"}).to_string().as_bytes()),
                b64url(claims.to_string().as_bytes())
            ),
            Motivo::Algoritmo,
        ),
        (
            "sem kid",
            jwt_assinado(&bf, &json!({"alg": "RS256"}), &claims),
            Motivo::SemKid,
        ),
        ("duas partes", "a.b".into(), Motivo::Formato),
        (
            // `crit` pede uma extensao que nao entendemos: recusa, ainda que assinado.
            "crit no cabecalho",
            jwt_assinado(
                &bf,
                &json!({"alg": "RS256", "kid": "k1", "crit": ["exp"]}),
                &claims,
            ),
            Motivo::Formato,
        ),
        (
            // Bem assinado e valido em tudo, mas acima do teto de 16 KiB.
            "token acima de 16 KiB",
            tk(
                &bf,
                "k1",
                &com(&|c| c["enchimento"] = json!("a".repeat(17_000))),
            ),
            Motivo::Formato,
        ),
    ];
    // Corpo trocado sob a assinatura de um token bom: o `aud` outro, a assinatura a mesma.
    let mut partes: Vec<String> = bom.split('.').map(str::to_string).collect();
    partes[1] = b64url(com(&|c| c["aud"] = json!("app-2")).to_string().as_bytes());
    tokens_ruins.push(("corpo trocado", partes.join("."), Motivo::Assinatura));

    let exigido = Exigido {
        emissores: &[EMISSOR],
        audiencia: "app-1",
    };
    let jw = Jwks::novo(&format!("{jwks_url}/keys")).unwrap();
    // As falhas se juntam e o teste reprova no fim, com todas: a prova real repoe varias
    // conferencias de uma vez e cada uma tem de aparecer pelo nome.
    let mut falhas: Vec<String> = Vec::new();
    let conferir_fora = |token: String, ex: &Exigido| {
        let jw2 = &jw;
        std::thread::scope(|s| {
            s.spawn(move || jwt::conferir(&token, jw2, ex, agora))
                .join()
                .unwrap()
        })
    };
    for (nome, token, motivo) in tokens_ruins.clone() {
        let r = conferir_fora(token, &exigido);
        if r.as_ref().err() != Some(&motivo) {
            falhas.push(format!("{nome}: {:?} em vez de {motivo:?}", r.map(|_| ())));
        }
    }
    // Audiencia configurada vazia nao casa com `aud` vazio: sem audiencia nao ha o que
    // conferir, e aceitar seria aceitar token de qualquer app.
    let aud_vazia = tk(&bf, "k1", &com(&|c| c["aud"] = json!("")));
    let r = conferir_fora(
        aud_vazia,
        &Exigido {
            emissores: &[EMISSOR],
            audiencia: "",
        },
    );
    if r.as_ref().err() != Some(&Motivo::Audiencia) {
        falhas.push(format!("audiencia vazia: {:?}", r.map(|_| ())));
    }
    // A folga de 5 min vale nos dois lados.
    for c in [
        com(&|c| c["exp"] = json!(agora - 200)),
        com(&|c| c["nbf"] = json!(agora + 200)),
    ] {
        let token = tk(&bf, "k1", &c);
        let jw2 = &jw;
        let ex = &exigido;
        std::thread::scope(|s| {
            s.spawn(move || jwt::conferir(&token, jw2, ex, agora))
                .join()
        })
        .unwrap()
        .expect("dentro da folga de 5 min");
    }

    let post = |p: PedidoWebhook| {
        let x = t.clone();
        bloq(move || x.post(&p))
    };
    let corpo_bom = act("msteams", &servico);
    // O token bom passa e a mensagem vai para a caixa.
    post(com_bearer(&bom, &corpo_bom, "chave=CHAVE-URL"))
        .await
        .unwrap();
    let x = t.clone();
    let l = bloq(move || x.receber(None, 0)).await.unwrap();
    assert_eq!(so_mensagens(&l)[0].texto.as_deref(), Some("faca X"));
    // Sem token, com outro esquema, ou com a chave da URL errada: 401.
    for (nome, p) in [
        ("sem token", pedido(&[], "chave=CHAVE-URL", &corpo_bom)),
        (
            "esquema Basic",
            pedido(
                &[("Authorization", &format!("Basic {bom}"))],
                "chave=CHAVE-URL",
                &corpo_bom,
            ),
        ),
        (
            "chave da URL errada",
            com_bearer(&bom, &corpo_bom, "chave=ERRADA"),
        ),
    ] {
        let r = post(p).await;
        if r.as_ref().err().map(|e| e.0) != Some(401) {
            falhas.push(format!("post {nome}: {r:?}"));
        }
    }
    // Todo token ruim e 401, e a resposta nao diz o que errou.
    for (nome, token, _) in tokens_ruins {
        let r = post(com_bearer(&token, &corpo_bom, "chave=CHAVE-URL")).await;
        if r.as_ref().err() != Some(&(401, "token nao confere".to_string())) {
            falhas.push(format!("post {nome}: {:?}", r.map(|_| ())));
        }
    }
    assert!(falhas.is_empty(), "{} falhas: {falhas:#?}", falhas.len());
    // Token bom que nao vale para ESTA Activity: 403.
    let outro_servico = act("msteams", &format!("{base}/emea/"));
    let canal_sem_endosso = act("skype", &servico);
    let sem_endosso = tk(&bf, "k-sem-endosso", &claims);
    for (token, corpo) in [
        (&bom, &outro_servico),
        (&bom, &canal_sem_endosso),
        (&sem_endosso, &corpo_bom),
    ] {
        let e = post(com_bearer(token, corpo, "chave=CHAVE-URL"))
            .await
            .unwrap_err();
        assert_eq!(e.0, 403, "{corpo}");
    }
    let x = t.clone();
    assert_eq!(
        so_mensagens(&bloq(move || x.receber(None, 0)).await.unwrap()).len(),
        1,
        "so o pedido bom entrou na caixa"
    );
    // O `conferir_activity` sozinho: claim sem serviceUrl tambem e 403.
    let sem_claim = jwt::Conferido {
        claims: json!({}),
        endossos: vec!["msteams".into()],
    };
    assert_eq!(
        conferir_activity(&sem_claim, &serde_json::from_str(&corpo_bom).unwrap()),
        Err(Motivo::NaoVaiAqui)
    );
    // O canal baixou o JWKS UMA vez: o `kid` fora da politica nao forcou recarga dentro do
    // intervalo minimo (sem o teto, cada kid inventado seria um pedido nosso a Microsoft).
    assert_eq!(
        jwks_log.lock().unwrap().len(),
        2,
        "1 do canal + 1 do `jw` do teste"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// O cache do JWKS: 24 h viram 300 ms aqui. Le uma vez, reusa, recarrega no `kid` novo e no
/// vencimento, e com o servico fora depois de vencer RECUSA em vez de usar a chave velha.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn jwks_recarrega_no_kid_novo_e_no_vencimento_e_falha_fechado() {
    use phxclaw_agent::canais::jwt::{self, Exigido, Jwks, Motivo};
    use phxclaw_test_support::pulado;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let dir = tmp();
    let Some(par) = par_rsa(&dir, "rot") else {
        pulado::pular(
            "openssl",
            "sem openssl nao ha como assinar o token de teste",
        );
        return;
    };
    // fase 0: so k1; fase 1: k1 e k2; fase 2: servico fora (500).
    let fase = Arc::new(AtomicUsize::new(0));
    let (f2, n) = (fase.clone(), par.n.clone());
    let (url, log) = falso(move |_| {
        let chave = |kid: &str| json!({"kty": "RSA", "kid": kid, "n": n, "e": "AQAB"});
        match f2.load(Ordering::SeqCst) {
            0 => (200, json!({"keys": [chave("k1")]}).to_string()),
            1 => (200, json!({"keys": [chave("k1"), chave("k2")]}).to_string()),
            _ => (500, "{}".into()),
        }
    })
    .await;
    let jw = Arc::new(
        Jwks::com_prazos(&url, Duration::from_millis(300), Duration::from_millis(0)).unwrap(),
    );
    let agora = jwt::agora();
    let claims = json!({"iss": "emissor", "aud": "aud", "exp": agora + 600});
    let tk = |kid: &str| jwt_assinado(&par, &json!({"alg": "RS256", "kid": kid}), &claims);
    let conferir = |token: String| {
        let j = jw.clone();
        bloq(move || {
            let ex = Exigido {
                emissores: &["emissor"],
                audiencia: "aud",
            };
            jwt::conferir(&token, &j, &ex, agora).map(|_| ())
        })
    };
    let pedidos = || log.lock().unwrap().len();
    conferir(tk("k1")).await.unwrap();
    conferir(tk("k1")).await.unwrap();
    assert_eq!(pedidos(), 1, "o segundo usa o cache");
    assert_eq!(conferir(tk("k2")).await, Err(Motivo::KidDesconhecido));
    assert_eq!(pedidos(), 2, "kid novo forca uma recarga");
    fase.store(1, Ordering::SeqCst);
    conferir(tk("k2")).await.unwrap();
    assert_eq!(pedidos(), 3, "a chave nova chegou na recarga");
    tokio::time::sleep(Duration::from_millis(350)).await;
    conferir(tk("k1")).await.unwrap();
    assert_eq!(pedidos(), 4, "o vencimento recarrega");
    fase.store(2, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(350)).await;
    assert_eq!(
        conferir(tk("k1")).await,
        Err(Motivo::ChavesIndisponiveis),
        "vencido e sem servico: recusa, nao usa a chave velha"
    );
    assert_eq!(Motivo::ChavesIndisponiveis.recusa().0, 503);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn googlechat_confere_o_jwt_nos_dois_modos_e_sai_pelo_webhook_do_espaco() {
    use phxclaw_agent::canais::googlechat::{
        CONTA_DO_CHAT, GoogleChat, JWKS_OIDC, JWKS_PROJETO, Verificacao,
    };
    use phxclaw_agent::canais::jwt::{self, Jwks};
    use phxclaw_test_support::pulado;
    let dir = tmp();
    let Some(par) = par_rsa(&dir, "gc") else {
        pulado::pular(
            "openssl",
            "sem openssl nao ha como assinar o token de teste",
        );
        return;
    };
    let jwks = json!({"keys": [{"kty": "RSA", "use": "sig", "alg": "RS256", "kid": "g1",
        "n": par.n, "e": "AQAB"}]});
    let (jwks_url, _) = falso(move |_| (200, jwks.to_string())).await;
    let b = broker_em(&dir).unwrap();
    let (base, log) = falso(|_| (200, json!({"name": "spaces/S/messages/9"}).to_string())).await;
    let saida = format!("{base}/v1/spaces/S/messages?key=K&token={TOKEN}");
    // O modo sai da audiencia, e o JWKS padrao do modo junto.
    let padrao = |aud: &str| {
        let mut visto = String::new();
        let _ = Verificacao::com_jwks(aud.into(), |p| {
            visto = p.to_string();
            Err("so olhando".into())
        });
        visto
    };
    assert_eq!(padrao("123456789012"), JWKS_PROJETO);
    assert_eq!(
        padrao("https://agente.exemplo.com/canais/googlechat/webhook"),
        JWKS_OIDC
    );
    assert!(
        Verificacao::nova("  ".into(), None).is_err(),
        "audiencia vazia"
    );
    let monta = |aud: &str, chave: bool| {
        let u = jwks_url.clone();
        Arc::new(
            GoogleChat::novo(
                Caixa::abrir(dir.join(format!("gc-{}.caixa.jsonl", chave as u8))).unwrap(),
                chave.then(|| cred(&b, "googlechat", "googlechat-chave_url", "CHAVE-URL")),
                cred(&b, "googlechat", "googlechat-saida_webhook", &saida),
                "spaces/S".into(),
                &base,
                politica_para(&base).unwrap(),
                Verificacao::com_jwks(aud.into(), move |_| Jwks::novo(&u)).unwrap(),
            )
            .unwrap(),
        )
    };
    let ev = json!({"type": "MESSAGE", "message": {"name": "spaces/S/messages/1",
        "text": "@App oi", "argumentText": " oi ", "sender": {"name": "users/1"},
        "space": {"name": "spaces/S"}}})
    .to_string();
    let agora = jwt::agora();
    let tk = |c: Value| jwt_assinado(&par, &json!({"alg": "RS256", "kid": "g1"}), &c);
    let post = |g: Arc<GoogleChat>, p: PedidoWebhook| bloq(move || g.post(&p).map(|_| ()));

    // Modo numero do projeto (e a chave da URL configurada, que soma).
    let g = monta("123456789012", true);
    let projeto = json!({"iss": CONTA_DO_CHAT, "aud": "123456789012", "exp": agora + 600});
    post(
        g.clone(),
        com_bearer(&tk(projeto.clone()), &ev, "chave=CHAVE-URL"),
    )
    .await
    .unwrap();
    let mut ruins = vec![
        ("sem token", pedido(&[], "chave=CHAVE-URL", &ev)),
        (
            "chave da URL errada",
            com_bearer(&tk(projeto.clone()), &ev, "chave=ERRADA"),
        ),
    ];
    for (nome, c) in [
        (
            "outra audiencia",
            json!({"iss": CONTA_DO_CHAT, "aud": "999", "exp": agora + 600}),
        ),
        (
            "emissor do outro modo",
            json!({"iss": "https://accounts.google.com", "aud": "123456789012", "exp": agora + 600}),
        ),
        (
            "vencido",
            json!({"iss": CONTA_DO_CHAT, "aud": "123456789012", "exp": agora - 400}),
        ),
    ] {
        ruins.push((nome, com_bearer(&tk(c), &ev, "chave=CHAVE-URL")));
    }
    // As falhas se juntam e reprovam no fim, todas pelo nome (ver o teste do Teams).
    let mut falhas: Vec<String> = Vec::new();
    for (nome, p) in ruins {
        let r = post(g.clone(), p).await;
        if r.as_ref().err().map(|e| e.0) != Some(401) {
            falhas.push(format!("{nome}: {r:?}"));
        }
    }
    let x = g.clone();
    let l = bloq(move || x.receber(None, 0)).await.unwrap();
    if so_mensagens(&l).len() != 1 {
        falhas.push(format!(
            "entraram {} na caixa, e so o bom",
            so_mensagens(&l).len()
        ));
    }
    assert_eq!(so_mensagens(&l)[0].texto.as_deref(), Some("oi"));

    // Modo URL do endpoint (ID token): o `email` assinado tem de ser o do Chat, verificado.
    let url_ep = "https://agente.exemplo.com/canais/googlechat/webhook";
    let g2 = monta(url_ep, false);
    let oidc = |email: &str, verificado: bool| {
        json!({"iss": "https://accounts.google.com", "aud": url_ep, "exp": agora + 600,
            "email": email, "email_verified": verificado})
    };
    post(
        g2.clone(),
        com_bearer(&tk(oidc(CONTA_DO_CHAT, true)), &ev, ""),
    )
    .await
    .unwrap();
    let mut sem_verificado = oidc(CONTA_DO_CHAT, true);
    sem_verificado
        .as_object_mut()
        .unwrap()
        .remove("email_verified");
    for (nome, c) in [
        ("outro email", oidc("alguem@gmail.com", true)),
        ("email nao verificado", oidc(CONTA_DO_CHAT, false)),
        // Ausente nao e verdadeiro: so `true` assinado conta.
        ("sem email_verified", sem_verificado),
        (
            // Com o `email` certo e verificado: so o emissor pode recusar este.
            "emissor do modo projeto",
            json!({"iss": CONTA_DO_CHAT, "aud": url_ep, "exp": agora + 600,
                "email": CONTA_DO_CHAT, "email_verified": true}),
        ),
    ] {
        let r = post(g2.clone(), com_bearer(&tk(c), &ev, "")).await;
        if r.as_ref().err() != Some(&(401, "token nao confere".to_string())) {
            falhas.push(format!("{nome}: {r:?}"));
        }
    }
    assert!(falhas.is_empty(), "{} falhas: {falhas:#?}", falhas.len());

    // A saida continua pelo webhook do espaco, e so dele.
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

/// A montagem pelo ambiente: o Teams sobe sem a chave da URL (o JWT e a guarda), e o Google
/// Chat NAO sobe sem audiencia -- dizendo qual variavel falta, em vez de subir sem conferir.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn teams_e_googlechat_montam_pelo_ambiente_com_o_jwt() {
    use phxclaw_agent::canais::ligar::ligar;
    let dir = tmp();
    let amb: HashMap<String, String> = [
        ("PHXCLAW_TEAMS_APP_ID", "app-1"),
        ("PHXCLAW_TEAMS_APP_SECRET", TOKEN),
        ("PHXCLAW_TEAMS_PERMITIDOS", "19:c1"),
        (
            "PHXCLAW_GOOGLECHAT_SAIDA_WEBHOOK",
            "https://chat.googleapis.com/v1/x?key=K",
        ),
        ("PHXCLAW_GOOGLECHAT_ESPACO", "spaces/S"),
        ("PHXCLAW_GOOGLECHAT_PERMITIDOS", "spaces/S"),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    let a = amb.clone();
    let l = ligar(
        "teams",
        &dir.join("t"),
        &move |k| a.get(k).cloned(),
        registro().0,
    )
    .await
    .unwrap();
    assert!(l.rotas.is_some());
    let a = amb.clone();
    let e = ligar(
        "googlechat",
        &dir.join("g"),
        &move |k| a.get(k).cloned(),
        registro().0,
    )
    .await
    .err()
    .unwrap();
    assert!(e.contains("PHXCLAW_GOOGLECHAT_AUDIENCIA"), "{e}");
    let mut a = amb.clone();
    a.insert("PHXCLAW_GOOGLECHAT_AUDIENCIA".into(), "123456789012".into());
    assert!(
        ligar(
            "googlechat",
            &dir.join("g"),
            &move |k| a.get(k).cloned(),
            registro().0
        )
        .await
        .is_ok()
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// O servidor de chaves lento (2 s por download) nao segura o token de `kid` conhecido:
/// o download corre fora da trava, um por vez, e quem chega durante ele usa o conjunto
/// atual. Com a trava presa durante o download, o token bom esperava os 2 s (RED medido).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn jwks_lento_nao_segura_token_de_kid_conhecido() {
    use phxclaw_agent::canais::jwt::{self, Exigido, Jwks, Motivo};
    use phxclaw_test_support::pulado;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Instant;
    let dir = tmp();
    let Some(par) = par_rsa(&dir, "lento") else {
        pulado::pular(
            "openssl",
            "sem openssl nao ha como assinar o token de teste",
        );
        return;
    };
    let lento = Arc::new(AtomicBool::new(false));
    let pedidos = Arc::new(AtomicUsize::new(0));
    let (l2, p2, n) = (lento.clone(), pedidos.clone(), par.n.clone());
    let app = Router::new().route(
        "/keys",
        axum::routing::get(move || {
            let (l, p, n) = (l2.clone(), p2.clone(), n.clone());
            async move {
                p.fetch_add(1, Ordering::SeqCst);
                let chave = |kid: &str| json!({"kty": "RSA", "kid": kid, "n": n, "e": "AQAB"});
                if l.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    json!({"keys": [chave("k1"), chave("k2")]}).to_string()
                } else {
                    json!({"keys": [chave("k1")]}).to_string()
                }
            }
        }),
    );
    let url = format!("{}/keys", servir(app).await);
    let agora = jwt::agora();
    let claims = json!({"iss": "emissor", "aud": "aud", "exp": agora + 600});
    let tk = |kid: &str| jwt_assinado(&par, &json!({"alg": "RS256", "kid": kid}), &claims);
    let conferir = |j: Arc<Jwks>, token: String| {
        bloq(move || {
            let ex = Exigido {
                emissores: &["emissor"],
                audiencia: "aud",
            };
            let t = Instant::now();
            (
                jwt::conferir(&token, &j, &ex, agora).map(|_| ()),
                t.elapsed(),
            )
        })
    };
    // Espera o download lento comecar (o servidor contou o pedido).
    let ate_pedir = |alvo: usize| {
        let p = pedidos.clone();
        async move {
            for _ in 0..200 {
                if p.load(Ordering::SeqCst) >= alvo {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("o download lento nao comecou");
        }
    };
    let mut falhas: Vec<String> = Vec::new();

    // 1) `kid` novo dispara o download lento; o `kid` conhecido nao espera por ele.
    let jw = Arc::new(Jwks::com_prazos(&url, Duration::from_secs(3600), Duration::ZERO).unwrap());
    assert_eq!(conferir(jw.clone(), tk("k1")).await.0, Ok(()));
    lento.store(true, Ordering::SeqCst);
    let novo = tokio::spawn(conferir(jw.clone(), tk("k2")));
    ate_pedir(2).await;
    let (r, t) = conferir(jw.clone(), tk("k1")).await;
    if r.is_err() || t > Duration::from_millis(1000) {
        falhas.push(format!("kid conhecido durante o download: {r:?} em {t:?}"));
    }
    // Outro `kid` desconhecido durante o download: 401 na hora, sem segundo download.
    let (r, t) = conferir(jw.clone(), tk("k3")).await;
    if r != Err(Motivo::KidDesconhecido) || t > Duration::from_millis(1000) {
        falhas.push(format!(
            "kid desconhecido durante o download: {r:?} em {t:?}"
        ));
    }
    let (r, _) = novo.await.unwrap();
    assert_eq!(r, Ok(()), "o kid novo chegou no download");
    let baixados = pedidos.load(Ordering::SeqCst);
    if baixados != 2 {
        falhas.push(format!("{baixados} downloads, e era um so"));
    }

    // 2) Cache vencido recarregando devagar: a chave do conjunto atual vale enquanto isso.
    lento.store(false, Ordering::SeqCst);
    let jw = Arc::new(Jwks::com_prazos(&url, Duration::from_millis(200), Duration::ZERO).unwrap());
    assert_eq!(conferir(jw.clone(), tk("k1")).await.0, Ok(()));
    tokio::time::sleep(Duration::from_millis(250)).await;
    lento.store(true, Ordering::SeqCst);
    let recarga = tokio::spawn(conferir(jw.clone(), tk("k1")));
    ate_pedir(4).await;
    let (r, t) = conferir(jw.clone(), tk("k1")).await;
    if r.is_err() || t > Duration::from_millis(1000) {
        falhas.push(format!("cache vencido durante a recarga: {r:?} em {t:?}"));
    }
    assert_eq!(recarga.await.unwrap().0, Ok(()));

    // 3) Sem conjunto nenhum ainda: quem chega durante o primeiro download leva 503.
    let jw = Arc::new(Jwks::com_prazos(&url, Duration::from_secs(3600), Duration::ZERO).unwrap());
    let primeiro = tokio::spawn(conferir(jw.clone(), tk("k1")));
    ate_pedir(5).await;
    let (r, t) = conferir(jw.clone(), tk("k1")).await;
    if r != Err(Motivo::ChavesIndisponiveis) || t > Duration::from_millis(1000) {
        falhas.push(format!("primeiro download em curso: {r:?} em {t:?}"));
    }
    assert_eq!(primeiro.await.unwrap().0, Ok(()));
    assert!(falhas.is_empty(), "{} falhas: {falhas:#?}", falhas.len());
    let _ = std::fs::remove_dir_all(dir);
}

/// O custo de UMA verificacao RSA, que qualquer um dispara com um `kid` publico e lixo do
/// tamanho da chave. Imprime a mediana; o numero que decide o teto em voo e o do binario
/// otimizado (o deste perfil vai junto, dito qual e).
#[test]
fn rsa_custo_de_uma_verificacao_2048_e_4096() {
    use phxclaw_agent::canais::rsa::ChavePublica;
    use std::time::Instant;
    let aqab = [1u8, 0, 1];
    let wy = vetores_rsa("wycheproof_rsa_2048_sha256.txt");
    let mut n4096 = vec![0x9bu8; 512];
    n4096[0] = 0xc5;
    n4096[511] |= 1;
    let perfil = if cfg!(debug_assertions) {
        "sem otimizacao"
    } else {
        "otimizado"
    };
    for n in [wy[0].n.clone(), n4096] {
        let k = ChavePublica::nova(&n, &aqab).unwrap();
        // Lixo abaixo do modulo (o primeiro byte 01 garante) e do tamanho certo: passa os
        // dois cortes baratos do passo 1 e 2 e paga a conta inteira.
        // (o `n` do Wycheproof traz o zero a esquerda do DER: o tamanho sai dos bits).
        let lixo = vec![0x01u8; k.bits().div_ceil(8)];
        let mut us: Vec<u128> = (0..9)
            .map(|_| {
                let t = Instant::now();
                assert!(!k.verificar(b"corpo", std::hint::black_box(&lixo)));
                t.elapsed().as_micros()
            })
            .collect();
        us.sort();
        println!(
            "rsa verificar {} bits: mediana {} us, min {} us ({perfil})",
            k.bits(),
            us[4],
            us[0]
        );
    }
}

/// A conferencia de assinatura tem teto de vagas em voo: acima dele, 429 na hora, ANTES
/// da conta. Sem o portao, dezesseis conferencias de lixo correm juntas (RED medido). E a
/// vaga volta: depois da rajada, o token bom passa.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn conferencia_de_assinatura_tem_teto_em_voo_e_429_acima_dele() {
    use phxclaw_agent::canais::jwt::{self, Exigido, Jwks, Motivo};
    use phxclaw_test_support::pulado;
    let dir = tmp();
    let Some(par) = par_rsa(&dir, "voo") else {
        pulado::pular(
            "openssl",
            "sem openssl nao ha como assinar o token de teste",
        );
        return;
    };
    let n = par.n.clone();
    let (url, _) = falso(move |_| {
        (
            200,
            json!({"keys": [{"kty": "RSA", "kid": "k1", "n": n, "e": "AQAB"}]}).to_string(),
        )
    })
    .await;
    let jw = Arc::new(Jwks::novo(&url).unwrap().com_teto_em_voo(2));
    let agora = jwt::agora();
    let bom = jwt_assinado(
        &par,
        &json!({"alg": "RS256", "kid": "k1"}),
        &json!({"iss": "emissor", "aud": "aud", "exp": agora + 600}),
    );
    // A assinatura trocada por lixo do tamanho da chave (e abaixo do modulo).
    let (corpo, _) = bom.rsplit_once('.').unwrap();
    let lixo = format!("{corpo}.{}", b64url(&[0x01u8; 256]));
    let j = jw.clone();
    let rodada = bloq(move || {
        let ex = Exigido {
            emissores: &["emissor"],
            audiencia: "aud",
        };
        assert_eq!(jwt::conferir(&bom, &j, &ex, agora).map(|_| ()), Ok(()));
        let largada = std::sync::Barrier::new(16);
        let motivos: Vec<Motivo> = std::thread::scope(|s| {
            let h: Vec<_> = (0..16)
                .map(|_| {
                    s.spawn(|| {
                        largada.wait();
                        (0..3)
                            .map(|_| jwt::conferir(&lixo, &j, &ex, agora).unwrap_err())
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            h.into_iter().flat_map(|x| x.join().unwrap()).collect()
        });
        let depois = jwt::conferir(&bom, &j, &ex, agora).map(|_| ());
        (motivos, depois)
    });
    let (motivos, depois) = rodada.await;
    let ocupado = motivos.iter().filter(|m| **m == Motivo::Ocupado).count();
    let assinatura = motivos.iter().filter(|m| **m == Motivo::Assinatura).count();
    println!("teto 2, 48 conferencias de lixo: {assinatura} pagaram a conta, {ocupado} 429");
    assert_eq!(ocupado + assinatura, 48, "{motivos:?}");
    assert!(ocupado > 0, "dezesseis em voo com teto 2 e nenhum 429");
    assert_eq!(depois, Ok(()), "a vaga volta depois da rajada");
    assert_eq!(Motivo::Ocupado.recusa().0, 429);
    let _ = std::fs::remove_dir_all(dir);
}

/// A URL do JWKS e a ancora de confianca: trocada, avisa ao subir; `http://` so loopback.
#[test]
fn jwks_trocado_avisa_e_http_fora_de_loopback_recusa() {
    use phxclaw_agent::canais::googlechat::{JWKS_PROJETO, Verificacao};
    use phxclaw_agent::canais::jwt::{Jwks, aviso_jwks};
    use phxclaw_agent::canais::teams::JWKS;
    assert_eq!(aviso_jwks("teams", JWKS, JWKS), None);
    let a = aviso_jwks("teams", "https://chaves.exemplo.com/k", JWKS).unwrap();
    assert!(
        a.contains("teams") && a.contains("https://chaves.exemplo.com/k") && a.contains(JWKS),
        "{a}"
    );
    assert!(aviso_jwks("googlechat", JWKS_PROJETO, JWKS_PROJETO).is_none());
    for u in ["http://chaves.exemplo.com/keys", "http://10.0.0.1/keys"] {
        assert!(Jwks::novo(u).is_err(), "{u}");
        assert!(Jwks::configurado("teams", Some(u), JWKS).is_err(), "{u}");
        assert!(Verificacao::nova("123".into(), Some(u)).is_err(), "{u}");
    }
    assert!(Jwks::configurado("teams", Some("http://127.0.0.1:9/keys"), JWKS).is_ok());
    assert!(Jwks::configurado("teams", None, JWKS).is_ok());
    assert!(Jwks::configurado("teams", Some("  "), JWKS).is_ok());
}

/// `iguais` nao conta o tamanho do segredo: comparar com um chute de 1 byte custa o mesmo
/// que com um do tamanho certo. Saindo cedo quando o tamanho difere, o chute curto voltava
/// em nanossegundos e o longo pagava o laco inteiro (RED medido).
#[test]
fn iguais_nao_conta_o_tamanho_do_segredo() {
    use phxclaw_agent::canais::cripto::iguais;
    use std::hint::black_box;
    use std::time::Instant;
    let segredo = vec![b'x'; 256 << 10];
    let mut longo = segredo.clone();
    *longo.last_mut().unwrap() = b'y';
    let curto = b"x".to_vec();
    assert!(iguais(&segredo, &segredo.clone()));
    assert!(!iguais(&segredo, &longo));
    assert!(!iguais(&segredo, &curto));
    assert!(!iguais(b"", b"x"));
    let medir = |chute: &[u8]| {
        (0..7)
            .map(|_| {
                let t = Instant::now();
                black_box(iguais(black_box(&segredo), black_box(chute)));
                t.elapsed()
            })
            .min()
            .unwrap()
    };
    let (t_curto, t_longo) = (medir(&curto), medir(&longo));
    println!("iguais: chute curto {t_curto:?}, chute do tamanho certo {t_longo:?}");
    assert!(
        t_curto * 4 >= t_longo,
        "o chute curto saiu {t_curto:?} contra {t_longo:?}: o tempo conta o tamanho"
    );
}

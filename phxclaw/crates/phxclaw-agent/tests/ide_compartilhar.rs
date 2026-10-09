//! O terminal compartilhado do IDE (gap `live_share`), pelo agente real em processo e o Helix
//! real no PTY: o anfitriao compartilha, o convidado ve a grade e NAO digita, o terminal
//! compartilhado nao tem o token da API no ambiente (R1), convite errado/lotado/revogado/
//! expirado e recusado, a revogacao derruba quem assiste, o ledger registra sem o segredo, e
//! a escrita so existe com o convite que a concede E o papel do terminal pelo RBAC.

use futures_util::{SinkExt, StreamExt};
use phxclaw_agent::agenda::Agenda;
use phxclaw_agent::api::{AgentFactory, ApiState, Limite, router};
use phxclaw_agent::motor::Agent;
use phxclaw_agent::rbac::{self, Usuarios};
use phxclaw_agent::tarefa::TaskStore;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message as WsMsg;

const TOKEN: &str = "token-do-compartilhamento-com-mais-de-24";

/// Os testes fixam `PHXCLAW_PROJETO` (ambiente do processo): um de cada vez.
static UM_DE_CADA_VEZ: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Pasta(PathBuf);
impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn pasta(nome: &str) -> Pasta {
    let p = std::env::temp_dir().join(format!(
        "phx-ide-comp-{nome}-{}",
        phxclaw_types::new_uuid_v7()
    ));
    std::fs::create_dir_all(p.join("projeto/src")).unwrap();
    std::fs::write(
        p.join("projeto/src/main.rs"),
        "fn main() {\n    println!(\"oi\");\n}\n",
    )
    .unwrap();
    Pasta(p)
}

fn sem_hx_ou_bwrap() -> bool {
    if phxclaw_terminal::helix::achar(None).is_none() {
        phxclaw_test_support::pulado::pular("hx", "hx ausente neste hospedeiro");
        return true;
    }
    if phxclaw_agent::arquivos::achar_bwrap().is_none() {
        phxclaw_test_support::pulado::pular("bwrap", "sem bwrap o terminal do IDE recusa");
        return true;
    }
    false
}

async fn subir(p: &Path, com_usuarios: bool) -> String {
    unsafe {
        std::env::set_var("PHXCLAW_PROJETO", p.join("projeto"));
        // O servidor de linguagem nao entra nesta prova; sem ele o Helix sobe mais leve.
        std::env::set_var("PHXCLAW_LSP", "0");
    }
    let store = TaskStore::new(p.join("tasks")).unwrap();
    let st2 = store.clone();
    let factory: AgentFactory = Arc::new(move |_: &str| {
        let llm = Arc::new(phxclaw_agent::ScriptedLlm::new(vec![]));
        Ok(Agent::new(llm, vec![], Default::default(), st2.clone()))
    });
    let state = ApiState {
        usuarios: if com_usuarios {
            Usuarios::da_pasta(p)
        } else {
            Default::default()
        },
        store,
        factory,
        default_model: "falso".into(),
        token: TOKEN.into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(p.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(1000)),
    };
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("127.0.0.1:{}", l.local_addr().unwrap().port());
    tokio::spawn(async move { axum::serve(l, router(state)).await.unwrap() });
    base
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Uma conexao ao websocket que guarda TUDO o que viu: a prova de «nao apareceu» le o
/// historico inteiro, nao so a ultima mensagem.
struct Fio {
    ws: Ws,
    vistos: Vec<Value>,
    fechou: bool,
}

impl Fio {
    async fn abrir(
        base: &str,
        rota: &str,
    ) -> (Self, tokio_tungstenite::tungstenite::http::HeaderMap) {
        let (ws, r) = tokio_tungstenite::connect_async(format!("ws://{base}{rota}"))
            .await
            .unwrap();
        (
            Self {
                ws,
                vistos: vec![],
                fechou: false,
            },
            r.headers().clone(),
        )
    }
    async fn mandar(&mut self, v: Value) {
        let _ = self.ws.send(WsMsg::Text(v.to_string().into())).await;
    }
    /// Le ate `ate` ser verdade para alguma mensagem (ou o prazo, ou o fio fechar).
    async fn esperar(&mut self, ate: impl Fn(&Value) -> bool, prazo: Duration) -> bool {
        let fim = tokio::time::Instant::now() + prazo;
        loop {
            match tokio::time::timeout_at(fim, self.ws.next()).await {
                Ok(Some(Ok(WsMsg::Text(t)))) => {
                    let v: Value = serde_json::from_str(t.as_str()).unwrap();
                    let ok = ate(&v);
                    self.vistos.push(v);
                    if ok {
                        return true;
                    }
                }
                Ok(Some(Ok(WsMsg::Close(_)))) | Ok(None) | Ok(Some(Err(_))) => {
                    self.fechou = true;
                    return false;
                }
                Ok(Some(Ok(_))) => {}
                Err(_) => return false,
            }
        }
    }
    /// O texto de todas as grades vistas ate agora.
    fn texto(&self) -> String {
        self.vistos
            .iter()
            .filter(|m| m["ev"] == "grade")
            .flat_map(linhas)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn linhas(m: &Value) -> Vec<String> {
    m["linhas_alteradas"]
        .as_array()
        .map(|ls| {
            ls.iter()
                .map(|l| {
                    l["trechos"]
                        .as_array()
                        .map(|ts| {
                            ts.iter()
                                .map(|t| t["texto"].as_str().unwrap_or(""))
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default()
}

fn grade_com(texto: &'static str) -> impl Fn(&Value) -> bool {
    move |m| m["ev"] == "grade" && linhas(m).iter().any(|l| l.contains(texto))
}

async fn anfitriao(base: &str, compartilhavel: bool) -> Fio {
    let (mut f, _) = Fio::abrir(base, "/v1/ide/terminal").await;
    f.mandar(json!({"op": "auth", "token": TOKEN})).await;
    f.mandar(
        json!({"op": "abrir", "programa": "helix", "colunas": 120, "linhas": 30,
                    "compartilhavel": compartilhavel}),
    )
    .await;
    assert!(
        f.esperar(grade_com("NOR"), Duration::from_secs(30)).await,
        "o Helix nao subiu: {:?}",
        f.vistos.last()
    );
    f
}

/// Um comando do Helix: Esc SOZINHO e depois a linha (ESC seguido de byte no mesmo lote e
/// Alt+tecla para o Helix).
async fn comando(f: &mut Fio, linha: &str) {
    f.mandar(json!({"op": "texto", "texto": "\u{1b}"})).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    f.mandar(json!({"op": "texto", "texto": format!(":{linha}\r")}))
        .await;
}

async fn post(base: &str, rota: &str, token: &str, corpo: Value) -> (u16, Value) {
    let r = reqwest::Client::new()
        .post(format!("http://{base}{rota}"))
        .bearer_auth(token)
        .json(&corpo)
        .send()
        .await
        .unwrap();
    let s = r.status().as_u16();
    (s, r.json().await.unwrap_or(Value::Null))
}

async fn convidado(base: &str, sessao: &str, token: &str, credencial: Option<&str>) -> Fio {
    let (mut f, _) = Fio::abrir(base, "/v1/ide/compartilhado").await;
    let mut entrar = json!({"op": "entrar", "sessao": sessao, "token": token});
    if let Some(c) = credencial {
        entrar["credencial"] = json!(c);
    }
    f.mandar(entrar).await;
    f
}

/// O caminho inteiro sem RBAC (o Bearer unico da API e o dono).
///
/// RED medido, cada um com `// REPOSTO` e restaurado por escrita:
/// - R1: `api.filter(|_| !compartilhavel)` trocado por `api` em `ide::ambiente_do_helix` -- o
///   `printenv` do terminal compartilhado desenhou o token na grade do convidado;
/// - somente leitura: `entrada: Some(c.controle.clone())` em `admitir` E sem `&& c.escreve` no
///   `tratar` -- a marca do convidado apareceu na grade do anfitriao (as duas camadas: com uma
///   so reposta, a outra segura);
/// - revogacao: `encerrar` sem o `remove` do mapa -- o convidado entrou depois do 200;
/// - expiracao: sem a conferencia do relogio no convidado e no `vigiar` -- o convidado seguiu
///   conectado depois do prazo.
#[tokio::test(flavor = "multi_thread")]
async fn o_convidado_ve_o_anfitriao_e_nao_digita_e_o_convite_morre_quando_deve() {
    if sem_hx_ou_bwrap() {
        return;
    }
    let _serial = UM_DE_CADA_VEZ.lock().await;
    let p = pasta("ws");
    let base = subir(&p.0, false).await;

    // Controle do R1: o terminal COMUM tem uma credencial no ambiente (a prova sabe ver a
    // credencial) -- a do terminal (`rbac::CredencialDoTerminal`), nunca o `api.token`.
    let mut comum = anfitriao(&base, false).await;
    comando(&mut comum, "sh printenv PHXCLAW_API_TOKEN").await;
    assert!(
        comum
            .esperar(
                |m| m["ev"] == "grade"
                    && linhas(m).iter().any(|l| l.contains(rbac::PREFIXO_SESSAO)),
                Duration::from_secs(10)
            )
            .await,
        "o controle nao viu a credencial no terminal comum: {}",
        comum.texto()
    );
    assert!(
        !comum.texto().contains(TOKEN),
        "o terminal comum leva o api.token mestre"
    );
    // E ele NAO se compartilha: a recusa pede reabrir sem a credencial.
    let (s, v) = post(&base, "/v1/ide/compartilhar", TOKEN, json!({})).await;
    assert_eq!(s, 409, "{v}");
    assert_eq!(v["reabrir"], true, "{v}");
    comando(&mut comum, "qa!").await;
    drop(comum);
    tokio::time::sleep(Duration::from_millis(300)).await;

    // O compartilhavel.
    let mut anf = anfitriao(&base, true).await;
    let (s, v) = post(
        &base,
        "/v1/ide/compartilhar",
        TOKEN,
        json!({"expira_em_s": 120, "max_convidados": 2}),
    )
    .await;
    assert_eq!(s, 200, "{v}");
    let sessao = v["sessao"].as_str().unwrap().to_string();
    let token = v["token"].as_str().unwrap().to_string();
    assert_eq!(token.len(), 64, "32 bytes em hexa: {token}");
    assert_eq!(v["escrita"], false);

    // O convidado entra, le, e recebe o retrato inteiro com a linha de estado.
    let (mut conv, cab) = Fio::abrir(&base, "/v1/ide/compartilhado").await;
    // Os cabecalhos da casa (pwa::blindar) valem tambem no aceite do websocket.
    assert!(
        cab.get("content-security-policy")
            .is_some_and(|v| v.to_str().unwrap().contains("default-src 'self'")),
        "{cab:?}"
    );
    assert_eq!(cab.get("x-frame-options").unwrap(), "DENY");
    conv.mandar(json!({"op": "entrar", "sessao": sessao, "token": token}))
        .await;
    assert!(
        conv.esperar(|m| m["ev"] == "entrou", Duration::from_secs(5))
            .await
    );
    assert_eq!(conv.vistos.last().unwrap()["escreve"], false);
    assert!(
        conv.esperar(grade_com("NOR"), Duration::from_secs(10))
            .await,
        "o convidado nao recebeu a grade: {:?}",
        conv.vistos
    );
    // O anfitriao ve o convidado chegar.
    assert!(
        anf.esperar(
            |m| m["ev"] == "compartilhamento"
                && m["convidados"].as_array().is_some_and(|c| c.len() == 1),
            Duration::from_secs(5)
        )
        .await
    );

    // O convidado tenta digitar: cada tentativa volta recusada, e NADA chega ao terminal.
    conv.mandar(json!({"op": "texto", "texto": "\u{1b}"})).await;
    conv.mandar(json!({"op": "texto", "texto": "iMARCA-DO-CONVIDADO"}))
        .await;
    conv.mandar(json!({"op": "colar", "texto": "COLA-DO-CONVIDADO"}))
        .await;
    conv.mandar(json!({"op": "tecla", "tecla": {"tecla": "x"}}))
        .await;
    conv.mandar(json!({"op": "redimensionar", "colunas": 20, "linhas": 5}))
        .await;
    // Controle: o que o ANFITRIAO digita chega ao convidado.
    comando(&mut anf, "echo MARCA-DO-ANFITRIAO").await;
    assert!(
        conv.esperar(grade_com("MARCA-DO-ANFITRIAO"), Duration::from_secs(10))
            .await,
        "o convidado nao viu o anfitriao: {}",
        conv.texto()
    );
    anf.esperar(|_| false, Duration::from_millis(800)).await;
    for t in ["MARCA-DO-CONVIDADO", "COLA-DO-CONVIDADO"] {
        assert!(
            !anf.texto().contains(t),
            "{t} chegou ao terminal do anfitriao"
        );
        assert!(!conv.texto().contains(t), "{t} chegou a grade");
    }
    assert!(
        !anf.vistos
            .iter()
            .any(|m| m["ev"] == "grade" && m["colunas"] == 20),
        "o convidado redimensionou o terminal"
    );
    // E cada tentativa voltou recusada, dizendo por que.
    assert!(
        conv.vistos
            .iter()
            .any(|m| m["ev"] == "erro"
                && m["erro"].as_str().unwrap_or("").contains("somente leitura")),
        "{:?}",
        conv.vistos
            .iter()
            .filter(|m| m["ev"] != "grade")
            .collect::<Vec<_>>()
    );

    // R1 de ponta a ponta: o terminal compartilhado nao tem o token; o convidado ve o fim do
    // comando e nao ve segredo nenhum.
    comando(&mut anf, "sh printenv PHXCLAW_API_TOKEN; echo FIM-DA-PROVA").await;
    assert!(
        conv.esperar(grade_com("FIM-DA-PROVA"), Duration::from_secs(10))
            .await,
        "o :sh nao rodou: {}",
        conv.texto()
    );
    assert!(
        !conv.texto().contains(TOKEN),
        "o token da API chegou ao convidado"
    );
    assert!(
        !anf.texto().contains(TOKEN) && !anf.texto().contains(rbac::PREFIXO_SESSAO),
        "o token da API esta no terminal compartilhado"
    );
    assert!(
        !conv.texto().contains(rbac::PREFIXO_SESSAO),
        "a credencial do terminal chegou ao convidado"
    );

    // Convite errado e lotado.
    let mut errado = convidado(&base, &sessao, &"0".repeat(64), None).await;
    assert!(
        errado
            .esperar(|m| m["ev"] == "erro", Duration::from_secs(5))
            .await
    );
    assert!(
        errado.vistos[0]["erro"].as_str().unwrap().contains("token"),
        "{:?}",
        errado.vistos
    );
    let mut segundo = convidado(&base, &sessao, &token, None).await;
    assert!(
        segundo
            .esperar(|m| m["ev"] == "entrou", Duration::from_secs(5))
            .await
    );
    let mut terceiro = convidado(&base, &sessao, &token, None).await;
    assert!(
        terceiro
            .esperar(|m| m["ev"] == "erro", Duration::from_secs(5))
            .await
    );
    assert!(
        terceiro.vistos[0]["erro"]
            .as_str()
            .unwrap()
            .contains("limite"),
        "{:?}",
        terceiro.vistos
    );

    // Revogacao: o 200 so volta depois de quem assistia cair, e ninguem mais entra.
    let (s, v) = post(&base, "/v1/ide/compartilhar/revogar", TOKEN, json!({})).await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["revogados"], 2, "{v}");
    for f in [&mut conv, &mut segundo] {
        f.esperar(|m| m["ev"] == "fim", Duration::from_secs(3))
            .await;
        assert!(
            f.vistos
                .iter()
                .any(|m| m["ev"] == "fim" && m["motivo"] == "revogado"),
            "sem fim revogado: {:?}",
            f.vistos.last().map(|m| (&m["ev"], &m["motivo"]))
        );
    }
    let mut tarde = convidado(&base, &sessao, &token, None).await;
    assert!(
        tarde
            .esperar(|m| m["ev"] == "erro", Duration::from_secs(5))
            .await
    );
    assert!(!tarde.vistos.iter().any(|m| m["ev"] == "entrou"));

    // Expiracao dura: convite de 2 s derruba quem esta dentro e recusa quem chega depois.
    let (s, v) = post(
        &base,
        "/v1/ide/compartilhar",
        TOKEN,
        json!({"expira_em_s": 2}),
    )
    .await;
    assert_eq!(s, 200, "{v}");
    let (sessao2, token2) = (
        v["sessao"].as_str().unwrap().to_string(),
        v["token"].as_str().unwrap().to_string(),
    );
    let mut curto = convidado(&base, &sessao2, &token2, None).await;
    assert!(
        curto
            .esperar(|m| m["ev"] == "entrou", Duration::from_secs(2))
            .await
    );
    curto
        .esperar(|m| m["ev"] == "fim", Duration::from_secs(6))
        .await;
    assert!(
        curto
            .vistos
            .iter()
            .any(|m| m["ev"] == "fim" && m["motivo"] == "expirado"),
        "o convidado seguiu depois do prazo: {:?}",
        curto.vistos.last().map(|m| (&m["ev"], &m["motivo"]))
    );
    let mut vencido = convidado(&base, &sessao2, &token2, None).await;
    assert!(
        vencido
            .esperar(|m| m["ev"] == "erro", Duration::from_secs(5))
            .await
    );

    // O ledger da sessao: criar, entrar (e a recusa), revogar -- e nenhum segredo.
    let ledger =
        std::fs::read_to_string(p.0.join("tasks").join(&sessao).join("evidence.jsonl")).unwrap();
    for acao in [
        "ide.compartilhar: criar",
        "ide.compartilhar: entrar",
        "ide.compartilhar: encerrar",
    ] {
        assert!(ledger.contains(acao), "sem {acao}: {ledger}");
    }
    assert!(ledger.contains("\"motivo\":\"revogado\""), "{ledger}");
    assert!(ledger.contains("token do convite invalido"), "{ledger}");
    assert!(
        !ledger.contains(&token) && !ledger.contains(TOKEN),
        "segredo no ledger"
    );

    comando(&mut anf, "qa!").await;
}

/// Com usuarios (RBAC ligado): criar e do dono; escrever pede o convite com escrita E a
/// credencial que a linha do terminal pede. Membro com convite de escrita entra LENDO; o
/// dono com convite de leitura tambem.
///
/// RED medido: `let escreve = c.escrita && pode_escrever` trocado por `c.escrita`
/// (`// REPOSTO`) -- o membro entrou com `escreve: true`.
#[tokio::test(flavor = "multi_thread")]
async fn escrever_pede_o_convite_e_o_papel_do_terminal_pelo_rbac() {
    if sem_hx_ou_bwrap() {
        return;
    }
    let _serial = UM_DE_CADA_VEZ.lock().await;
    let p = pasta("rbac");
    let base = subir(&p.0, true).await;
    let criar = |nome: &str, papel: &str, projetos: &[&str]| {
        let mut a = vec![
            "criar".to_string(),
            nome.into(),
            "--papel".into(),
            papel.into(),
        ];
        for x in projetos {
            a.push("--projeto".into());
            a.push((*x).into());
        }
        rbac::comando(&p.0, &a)
            .unwrap()
            .lines()
            .find(|l| l.starts_with(rbac::PREFIXO_TOKEN))
            .unwrap()
            .to_string()
    };
    let membro = criar("ana", "member", &["alfa"]);
    let admin = criar("bia", "admin", &[]);

    let mut anf = anfitriao(&base, true).await;
    // A rota de criar esta atras do portao: admin nao alcanca (o terminal e do dono).
    let (s, _) = post(&base, "/v1/ide/compartilhar", &admin, json!({})).await;
    assert_eq!(s, 403);
    let (s, v) = post(
        &base,
        "/v1/ide/compartilhar",
        TOKEN,
        json!({"escrita": true}),
    )
    .await;
    assert_eq!(s, 200, "{v}");
    let (sessao, token) = (
        v["sessao"].as_str().unwrap().to_string(),
        v["token"].as_str().unwrap().to_string(),
    );

    // Sem credencial e com a do membro: entra lendo.
    for cred in [None, Some(membro.as_str()), Some(admin.as_str())] {
        let mut c = convidado(&base, &sessao, &token, cred).await;
        assert!(
            c.esperar(|m| m["ev"] == "entrou", Duration::from_secs(5))
                .await
        );
        assert_eq!(c.vistos.last().unwrap()["escreve"], false, "{cred:?}");
    }
    // Com a do dono e o convite de escrita: escreve, e o texto chega ao terminal.
    let mut dono = convidado(&base, &sessao, &token, Some(TOKEN)).await;
    assert!(
        dono.esperar(|m| m["ev"] == "entrou", Duration::from_secs(5))
            .await
    );
    assert_eq!(dono.vistos.last().unwrap()["escreve"], true);
    dono.mandar(json!({"op": "texto", "texto": "\u{1b}"})).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    dono.mandar(json!({"op": "texto", "texto": ":echo ESCRITA-DO-CONVIDADO\r"}))
        .await;
    assert!(
        anf.esperar(grade_com("ESCRITA-DO-CONVIDADO"), Duration::from_secs(10))
            .await,
        "a escrita concedida nao chegou: {}",
        anf.texto()
    );

    // Convite SEM escrita: nem o dono escreve por ele.
    let (s, v) = post(&base, "/v1/ide/compartilhar", TOKEN, json!({})).await;
    assert_eq!(s, 200, "{v}");
    let mut so_le = convidado(
        &base,
        v["sessao"].as_str().unwrap(),
        v["token"].as_str().unwrap(),
        Some(TOKEN),
    )
    .await;
    assert!(
        so_le
            .esperar(|m| m["ev"] == "entrou", Duration::from_secs(5))
            .await
    );
    assert_eq!(so_le.vistos.last().unwrap()["escreve"], false);

    comando(&mut anf, "qa!").await;
}

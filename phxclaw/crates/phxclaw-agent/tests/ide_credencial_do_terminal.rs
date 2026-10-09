//! O terminal do IDE nao leva o `api.token` MESTRE: o `hx` recebe a credencial de SESSAO do
//! terminal (`rbac::CredencialDoTerminal`), que so vale na rota da completacao por IA (a unica
//! para a qual o `hx` usa token), com o papel ATUAL de quem abriu, e morre com o usuario
//! revogado, com o terminal fechado e com o prazo.
//!
//! O ambiente do `hx` e lido de verdade: o `hx` aqui e um executavel FALSO (`desktop.hx`) que
//! imprime o `PHXCLAW_API_TOKEN` que recebeu e espera -- o caminho inteiro do websocket, do
//! `abrir` e do bwrap e o de producao; so o programa no fim dele e outro.
//!
//! RED medido em 09/10/2026, cada um com `// REPOSTO` e restaurado por escrita:
//! - R6 `abrir(.., Some(s.token.as_str()), ..)` no `ide::sessao` (o defeito de origem): a
//!   grade mostrou o token mestre (`o_terminal_do_usuario_nao_leva_o_token_mestre...`);
//! - R7 sem a conferencia de quem abriu em `rbac::acesso_da_sessao` (o ramo `Usuario` sem
//!   procurar na lista): a credencial seguiu valendo depois do `usuario remover`;
//! - R8 sem a conferencia da rota em `acesso_da_sessao`: a credencial abriu OUTRO terminal
//!   pelo websocket (que so passa pelo portao). As rotas HTTP seguiram recusando: o `auth` de
//!   cada uma nao aceita a credencial -- segunda trava, e por isso a prova e o websocket;
//! - R9 sem a reconferencia do `conferir_rota` no relogio do `ide::sessao`: o terminal do
//!   usuario revogado seguiu aberto.

use futures_util::{SinkExt, StreamExt};
use phxclaw_agent::agenda::Agenda;
use phxclaw_agent::api::{AgentFactory, ApiState, Limite, router};
use phxclaw_agent::ide::ROTA_COMPLETAR;
use phxclaw_agent::motor::Agent;
use phxclaw_agent::rbac::{self, CredencialDoTerminal, Usuarios};
use phxclaw_agent::tarefa::TaskStore;
use phxclaw_agent_core::{BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, ToolSpec, Usage};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message as WsMsg;

const TOKEN: &str = "token-mestre-do-terminal-com-mais-de-24";

/// Os testes fixam `PHXCLAW_PROJETO` (ambiente do processo): um de cada vez.
static UM_DE_CADA_VEZ: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct ModeloFalso;
impl Llm for ModeloFalso {
    fn id(&self) -> String {
        "falso:ide".into()
    }
    fn chat<'a>(
        &'a self,
        _m: &'a [Message],
        _t: &'a [ToolSpec],
        _o: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async move {
            Ok(LlmReply {
                content: "    a + b".into(),
                tool_calls: vec![],
                usage: Usage::default(),
                model: "falso-1".into(),
            })
        })
    }
}

struct Pasta(PathBuf);
impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn pasta(nome: &str) -> Pasta {
    let p = std::env::temp_dir().join(format!(
        "phx-ide-cred-{nome}-{}",
        phxclaw_types::new_uuid_v7()
    ));
    std::fs::create_dir_all(p.join("projeto/src")).unwrap();
    std::fs::write(p.join("projeto/src/main.rs"), "fn main() {}\n").unwrap();
    Pasta(p)
}

/// O `hx` falso, um por processo (a configuracao do processo le `desktop.hx` uma vez).
fn hx_falso() -> &'static Path {
    static H: OnceLock<PathBuf> = OnceLock::new();
    H.get_or_init(|| {
        let bin = std::env::temp_dir()
            .join(format!("phx-hx-falso-{}", std::process::id()))
            .join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let hx = bin.join("hx");
        std::fs::write(
            &hx,
            "#!/bin/sh\nprintf 'TOK[%s]\\n' \"$PHXCLAW_API_TOKEN\"\nexec sleep 600\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hx, std::fs::Permissions::from_mode(0o755)).unwrap();
        unsafe {
            std::env::set_var("PHXCLAW_HX", &hx);
            std::env::set_var("PHXCLAW_LSP", "0");
        }
        hx
    })
}

fn sem_bwrap() -> bool {
    if phxclaw_agent::arquivos::achar_bwrap().is_none() {
        phxclaw_test_support::pulado::pular("bwrap", "sem bwrap o terminal do IDE recusa");
        return true;
    }
    false
}

fn estado(p: &Path, com_usuarios: bool) -> ApiState {
    let store = TaskStore::new(p.join("tasks")).unwrap();
    let st2 = store.clone();
    let factory: AgentFactory = Arc::new(move |_: &str| {
        Ok(Agent::new(
            Arc::new(ModeloFalso),
            vec![],
            Default::default(),
            st2.clone(),
        ))
    });
    ApiState {
        usuarios: if com_usuarios {
            Usuarios::da_pasta(p)
        } else {
            Default::default()
        },
        store,
        factory,
        default_model: "falso:ide".into(),
        token: TOKEN.into(),
        running: Arc::new(Mutex::new(HashMap::new())),
        agenda: Arc::new(Mutex::new(Agenda::open(p.join("agenda.json")).unwrap())),
        webhook_origins: vec![],
        limite: Arc::new(Limite::por_minuto(1000)),
    }
}

async fn servir(s: ApiState) -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("127.0.0.1:{}", l.local_addr().unwrap().port());
    tokio::spawn(async move { axum::serve(l, router(s)).await.unwrap() });
    base
}

fn criar(p: &Path, nome: &str, papel: &str, projetos: &[&str]) -> String {
    let mut a = vec![
        "criar".to_string(),
        nome.into(),
        "--papel".into(),
        papel.into(),
    ];
    for x in projetos {
        a.extend(["--projeto".to_string(), (*x).into()]);
    }
    rbac::comando(p, &a)
        .unwrap()
        .lines()
        .find(|l| l.starts_with(rbac::PREFIXO_TOKEN))
        .unwrap()
        .to_string()
}

async fn pedir(base: &str, metodo: reqwest::Method, rota: &str, token: &str) -> u16 {
    let mut r = reqwest::Client::new()
        .request(metodo.clone(), format!("http://{base}{rota}"))
        .bearer_auth(token);
    if metodo == reqwest::Method::POST {
        r = r.json(&json!({"arquivo": "src/main.rs", "linguagem": "rust",
                           "antes": "fn soma(a: i32, b: i32) -> i32 {\n", "depois": "}",
                           "objective": "x"}));
    }
    r.send().await.unwrap().status().as_u16()
}

async fn completar(base: &str, token: &str) -> u16 {
    pedir(base, reqwest::Method::POST, ROTA_COMPLETAR, token).await
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Abre o terminal com `token`, manda abrir o «helix» e le a grade ate a linha `TOK[...]`
/// que o `hx` falso imprime. Devolve o websocket (aberto) e o que estava entre os colchetes.
async fn abrir_terminal(base: &str, token: &str) -> (Ws, String) {
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{base}/v1/ide/terminal"))
        .await
        .unwrap();
    for m in [
        json!({"op": "auth", "token": token}),
        json!({"op": "abrir", "programa": "helix", "colunas": 120, "linhas": 10}),
    ] {
        ws.send(WsMsg::Text(m.to_string().into())).await.unwrap();
    }
    let fim = tokio::time::Instant::now() + Duration::from_secs(20);
    let mut vistos = vec![];
    loop {
        let m = tokio::time::timeout_at(fim, ws.next())
            .await
            .unwrap_or_else(|_| panic!("o hx falso nao imprimiu: {vistos:?}"));
        let Some(Ok(WsMsg::Text(t))) = m else {
            panic!("o terminal fechou: {m:?} {vistos:?}")
        };
        let v: Value = serde_json::from_str(t.as_str()).unwrap();
        let texto = v.to_string();
        vistos.push(v.clone());
        if v["ev"] == "erro" {
            panic!("{v}");
        }
        if let Some(i) = texto.find("TOK[") {
            let resto = &texto[i + 4..];
            let tok = resto[..resto.find(']').unwrap()].to_string();
            return (ws, tok);
        }
    }
}

/// Espera o servidor mandar o erro e fechar o terminal (ou o prazo).
async fn fechado_com_erro(ws: &mut Ws, prazo: Duration) -> Option<Value> {
    let fim = tokio::time::Instant::now() + prazo;
    loop {
        match tokio::time::timeout_at(fim, ws.next()).await {
            Ok(Some(Ok(WsMsg::Text(t)))) => {
                let v: Value = serde_json::from_str(t.as_str()).unwrap();
                if v["ev"] == "erro" {
                    return Some(v);
                }
            }
            Ok(Some(Ok(_))) => {}
            _ => return None,
        }
    }
}

/// Com RBAC: o dono-USUARIO abre o terminal; o `hx` recebe uma credencial `phxs_` (nao o
/// mestre, nao o token dele), que serve a completacao e nada mais; o membro nem abre o
/// terminal; revogado o usuario, a credencial morre no pedido seguinte e o terminal fecha.
#[tokio::test(flavor = "multi_thread")]
async fn o_terminal_do_usuario_nao_leva_o_token_mestre_e_morre_com_ele() {
    if sem_bwrap() {
        return;
    }
    let _serial = UM_DE_CADA_VEZ.lock().await;
    hx_falso();
    let p = pasta("rbac");
    unsafe {
        std::env::set_var("PHXCLAW_PROJETO", p.0.join("projeto"));
    }
    let ana = criar(&p.0, "ana", "owner", &[]);
    let caio = criar(&p.0, "caio", "member", &["vendas"]);
    let base = servir(estado(&p.0, true)).await;

    // O membro nao abre o terminal (a linha da matriz e do dono).
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{base}/v1/ide/terminal"))
        .await
        .unwrap();
    ws.send(WsMsg::Text(
        json!({"op": "auth", "token": caio}).to_string().into(),
    ))
    .await
    .unwrap();
    assert!(
        fechado_com_erro(&mut ws, Duration::from_secs(5))
            .await
            .is_some(),
        "o membro abriu o terminal"
    );

    let (mut ws, sessao) = abrir_terminal(&base, &ana).await;
    assert!(
        sessao.starts_with(rbac::PREFIXO_SESSAO),
        "o hx nao recebeu a credencial do terminal: {sessao:?}"
    );
    assert_ne!(sessao, TOKEN, "o hx recebeu o api.token mestre");
    assert_ne!(sessao, ana, "o hx recebeu o token do usuario");
    // Serve a completacao, e so ela.
    assert_eq!(completar(&base, &sessao).await, 200);
    assert_eq!(
        pedir(&base, reqwest::Method::GET, "/v1/tasks", &sessao).await,
        401
    );
    assert_eq!(
        pedir(&base, reqwest::Method::GET, "/v1/config", &sessao).await,
        401
    );
    assert_eq!(
        pedir(&base, reqwest::Method::POST, "/v1/tasks", &sessao).await,
        401
    );
    // Nem abre outro terminal: o websocket confere so pelo portao (`conferir_rota`), sem o
    // `auth` da rota HTTP por tras -- e onde a regra da rota da credencial e a unica trava.
    let (mut outro, _) = tokio_tungstenite::connect_async(format!("ws://{base}/v1/ide/terminal"))
        .await
        .unwrap();
    outro
        .send(WsMsg::Text(
            json!({"op": "auth", "token": sessao}).to_string().into(),
        ))
        .await
        .unwrap();
    let m = tokio::time::timeout(Duration::from_secs(5), outro.next())
        .await
        .unwrap();
    assert!(
        matches!(&m, Some(Ok(WsMsg::Text(t))) if t.as_str().contains("\"erro\"")),
        "a credencial do terminal abriu um terminal: {m:?}"
    );

    // Revogar a ana mata a credencial no pedido seguinte e fecha o terminal dela.
    rbac::comando(&p.0, &["remover".into(), "ana".into()]).unwrap();
    assert_eq!(completar(&base, &sessao).await, 401);
    let e = fechado_com_erro(&mut ws, Duration::from_secs(5)).await;
    assert!(
        e.as_ref()
            .is_some_and(|v| v["erro"].as_str().unwrap_or("").contains("revogado")),
        "o terminal do usuario revogado seguiu aberto: {e:?}"
    );
}

/// Sem RBAC (so o `api.token`): o terminal tambem nao leva o mestre; a credencial vale na
/// completacao, nao em outra rota, e morre quando o terminal fecha.
#[tokio::test(flavor = "multi_thread")]
async fn sem_usuarios_o_terminal_tambem_nao_leva_o_token_mestre() {
    if sem_bwrap() {
        return;
    }
    let _serial = UM_DE_CADA_VEZ.lock().await;
    hx_falso();
    let p = pasta("legado");
    unsafe {
        std::env::set_var("PHXCLAW_PROJETO", p.0.join("projeto"));
    }
    let base = servir(estado(&p.0, false)).await;
    let (mut ws, sessao) = abrir_terminal(&base, TOKEN).await;
    assert!(sessao.starts_with(rbac::PREFIXO_SESSAO), "{sessao:?}");
    assert_ne!(sessao, TOKEN);
    assert_eq!(completar(&base, &sessao).await, 200);
    assert_eq!(
        pedir(&base, reqwest::Method::GET, "/v1/tasks", &sessao).await,
        401
    );
    ws.send(WsMsg::Text(json!({"op": "fechar"}).to_string().into()))
        .await
        .unwrap();
    // O fechar e processado pelo laco da sessao: espera ate a credencial morrer.
    let mut status = 0;
    for _ in 0..50 {
        status = completar(&base, &sessao).await;
        if status == 401 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(status, 401, "a credencial sobreviveu ao terminal fechado");
}

/// A credencial sem terminal nenhum (roda sem bwrap): so de quem tem token, morta com a chave
/// trocada de quem abriu, com o prazo e com o `Drop` (o terminal que fecha).
#[tokio::test(flavor = "multi_thread")]
async fn a_credencial_morre_com_a_chave_trocada_o_prazo_e_o_terminal() {
    let p = pasta("regras");
    let ana = criar(&p.0, "ana", "owner", &[]);
    let s = estado(&p.0, true);
    let base = servir(s.clone()).await;
    let c = CredencialDoTerminal::emitir(&s, &ana, "POST", ROTA_COMPLETAR).unwrap();
    assert!(!format!("{c:?}").contains(c.token()));
    assert_eq!(completar(&base, c.token()).await, 200);
    // Token que nao e de ninguem nao emite.
    assert!(CredencialDoTerminal::emitir(&s, "phxu_ninguem", "POST", ROTA_COMPLETAR).is_err());
    // Trocar a chave da ana derruba a credencial emitida com a chave velha.
    rbac::comando(&p.0, &["chave".into(), "ana".into()]).unwrap();
    assert_eq!(completar(&base, c.token()).await, 401);
    // O prazo: emitida ja vencida, nao vale.
    let ana2 = criar(&p.0, "bia", "owner", &[]);
    let vencida =
        CredencialDoTerminal::emitir_com(&s, &ana2, "POST", ROTA_COMPLETAR, Duration::ZERO)
            .unwrap();
    assert_eq!(completar(&base, vencida.token()).await, 401);
    // Solta (o terminal fechou): deixa de valer.
    let viva = CredencialDoTerminal::emitir(&s, &ana2, "POST", ROTA_COMPLETAR).unwrap();
    let t = viva.token().to_string();
    assert_eq!(completar(&base, &t).await, 200);
    drop(viva);
    assert_eq!(completar(&base, &t).await, 401);
}

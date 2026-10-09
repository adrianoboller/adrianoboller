//! O IDE no navegador: o que a tela IDE do PWA precisa do agente para ser o MESMO IDE do
//! aplicativo de mesa, sem o Tauri.
//!
//! Tres rotas, todas com o Bearer da API:
//! - `GET /v1/ide/terminal` (websocket): um Helix num PTY na pasta do projeto, pelo mesmo
//!   `phxclaw-terminal` do desktop. A tela manda tecla, colar, tamanho e rolagem; recebe a
//!   grade por diferenca. O token vai na PRIMEIRA mensagem (o navegador nao poe cabecalho
//!   num websocket) e e conferido por `rbac::conferir_rota`: a MESMA decisao do portao,
//!   com a linha desta rota na matriz (o portao a deixa passar sem token, porque o token
//!   ainda nao chegou; sem usuarios, e a `auth` de sempre). Uma sessao por
//!   usuario: a conexao nova fecha a anterior, em vez de a aba velha prender o terminal.
//!   SO o Helix abre por aqui: um bash livre pela rede seria o `execute_shell` sem a
//!   politica dele. E ele roda no MESMO bwrap dos outros processos
//!   (`processo::terminal_no_bwrap`): o projeto em `/work`, a pasta do agente mascarada, e
//!   o `:sh` dele preso ao mesmo sandbox. Sem bwrap, recusa.
//! - `GET /v1/ide/simbolos?arquivo=`: os simbolos do arquivo (documentSymbol) pelo `lsp.rs`,
//!   para a barra de caminho acima do editor.
//! - `GET /v1/ide/arquivo?caminho=`: o texto do arquivo aberto no Helix, para o minimapa da
//!   tela. Pelo MESMO `confine` das ferramentas e do `simbolos` (pasta do projeto, ou absoluto
//!   dentro de uma raiz do workspace, nunca a area reservada do agente) e com teto de bytes;
//!   o caminho real e conferido de novo no descritor aberto. E o arquivo EM DISCO: o buffer e do
//!   Helix, outro processo, e o que nao foi salvo nao aparece (limite declarado na tela).
//! - `POST /v1/ide/completar`: uma continuacao de codigo pelo modelo configurado do agente
//!   (o mesmo provedor das tarefas), que o `phxclaw-snippet-ls` oferece ao Helix como item
//!   «IA». Teto de tokens e de tempo vindos do ambiente, com padrao curto: completacao que
//!   demora mais que um segundo ou dois ja nao e completacao.

use crate::api::{ApiState, auth};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::SinkExt;
use phxclaw_agent_core::{LlmOptions, Message as Msg};
use phxclaw_terminal::{Atualizacao, Tamanho, Tecla, Terminal};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// A rota do terminal: a mesma constante no `route` e na conferencia do token, para a
/// linha da matriz que a sessao consulta ser a da rota que o navegador abriu.
pub const ROTA_TERMINAL: &str = "/v1/ide/terminal";

pub fn rotas() -> Router<ApiState> {
    Router::new()
        .route(ROTA_TERMINAL, get(terminal))
        .route("/v1/ide/simbolos", get(simbolos))
        .route("/v1/ide/arquivo", get(arquivo))
        .route("/v1/ide/completar", post(completar))
        // O explorador de testes e a loja de plugins da tela: as MESMAS funcoes da CLI
        // (`phxclaw testes`, `phxclaw plugins`) e das ferramentas test_list/test_run e
        // plugin_catalog -- ExploradorDeTestes e Loja, sem segunda montagem.
        .route("/v1/ide/testes", get(testes_listar))
        .route("/v1/ide/testes/rodar", post(testes_rodar))
        .route("/v1/plugins/catalogo", get(plugins_catalogo))
        .route("/v1/plugins/instalar", post(plugins_instalar))
}

type Erro = (StatusCode, Json<Value>);

fn erro(code: StatusCode, msg: impl Into<String>) -> Erro {
    (code, Json(json!({"error": msg.into()})))
}

/// A pasta que o IDE abre: a do projeto do agente (`PHXCLAW_PROJETO` ou a pasta corrente),
/// a mesma do historico de gravacoes e dos hooks.
fn pasta_do_projeto() -> Result<PathBuf, String> {
    crate::montagem::raiz_do_projeto()
        .filter(|p| p.is_dir())
        .ok_or_else(|| "pasta do projeto inexistente (PHXCLAW_PROJETO)".to_string())
}

// ---------------------------------------------------------------- terminal (websocket)

/// Quanto tempo a conexao tem para mandar o token antes de ser fechada: uma conexao muda
/// nao pode segurar o aceite para sempre.
const PRAZO_DO_TOKEN: Duration = Duration::from_secs(10);

/// As sessoes vivas por usuario: a chave e o resumo do token (nunca o token), e o valor e a
/// bandeira que manda a sessao anterior fechar quando chega a nova.
fn vivas() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    static V: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
    V.get_or_init(|| Mutex::new(HashMap::new()))
}

fn chave_do_usuario(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    format!("{:x}", h.finalize())
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
enum Pedido {
    Auth {
        token: String,
    },
    Abrir {
        programa: String,
        colunas: u16,
        linhas: u16,
    },
    Tecla {
        tecla: Tecla,
    },
    Colar {
        texto: String,
    },
    Texto {
        texto: String,
    },
    Redimensionar {
        colunas: u16,
        linhas: u16,
    },
    Rolar {
        linhas: i32,
    },
    Fechar,
}

async fn terminal(State(s): State<ApiState>, h: HeaderMap, ws: WebSocketUpgrade) -> Response {
    // A URL por onde o snippet-ls fala com ESTE agente: o Host por onde a tela chegou, ou
    // a chave `ide.api_url` quando o operador sabe melhor (atras de um proxy, por exemplo).
    let api = crate::config::texto("ide.api_url")
        .ok()
        .flatten()
        .or_else(|| {
            h.get(header::HOST)
                .and_then(|v| v.to_str().ok())
                .filter(|v| v.contains(':'))
                .map(|v| format!("http://{v}"))
        });
    ws.on_upgrade(move |sock| sessao(s, sock, api))
}

async fn mandar(sock: &mut WebSocket, v: Value) -> bool {
    sock.send(Message::Text(v.to_string().into())).await.is_ok()
}

async fn sessao(s: ApiState, mut sock: WebSocket, api: Option<String>) {
    // 1) o token, na primeira mensagem e dentro do prazo.
    let primeira = tokio::time::timeout(PRAZO_DO_TOKEN, sock.recv()).await;
    let token = match primeira {
        Ok(Some(Ok(Message::Text(t)))) => match serde_json::from_str::<Pedido>(t.as_str()) {
            Ok(Pedido::Auth { token }) => token,
            _ => String::new(),
        },
        _ => String::new(),
    };
    let mut h = HeaderMap::new();
    if let Ok(v) = format!("Bearer {token}").parse() {
        h.insert(header::AUTHORIZATION, v);
    }
    if crate::rbac::conferir_rota(&s, axum::http::Method::GET, ROTA_TERMINAL, &h).is_err() {
        let _ = mandar(
            &mut sock,
            json!({"ev": "erro", "erro": "token ausente ou invalido"}),
        )
        .await;
        let _ = sock.close().await;
        return;
    }
    // 2) uma sessao por usuario: a anterior recebe a ordem de fechar.
    let chave = chave_do_usuario(&token);
    let minha = Arc::new(AtomicBool::new(false));
    if let Ok(mut v) = vivas().lock()
        && let Some(antiga) = v.insert(chave.clone(), minha.clone())
    {
        antiga.store(true, Ordering::SeqCst);
    }
    let _ = mandar(&mut sock, json!({"ev": "pronto"})).await;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Atualizacao>();
    let mut term: Option<(String, Terminal)> = None;
    let mut relogio = tokio::time::interval(Duration::from_millis(250));
    loop {
        tokio::select! {
            m = sock.recv() => {
                let Some(Ok(m)) = m else { break };
                let texto = match m {
                    Message::Text(t) => t,
                    Message::Close(_) => break,
                    _ => continue,
                };
                let pedido = match serde_json::from_str::<Pedido>(texto.as_str()) {
                    Ok(p) => p,
                    Err(e) => {
                        if !mandar(&mut sock, json!({"ev": "erro", "erro": format!("pedido invalido: {e}")})).await { break }
                        continue;
                    }
                };
                match pedido {
                    Pedido::Auth { .. } => {}
                    Pedido::Abrir { programa, colunas, linhas } => {
                        let r = abrir(&programa, colunas, linhas, &s.token, api.as_deref(), tx.clone());
                        match r {
                            Ok((id, t)) => {
                                let aberto = json!({"ev": "aberto", "id": id, "pid": t.pid(), "programa": "helix",
                                                    "cwd": t_cwd()});
                                term = Some((id, t));
                                if !mandar(&mut sock, aberto).await { break }
                            }
                            Err(e) => { if !mandar(&mut sock, json!({"ev": "erro", "erro": e})).await { break } }
                        }
                    }
                    Pedido::Tecla { tecla } => { if let Some((_, t)) = &term { let _ = t.tecla(&tecla); } }
                    Pedido::Colar { texto } => { if let Some((_, t)) = &term { let _ = t.colar(&texto); } }
                    Pedido::Texto { texto } => { if let Some((_, t)) = &term { let _ = t.escrever(texto.as_bytes()); } }
                    Pedido::Redimensionar { colunas, linhas } => {
                        if let Some((_, t)) = &term { let _ = t.redimensionar(Tamanho { colunas, linhas }); }
                    }
                    Pedido::Rolar { linhas } => { if let Some((_, t)) = &term { t.rolar(linhas); } }
                    Pedido::Fechar => { term = None; }
                }
            }
            g = rx.recv() => {
                let Some(g) = g else { break };
                let Some((id, _)) = &term else { continue };
                let mut v = serde_json::to_value(&g).unwrap_or(Value::Null);
                v["ev"] = json!("grade");
                v["id"] = json!(id);
                if !mandar(&mut sock, v).await { break }
            }
            _ = relogio.tick() => {
                if minha.load(Ordering::SeqCst) {
                    let _ = mandar(&mut sock, json!({"ev": "erro", "erro": "sessao substituida por outra conexao do mesmo usuario"})).await;
                    break;
                }
            }
        }
    }
    // Soltar o terminal mata o hx; a entrada do mapa so sai se ainda for a minha.
    drop(term);
    if let Ok(mut v) = vivas().lock()
        && v.get(&chave).is_some_and(|b| Arc::ptr_eq(b, &minha))
    {
        v.remove(&chave);
    }
    let _ = sock.close().await;
}

fn t_cwd() -> String {
    pasta_do_projeto()
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

/// So o Helix, pelo nome: a tela nao escolhe executavel.
fn abrir(
    programa: &str,
    colunas: u16,
    linhas: u16,
    token: &str,
    api: Option<&str>,
    tx: tokio::sync::mpsc::UnboundedSender<Atualizacao>,
) -> Result<(String, Terminal), String> {
    if programa != "helix" {
        return Err(
            "pelo navegador so o Helix abre; o terminal bash fica no aplicativo de mesa".into(),
        );
    }
    let cwd = pasta_do_projeto()?;
    let raizes = phxclaw_workspace::raizes(&cwd.join(".phxclaw"))?;
    let mut env = vec![];
    if !raizes.is_empty() {
        let lista = phxclaw_workspace::variavel(&raizes);
        env.push(("PHXCLAW_RAIZES".into(), lista));
    }
    // A completacao por IA do snippet-ls fala com este agente pelo mesmo token de quem
    // abriu o terminal: e o token dele, nao um segredo novo.
    if let Some(api) = api {
        let url = format!("{api}/v1/ide/completar");
        env.push(("PHXCLAW_IA_COMPLETAR".into(), url));
        env.push(("PHXCLAW_API_TOKEN".into(), token.to_string()));
    }
    let pedido = crate::config::texto("desktop.hx")
        .ok()
        .flatten()
        .map(PathBuf::from);
    let p = phxclaw_terminal::helix::programa(&cwd, pedido, env)?;
    // No bwrap, com a pasta do agente mascarada. Sem bwrap nao abre: fora do sandbox o hx le
    // qualquer caminho absoluto do hospedeiro (a pasta do agente onde quer que more), entao
    // nao ha caso em que abri-lo assim seja seguro.
    let p = crate::processo::terminal_no_bwrap(p, &raizes)?;
    let id = phxclaw_types::new_uuid_v7().to_string();
    let t = Terminal::abrir(p, Tamanho { colunas, linhas }, move |g| {
        let _ = tx.send(g);
    })
    .map_err(|e| e.to_string())?;
    Ok((id, t))
}

// ---------------------------------------------------------------- simbolos

#[derive(Deserialize)]
pub struct ConsultaDeSimbolos {
    arquivo: String,
    /// Segundos de espera pelo servidor (padrao 30; teto 120).
    prazo: Option<u64>,
}

fn lsp() -> Option<Arc<crate::lsp::Lsp>> {
    static L: OnceLock<Option<Arc<crate::lsp::Lsp>>> = OnceLock::new();
    L.get_or_init(crate::lsp::Lsp::do_hospedeiro).clone()
}

/// Um simbolo como a barra o mostra: nome, tipo em texto, linha (a partir de 1) e filhos.
fn simbolo_da_barra(s: &Value) -> Value {
    let linha = s["selectionRange"]["start"]["line"]
        .as_u64()
        .or_else(|| s["location"]["range"]["start"]["line"].as_u64())
        .unwrap_or(0)
        + 1;
    let filhos: Vec<Value> = s["children"]
        .as_array()
        .map(|f| f.iter().map(simbolo_da_barra).collect())
        .unwrap_or_default();
    json!({
        "nome": s["name"].as_str().unwrap_or("?"),
        "tipo": crate::lsp::tipo_de_simbolo(s["kind"].as_u64().unwrap_or(0)),
        "linha": linha,
        "filhos": filhos,
    })
}

async fn simbolos(
    State(s): State<ApiState>,
    h: HeaderMap,
    Query(q): Query<ConsultaDeSimbolos>,
) -> Result<Json<Value>, Erro> {
    auth(&s, &h)?;
    let Some(l) = lsp() else {
        return Err(erro(
            StatusCode::SERVICE_UNAVAILABLE,
            "nenhum servidor de linguagem neste hospedeiro (bwrap ou servidores ausentes)",
        ));
    };
    let cwd = pasta_do_projeto().map_err(|e| erro(StatusCode::SERVICE_UNAVAILABLE, e))?;
    let prazo = Duration::from_secs(q.prazo.unwrap_or(30).clamp(1, 120));
    let r = l
        .simbolos_do_arquivo(&cwd, &q.arquivo, prazo)
        .await
        .map_err(|e| erro(StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;
    let Some(r) = r else {
        return Err(erro(
            StatusCode::GATEWAY_TIMEOUT,
            format!(
                "o servidor de linguagem ainda esta indexando o projeto ({} s)",
                prazo.as_secs()
            ),
        ));
    };
    let lista: Vec<Value> = r
        .as_array()
        .map(|a| a.iter().map(simbolo_da_barra).collect())
        .unwrap_or_default();
    Ok(Json(json!({"arquivo": q.arquivo, "simbolos": lista})))
}

// ---------------------------------------------------------------- arquivo (minimapa)

/// Teto do arquivo que o minimapa le: 2 MiB. O minimapa desenha 1 px por caractere; acima
/// disso nao e codigo-fonte que alguem le pelo mapa, e a resposta inteira iria pela rede a
/// cada troca de arquivo.
pub const TETO_DO_ARQUIVO: u64 = 2 * 1024 * 1024;

#[derive(Deserialize)]
pub struct ConsultaDeArquivo {
    #[serde(alias = "arquivo")]
    caminho: String,
}

async fn arquivo(
    State(s): State<ApiState>,
    h: HeaderMap,
    Query(q): Query<ConsultaDeArquivo>,
) -> Result<Json<Value>, Erro> {
    auth(&s, &h)?;
    let cwd = pasta_do_projeto().map_err(|e| erro(StatusCode::SERVICE_UNAVAILABLE, e))?;
    // Disco fora do laco assincrono: um disco lento (ou o que sobrar de espera no `open`)
    // prende uma thread do pool de bloqueio, nunca o worker que atende as outras rotas.
    let caminho = q.caminho.clone();
    let bytes = tokio::task::spawn_blocking(move || ler_para_o_minimapa(&cwd, &caminho))
        .await
        .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))??;
    if bytes.iter().take(8192).any(|b| *b == 0) {
        return Err(erro(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            format!("{}: arquivo binario, sem minimapa", q.caminho),
        ));
    }
    let texto = String::from_utf8_lossy(&bytes);
    Ok(Json(json!({
        "caminho": q.caminho,
        "bytes": bytes.len(),
        "linhas": texto.lines().count(),
        "texto": texto,
    })))
}

/// `O_NONBLOCK` sem crate nova: o valor do `asm-generic/fcntl.h` vale nas arquiteturas em
/// que o agente roda; alpha, mips, parisc e sparc tem outro numero e caem no 0 (abrir
/// comum), com a conferencia do tipo pelo descritor continuando de pe.
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "x86_64",
        target_arch = "x86",
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "riscv64",
        target_arch = "powerpc64",
        target_arch = "s390x",
        target_arch = "loongarch64"
    )
))]
const SEM_ESPERA: i32 = 0o4000;
#[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd"))]
const SEM_ESPERA: i32 = 0x0004;
#[cfg(not(any(
    all(
        target_os = "linux",
        any(
            target_arch = "x86_64",
            target_arch = "x86",
            target_arch = "aarch64",
            target_arch = "arm",
            target_arch = "riscv64",
            target_arch = "powerpc64",
            target_arch = "s390x",
            target_arch = "loongarch64"
        )
    ),
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd"
)))]
const SEM_ESPERA: i32 = 0;

/// Le o arquivo do minimapa abrindo UMA vez e conferindo o que foi aberto, nao o nome.
///
/// O nome passa pelo `confine` (pasta do projeto, raizes e a area reservada do agente), mas
/// entre conferir o nome e abrir o arquivo o caminho pode virar outro -- um symlink trocado
/// ou um FIFO no lugar. Por isso: abre sem esperar escritor (um FIFO abriria e prenderia a
/// thread ate alguem escrever), pergunta ao kernel o caminho REAL do descritor e o confere
/// de novo, e so le se o descritor for arquivo comum. A recusa nao diz o motivo: dizer
/// «area do agente» confirmaria o que existe la dentro.
fn ler_para_o_minimapa(cwd: &std::path::Path, caminho: &str) -> Result<Vec<u8>, Erro> {
    use std::io::Read;
    let recusado = || {
        erro(
            StatusCode::FORBIDDEN,
            format!("{caminho}: caminho recusado"),
        )
    };
    let alvo = crate::tarefa::confine(cwd, caminho).map_err(|_| recusado())?;
    let mut abrir = std::fs::OpenOptions::new();
    abrir.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        abrir.custom_flags(SEM_ESPERA);
    }
    let _ = SEM_ESPERA;
    let f = abrir.open(&alvo).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            erro(StatusCode::NOT_FOUND, format!("{caminho}: nao encontrado"))
        }
        _ => erro(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{caminho}: nao abriu"),
        ),
    })?;
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd;
        let real = std::fs::read_link(format!("/proc/self/fd/{}", f.as_raw_fd()))
            .map_err(|_| recusado())?;
        crate::tarefa::confere_aberto(cwd, &real).map_err(|_| recusado())?;
    }
    let m = f.metadata().map_err(|_| recusado())?;
    if !m.is_file() {
        return Err(erro(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("{caminho}: nao e arquivo"),
        ));
    }
    // O teto vale no que se LE, nao so no tamanho dito antes: o arquivo pode crescer entre
    // o `metadata` e a leitura.
    let mut bytes = Vec::new();
    f.take(TETO_DO_ARQUIVO + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| erro(StatusCode::UNPROCESSABLE_ENTITY, format!("{caminho}: {e}")))?;
    if bytes.len() as u64 > TETO_DO_ARQUIVO {
        return Err(erro(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("{caminho}: acima do teto de {TETO_DO_ARQUIVO} bytes do minimapa"),
        ));
    }
    Ok(bytes)
}

// ---------------------------------------------------------------- completar

#[derive(Deserialize)]
pub struct PedidoDeCompletacao {
    #[serde(default)]
    arquivo: String,
    #[serde(default)]
    linguagem: String,
    antes: String,
    #[serde(default)]
    depois: String,
}

/// Tetos da completacao, do catalogo (`ide.ia_tokens`, padrao 64; `ide.ia_ms`, padrao
/// 6000): curtos de proposito, porque completacao que demora nao e completacao.
fn tetos() -> (u32, Duration) {
    let n = |k: &str, p: u64| {
        crate::config::valor(k)
            .ok()
            .flatten()
            .and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .unwrap_or(p)
    };
    (
        n("ide.ia_tokens", 64).clamp(8, 1024) as u32,
        Duration::from_millis(n("ide.ia_ms", 6000).clamp(200, 60_000)),
    )
}

/// Tira a cerca de codigo que alguns modelos poem mesmo quando se pede para nao por.
pub fn sem_cerca(t: &str) -> String {
    let t = t.trim_matches('\n');
    let Some(resto) = t.strip_prefix("```") else {
        return t.to_string();
    };
    let corpo = resto.split_once('\n').map(|(_, c)| c).unwrap_or("");
    corpo
        .strip_suffix("```")
        .unwrap_or(corpo)
        .trim_end_matches('\n')
        .to_string()
}

/// Os ultimos `n` caracteres: o contexto que vai ao modelo e o que esta perto do cursor.
fn cauda(t: &str, n: usize) -> &str {
    let i = t
        .char_indices()
        .rev()
        .nth(n.saturating_sub(1))
        .map(|(i, _)| i)
        .unwrap_or(0);
    &t[i..]
}

fn cabeca(t: &str, n: usize) -> &str {
    let i = t.char_indices().nth(n).map(|(i, _)| i).unwrap_or(t.len());
    &t[..i]
}

async fn completar(
    State(s): State<ApiState>,
    h: HeaderMap,
    Json(p): Json<PedidoDeCompletacao>,
) -> Result<Json<Value>, Erro> {
    auth(&s, &h)?;
    let agente = (s.factory)(&s.default_model)
        .map_err(|e| erro(StatusCode::SERVICE_UNAVAILABLE, format!("modelo: {e}")))?;
    let (tokens, prazo) = tetos();
    let mensagens = vec![
        Msg::system(
            "You complete source code at the cursor. Reply ONLY with the code that \
             continues from the cursor: no explanation, no code fence, no repetition \
             of the text before the cursor.",
        ),
        Msg::user(format!(
            "File: {}\nLanguage: {}\n--- before the cursor ---\n{}\n--- after the cursor ---\n{}\n--- continue at the cursor ---",
            p.arquivo,
            p.linguagem,
            cauda(&p.antes, 4000),
            cabeca(&p.depois, 1000)
        )),
    ];
    let opcoes = LlmOptions {
        max_output_tokens: tokens,
        temperature: 0.1,
    };
    let r = tokio::time::timeout(prazo, agente.llm.chat(&mensagens, &[], &opcoes))
        .await
        .map_err(|_| {
            erro(
                StatusCode::GATEWAY_TIMEOUT,
                format!("o modelo nao respondeu em {} ms", prazo.as_millis()),
            )
        })?
        .map_err(|e| erro(StatusCode::BAD_GATEWAY, format!("modelo: {e}")))?;
    Ok(Json(
        json!({"texto": sem_cerca(&r.content), "modelo": r.model}),
    ))
}

// ---------------------------------------------------------------- explorador de testes

/// Os motores de teste do hospedeiro, sondados uma vez (a CLI sonda a cada chamada; a API
/// atende muitas).
fn explorador() -> Option<&'static crate::testes::ExploradorDeTestes> {
    static E: OnceLock<Option<crate::testes::ExploradorDeTestes>> = OnceLock::new();
    E.get_or_init(|| {
        crate::arquivos::achar_bwrap().and_then(crate::testes::ExploradorDeTestes::detectar)
    })
    .as_ref()
}

/// Prazo de uma listagem ou corrida de testes pela API: o mesmo da CLI.
const PRAZO_DOS_TESTES: Duration = Duration::from_secs(900);

#[derive(Deserialize, Default)]
pub struct PedidoDeTestes {
    /// Pasta do projeto a listar, relativa a pasta do IDE (padrao `.`).
    #[serde(default)]
    caminho: Option<String>,
    #[serde(default)]
    linguagem: Option<String>,
    /// So no rodar: o no (`CRATE`, `CRATE/modulo`, `CRATE/modulo::teste`; Python: o nodeid).
    #[serde(default)]
    no: Option<String>,
}

fn contexto_do_ide() -> Result<phxclaw_agent_core::ToolContext, Erro> {
    let cwd = pasta_do_projeto().map_err(|e| erro(StatusCode::SERVICE_UNAVAILABLE, e))?;
    Ok(crate::testes::contexto_da_cli(cwd, PRAZO_DOS_TESTES))
}

fn sem_motor() -> Erro {
    erro(
        StatusCode::SERVICE_UNAVAILABLE,
        "explorador de testes indisponivel: sem bwrap, ou nem toolchain Rust nem Python no hospedeiro",
    )
}

async fn testes_listar(
    State(s): State<ApiState>,
    h: HeaderMap,
    Query(q): Query<PedidoDeTestes>,
) -> Result<Json<Value>, Erro> {
    auth(&s, &h)?;
    let e = explorador().ok_or_else(sem_motor)?;
    let ctx = contexto_do_ide()?;
    let v = e
        .listar(
            &ctx,
            q.caminho.as_deref().unwrap_or("."),
            q.linguagem.as_deref(),
        )
        .await
        .map_err(|e| erro(StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;
    Ok(Json(v))
}

async fn testes_rodar(
    State(s): State<ApiState>,
    h: HeaderMap,
    Json(p): Json<PedidoDeTestes>,
) -> Result<Json<Value>, Erro> {
    auth(&s, &h)?;
    let e = explorador().ok_or_else(sem_motor)?;
    let no =
        p.no.as_deref()
            .filter(|n| !n.trim().is_empty())
            .ok_or_else(|| erro(StatusCode::BAD_REQUEST, "falta 'no'"))?;
    let ctx = contexto_do_ide()?;
    let v = e
        .rodar(
            &ctx,
            p.caminho.as_deref().unwrap_or("."),
            no,
            p.linguagem.as_deref(),
        )
        .await
        .map_err(|e| erro(StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;
    Ok(Json(v))
}

// ---------------------------------------------------------------- loja de plugins

#[derive(Deserialize, Default)]
pub struct PedidoDaLoja {
    #[serde(default)]
    busca: Option<String>,
    #[serde(default)]
    nome: Option<String>,
}

/// A loja da configuracao, ou a recusa que diz a chave que falta (a mesma frase da CLI).
fn loja() -> Result<crate::loja::Loja, Erro> {
    crate::loja::Loja::da_configuracao()
        .map_err(|e| erro(StatusCode::SERVICE_UNAVAILABLE, e))?
        .ok_or_else(|| {
            erro(
                StatusCode::SERVICE_UNAVAILABLE,
                "pacotes.catalogo (PHXCLAW_PACOTES_CATALOGO) nao definido: nao ha loja",
            )
        })
}

async fn plugins_catalogo(
    State(s): State<ApiState>,
    h: HeaderMap,
    Query(q): Query<PedidoDaLoja>,
) -> Result<Json<Value>, Erro> {
    auth(&s, &h)?;
    let fichas = loja()?
        .fichas(q.busca.as_deref())
        .await
        .map_err(|e| erro(StatusCode::BAD_GATEWAY, e))?;
    Ok(Json(json!({"plugins": fichas})))
}

async fn plugins_instalar(
    State(s): State<ApiState>,
    h: HeaderMap,
    Json(p): Json<PedidoDaLoja>,
) -> Result<Json<Value>, Erro> {
    auth(&s, &h)?;
    let nome = p
        .nome
        .as_deref()
        .filter(|n| !n.trim().is_empty())
        .ok_or_else(|| erro(StatusCode::BAD_REQUEST, "falta 'nome'"))?;
    let i = loja()?
        .instalar(nome)
        .await
        .map_err(|e| erro(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(
        json!({"nome": i.nome, "versao": i.versao, "caminho": i.caminho.display().to_string()}),
    ))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_cerca_sai_e_o_codigo_fica() {
        assert_eq!(sem_cerca("```rust\nlet x = 1;\n```"), "let x = 1;");
        assert_eq!(sem_cerca("let x = 1;\n"), "let x = 1;");
        assert_eq!(sem_cerca("```\n```"), "");
    }

    #[test]
    fn o_contexto_e_recortado_perto_do_cursor() {
        assert_eq!(cauda("abcdef", 3), "def");
        assert_eq!(cauda("ab", 3), "ab");
        assert_eq!(cabeca("abcdef", 2), "ab");
        assert_eq!(cabeca("é", 5), "é");
    }

    #[test]
    fn so_o_helix_abre_pelo_navegador() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let e = abrir("bash", 80, 24, "t", None, tx).err().expect("recusa");
        assert!(e.contains("so o Helix"), "{e}");
    }

    #[test]
    fn a_chave_da_sessao_nunca_e_o_token() {
        let k = chave_do_usuario("segredo-do-token");
        assert_ne!(k, "segredo-do-token");
        assert!(!k.contains("segredo"));
        assert_eq!(k.len(), 64);
    }
}

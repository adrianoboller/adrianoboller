//! O endpoint MCP sobre HTTP -- provado PELO SOQUETE.
//!
//! Os testes unitarios do `mcp.rs` chamam `Ponte::atender` e conferem a
//! traducao; os do `mcp_stdio.rs` provam o transporte pelo CANO de um processo.
//! Este arquivo prova o terceiro caminho: o mesmo JSON-RPC 2.0 entrando pela
//! porta REST, por HTTP, como o N8n e o Claude Code o falam. E a licao do
//! BULKINSERT outra vez -- o que depende do sistema operacional (aqui, um POST
//! HTTP num soquete de verdade) se prova contra o sistema operacional, e nao
//! por teste unitario.
//!
//! # A prova real que este arquivo carrega (papel F)
//!
//! Nos DOIS sentidos, como manda a pétrea:
//!
//! * uma LEITURA pelo MCP devolve os dados que foram gravados; e
//! * uma ESCRITA pelo MCP e RECUSADA -- e a recusa e a fronteira da fase 1.
//!   `escrita_pelo_mcp_http_e_recusada` FALHA se alguem ligar a escrita nesta
//!   porta (trocar `Ponte::nova` por `.com_escrita(true)` no `mcp_http`): o
//!   `phx_inserir` deixaria de vir com o erro "somente de leitura" e a linha
//!   proibida apareceria no `varrer` seguinte.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

/// O token do PROTOCOLO -- o que abre a porta de dados.
const TOKEN: &str = "tok-protocolo-mcp";
/// O segredo da porta REST -- o `Bearer` que o cliente HTTP apresenta. Ele
/// SUBSTITUI o token do protocolo nesta porta, entao os dois sao diferentes de
/// proposito: se fossem iguais, o teste do 401 nao provaria nada.
const BEARER: &str = "bearer-do-rest";

fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn pasta(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("mcp-http-{nome}"))
}

/// Sobe um servidor com a porta de dados E a porta REST ligadas.
fn subir(base: &std::path::Path, dados: u16, rest: u16) -> Arc<Servidor> {
    let mut c = Config {
        bind: format!("127.0.0.1:{dados}"),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    c.web.ligado = false;
    c.rest.ligado = true;
    c.rest.bind = format!("127.0.0.1:{rest}");
    c.rest.token = BEARER.into();
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    esperar_porta(dados).expect("a porta de dados nao subiu");
    esperar_porta(rest).expect("a porta REST nao subiu");
    s
}

fn esperar_porta(porta: u16) -> Result<(), String> {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(3);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(format!("a porta {porta} nao abriu em 3 s"))
}

/// Um pedido pela porta de DADOS (JSON Lines), so para montar o cenario.
fn dados(porta: u16, linha: &str) -> String {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    let _ = writeln!(escrita, "{linha}");
    let mut r = String::new();
    leitor.read_line(&mut r).unwrap();
    r
}

/// Um pedido HTTP cru, e a resposta como (codigo, corpo).
///
/// `bearer` `None` NAO manda o cabecalho `Authorization` -- e o caso de quem
/// bate na porta sem a chave dela.
fn http(
    porta: u16,
    metodo: &str,
    caminho: &str,
    bearer: Option<&str>,
    corpo: &str,
) -> (u16, String) {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let autorizacao = match bearer {
        Some(b) => format!("Authorization: Bearer {b}\r\n"),
        None => String::new(),
    };
    let _ = write!(
        escrita,
        "{metodo} {caminho} HTTP/1.1\r\nHost: localhost\r\n{autorizacao}\
         Content-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{corpo}",
        corpo.len()
    );
    let mut resposta = String::new();
    BufReader::new(fluxo).read_to_string(&mut resposta).unwrap();
    let codigo = resposta
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .unwrap_or_else(|| panic!("resposta HTTP sem codigo: {resposta}"));
    let corpo = resposta
        .split_once("\r\n\r\n")
        .map(|(_, c)| c.to_string())
        .unwrap_or_default();
    (codigo, corpo)
}

/// Um POST /mcp com o `Bearer` certo -- o caminho feliz do cliente MCP.
fn mcp(porta: u16, corpo: &str) -> (u16, String) {
    http(porta, "POST", "/mcp", Some(BEARER), corpo)
}

/// Monta o cenario: um banco com uma tabela e duas linhas de verdade.
fn semear(dados_porta: u16) {
    let t = format!("\"token\":\"{TOKEN}\"");
    let _ = dados(
        dados_porta,
        &format!("{{{t},\"op\":\"criar_database\",\"database\":\"loja\"}}"),
    );
    let r = dados(
        dados_porta,
        &format!(
            "{{{t},\"op\":\"criar_tabela\",\"database\":\"loja\",\"tabela\":\"clientes\",\
             \"colunas\":[{{\"nome\":\"id\",\"tipo\":\"Int4\",\"obrigatoria\":true}},\
             {{\"nome\":\"nome\",\"tipo\":\"Str(40)\"}},{{\"nome\":\"cidade\",\"tipo\":\"Str(40)\"}}]}}"
        ),
    );
    assert!(r.contains("\"ok\":true"), "criar_tabela: {r}");
    for (id, nome, cidade) in [(1, "Maria", "Blumenau"), (2, "Joao", "Joinville")] {
        let r = dados(
            dados_porta,
            &format!(
                "{{{t},\"op\":\"inserir\",\"database\":\"loja\",\"tabela\":\"clientes\",\
                 \"valores\":{{\"id\":{id},\"nome\":\"{nome}\",\"cidade\":\"{cidade}\"}}}}"
            ),
        );
        assert!(r.contains("\"ok\":true"), "inserir {nome}: {r}");
    }
}

/// O aperto de mao completo por HTTP: initialize, a notificacao (que responde
/// 202 SEM corpo), tools/list, e uma LEITURA que devolve os dados.
#[test]
fn o_aperto_de_mao_e_a_leitura_atravessam_o_http() {
    let base = pasta("aperto");
    let (d, r) = (porta_livre(), porta_livre());
    let _s = subir(&base, d, r);
    semear(d);

    // initialize.
    let (codigo, corpo) = mcp(
        r,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
    );
    assert_eq!(codigo, 200, "{corpo}");
    let init = Json::analisar(&corpo).unwrap();
    let res = init.campo("result").expect("initialize falhou");
    assert_eq!(res.texto_ou("protocolVersion", ""), "2025-06-18");
    assert_eq!(
        res.campo("serverInfo").unwrap().texto_ou("name", ""),
        "phxsql"
    );

    // A notificacao NAO recebe corpo, e o codigo e 202 -- responder qualquer
    // coisa aqui quebraria o cliente no `notifications/initialized`.
    let (codigo, corpo) = mcp(
        r,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    );
    assert_eq!(codigo, 202, "notificacao tem de responder 202");
    assert!(
        corpo.is_empty(),
        "notificacao nao pode ter corpo: {corpo:?}"
    );

    // tools/list: as de leitura aparecem, as de escrita NAO.
    let (codigo, corpo) = mcp(r, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
    assert_eq!(codigo, 200, "{corpo}");
    let nomes: Vec<String> = Json::analisar(&corpo)
        .unwrap()
        .campo("result")
        .unwrap()
        .campo("tools")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|t| t.texto_ou("name", "").to_string())
        .collect();
    assert!(nomes.contains(&"phx_varrer".to_string()), "{nomes:?}");
    assert!(nomes.contains(&"phx_esquema".to_string()), "{nomes:?}");
    assert!(
        !nomes.contains(&"phx_inserir".to_string()),
        "somente leitura: a escrita nao pode ser nem anunciada -- {nomes:?}"
    );

    // A LEITURA devolve os dados gravados.
    let (codigo, corpo) = mcp(
        r,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call",
            "params":{"name":"phx_varrer","arguments":{"database":"loja","tabela":"clientes","max":100}}}"#,
    );
    assert_eq!(codigo, 200, "{corpo}");
    let res = Json::analisar(&corpo).unwrap();
    let res = res.campo("result").unwrap();
    assert_eq!(res.campo("isError").unwrap().booleano(), Some(false));
    let texto = res.campo("content").unwrap().lista().unwrap()[0]
        .texto_ou("text", "")
        .to_string();
    assert!(
        texto.contains("Maria"),
        "a leitura nao trouxe os dados: {texto}"
    );
    assert!(texto.contains("Blumenau"), "{texto}");
    assert!(texto.contains("Joao"), "{texto}");
}

/// **A fronteira da fase 1, nos dois sentidos.** A escrita e recusada com o
/// motivo, e a linha proibida NAO fica no banco. Ligue a escrita nesta porta e
/// as duas asercoes caem.
#[test]
fn escrita_pelo_mcp_http_e_recusada() {
    let base = pasta("recusa");
    let (d, r) = (porta_livre(), porta_livre());
    let _s = subir(&base, d, r);
    semear(d);

    let (codigo, corpo) = mcp(
        r,
        r#"{"jsonrpc":"2.0","id":9,"method":"tools/call",
            "params":{"name":"phx_inserir",
                      "arguments":{"database":"loja","tabela":"clientes",
                                   "valores":{"id":99,"nome":"NaoDeviaExistir","cidade":"Nenhures"}}}}"#,
    );
    assert_eq!(codigo, 200, "{corpo}");
    let res = Json::analisar(&corpo).unwrap();
    let msg = res
        .campo("error")
        .expect("a escrita passou -- a fronteira da fase 1 caiu")
        .texto_ou("message", "")
        .to_string();
    assert!(msg.contains("somente de leitura"), "{msg}");

    // E a prova de que a escrita nao aconteceu por outro caminho: a linha
    // proibida nao esta no banco. So o erro nao bastaria -- ele poderia mentir.
    let (_, corpo) = mcp(
        r,
        r#"{"jsonrpc":"2.0","id":10,"method":"tools/call",
            "params":{"name":"phx_varrer","arguments":{"database":"loja","tabela":"clientes","max":100}}}"#,
    );
    let texto = Json::analisar(&corpo)
        .unwrap()
        .campo("result")
        .unwrap()
        .campo("content")
        .unwrap()
        .lista()
        .unwrap()[0]
        .texto_ou("text", "")
        .to_string();
    assert!(
        !texto.contains("NaoDeviaExistir"),
        "a escrita recusada mesmo assim gravou: {texto}"
    );
}

/// O `Bearer` errado nao abre a porta -- 401, e a mesma regra que o `/v1/<op>`
/// sempre teve, agora num lugar so (`token_do_rest`).
#[test]
fn bearer_errado_e_recusado_no_mcp_e_no_rest() {
    let base = pasta("bearer");
    let (d, r) = (porta_livre(), porta_livre());
    let _s = subir(&base, d, r);

    // MCP com o Bearer errado.
    let (codigo, _) = http(
        r,
        "POST",
        "/mcp",
        Some("chave-roubada"),
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
    );
    assert_eq!(codigo, 401, "Bearer errado tinha de ser recusado no /mcp");

    // MCP sem Bearer nenhum.
    let (codigo, _) = http(
        r,
        "POST",
        "/mcp",
        None,
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
    );
    assert_eq!(codigo, 401, "sem Bearer tinha de ser recusado no /mcp");

    // A REDE DE SEGURANCA DO REFATOR: o `/v1/<op>` continua autenticando pelo
    // MESMO helper. Com o Bearer certo passa; com o errado, 401. Se a extracao
    // do `token_do_rest` tivesse quebrado a porta REST, estas duas cairiam.
    let (codigo, corpo) = http(r, "POST", "/v1/ping", Some(BEARER), "{}");
    assert_eq!(codigo, 200, "o /v1/ping parou de autenticar: {corpo}");
    assert!(corpo.contains("\"ok\":true"), "{corpo}");
    let (codigo, _) = http(r, "POST", "/v1/ping", Some("chave-roubada"), "{}");
    assert_eq!(codigo, 401, "o /v1/ping parou de recusar o Bearer errado");
}

/// Um GET no endpoint nao abre SSE -- responde 405, e nao pendura o cliente
/// esperando um fluxo que nunca vem.
#[test]
fn get_no_mcp_responde_405_sem_sse() {
    let base = pasta("get");
    let (d, r) = (porta_livre(), porta_livre());
    let _s = subir(&base, d, r);
    let (codigo, _) = http(r, "GET", "/mcp", Some(BEARER), "");
    assert_eq!(codigo, 405, "GET /mcp tinha de ser 405 (sem SSE)");
}

/// **O defeito do LOTE, provado pelo soquete.** Um lote JSON-RPC (array) recebe
/// um erro com corpo -- e nao o 202 sem corpo que o silencio do defeito
/// produziria, deixando o cliente pendurado. Reponha o silencio no `atender`
/// (tire o guarda do array) e este teste cai: o codigo viraria 202 e nao
/// haveria `error` para ler.
#[test]
fn um_lote_jsonrpc_pelo_http_recebe_erro_e_nao_silencio() {
    let base = pasta("lote");
    let (d, r) = (porta_livre(), porta_livre());
    let _s = subir(&base, d, r);
    let (codigo, corpo) = mcp(
        r,
        r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","id":2,"method":"ping"}]"#,
    );
    assert_eq!(
        codigo, 200,
        "um lote tem de receber resposta, nao 202 mudo: {corpo}"
    );
    let erro = Json::analisar(&corpo).unwrap();
    assert_eq!(
        erro.campo("error")
            .unwrap()
            .campo("code")
            .unwrap()
            .inteiro(),
        Some(-32_600),
        "{corpo}"
    );
}

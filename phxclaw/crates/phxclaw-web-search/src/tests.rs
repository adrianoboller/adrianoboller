use super::*;
use phxclaw_egress_broker::EgressPolicy;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

// ------------------------------------------------------------ parse sem rede

#[test]
fn ddg_html_decodifica_uddg_pula_anuncio_e_repetido() {
    let hits = parse_duckduckgo(include_str!("../fixtures/ddg.html"), 10);
    let urls: Vec<_> = hits.iter().map(|h| h.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://www.rust-lang.org/",
            "https://en.wikipedia.org/wiki/Rust_(programming_language)?a=1&b=2",
            "https://doc.rust-lang.org/book/",
        ]
    );
    assert_eq!(hits[0].title, "Rust Programming Language");
    assert_eq!(
        hits[0].snippet,
        "A language empowering everyone to build reliable and efficient software & tools."
    );
    assert_eq!(
        hits[1].snippet,
        "Rust is a general-purpose programming language emphasizing performance."
    );
}

#[test]
fn ddg_respeita_o_maximo() {
    let hits = parse_duckduckgo(include_str!("../fixtures/ddg.html"), 1);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].url, "https://www.rust-lang.org/");
}

#[test]
fn ddg_lite_tambem_se_le() {
    let hits = parse_duckduckgo(include_str!("../fixtures/ddg_lite.html"), 10);
    assert_eq!(hits.len(), 2, "{hits:?}");
    assert_eq!(hits[0].url, "https://www.rust-lang.org/learn");
    assert_eq!(hits[0].title, "Learn Rust");
    assert_eq!(hits[0].snippet, "Get started with Rust today.");
    assert_eq!(hits[1].snippet, "The Rust community's crate registry");
}

#[test]
fn ddg_target_casos() {
    assert_eq!(
        ddg_target("//duckduckgo.com/l/?uddg=https%3A%2F%2Fa.com%2Fx%3Fy%3D1&rut=z").as_deref(),
        Some("https://a.com/x?y=1")
    );
    assert_eq!(
        ddg_target("https://duckduckgo.com/y.js?ad_domain=a.com"),
        None
    );
    assert_eq!(
        ddg_target("//duckduckgo.com/l/?uddg=javascript%3Aalert(1)"),
        None
    );
    assert_eq!(
        ddg_target("https://b.org/").as_deref(),
        Some("https://b.org/")
    );
}

#[test]
fn desafio_anti_robo_se_reconhece() {
    assert!(is_ddg_challenge(
        "<div class=\"anomaly-modal__title\">Unfortunately, bots use DuckDuckGo too.</div>"
    ));
    assert!(!is_ddg_challenge(include_str!("../fixtures/ddg.html")));
}

#[test]
fn searxng_json() {
    let hits = parse_searxng(include_str!("../fixtures/searxng.json"), 10).unwrap();
    assert_eq!(hits.len(), 2, "{hits:?}");
    assert_eq!(hits[0].url, "https://www.rust-lang.org/");
    assert_eq!(hits[0].snippet, "A language empowering everyone.");
    assert_eq!(hits[1].snippet, "");
    assert!(matches!(
        parse_searxng("{\"erro\":1}", 5),
        Err(WebSearchError::Parse { .. })
    ));
}

#[test]
fn brave_json() {
    let hits = parse_brave(include_str!("../fixtures/brave.json"), 10).unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(
        hits[0].snippet,
        "A language empowering everyone to build reliable & efficient software."
    );
    assert_eq!(hits[1].title, "Rust \u{2014} Wikipedia");
    assert!(parse_brave("{\"type\":\"search\"}", 5).unwrap().is_empty());
}

fn broker_com(origens: &[&str]) -> Arc<EgressBroker> {
    let mut p = EgressPolicy {
        enabled: true,
        allow_http: true,
        ..EgressPolicy::default()
    };
    for o in origens {
        p.allowed_origins.insert((*o).into());
    }
    Arc::new(EgressBroker::new(p))
}

#[test]
fn escolha_do_backend_pelas_variaveis() {
    let b = broker_com(&[]);
    let so = |pares: &'static [(&'static str, &'static str)]| {
        move |k: &str| {
            pares
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        }
    };
    let s = from_vars(
        so(&[
            ("PHXCLAW_SEARXNG_URL", "https://searx.local/sub"),
            ("BRAVE_API_KEY", "k"),
        ]),
        b.clone(),
    )
    .unwrap();
    assert_eq!(s.name(), "searxng");
    assert_eq!(s.origins(), ["https://searx.local"]);
    let s = from_vars(so(&[("BRAVE_API_KEY", "k")]), b.clone()).unwrap();
    assert_eq!(s.name(), "brave");
    let s = from_vars(so(&[]), b.clone()).unwrap();
    assert_eq!(s.name(), "duckduckgo");
    assert_eq!(
        s.origins(),
        ["https://html.duckduckgo.com", "https://lite.duckduckgo.com"]
    );
    assert!(from_vars(so(&[("PHXCLAW_SEARXNG_URL", "nao e url")]), b).is_err());
}

#[test]
fn searxng_em_subcaminho_acrescenta_search() {
    let s = SearxngBackend::new(broker_com(&[]), "https://h.example/searx").unwrap();
    assert_eq!(s.endpoint.as_str(), "https://h.example/searx/search");
}

// ------------------------------------------------------------ servidor local

struct Servidor {
    origem: String,
    toques: Arc<AtomicUsize>,
    pedidos: Arc<Mutex<Vec<String>>>,
}

/// Servidor HTTP minimo em loopback: responde `(content-type, corpo)` a cada conexao e
/// guarda o pedido cru, para o teste conferir o que de fato saiu pelo fio.
fn servidor(content_type: &str, corpo: &str) -> Servidor {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let origem = format!("http://{}", l.local_addr().unwrap());
    let toques = Arc::new(AtomicUsize::new(0));
    let pedidos = Arc::new(Mutex::new(Vec::new()));
    let resposta = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
        corpo.len()
    );
    let (t, p) = (Arc::clone(&toques), Arc::clone(&pedidos));
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { break };
            t.fetch_add(1, Ordering::SeqCst);
            p.lock().unwrap().push(ler_pedido(&mut s));
            let _ = s.write_all(resposta.as_bytes());
        }
    });
    Servidor {
        origem,
        toques,
        pedidos,
    }
}

/// Le cabecalho e corpo pelo Content-Length: um `read` so pode parar antes do corpo do
/// POST, e o teste que confere o formulario passaria ou falharia por sorte.
fn ler_pedido(s: &mut std::net::TcpStream) -> String {
    let mut buf = Vec::new();
    let mut pedaco = [0u8; 4096];
    loop {
        let n = s.read(&mut pedaco).unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&pedaco[..n]);
        let txt = String::from_utf8_lossy(&buf).to_string();
        if let Some(fim) = txt.find("\r\n\r\n") {
            let tam = txt
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("content-length")
                        .then(|| v.trim().parse::<usize>().ok())?
                })
                .unwrap_or(0);
            if buf.len() >= fim + 4 + tam {
                break;
            }
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

#[tokio::test]
async fn ddg_pelo_broker_com_loopback_permitido() {
    let srv = servidor(
        "text/html; charset=utf-8",
        include_str!("../fixtures/ddg.html"),
    );
    let b = broker_com(&[&srv.origem]);
    let ddg = DuckDuckGoHtmlBackend::with_endpoints(b, vec![format!("{}/html/", srv.origem)]);
    let hits = ddg.search("rust & lang", 5).await.unwrap();
    assert_eq!(hits.len(), 3);
    assert_eq!(hits[0].url, "https://www.rust-lang.org/");
    let pedido = srv.pedidos.lock().unwrap()[0].clone();
    assert!(pedido.starts_with("POST /html/ "), "{pedido}");
    assert!(pedido.ends_with("q=rust+%26+lang"), "{pedido}");
}

#[tokio::test]
async fn fetch_readable_pelo_broker() {
    let srv = servidor(
        "text/html; charset=utf-8",
        include_str!("../fixtures/pagina.html"),
    );
    let b = broker_com(&[&srv.origem]);
    let p = fetch_readable(&b, &format!("{}/artigos/rust.html", srv.origem))
        .await
        .unwrap();
    assert_eq!(p.title, "Rust & o PhxClaw \u{2014} guia");
    assert!(p.text.starts_with("# Por que Rust?"));
    assert!(!p.text.contains("rastreador"));
    assert!(!p.truncated);
    assert!(
        p.links
            .iter()
            .any(|l| l.url == format!("{}/instalar?so=linux&arq=x86", srv.origem))
    );
}

#[tokio::test]
async fn fetch_corta_no_teto_e_avisa() {
    let srv = servidor("text/plain", "linha um\n\n\n\nlinha dois\n");
    let b = broker_com(&[&srv.origem]);
    let p = fetch_readable_with(&b, &srv.origem, 8).await.unwrap();
    assert!(p.truncated);
    assert_eq!(p.text, "linha um");
    let p = fetch_readable(&b, &srv.origem).await.unwrap();
    assert_eq!(p.text, "linha um\n\nlinha dois");
    assert!(!p.truncated);
}

#[tokio::test]
async fn fetch_recusa_tipo_binario() {
    let srv = servidor("application/octet-stream", "\u{0}\u{1}binario");
    let b = broker_com(&[&srv.origem]);
    let r = fetch_readable(&b, &srv.origem).await;
    assert!(matches!(r, Err(WebSearchError::ContentType(_))), "{r:?}");
}

#[tokio::test]
async fn origem_fora_da_lista_e_recusada_sem_tocar_o_servidor() {
    let srv = servidor("text/html", "<p>segredo</p>");
    // O broker permite outra origem de loopback, nao esta: porta diferente e origem
    // diferente, e e isso que impede o agente de varrer as portas da propria maquina.
    let b = broker_com(&["http://127.0.0.1:1"]);
    let r = fetch_readable(&b, &srv.origem).await;
    assert!(
        matches!(r, Err(WebSearchError::Egress(EgressError::OriginDenied(_)))),
        "{r:?}"
    );
    let ddg = DuckDuckGoHtmlBackend::with_endpoints(b, vec![srv.origem.clone()]);
    let r = ddg.search("x", 3).await;
    assert!(matches!(r, Err(WebSearchError::AllFailed(_))), "{r:?}");
    assert_eq!(
        srv.toques.load(Ordering::SeqCst),
        0,
        "o servidor foi tocado"
    );
}

// ------------------------------------------------------------ rede real

/// Busca real pela internet. `cargo test -p phxclaw-web-search -- --ignored --nocapture`.
#[tokio::test]
#[ignore = "usa a internet"]
async fn real_duckduckgo_e_leitura_do_primeiro_resultado() {
    let b = broker_com(&["https://html.duckduckgo.com", "https://lite.duckduckgo.com"]);
    // Cada origem sozinha primeiro, para o relatorio dizer qual respondeu e o erro exato
    // da que falhou, em vez de o plano B esconder a queda do plano A.
    for e in [DDG_HTML, DDG_LITE] {
        let so = DuckDuckGoHtmlBackend::with_endpoints(b.clone(), vec![e.into()]);
        match so.search("rust programming language", 10).await {
            Ok(h) => println!("{e}: {} resultados", h.len()),
            Err(err) => println!("{e}: FALHOU: {err}"),
        }
    }
    let ddg = DuckDuckGoHtmlBackend::new(b);
    let hits = ddg.search("rust programming language", 10).await.unwrap();
    println!("resultados: {}", hits.len());
    for h in &hits {
        println!("  {} | {} | {}", h.title, h.url, h.snippet);
    }
    assert!(!hits.is_empty());
    // A leitura da pagina pede a origem do resultado na lista, e so ela: a politica
    // cresce por decisao explicita, resultado a resultado.
    let mut lida = None;
    for h in hits.iter().take(3) {
        let u = Url::parse(&h.url).unwrap();
        let b = broker_com(&[&origin_of(&u)]);
        match fetch_readable(&b, &h.url).await {
            Ok(p) => {
                lida = Some(p);
                break;
            }
            Err(e) => println!("falhou ao ler {}: {e}", h.url),
        }
    }
    let p = lida.expect("nenhum dos tres primeiros resultados se leu");
    println!(
        "pagina: {} | titulo {:?} | {} chars | {} links\n{}",
        p.final_url,
        p.title,
        p.text.len(),
        p.links.len(),
        p.text.chars().take(600).collect::<String>()
    );
    assert!(!p.text.is_empty());
}

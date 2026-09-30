//! Prova real: Chromium de verdade contra servidores HTTP locais. Pula com
//! motivo escrito quando nao ha Chromium, para nao virar falso verde calado.

use phxclaw_browser::{Browser, BrowserError, BrowserPolicy, LaunchOptions, find_chromium};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

struct Servidor {
    porta: u16,
    conexoes: Arc<AtomicUsize>,
}

impl Servidor {
    fn origem(&self) -> String {
        format!("http://127.0.0.1:{}", self.porta)
    }
}

type Rota = Arc<dyn Fn(&str) -> (u16, Vec<(String, String)>, String) + Send + Sync>;

fn subir(rota: Rota) -> Servidor {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = l.local_addr().unwrap().port();
    let conexoes = Arc::new(AtomicUsize::new(0));
    let c = conexoes.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(s) = s else { continue };
            c.fetch_add(1, Ordering::SeqCst);
            let rota = rota.clone();
            std::thread::spawn(move || atender(s, rota));
        }
    });
    Servidor { porta, conexoes }
}

fn atender(mut s: TcpStream, rota: Rota) {
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    let mut buf = Vec::new();
    let mut pedaco = [0u8; 2048];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        match s.read(&mut pedaco) {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&pedaco[..n]),
        }
    }
    let req = String::from_utf8_lossy(&buf);
    let alvo = req.split_whitespace().nth(1).unwrap_or("/").to_string();
    let (status, cabecalhos, corpo) = rota(&alvo);
    let frase = match status {
        200 => "OK",
        302 => "Found",
        _ => "Not Found",
    };
    let mut resp = format!(
        "HTTP/1.1 {status} {frase}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n",
        corpo.len()
    );
    for (k, v) in cabecalhos {
        resp.push_str(&format!("{k}: {v}\r\n"));
    }
    resp.push_str("\r\n");
    resp.push_str(&corpo);
    let _ = s.write_all(resp.as_bytes());
}

fn escapar(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Servidor proibido (nao listado na politica) e servidor permitido, que
/// aponta para o proibido por link, redirect, popup e fetch.
fn cenario() -> (Servidor, Servidor) {
    let proibido = subir(Arc::new(|_| (200, vec![], "segredo interno".to_string())));
    let p2 = proibido.porta;
    let permitido = subir(Arc::new(move |alvo: &str| {
        let (caminho, consulta) = alvo.split_once('?').unwrap_or((alvo, ""));
        match caminho {
            "/" => (
                200,
                vec![],
                format!(
                    "<html><head><title>Inicio PhxClaw</title></head><body>\
                     <h1>Bem-vindo</h1><p>Ola do servidor de teste</p>\
                     <p><a id=\"ir\" href=\"/destino\">Ir ao destino</a></p>\
                     <p><a id=\"proibido\" href=\"http://127.0.0.1:{p2}/segredo\">proibido</a></p>\
                     <p><a id=\"popup\" target=\"_blank\" href=\"/destino\">nova janela</a></p>\
                     <form action=\"/enviar\" method=\"get\"><input id=\"q\" name=\"q\">\
                     <button id=\"ok\" type=\"submit\">Enviar</button></form>\
                     <ul><li>um</li><li>dois</li></ul></body></html>"
                ),
            ),
            "/destino" => (
                200,
                vec![],
                "<html><head><title>Destino</title></head><body>Voce chegou ao destino</body></html>"
                    .to_string(),
            ),
            "/enviar" => {
                let q = url::form_urlencoded::parse(consulta.as_bytes())
                    .find(|(k, _)| k == "q")
                    .map(|(_, v)| v.into_owned())
                    .unwrap_or_default();
                (
                    200,
                    vec![],
                    format!("<html><body><p>Voce digitou: {}</p></body></html>", escapar(&q)),
                )
            }
            "/redir" => (
                302,
                vec![(
                    "Location".to_string(),
                    format!("http://127.0.0.1:{p2}/segredo"),
                )],
                String::new(),
            ),
            _ => (404, vec![], "nao achei".to_string()),
        }
    }));
    (permitido, proibido)
}

async fn lancar(permitido: &Servidor) -> Option<Browser> {
    if find_chromium().is_none() {
        eprintln!(
            "PULADO: Chromium nao encontrado (defina PHXCLAW_CHROMIUM); a prova real nao rodou"
        );
        return None;
    }
    let politica = BrowserPolicy::only([permitido.origem()]);
    Some(
        Browser::launch(LaunchOptions::with_policy(politica))
            .await
            .expect("lancar chromium"),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn navega_le_texto_links_markdown_e_avalia_js() {
    let (ok, _proibido) = cenario();
    let Some(b) = lancar(&ok).await else { return };
    let p = b.new_page().await.unwrap();
    p.goto(&format!("{}/", ok.origem())).await.unwrap();

    assert_eq!(p.title().await.unwrap(), "Inicio PhxClaw");
    assert_eq!(p.url().await.unwrap(), format!("{}/", ok.origem()));
    let texto = p.text().await.unwrap();
    assert!(texto.contains("Ola do servidor de teste"), "{texto}");

    let links = p.links().await.unwrap();
    let destino = format!("{}/destino", ok.origem());
    assert!(
        links
            .iter()
            .any(|l| l.href == destino && l.text == "Ir ao destino"),
        "{links:?}"
    );

    let md = p.markdown_like().await.unwrap();
    assert!(md.contains("# Bem-vindo"), "{md}");
    assert!(md.contains(&format!("[Ir ao destino]({destino})")), "{md}");
    assert!(md.contains("- um") && md.contains("- dois"), "{md}");

    assert_eq!(p.eval("1 + 2").await.unwrap(), serde_json::json!(3));
    assert_eq!(
        p.eval("Promise.resolve({a: [1, 'x']})").await.unwrap(),
        serde_json::json!({"a": [1, "x"]})
    );
    let e = p.eval("throw new Error('quebrou')").await.unwrap_err();
    assert!(
        matches!(e, BrowserError::JavaScript(ref m) if m.contains("quebrou")),
        "{e:?}"
    );
    let e = p.click("#nao-existe").await.unwrap_err();
    assert!(matches!(e, BrowserError::ElementNotFound(_)), "{e:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn clica_link_e_envia_formulario() {
    let (ok, _proibido) = cenario();
    let Some(b) = lancar(&ok).await else { return };
    let p = b.new_page().await.unwrap();
    let raiz = format!("{}/", ok.origem());

    p.goto(&raiz).await.unwrap();
    p.click_and_wait("#ir").await.unwrap();
    assert_eq!(p.url().await.unwrap(), format!("{}/destino", ok.origem()));
    assert!(p.text().await.unwrap().contains("Voce chegou ao destino"));

    // Envio por Enter no campo.
    p.goto(&raiz).await.unwrap();
    p.type_text("#q", "ola phxclaw 123 & <b>").await.unwrap();
    p.press_enter().await.unwrap();
    p.wait_for_load().await.unwrap();
    let t = p.text().await.unwrap();
    assert!(t.contains("Voce digitou: ola phxclaw 123 & <b>"), "{t}");

    // Envio pelo botao.
    p.goto(&raiz).await.unwrap();
    p.type_text("#q", "pelo botao").await.unwrap();
    p.click_and_wait("#ok").await.unwrap();
    let t = p.text().await.unwrap();
    assert!(t.contains("Voce digitou: pelo botao"), "{t}");
}

#[tokio::test(flavor = "multi_thread")]
async fn captura_de_tela_e_png_valido() {
    let (ok, _proibido) = cenario();
    let Some(b) = lancar(&ok).await else { return };
    let p = b.new_page().await.unwrap();
    p.goto(&format!("{}/", ok.origem())).await.unwrap();
    let png = p.screenshot_png().await.unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "assinatura PNG");
    assert_eq!(&png[12..16], b"IHDR");
    let largura = u32::from_be_bytes(png[16..20].try_into().unwrap());
    let altura = u32::from_be_bytes(png[20..24].try_into().unwrap());
    assert!(largura > 0 && altura > 0, "{largura}x{altura}");
}

#[tokio::test(flavor = "multi_thread")]
async fn redirect_para_origem_proibida_e_bloqueado() {
    let (ok, proibido) = cenario();
    let Some(b) = lancar(&ok).await else { return };
    let p = b.new_page().await.unwrap();
    let r = p.goto(&format!("{}/redir", ok.origem())).await;
    // Folga para uma conexao atrasada aparecer no contador, se existisse.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        proibido.conexoes.load(Ordering::SeqCst),
        0,
        "o servidor proibido recebeu conexao: o redirect escapou da politica ({r:?})"
    );
    match r {
        Err(BrowserError::NavigationBlocked { url, .. }) => {
            assert!(url.contains(&proibido.porta.to_string()), "{url}")
        }
        outro => panic!("esperava NavigationBlocked, veio {outro:?}"),
    }
    assert!(
        ok.conexoes.load(Ordering::SeqCst) >= 1,
        "a primeira perna tem de ter ido"
    );
    assert!(
        b.blocked_requests()
            .iter()
            .any(|x| x.url.contains(&format!(":{}/segredo", proibido.porta)))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn navegacao_e_rede_iniciadas_pela_pagina_passam_pela_politica() {
    let (ok, proibido) = cenario();
    let Some(b) = lancar(&ok).await else { return };
    let p = b.new_page().await.unwrap();
    let raiz = format!("{}/", ok.origem());
    p.goto(&raiz).await.unwrap();

    // Clique num link para a origem proibida. Espera o bloqueio ser registrado antes de
    // navegar de novo: sob carga, o goto seguinte cancelava a navegacao do clique antes de
    // ela ser tentada, e o teste falhava sem furo nenhum (o proibido seguia com 0 conexoes).
    p.click("#proibido").await.unwrap();
    let alvo = format!(":{}/segredo", proibido.porta);
    for _ in 0..50 {
        if b.blocked_requests().iter().any(|x| x.url.contains(&alvo)) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // fetch() da propria pagina: SSRF pela porta dos sub-recursos.
    let js = format!(
        "fetch('http://127.0.0.1:{}/api', {{mode: 'no-cors'}}).then(() => 'passou', () => 'barrado')",
        proibido.porta
    );
    // O clique pode ter levado a uma pagina de erro; o fetch roda de novo
    // a partir da origem permitida para nao depender disso.
    p.goto(&raiz).await.unwrap();
    assert_eq!(p.eval(&js).await.unwrap(), serde_json::json!("barrado"));

    // Popup (target=_blank) e fechado, nao navega sozinho.
    p.click("#popup").await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(proibido.conexoes.load(Ordering::SeqCst), 0);
    let bloqueios = b.blocked_requests();
    assert!(
        bloqueios
            .iter()
            .any(|x| x.url.contains(&format!(":{}/segredo", proibido.porta))),
        "{bloqueios:?}"
    );
    assert!(
        bloqueios
            .iter()
            .any(|x| x.url.contains(&format!(":{}/api", proibido.porta))),
        "{bloqueios:?}"
    );
    assert!(
        bloqueios.iter().any(|x| x.reason.contains("popup")),
        "{bloqueios:?}"
    );
    // So a pagina do agente acessou o servidor permitido pelo popup? Nao: o
    // popup foi fechado antes de rodar, entao /destino nao pode ter sido
    // pedido por ele. A pagina principal continua utilizavel.
    assert_eq!(p.title().await.unwrap(), "Inicio PhxClaw");
}

#[tokio::test(flavor = "multi_thread")]
async fn goto_direto_proibido_recusa_sem_tocar_a_rede() {
    let (ok, proibido) = cenario();
    let Some(b) = lancar(&ok).await else { return };
    let p = b.new_page().await.unwrap();
    for u in [
        format!("{}/segredo", proibido.origem()),
        "file:///etc/passwd".to_string(),
        "http://169.254.169.254/latest/meta-data".to_string(),
    ] {
        let e = p.goto(&u).await.unwrap_err();
        assert!(matches!(e, BrowserError::PolicyDenied { .. }), "{u}: {e:?}");
    }
    assert_eq!(proibido.conexoes.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn politica_padrao_nega_ate_a_origem_do_teste() {
    let (ok, _proibido) = cenario();
    if find_chromium().is_none() {
        eprintln!("PULADO: Chromium nao encontrado");
        return;
    }
    let b = Browser::launch(LaunchOptions::default()).await.unwrap();
    let p = b.new_page().await.unwrap();
    let e = p.goto(&format!("{}/", ok.origem())).await.unwrap_err();
    assert!(matches!(e, BrowserError::PolicyDenied { .. }), "{e:?}");
    assert_eq!(ok.conexoes.load(Ordering::SeqCst), 0);
}

fn processos_com(texto: &str) -> Vec<u32> {
    let mut v = Vec::new();
    if let Ok(dir) = std::fs::read_dir("/proc") {
        for e in dir.flatten() {
            let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            if let Ok(cmd) = std::fs::read(e.path().join("cmdline"))
                && String::from_utf8_lossy(&cmd).contains(texto)
            {
                v.push(pid);
            }
        }
    }
    v
}

#[tokio::test(flavor = "multi_thread")]
async fn drop_mata_o_processo_e_apaga_o_perfil() {
    let (ok, _proibido) = cenario();
    let Some(b) = lancar(&ok).await else { return };
    let perfil = b.profile_dir().to_path_buf();
    let nome = perfil.file_name().unwrap().to_string_lossy().into_owned();
    assert!(perfil.is_dir());
    assert!(
        !processos_com(&nome).is_empty(),
        "o chromium devia estar vivo"
    );
    drop(b);
    assert!(!perfil.exists(), "perfil ficou em {}", perfil.display());
    // Filhos do Chromium saem logo depois do pai; espera curta e medida.
    let mut vivos = processos_com(&nome);
    for _ in 0..40 {
        if vivos.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        vivos = processos_com(&nome);
    }
    assert!(
        vivos.is_empty(),
        "processos sobreviveram ao Drop: {vivos:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn close_ordeiro_tambem_limpa() {
    let (ok, _proibido) = cenario();
    let Some(b) = lancar(&ok).await else { return };
    let perfil = b.profile_dir().to_path_buf();
    b.close().await.unwrap();
    assert!(!perfil.exists());
}

//! O projeto Flutter gerado, com o SDK de verdade: analisa sem problema, roda o teste de
//! widget que ele mesmo traz (itens somam, 31/02 recusada, calendario do DateValid),
//! compila para a web e abre no Chromium (o Flutter cria a sua vista e carrega as fontes).
//!
//! Precisa do SDK Flutter (PATH, ou PHXCLAW_FLUTTER, ou /opt/flutter/bin/flutter); sem
//! ele o teste diz que pulou.

use phxclaw_browser::{Browser, BrowserPolicy, LaunchOptions};
use phxclaw_test_support::pulado;
use std::path::{Path, PathBuf};
use std::process::Command;

fn flutter() -> Option<PathBuf> {
    std::env::var_os("PHXCLAW_FLUTTER")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("PATH").and_then(|p| {
                std::env::split_paths(&p)
                    .map(|d| d.join("flutter"))
                    .find(|f| f.is_file())
            })
        })
        .or_else(|| Some(PathBuf::from("/opt/flutter/bin/flutter")).filter(|p| p.is_file()))
}

/// Roda um comando do flutter com prazo. Travado (medido: com o disco cheio o `flutter test`
/// falhava e ficava pendurado meia hora, segurando a suite inteira), o grupo de processos
/// inteiro morre e o teste falha dizendo onde -- o `timeout` do shell matava so o filho e
/// deixava o compilador Dart orfao.
fn roda(f: &Path, dir: &Path, args: &[&str]) -> String {
    use std::os::unix::process::CommandExt;
    let saida = std::env::temp_dir().join(format!("phx-flutter-{}.log", std::process::id()));
    let log = std::fs::File::create(&saida).unwrap();
    let mut filho = Command::new(f)
        .args(args)
        .current_dir(dir)
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .process_group(0)
        .spawn()
        .unwrap();
    let prazo = std::time::Instant::now() + std::time::Duration::from_secs(900);
    let status = loop {
        if let Some(st) = filho.try_wait().unwrap() {
            break st;
        }
        if std::time::Instant::now() > prazo {
            let _ = Command::new("kill")
                .args(["-KILL", &format!("-{}", filho.id())])
                .status();
            let _ = filho.wait();
            panic!(
                "flutter {args:?} passou de 15 min e foi encerrado:\n{}",
                std::fs::read_to_string(&saida).unwrap_or_default()
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    };
    let t = std::fs::read_to_string(&saida).unwrap_or_default();
    let _ = std::fs::remove_file(&saida);
    assert!(status.success(), "flutter {args:?} falhou:\n{t}");
    t
}

fn servir_pasta(raiz: PathBuf) -> String {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { break };
            let mut b = [0u8; 4096];
            let n = s.read(&mut b).unwrap_or(0);
            let req = String::from_utf8_lossy(&b[..n]);
            let c = req.split_whitespace().nth(1).unwrap_or("/");
            let rel = match c.split(['?', '#']).next().unwrap_or("/") {
                "/" => "index.html",
                c => c.trim_start_matches('/'),
            };
            let tipo = match rel.rsplit('.').next() {
                Some("js" | "mjs") => "text/javascript",
                Some("wasm") => "application/wasm",
                Some("json") => "application/json",
                Some("html") => "text/html; charset=utf-8",
                _ => "application/octet-stream",
            };
            let (st, corpo) = match (rel.contains(".."), std::fs::read(raiz.join(rel))) {
                (false, Ok(c)) => ("200 OK", c),
                _ => ("404 Not Found", b"nao".to_vec()),
            };
            let _ = s.write_all(
                format!(
                    "HTTP/1.1 {st}\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    corpo.len()
                )
                .as_bytes(),
            );
            let _ = s.write_all(&corpo);
        }
    });
    base
}

#[tokio::test]
async fn projeto_flutter_analisa_testa_compila_e_abre() {
    let Some(f) = flutter() else {
        pulado::pular("flutter", "SDK Flutter ausente");
        return;
    };
    let (app, _) = phxclaw_ui_ir::from_sql("Vendas", include_str!("fixtures/pedidos.sql"));
    let dir = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/ui-ir/flutter"
    ));
    let _ = std::fs::remove_dir_all(dir.join("lib"));
    let _ = std::fs::remove_dir_all(dir.join("test"));
    for (p, c) in phxclaw_ui_ir::flutter::render(&app) {
        let alvo = dir.join(p);
        std::fs::create_dir_all(alvo.parent().unwrap()).unwrap();
        std::fs::write(alvo, c).unwrap();
    }
    roda(
        &f,
        &dir,
        &[
            "create",
            "--platforms",
            "web",
            "--project-name",
            "vendas_erp",
            ".",
        ],
    );
    // o create nao pode ter trocado o que o gerador escreveu
    let main = std::fs::read_to_string(dir.join("lib/main.dart")).unwrap();
    assert!(
        main.contains("class AppErp"),
        "flutter create sobrescreveu lib/main.dart"
    );
    let a = roda(&f, &dir, &["analyze"]);
    assert!(a.contains("No issues found"), "{a}");
    let t = roda(&f, &dir, &["test"]);
    assert!(t.contains("All tests passed"), "{t}");
    roda(&f, &dir, &["build", "web", "--no-web-resources-cdn"]);

    let base = servir_pasta(dir.join("build/web"));
    let b = Browser::launch(LaunchOptions::with_policy(BrowserPolicy::only([
        base.clone()
    ])))
    .await
    .unwrap();
    let p = b.new_page().await.unwrap();
    p.goto(&format!("{base}/#pedido_documento")).await.unwrap();
    // pronto = o Flutter criou a sua vista e carregou as fontes do app (o titulo da aba
    // nao serve: fica o do index.html do create)
    let mut pronto = false;
    for _ in 0..300 {
        let r = p
            .eval("!!document.querySelector('flutter-view') && performance.getEntriesByType('resource').some(r=>r.name.endsWith('.ttf'))")
            .await
            .unwrap();
        if r == serde_json::json!(true) {
            pronto = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(
        pronto,
        "o app Flutter nao subiu; bloqueios: {:?}; idioma: {}",
        b.blocked_requests(),
        p.eval("navigator.language").await.unwrap()
    );
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    let png = p.screenshot_png().await.unwrap();
    std::fs::write(dir.join("../flutter-pedido.png"), &png).unwrap();
    // o que a pagina tentou buscar fora da origem (CDN de fontes/canvaskit) aparece aqui
    let fora = b.blocked_requests();
    assert!(
        fora.is_empty(),
        "o build web buscou fora da origem: {fora:?}"
    );
    b.close().await.unwrap();
}

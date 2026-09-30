//! O projeto React gerado, construido com o esbuild de verdade e exercitado no Chromium:
//! as mesmas provas do renderizador HTML, digitadas tecla a tecla (num input controlado
//! pelo React, atribuir `.value` por script nao dispara o onChange -- so digitacao prova).
//!
//! Precisa de `npm` e do registro de pacotes; sem `npm` o teste diz que pulou.

use phxclaw_browser::{Browser, BrowserPolicy, LaunchOptions};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;

fn servir_pasta(raiz: PathBuf) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { break };
            let mut b = [0u8; 4096];
            let n = s.read(&mut b).unwrap_or(0);
            let req = String::from_utf8_lossy(&b[..n]);
            let caminho = req.split_whitespace().nth(1).unwrap_or("/");
            let rel = match caminho.split(['?', '#']).next().unwrap_or("/") {
                "/" => "index.html",
                c => c.trim_start_matches('/'),
            };
            let (st, tipo, corpo) = match (rel.contains(".."), std::fs::read(raiz.join(rel))) {
                (false, Ok(c)) => (
                    "200 OK",
                    if rel.ends_with(".js") {
                        "text/javascript"
                    } else {
                        "text/html; charset=utf-8"
                    },
                    c,
                ),
                _ => ("404 Not Found", "text/plain", b"nao".to_vec()),
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

fn npm(dir: &Path, args: &[&str]) {
    let s = Command::new("npm")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("npm");
    assert!(s.success(), "npm {args:?} falhou");
}

#[tokio::test]
async fn projeto_react_constroi_e_funciona_no_chromium() {
    if phxclaw_browser::find_chromium().is_none()
        || Command::new("npm").arg("--version").output().is_err()
    {
        eprintln!("chromium ou npm ausente: pulado");
        return;
    }
    let sql = include_str!("fixtures/pedidos.sql");
    let (app, _) = phxclaw_ui_ir::from_sql("Vendas", sql);
    let dir = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/ui-ir/react"
    ));
    for (p, c) in phxclaw_ui_ir::react::render(&app) {
        let alvo = dir.join(p);
        std::fs::create_dir_all(alvo.parent().unwrap()).unwrap();
        std::fs::write(alvo, c).unwrap();
    }
    let _ = std::fs::remove_file(dir.join("dist/app.js"));
    npm(&dir, &["install", "--no-audit", "--no-fund", "--silent"]);
    npm(&dir, &["run", "build", "--silent"]);
    assert!(dir.join("dist/app.js").is_file());

    let base = servir_pasta(dir.clone());
    let b = Browser::launch(LaunchOptions::with_policy(BrowserPolicy::only([
        base.clone()
    ])))
    .await
    .unwrap();
    let p = b.new_page().await.unwrap();
    p.goto(&format!("{base}/#pedido_documento")).await.unwrap();
    for _ in 0..50 {
        if p.eval("!!document.querySelector('#pedido_documento [data-add-item]')")
            .await
            .unwrap()
            == serde_json::json!(true)
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    for _ in 0..2 {
        p.click("#pedido_documento [data-add-item]").await.unwrap();
    }
    let linhas = p
        .eval("document.querySelectorAll('#pedido_documento table.itens tbody tr').length")
        .await
        .unwrap();
    assert_eq!(linhas, serde_json::json!(2));
    for (i, v) in [(0, "10,50"), (1, "1.234,25")] {
        let sel = format!(
            "#pedido_documento table.itens tbody tr:nth-child({}) [name=vl_total]",
            i + 1
        );
        for ch in v.chars() {
            p.type_text(&sel, &ch.to_string()).await.unwrap();
        }
    }
    let total = p
        .eval("document.querySelector('#pedido_documento output[data-soma=vl_total]').textContent")
        .await
        .unwrap();
    assert_eq!(
        total.as_str().unwrap().replace('\u{a0}', " "),
        "R$ 1.244,75"
    );
    // remover um item recalcula
    p.click("#pedido_documento table.itens tbody tr:nth-child(1) [aria-label='Remover item']")
        .await
        .unwrap();
    let total = p
        .eval("document.querySelector('#pedido_documento output[data-soma=vl_total]').textContent")
        .await
        .unwrap();
    assert_eq!(
        total.as_str().unwrap().replace('\u{a0}', " "),
        "R$ 1.234,25"
    );
    // data: mesma mascara e mesma recusa de 31/02 do HTML
    let data = "#pedido_documento [name=dt_emissao]";
    for ch in "31022026".chars() {
        p.type_text(data, &ch.to_string()).await.unwrap();
    }
    let v = p
        .eval("(()=>{const i=document.querySelector('#pedido_documento [name=dt_emissao]');return [i.value,i.validity.valid]})()")
        .await
        .unwrap();
    assert_eq!(v, serde_json::json!(["31/02/2026", false]));
    // sair do campo marca a data invalida na tela (borda de erro), nao so na validacao
    p.click("main h1").await.unwrap();
    let marcada = p
        .eval("(()=>{const c=n=>getComputedStyle(document.querySelector('#pedido_documento [name='+n+']')).borderTopColor;return c('dt_emissao')!==c('vl_frete')})()")
        .await
        .unwrap();
    assert_eq!(marcada, serde_json::json!(true));
    let png = p.screenshot_png().await.unwrap();
    std::fs::write(dir.join("../react-pedido.png"), &png).unwrap();
    // navegacao pelo menu troca a tela
    p.click("nav a[href='#cliente_cadastro']").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let t = p
        .eval("document.querySelector('main h1').textContent")
        .await
        .unwrap();
    assert_eq!(t, serde_json::json!("Cadastro de cliente"));
    b.close().await.unwrap();
}

//! O prototipo gerado, exercitado no Chromium de verdade: interface so se prova exercitando.

use phxclaw_browser::{Browser, BrowserPolicy, LaunchOptions};
use std::io::{Read, Write};
use std::net::TcpListener;

fn servir(html: String) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { break };
            let mut b = [0u8; 2048];
            let _ = s.read(&mut b);
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}", html.len()).as_bytes());
        }
    });
    base
}

#[tokio::test]
async fn pedido_adiciona_itens_e_recalcula_total_no_chromium() {
    if phxclaw_browser::find_chromium().is_none() {
        eprintln!("chromium ausente: pulado");
        return;
    }
    let sql = include_str!("fixtures/pedidos.sql");
    let (app, _) = phxclaw_ui_ir::from_sql("Vendas", sql);
    let base = servir(phxclaw_ui_ir::html::render(&app));
    let b = Browser::launch(LaunchOptions::with_policy(BrowserPolicy::only([
        base.clone()
    ])))
    .await
    .unwrap();
    let p = b.new_page().await.unwrap();
    p.goto(&format!("{base}/#pedido_documento")).await.unwrap();
    let visivel = p
        .eval("document.getElementById('pedido_documento').hidden")
        .await
        .unwrap();
    assert_eq!(
        visivel,
        serde_json::json!(false),
        "tela do pedido nao abriu pelo hash"
    );
    for _ in 0..2 {
        p.click("#pedido_documento [data-add-item]").await.unwrap();
    }
    let linhas = p
        .eval("document.querySelectorAll('#pedido_documento table.itens tbody tr').length")
        .await
        .unwrap();
    assert_eq!(linhas, serde_json::json!(2));
    // digita valores com virgula decimal, como o usuario brasileiro digita
    p.eval("(()=>{const i=[...document.querySelectorAll('#pedido_documento table.itens [name=vl_total]')];\
        i[0].value='10,50';i[1].value='1.234,25';i.forEach(x=>x.dispatchEvent(new Event('input',{bubbles:true})));})()").await.unwrap();
    let total = p
        .eval("document.querySelector('#pedido_documento output[data-soma=vl_total]').textContent")
        .await
        .unwrap();
    assert_eq!(
        total.as_str().unwrap().replace('\u{a0}', " "),
        "R$ 1.244,75"
    );
    // data digitada tecla a tecla, como o usuario: a mascara poe as barras e recusa 31/02
    let data = "#pedido_documento [name=dt_emissao]";
    for ch in "31022026".chars() {
        p.type_text(data, &ch.to_string()).await.unwrap();
    }
    let v = p
        .eval("(()=>{const i=document.querySelector('#pedido_documento [name=dt_emissao]');return [i.value,i.validity.valid]})()")
        .await
        .unwrap();
    assert_eq!(
        v,
        serde_json::json!(["31/02/2026", false]),
        "31/02 tem de ser recusada"
    );
    p.eval("document.querySelector('#pedido_documento [name=dt_emissao]').value=''")
        .await
        .unwrap();
    for ch in "1503202699".chars() {
        p.type_text(data, &ch.to_string()).await.unwrap();
    }
    let v = p
        .eval("(()=>{const i=document.querySelector('#pedido_documento [name=dt_emissao]');return [i.value,i.validity.valid,i.placeholder]})()")
        .await
        .unwrap();
    assert_eq!(v, serde_json::json!(["15/03/2026", true, "dd/mm/aaaa"]));
    // mesma regra do DateValid (juliano ate 04/10/1582) que o Rust e o WLanguage gerados
    for (digitos, valida) in [("29021500", true), ("10101582", false), ("29021900", false)] {
        p.eval("document.querySelector('#pedido_documento [name=dt_emissao]').value=''")
            .await
            .unwrap();
        for ch in digitos.chars() {
            p.type_text(data, &ch.to_string()).await.unwrap();
        }
        let ok = p
            .eval("document.querySelector('#pedido_documento [name=dt_emissao]').validity.valid")
            .await
            .unwrap();
        assert_eq!(ok, serde_json::json!(valida), "{digitos}");
    }
    // o cadastro de cliente tem obrigatorios marcados e o lookup do pedido aponta para cliente
    let obrig = p
        .eval("document.querySelectorAll('#cliente_cadastro [required]').length")
        .await
        .unwrap();
    assert!(obrig.as_u64().unwrap() >= 2, "{obrig}");
    let png = p.screenshot_png().await.unwrap();
    std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/ui-ir")).unwrap();
    std::fs::write(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/ui-ir/pedido.png"),
        &png,
    )
    .unwrap();
    p.goto(&format!("{base}/#cliente_cadastro")).await.unwrap();
    let png = p.screenshot_png().await.unwrap();
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/ui-ir/cliente.png"
        ),
        &png,
    )
    .unwrap();
    b.close().await.unwrap();
}

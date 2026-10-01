//! A interface web (`apps/phxclaw-ui`) servida como aplicativo instalavel (PWA): a mesma
//! tela no navegador, no celular e no controle remoto. Um cliente so, porque tres
//! clientes divergiriam no dia em que um ganhasse uma rota que os outros nao tem.
//!
//! So arquivo da pasta da interface sai daqui, conferido como o `confine` das tarefas
//! (componente normal, forma canonica dentro da pasta). A pasta e `PHXCLAW_UI_DIR`, ou a
//! do fonte quando o binario roda da arvore de desenvolvimento. Os arquivos NAO sao
//! embutidos: a tela muda num ritmo diferente do motor, e embutir faria cada ajuste de
//! texto exigir recompilar o agente.

use axum::Router;
use axum::extract::Path;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use std::path::{Component, PathBuf};

/// A pasta da interface, ou `None` (e entao nenhuma rota de tela existe).
pub fn pasta_da_interface() -> Option<PathBuf> {
    let candidatas = [
        std::env::var_os("PHXCLAW_UI_DIR").map(PathBuf::from),
        Some(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/phxclaw-ui"
        ))),
        std::env::current_exe()
            .ok()
            .and_then(|e| Some(e.parent()?.join("../share/phxclaw/ui"))),
    ];
    candidatas
        .into_iter()
        .flatten()
        .find(|p| p.join("index.html").is_file())
        .and_then(|p| std::fs::canonicalize(p).ok())
}

fn tipo(caminho: &str) -> &'static str {
    match caminho.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Le um arquivo da interface; caminho que sai da pasta ou nao existe e 404, sem dizer
/// qual dos dois (dizer seria responder «existe» para quem sonda o disco).
pub fn ler(pasta: &std::path::Path, rel: &str) -> Option<Vec<u8>> {
    let r = std::path::Path::new(rel);
    if rel.is_empty() || !r.components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    let alvo = std::fs::canonicalize(pasta.join(r)).ok()?;
    if !alvo.starts_with(pasta) || !alvo.is_file() {
        return None;
    }
    std::fs::read(alvo).ok()
}

fn servir_arquivo(pasta: &std::path::Path, rel: &str) -> Response {
    match ler(pasta, rel) {
        Some(b) => {
            let mut r = (
                [(header::CONTENT_TYPE, tipo(rel))],
                axum::body::Body::from(b),
            )
                .into_response();
            // O service worker e a pagina tem de ser conferidos a cada visita: e assim
            // que versao nova chega ao celular que ja instalou o aplicativo.
            if rel == "sw.js" || rel == "index.html" {
                r.headers_mut()
                    .insert(header::CACHE_CONTROL, "no-cache".parse().expect("fixo"));
            }
            r
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Arquivos da raiz que a tela usa. So eles viram rota: o resto da pasta (testes, dados
/// de prova) nao fica exposto por estar ali.
pub const ARQUIVOS_DA_RAIZ: &[&str] = &["index.html", "manifest.webmanifest", "sw.js"];

/// As rotas da tela: `/`, os `ARQUIVOS_DA_RAIZ` e `/assets/*`. Sem pasta da interface,
/// router vazio.
pub fn rotas<S: Clone + Send + Sync + 'static>() -> Router<S> {
    let Some(pasta) = pasta_da_interface() else {
        return Router::new();
    };
    let p = pasta.clone();
    let mut r = Router::new().route(
        "/",
        get(move || {
            let p = p.clone();
            async move { servir_arquivo(&p, "index.html") }
        }),
    );
    for a in ARQUIVOS_DA_RAIZ {
        let p = pasta.clone();
        r = r.route(
            &format!("/{a}"),
            get(move || {
                let p = p.clone();
                async move { servir_arquivo(&p, a) }
            }),
        );
    }
    r.route(
        "/assets/{*resto}",
        get(move |Path(resto): Path<String>| {
            let p = pasta.clone();
            async move { servir_arquivo(&p, &format!("assets/{resto}")) }
        }),
    )
}

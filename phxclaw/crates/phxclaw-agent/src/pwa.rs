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
use axum::extract::{Path, Request};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use std::path::{Component, PathBuf};

/// A CSP da interface. Medido em 09/10/2026 contra a tela inteira (roteiros de
/// `tests/desktop/`, zero violacao no console):
///
/// - `script-src 'self'` sem `unsafe-inline` nem `unsafe-eval`: nenhum `<script>` em linha,
///   nenhum `on*=` e nenhum `eval`/`new Function` na tela -- o `tema.js` ja tinha saido do
///   `<head>` por isso. E a diretiva que separa XSS de texto inofensivo.
/// - `style-src 'unsafe-inline'` e o preco medido do phx-grid: ele monta `style="..."` por
///   `innerHTML` (largura de coluna, recuo de grupo, barra do Gantt) e cria `<style>` por
///   instancia. Sem ele a grade perde o desenho; com ele o pior caso de injecao e CSS, que
///   nao executa nada.
/// - `worker-src blob:`: o phx-grid ordena e agrega em worker montado de `Blob`.
/// - `img-src data:`: o ruido do fundo (`app.css`) e SVG em `data:`.
/// - `connect-src 'self'`: a API e o websocket do terminal do IDE sao da mesma origem (a
///   CSP 3 casa `ws:`/`wss:` do mesmo host com `'self'`; medido no Chromium pelo
///   `tests/desktop/ide_web.mjs`, o WebKit do iPhone nao foi medido).
/// - Medido o preco de cada recusa: com `style-src 'self'` so as telas Agentes e Cubo davam
///   81 violacoes (79 `style-src-attr`, 2 `style-src-elem`).
///
/// O `tauri.conf.json` repete esta politica e so acrescenta o canal do host (`ipc:`); o
/// teste `a_csp_do_desktop_e_a_do_servidor` reprova se as duas divergirem.
pub const CSP: &str = concat!(
    "default-src 'self'; ",
    "script-src 'self'; ",
    "style-src 'self' 'unsafe-inline'; ",
    "img-src 'self' data: blob:; ",
    "font-src 'self'; ",
    "connect-src 'self'; ",
    "worker-src 'self' blob:; ",
    "manifest-src 'self'; ",
    "frame-src 'none'; ",
    "object-src 'none'; ",
    "base-uri 'none'; ",
    "frame-ancestors 'none'; ",
    "form-action 'self'"
);

/// O que a pagina pode pedir ao navegador. Medido: a tela nao usa camera, microfone nem
/// localizacao (a voz roda no agente, nao no navegador); a area de transferencia fica
/// porque o phx-grid copia a selecao e o IDE cola no terminal.
pub const PERMISSOES: &str = "camera=(), microphone=(), geolocation=(), payment=(), usb=(), \
serial=(), hid=(), midi=(), display-capture=()";

/// Os cabecalhos de seguranca de toda resposta deste servidor, num lugar so. Rota que ja
/// decidiu o seu (o artefato e o site com `sandbox`, o canvas com `frame-ancestors 'self'`)
/// fica com o dela: a decisao de quem conhece o conteudo vale mais que a regra geral.
pub const CABECALHOS: &[(&str, &str)] = &[
    ("content-security-policy", CSP),
    ("x-frame-options", "DENY"),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "no-referrer"),
    ("permissions-policy", PERMISSOES),
    ("cross-origin-opener-policy", "same-origin"),
    ("cross-origin-resource-policy", "same-origin"),
];

/// Resposta sem `Cache-Control` e dado (tarefa, configuracao, erro): nao fica em disco do
/// navegador nem em proxy. Os arquivos da tela dizem o deles em `servir_arquivo`.
pub const SEM_CACHE: &str = "no-store";

/// Poe os cabecalhos de seguranca que faltam. O `x-frame-options` acompanha a CSP: rota com
/// CSP propria (o widget do canvas, que se deixa enquadrar pela propria origem) nao ganha
/// um DENY que a contradiria.
pub fn blindar_cabecalhos(h: &mut HeaderMap) {
    let csp_propria = h.contains_key(header::CONTENT_SECURITY_POLICY);
    for (nome, valor) in CABECALHOS {
        let nome = HeaderName::from_static(nome);
        if csp_propria && nome == header::X_FRAME_OPTIONS {
            continue;
        }
        if !h.contains_key(&nome) {
            h.insert(nome, HeaderValue::from_static(valor));
        }
    }
    if !h.contains_key(header::CACHE_CONTROL) {
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static(SEM_CACHE));
    }
}

/// O middleware: UM ponto para a API do `servir` e para a ponte (`remoto.rs`), montado por
/// FORA do portao do RBAC, para que o 401 e o 404 tambem saiam blindados.
pub async fn blindar(req: Request, next: Next) -> Response {
    let mut r = next.run(req).await;
    blindar_cabecalhos(r.headers_mut());
    r
}

/// A rota da politica da tela. Publica de proposito: a dissuasao vale antes do login.
pub const ROTA_POLITICA: &str = "/ui/politica";

/// `ui.bloquear_inspecao`, ligado por padrao. Config ilegivel tambem liga: a duvida cai no
/// lado que o dono pediu.
pub fn bloquear_inspecao() -> bool {
    crate::config::booleano_de("ui.bloquear_inspecao").unwrap_or(true)
}

/// O que a tela pergunta ao abrir. O bloqueio de F12 e do menu de contexto no navegador e
/// DISSUASAO, nao seguranca: o menu do navegador, o view-source e qualquer cliente HTTP
/// alcancam o mesmo. A protecao de verdade e a CSP acima, o RBAC e o segredo fora do cliente.
async fn politica() -> Response {
    axum::Json(serde_json::json!({ "bloquear_inspecao": bloquear_inspecao() })).into_response()
}

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
            // Conferido a cada visita: e assim que versao nova chega ao celular que ja
            // instalou o aplicativo (o service worker e a pagina), e e o que o arquivo sem
            // validador ja recebia na pratica. Dito aqui para o `no-store` do dado, que o
            // `blindar` poe em quem nao disse nada, nao alcancar a casca.
            r.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
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
    let mut r = Router::new().route(ROTA_POLITICA, get(politica)).route(
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

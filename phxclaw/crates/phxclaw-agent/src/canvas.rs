//! Canvas: o agente monta um widget HTML (marcacao, estilo e um script) e o `servir` o
//! hospeda num iframe isolado em `/canvas/<tarefa>/<nome>`.
//!
//! Decisoes que valem saber:
//! - **o script passa pelo tree-sitter antes de gravar**: erro de sintaxe volta ao modelo
//!   com linha e coluna, em vez de virar uma tela branca que ninguem sabe explicar;
//! - **o navegador so roda o script conferido**: a CSP do widget leva o SHA-256 dele em
//!   `script-src`, entao `<script>` escondido na marcacao e `onclick=` em atributo nao
//!   executam. Conferir a sintaxe de um script e deixar outro rodar ao lado seria conferir
//!   nada;
//! - **origem opaca e sem rede**: `sandbox allow-scripts` sem `allow-same-origin` tira do
//!   widget os cookies, o `localStorage` e o DOM da pagina da API, e `connect-src 'none'`
//!   tira o `fetch` -- o JavaScript que o modelo escreveu nao alcanca o token nem a API;
//! - **o widget servido mora FORA de `work/`**, como a marca do `publish_site`: o shell do
//!   agente so enxerga `work/`, e trocaria o arquivo conferido por outro nao conferido.

use crate::api::ApiState;
use crate::tarefa::confine;
use axum::Router;
use axum::extract::{Path as Caminho, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use base64::Engine;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Tetos do que o modelo manda: um widget e uma tela, nao um aplicativo empacotado.
const MAX_HTML: usize = 256 * 1024;
const MAX_SCRIPT: usize = 256 * 1024;
const MAX_CSS: usize = 64 * 1024;

pub struct CanvasTool {
    /// Base publica da API, a mesma do `publish_site`.
    pub base_url: String,
}

/// Nome do widget: vira nome de arquivo e trecho de URL, entao so minusculas, digitos,
/// `-` e `_`.
pub fn nome_valido(n: &str) -> bool {
    (1..=64).contains(&n.len())
        && n.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Onde o widget conferido mora: pasta da tarefa, fora de `work/`.
fn guardado(tarefa: &Path, nome: &str) -> PathBuf {
    tarefa.join("canvas").join(format!("{nome}.json"))
}

fn escapar_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Confere o script pelo parser de JavaScript do tree-sitter. O erro diz linha e coluna,
/// que e o que o modelo precisa para consertar sem reescrever tudo.
pub fn conferir_script(script: &str) -> Result<(), String> {
    match phxclaw_repo_intelligence::primeiro_erro_de_sintaxe("javascript", script)? {
        None => Ok(()),
        Some(e) => Err(format!(
            "script com erro de sintaxe na linha {}, coluna {} ({}); o widget nao foi gravado",
            e.linha, e.coluna, e.tipo
        )),
    }
}

/// Documento do widget e o hash do script para a CSP.
fn montar(titulo: &str, html: &str, css: &str, script: &str) -> (String, String) {
    let doc = format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>{}</title><style>{css}</style></head>\n<body>\n{html}\n<script>{script}</script>\n\
</body></html>\n",
        escapar_html(titulo)
    );
    let hash = base64::engine::general_purpose::STANDARD.encode(Sha256::digest(script));
    (doc, hash)
}

/// A CSP do widget. `sandbox allow-scripts` sem `allow-same-origin` da origem opaca; o
/// hash em `script-src` e o unico script que roda.
pub fn csp_do_widget(hash_script: &str) -> String {
    format!(
        "sandbox allow-scripts; default-src 'none'; script-src 'sha256-{hash_script}'; \
style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data: blob:; \
connect-src 'none'; form-action 'none'; base-uri 'none'; frame-ancestors 'self'"
    )
}

impl Tool for CanvasTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "canvas".into(),
            description: "Create or replace an interactive HTML widget shown to the user in an \
isolated frame, and get its URL. 'html' is the body markup, 'css' the styles, 'script' the \
JavaScript (checked for syntax errors; only this script runs, inline handlers like onclick= \
and <script> tags inside 'html' are blocked; no network access, no cookies)."
                .into(),
            parameters: json!({"type":"object","properties":{
                "name":{"type":"string","description":"lowercase id: letters, digits, - and _"},
                "title":{"type":"string"},
                "html":{"type":"string"},
                "css":{"type":"string"},
                "script":{"type":"string"}
            },"required":["name","html"]}),
        }
    }
    fn capability(&self) -> &'static str {
        // Mesmo risco do site publicado: conteudo do agente servido em origem opaca.
        "site.publish"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let campo = |k: &str| args.get(k).and_then(Value::as_str).unwrap_or("");
            let nome = campo("name").trim();
            if !nome_valido(nome) {
                return Err(ToolError::InvalidArguments(format!(
                    "nome de widget invalido: '{nome}' (minusculas, digitos, - e _, ate 64)"
                )));
            }
            let (html, css, script) = (campo("html"), campo("css"), campo("script"));
            for (k, v, teto) in [
                ("html", html, MAX_HTML),
                ("css", css, MAX_CSS),
                ("script", script, MAX_SCRIPT),
            ] {
                if v.len() > teto {
                    return Err(ToolError::InvalidArguments(format!(
                        "'{k}' tem {} bytes, acima do teto de {teto}",
                        v.len()
                    )));
                }
            }
            // Fechar a tag no meio do texto cortaria o documento num lugar que o parser de
            // JavaScript nao ve: o navegador rodaria um pedaco do script conferido.
            if script.to_ascii_lowercase().contains("</script") {
                return Err(ToolError::InvalidArguments(
                    "o script nao pode conter '</script'; escreva '<\\/script' dentro de strings"
                        .into(),
                ));
            }
            if css.to_ascii_lowercase().contains("</style") {
                return Err(ToolError::InvalidArguments(
                    "o css nao pode conter '</style'".into(),
                ));
            }
            conferir_script(script).map_err(ToolError::InvalidArguments)?;
            let titulo = match campo("title").trim() {
                "" => nome,
                t => t,
            };
            let (doc, hash) = montar(titulo, html, css, script);
            let tarefa = ctx
                .workdir
                .parent()
                .ok_or_else(|| ToolError::Failed("pasta da tarefa".into()))?;
            let alvo = guardado(tarefa, nome);
            if let Some(d) = alvo.parent() {
                std::fs::create_dir_all(d).map_err(|e| ToolError::Failed(e.to_string()))?;
            }
            std::fs::write(
                &alvo,
                serde_json::to_vec(&json!({"html": doc, "script_sha256": hash}))
                    .unwrap_or_default(),
            )
            .map_err(|e| ToolError::Failed(e.to_string()))?;
            // Copia em `work/` so para o usuario baixar: a que se serve e a guardada.
            let rel = format!("canvas/{nome}.html");
            let copia = confine(&ctx.workdir, &rel).map_err(ToolError::Denied)?;
            if let Some(d) = copia.parent() {
                std::fs::create_dir_all(d).map_err(|e| ToolError::Failed(e.to_string()))?;
            }
            std::fs::write(&copia, &doc).map_err(|e| ToolError::Failed(e.to_string()))?;
            Ok(ToolOutput {
                content: format!(
                    "widget publicado: {}/canvas/{}/{nome}",
                    self.base_url.trim_end_matches('/'),
                    ctx.task_id
                ),
                artifacts: vec![
                    crate::motor::artifact_for(&ctx.workdir, &rel)
                        .map_err(|e| ToolError::Failed(e.to_string()))?,
                ],
            })
        })
    }
}

// ---------------------------------------------------------------- servir

/// As duas rotas do canvas, juntadas ao roteador da API.
pub fn rotas() -> Router<ApiState> {
    Router::new()
        .route("/canvas/{id}/{nome}", get(hospedeira))
        .route("/canvas/{id}/{nome}/widget", get(widget))
}

fn nao_achado() -> Response {
    (StatusCode::NOT_FOUND, "widget inexistente").into_response()
}

fn ler(s: &ApiState, id: &str, nome: &str) -> Option<Value> {
    if !nome_valido(nome) {
        return None;
    }
    let b = std::fs::read(guardado(&s.store.dir(id), nome)).ok()?;
    serde_json::from_slice(&b).ok()
}

/// Pagina de fora: so um iframe com `sandbox="allow-scripts"`. O atributo repete o que o
/// cabecalho do widget ja impoe -- quem abrir a URL do widget direto continua isolado pelo
/// cabecalho, e quem o embutir aqui fica isolado pelos dois.
async fn hospedeira(
    State(s): State<ApiState>,
    Caminho((id, nome)): Caminho<(String, String)>,
) -> Response {
    if ler(&s, &id, &nome).is_none() {
        return nao_achado();
    }
    let pagina = format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{n}</title>\
<style>html,body{{margin:0;height:100%}}iframe{{border:0;width:100%;height:100%}}</style>\
</head><body><iframe sandbox=\"allow-scripts\" src=\"/canvas/{id}/{n}/widget\" \
title=\"{n}\"></iframe></body></html>\n",
        n = escapar_html(&nome),
        id = escapar_html(&id),
    );
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8".to_string()),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; style-src 'unsafe-inline'; frame-src 'self'; \
base-uri 'none'; form-action 'none'"
                    .to_string(),
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        pagina,
    )
        .into_response()
}

async fn widget(
    State(s): State<ApiState>,
    Caminho((id, nome)): Caminho<(String, String)>,
) -> Response {
    let Some(v) = ler(&s, &id, &nome) else {
        return nao_achado();
    };
    let (Some(doc), Some(hash)) = (
        v.get("html").and_then(Value::as_str),
        v.get("script_sha256").and_then(Value::as_str),
    ) else {
        return nao_achado();
    };
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8".to_string()),
            (header::CONTENT_SECURITY_POLICY, csp_do_widget(hash)),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            (header::REFERRER_POLICY, "no-referrer".to_string()),
        ],
        doc.to_string(),
    )
        .into_response()
}

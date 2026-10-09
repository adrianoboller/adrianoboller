//! O tunel remoto do editor: o terminal e o servidor de linguagem do projeto atravessam a
//! PONTE (`remoto.rs`) pelo mesmo canal multiplexado das rotas de tarefa, com a mesma
//! autenticacao -- o Bearer da ponte no cliente, o Bearer da API no agente.
//!
//! A ponte carrega pedido-resposta (um `device.command` por pedido), nao fluxo: por isso
//! estas rotas sao de pedido-resposta, e nao o websocket do `ide.rs`. O terminal remoto
//! executa UMA linha e devolve a saida; o LSP responde UMA consulta.
//!
//! Duas decisoes de seguranca, as mesmas do resto da casa:
//! - **O comando roda no bwrap do `shell`** (`run_in_workdir`), com a raiz do projeto em
//!   `/work`, sem rede. Terminal remoto fora do sandbox seria o unico codigo que chega pela
//!   rede com o disco inteiro do hospedeiro.
//! - **As regras de comando do projeto valem** (`.phxclaw/regras.json`, as mesmas do
//!   portao do motor): `negar` recusa, e `perguntar` tambem -- ninguem acompanha um
//!   pedido do tunel para responder a pergunta.
//!
//! O LSP e a MESMA `LspTool` do agente (so leitura, lista fechada de metodos), com a raiz
//! do projeto como pasta da tarefa.

use crate::api::ApiState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use phxclaw_agent_core::{Tool, ToolContext};
use phxclaw_sandbox::{WorkdirCommand, run_in_workdir};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

pub const ROTA_TERMINAL: &str = "/v1/tunel/terminal";
pub const ROTA_LSP: &str = "/v1/tunel/lsp";
/// Prazo de uma linha do terminal remoto; o cliente pode pedir menos, nunca mais.
pub const PRAZO_MAX: Duration = Duration::from_secs(60);
/// Teto da saida devolvida (a ponte ja limita o corpo; isto e o que o bwrap captura).
pub const SAIDA_MAX: usize = 256 * 1024;
/// A tarefa fixa das sessoes do LSP do tunel (uma por processo, encerrada com ele).
pub const TAREFA: &str = "tunel-remoto";

/// A recusa das funcoes de dentro: codigo e motivo; vira `Response` so no handler.
pub type Recusa = (StatusCode, String);

fn erro(status: StatusCode, m: impl Into<String>) -> Recusa {
    (status, m.into())
}

fn resposta(r: Recusa) -> Response {
    (r.0, Json(json!({"error": r.1}))).into_response()
}

fn raiz() -> Result<PathBuf, Recusa> {
    crate::montagem::raiz_do_projeto().ok_or_else(|| {
        erro(
            StatusCode::UNPROCESSABLE_ENTITY,
            "sem projeto (PHXCLAW_PROJETO ou a pasta corrente do agente)",
        )
    })
}

/// A recusa das regras do projeto para uma linha, se houver: a MESMA leitura do portao do
/// motor (`montagem::regras_do_projeto`, que diz por que vale sem confianca).
pub fn recusa_das_regras(linha: &str) -> Option<String> {
    let v = crate::montagem::regras_do_projeto()?.avaliar(linha);
    match v.decisao {
        crate::regras::Decisao::Permitir => None,
        crate::regras::Decisao::Perguntar => Some(format!(
            "a regra pede confirmacao e ninguem acompanha o tunel: {}",
            v.explicacao
        )),
        crate::regras::Decisao::Negar => Some(v.explicacao),
    }
}

/// Executa uma linha na raiz do projeto, no bwrap, e devolve `{exit_code, stdout, stderr,
/// truncada}`. E o que o terminal remoto do IDE chama, por fora da ponte.
pub async fn executar(linha: &str, prazo: Duration) -> Result<Value, Recusa> {
    let raiz = raiz()?;
    if linha.trim().is_empty() {
        return Err(erro(StatusCode::UNPROCESSABLE_ENTITY, "comando vazio"));
    }
    if let Some(m) = recusa_das_regras(linha) {
        return Err(erro(StatusCode::FORBIDDEN, m));
    }
    let Some(bwrap) = crate::arquivos::achar_bwrap() else {
        return Err(erro(
            StatusCode::SERVICE_UNAVAILABLE,
            "sem bwrap: o terminal remoto so roda no sandbox",
        ));
    };
    let cmd = WorkdirCommand {
        workdir: raiz,
        script: linha.to_string(),
        timeout: prazo.min(PRAZO_MAX),
        network: false,
        max_output_bytes: SAIDA_MAX,
    };
    let r = tokio::task::spawn_blocking(move || run_in_workdir(&bwrap, &cmd))
        .await
        .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| erro(StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(json!({
        "exit_code": r.exit_code,
        "stdout": r.stdout,
        "stderr": r.stderr,
        "truncada": r.truncated,
    }))
}

async fn terminal(State(s): State<ApiState>, h: HeaderMap, corpo: Json<Value>) -> Response {
    if let Err(e) = crate::api::auth(&s, &h) {
        return e.into_response();
    }
    let Some(linha) = corpo.get("comando").and_then(Value::as_str) else {
        return resposta(erro(
            StatusCode::UNPROCESSABLE_ENTITY,
            "esperado {\"comando\": linha}",
        ));
    };
    let prazo = corpo
        .get("prazo_s")
        .and_then(Value::as_u64)
        .map(Duration::from_secs)
        .unwrap_or(PRAZO_MAX);
    match executar(linha, prazo).await {
        Ok(v) => Json(v).into_response(),
        Err(r) => resposta(r),
    }
}

/// O LSP do hospedeiro, aberto uma vez por processo (as sessoes por linguagem vivem nele).
fn lsp() -> Option<Arc<crate::lsp::Lsp>> {
    static L: OnceLock<Option<Arc<crate::lsp::Lsp>>> = OnceLock::new();
    L.get_or_init(crate::lsp::Lsp::do_hospedeiro).clone()
}

/// Uma consulta ao servidor de linguagem (`action`, `path`, `line`, `column`, `query`),
/// pela MESMA ferramenta `lsp` do agente, na raiz do projeto.
pub async fn consultar(args: Value) -> Result<Value, Recusa> {
    let raiz = raiz()?;
    let Some(l) = lsp() else {
        return Err(erro(
            StatusCode::SERVICE_UNAVAILABLE,
            "sem servidor de linguagem no hospedeiro (ou sem bwrap)",
        ));
    };
    let ctx = ToolContext {
        task_id: TAREFA.into(),
        workdir: raiz,
        timeout: PRAZO_MAX,
    };
    let t = crate::lsp::LspTool { lsp: l };
    match t.run(args, &ctx).await {
        Ok(saida) => Ok(json!({"texto": saida.content})),
        Err(e) => Err(erro(StatusCode::UNPROCESSABLE_ENTITY, e.to_string())),
    }
}

async fn lsp_rota(State(s): State<ApiState>, h: HeaderMap, corpo: Json<Value>) -> Response {
    if let Err(e) = crate::api::auth(&s, &h) {
        return e.into_response();
    }
    match consultar(corpo.0).await {
        Ok(v) => Json(v).into_response(),
        Err(r) => resposta(r),
    }
}

/// `POST /v1/tunel/terminal` e `POST /v1/tunel/lsp`, com o Bearer da API; a ponte as
/// deixa passar (`remoto::permitido`).
pub fn rotas() -> Router<ApiState> {
    Router::new()
        .route(ROTA_TERMINAL, post(terminal))
        .route(ROTA_LSP, post(lsp_rota))
}

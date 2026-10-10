//! A rota REST de transcricao de voz (STT) que o microfone da vista de Conversa usa.
//!
//! `POST /v1/voz/transcrever`: corpo com o audio cru (`audio/webm;codecs=opus` ou
//! `audio/wav`), resposta `{"texto": ...}`. A autenticacao e o Bearer da API, como toda
//! rota de `/v1`, e o portao do RBAC pede a capacidade propria da rota (a linha em
//! `rbac::MATRIZ`): transcrever e uma OPERACAO, nao leitura, entao o leitor nao alcanca.
//!
//! Decisoes que valem saber:
//! - **o motor e o MESMO `TranscribeTool::transcrever`** da ferramenta `transcribe` e da
//!   conversa `phxclaw voz` -- nao ha um segundo whisper. A rota so entrega o audio a ele;
//! - **o audio recebido mora num temp EFEMERO e confinado** (`PastaTemp`), apagado ao sair
//!   do escopo mesmo no erro e no estouro do prazo. A pasta da tarefa nao entra aqui: a
//!   transcricao da Conversa nao e de tarefa nenhuma;
//! - **o perfil offline recusa a nuvem por nome**, pela guarda que o `transcrever` ja carrega
//!   (`PerfilDeVoz::recusa_rede`): o `do_ambiente` monta o provedor configurado, e a guarda
//!   decide se a rede pode ser tocada -- a rota nao reimplementa a decisao;
//! - **o teto do corpo e o `MAX_AUDIO_ENVIADO` da frente de voz** (entrada externa sem teto e
//!   um alto de memoria): o mesmo teto do audio que o `transcribe` manda para a nuvem;
//! - **o erro nunca leva caminho cru**: a falha do motor vira uma frase fixa, e so a recusa
//!   de configuracao e de perfil (que nomeiam a variavel ou o provedor, nao um arquivo) sai
//!   como veio.

use crate::api::ApiState;
use crate::elevenlabs::MAX_AUDIO_ENVIADO;
use crate::visao::{PastaTemp, TranscribeTool};
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use phxclaw_agent_core::ToolError;
use serde_json::json;
use std::time::Duration;

pub const ROTA: &str = "/v1/voz/transcrever";
/// Prazo de uma transcricao pela rota: uma fala do microfone e curta; alem disto o motor e
/// cortado (o `transcrever` passa o prazo ao whisper e ao filho).
pub const PRAZO: Duration = Duration::from_secs(120);

/// A recusa das funcoes de dentro: codigo e motivo; vira `Response` so no handler.
pub type Recusa = (StatusCode, String);

/// A extensao do arquivo temporario a partir do `content-type`, so os dois do contrato. Vira
/// extensao porque o motor (whisper local e o multipart da nuvem) nomeia o arquivo por ela.
/// `None`: tipo ausente ou fora do contrato.
fn extensao_do_tipo(content_type: Option<&str>) -> Option<&'static str> {
    let base = content_type?.split(';').next()?.trim().to_ascii_lowercase();
    match base.as_str() {
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("wav"),
        "audio/webm" => Some("webm"),
        _ => None,
    }
}

/// O `ToolError` do motor em recusa HTTP, SEM caminho cru: a falha do motor vira uma frase
/// fixa (a mensagem interna pode citar o arquivo temporario), e a recusa de configuracao e a
/// de perfil saem como vieram -- elas nomeiam a variavel ou o provedor, nunca um arquivo.
fn do_erro_do_motor(e: ToolError) -> Recusa {
    match e {
        ToolError::Denied(m) => (StatusCode::SERVICE_UNAVAILABLE, m),
        ToolError::InvalidArguments(m) => (StatusCode::UNPROCESSABLE_ENTITY, m),
        ToolError::Timeout(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            "a transcricao excedeu o prazo".into(),
        ),
        ToolError::Failed(_) => (StatusCode::BAD_GATEWAY, "a transcricao falhou".into()),
    }
}

/// Recebe o audio cru, grava num temp confinado e efemero, transcreve pelo MESMO
/// `TranscribeTool::transcrever` e devolve o texto; o temp se apaga ao sair daqui (o `Drop`
/// do `PastaTemp`), inclusive no erro. Nao le configuracao: quem monta o `ouvir` e a rota
/// (pelo `do_ambiente`) ou o teste (direto, com o motor dube).
pub async fn transcrever_audio(
    ouvir: &TranscribeTool,
    content_type: Option<&str>,
    audio: &[u8],
    idioma: Option<String>,
    prazo: Duration,
) -> Result<String, Recusa> {
    // O teto vem antes de tudo: entrada externa, e o audio grande se recusa antes de tocar o
    // disco ou o motor.
    if audio.len() as u64 > MAX_AUDIO_ENVIADO {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "audio com {} bytes, acima do teto de {MAX_AUDIO_ENVIADO}",
                audio.len()
            ),
        ));
    }
    if audio.is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "corpo de audio vazio".into(),
        ));
    }
    let ext = extensao_do_tipo(content_type).ok_or((
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "content-type do audio tem de ser audio/wav ou audio/webm".into(),
    ))?;
    let tmp = PastaTemp::nova("phx-stt-rest").map_err(do_erro_do_motor)?;
    let arquivo = tmp.0.join(format!("entrada.{ext}"));
    // O erro de gravacao nao leva o caminho: ele mora num temp que o cliente nem conhece.
    std::fs::write(&arquivo, audio).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "nao foi possivel gravar o audio recebido".to_string(),
        )
    })?;
    ouvir
        .transcrever(arquivo, idioma, prazo)
        .await
        .map(|t| t.trim().to_string())
        .map_err(do_erro_do_motor)
}

/// Confere o Bearer da API lendo SO os cabecalhos. Existe porque `Bytes` e um `FromRequest`
/// que materializa o corpo INTEIRO (ate o teto) ANTES do corpo do handler: sem este extrator,
/// um pedido sem credencial faria o servidor alocar os ~200 MiB so para responder 401 -- a
/// classe do alto de memoria pre-credencial (pedido 434). Como `FromRequestParts` corre sobre
/// as partes, ANTES de qualquer `FromRequest`, por aqui a recusa chega antes do primeiro byte
/// do corpo. A decisao e a MESMA `crate::api::auth`: nao se duplica a regra, so se muda a
/// ORDEM em que ela roda.
struct ApiAuth;

impl FromRequestParts<ApiState> for ApiAuth {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, s: &ApiState) -> Result<Self, Self::Rejection> {
        match crate::api::auth(s, &parts.headers) {
            Ok(()) => Ok(ApiAuth),
            Err(e) => Err(e.into_response()),
        }
    }
}

async fn transcrever(
    _: ApiAuth,
    State(s): State<ApiState>,
    h: HeaderMap,
    corpo: Bytes,
) -> Response {
    // O `ApiAuth` ja recusou o pedido sem credencial antes de o corpo ser coletado; a chamada
    // aqui fica como defesa em profundidade, pelo MESMO `auth`, para a rota nao depender da
    // ordem de extracao continuar valendo se a assinatura mudar amanha.
    if let Err(e) = crate::api::auth(&s, &h) {
        return e.into_response();
    }
    let content_type = h
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    // O motor sai do ambiente do agente (`voz.stt.provedor`, `voz.whisper.*`, `voz.perfil`):
    // o MESMO que a ferramenta `transcribe` monta. O perfil offline e honrado la dentro.
    let ouvir = TranscribeTool::do_ambiente(&crate::config::pasta_padrao());
    match transcrever_audio(&ouvir, content_type.as_deref(), &corpo, None, PRAZO).await {
        Ok(texto) => Json(json!({ "texto": texto })).into_response(),
        Err((codigo, motivo)) => (codigo, Json(json!({ "error": motivo }))).into_response(),
    }
}

/// `POST /v1/voz/transcrever`, com o Bearer da API. O teto do corpo e o do audio enviado
/// mais uma folga: assim o `transcrever_audio` e quem devolve a recusa com o motivo, em vez
/// de o corpo ser cortado pelo axum antes de chegar ao handler.
pub fn rotas() -> Router<ApiState> {
    Router::new()
        .route(ROTA, post(transcrever))
        .layer(DefaultBodyLimit::max(
            (MAX_AUDIO_ENVIADO + (1 << 20)) as usize,
        ))
}

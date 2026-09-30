use thiserror::Error;

/// Erros do navegador operado pelo agente. Tipados para que o chamador
/// distinga "a politica recusou" (decisao, nao falha) de "o Chromium caiu".
#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("chromium nao encontrado; caminhos tentados: {tried:?}")]
    ExecutableNotFound { tried: Vec<String> },
    #[error("falha ao lancar o chromium: {0}")]
    Launch(String),
    #[error("tempo esgotado em {operation} ({timeout_ms} ms)")]
    Timeout { operation: String, timeout_ms: u64 },
    #[error("websocket: {0}")]
    WebSocket(String),
    #[error("conexao com o chromium fechada")]
    ConnectionClosed,
    #[error("CDP {method} falhou: {message}")]
    Cdp { method: String, message: String },
    #[error("politica recusou {url}: {reason}")]
    PolicyDenied { url: String, reason: String },
    #[error("navegacao bloqueada pela politica em {url}: {reason}")]
    NavigationBlocked { url: String, reason: String },
    #[error("navegacao para {url} falhou: {error}")]
    Navigation { url: String, error: String },
    #[error("erro de javascript: {0}")]
    JavaScript(String),
    #[error("elemento nao encontrado: {0}")]
    ElementNotFound(String),
    #[error("resposta inesperada do CDP: {0}")]
    Protocol(String),
    #[error("E/S: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, BrowserError>;

pub(crate) fn timeout_err(operation: &str, timeout: std::time::Duration) -> BrowserError {
    BrowserError::Timeout {
        operation: operation.to_string(),
        timeout_ms: timeout.as_millis() as u64,
    }
}

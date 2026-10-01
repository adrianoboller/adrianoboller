#![forbid(unsafe_code)]
//! Contrato comum do agente autonomo do PhxClaw: mensagens, chamadas de ferramenta, o
//! trait do modelo de linguagem e o trait da ferramenta.
//!
//! Existe para que o motor do agente, os provedores (Ollama, OpenAI, Anthropic, Gemini) e
//! as ferramentas (navegador, busca, shell, documentos) falem UMA lingua so. Cada provedor
//! traduz o seu formato de fio para estes tipos; o motor nunca ve o formato de fio.
//!
//! Os traits devolvem `BoxFut` em vez de `async fn` para continuarem usaveis como `dyn`:
//! o motor guarda `Arc<dyn Llm>` e `Arc<dyn Tool>` escolhidos em tempo de execucao.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use thiserror::Error;

pub mod tarefa;

pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
    /// Resultado de uma ferramenta, devolvido ao modelo.
    Tool,
}

/// Pedido do modelo para rodar uma ferramenta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Identificador que o provedor usa para casar o resultado com o pedido.
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    /// So em mensagens do assistente.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// So em mensagens `Tool`: a qual `ToolCall.id` este resultado responde.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// So em mensagens `Tool`: o nome da ferramenta (alguns provedores exigem).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// Imagens que o modelo VE junto do texto (so em mensagens `User`). Cada provedor as
    /// traduz para o bloco de imagem dele; o OCR continua existindo, mas texto extraido
    /// nao e a imagem -- grafico, layout e foto se perdem nele.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImagemAnexa>,
}

/// Uma imagem anexada a mensagem: o tipo e os bytes em base64, que e o que os quatro
/// provedores pedem no fio. Guardar base64 e nao bytes poupa recodificar a cada passo,
/// porque o historico inteiro viaja de novo em toda volta do laco.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImagemAnexa {
    /// `image/png`, `image/jpeg`, `image/gif` ou `image/webp`.
    pub media_type: String,
    pub base64: String,
}

impl Message {
    pub fn system(t: impl Into<String>) -> Self {
        Self::plain(Role::System, t)
    }
    pub fn user(t: impl Into<String>) -> Self {
        Self::plain(Role::User, t)
    }
    pub fn user_com_imagens(t: impl Into<String>, images: Vec<ImagemAnexa>) -> Self {
        Self {
            images,
            ..Self::plain(Role::User, t)
        }
    }
    pub fn assistant(t: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            tool_calls,
            ..Self::plain(Role::Assistant, t)
        }
    }
    pub fn tool_result(call: &ToolCall, t: impl Into<String>) -> Self {
        Self {
            tool_call_id: Some(call.id.clone()),
            tool_name: Some(call.name.clone()),
            ..Self::plain(Role::Tool, t)
        }
    }
    fn plain(role: Role, t: impl Into<String>) -> Self {
        Self {
            role,
            content: t.into(),
            tool_calls: vec![],
            tool_call_id: None,
            tool_name: None,
            images: vec![],
        }
    }
}

/// Descricao de ferramenta no formato JSON Schema que os quatro provedores aceitam.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema (type=object) dos argumentos.
    pub parameters: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Tempo que o provedor diz ter gastado GERANDO os `output_tokens`, em ns (o
    /// `eval_duration` do Ollama). So existe onde o provedor mede: tokens/s calculado pelo
    /// relogio de parede misturaria rede e fila com geracao, e os de nuvem nao informam.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duracao_geracao_ns: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmReply {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Usage,
    /// Nome do modelo que respondeu, como o provedor o informou.
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmOptions {
    pub max_output_tokens: u32,
    pub temperature: f32,
}

impl Default for LlmOptions {
    fn default() -> Self {
        // Limite sempre presente: medido nesta base, geracao sem teto prende o provedor ate
        // o timeout (Ollama gerou 2.581+ tokens em 2 min a 0,8 de temperatura).
        Self {
            max_output_tokens: 1024,
            temperature: 0.2,
        }
    }
}

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("provider transport error: {0}")]
    Transport(String),
    #[error("provider returned status {status}: {body}")]
    Api { status: u16, body: String },
    #[error("provider response could not be parsed: {0}")]
    Parse(String),
    #[error("provider denied by policy: {0}")]
    Denied(String),
    #[error("credential unavailable: {0}")]
    Credential(String),
}

/// Um modelo de linguagem, qualquer que seja o provedor.
pub trait Llm: Send + Sync {
    /// Identificador estavel, ex.: "ollama:qwen2.5:1.5b", "openai:gpt-5".
    fn id(&self) -> String;
    fn chat<'a>(
        &'a self,
        messages: &'a [Message],
        tools: &'a [ToolSpec],
        options: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>>;
}

/// Artefato produzido por uma ferramenta (arquivo gerado, captura, pagina baixada).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artifact {
    /// Caminho relativo ao diretorio da tarefa.
    pub path: String,
    pub media_type: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolOutput {
    /// Texto devolvido ao modelo (ja truncado pela ferramenta a um tamanho razoavel).
    pub content: String,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
}

impl ToolOutput {
    pub fn text(t: impl Into<String>) -> Self {
        Self {
            content: t.into(),
            artifacts: vec![],
        }
    }
}

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("denied by policy: {0}")]
    Denied(String),
    #[error("tool failed: {0}")]
    Failed(String),
    #[error("tool timed out after {0} ms")]
    Timeout(u64),
}

/// Contexto de uma execucao de ferramenta: onde gravar e o que e permitido.
#[derive(Debug, Clone)]
pub struct ToolContext {
    pub task_id: String,
    /// Diretorio de trabalho da tarefa; ferramentas nunca escrevem fora dele.
    pub workdir: std::path::PathBuf,
    /// Prazo da chamada. A ferramenta que lanca processo o repassa ao processo: o motor
    /// cortar so o futuro deixava o filho rodando (medido: `sleep 5` sobreviveu ao corte).
    pub timeout: std::time::Duration,
}

pub trait Tool: Send + Sync {
    fn spec(&self) -> ToolSpec;
    /// Capacidade que a politica tem de conceder para a ferramenta rodar
    /// (ex.: "web.browse", "web.search", "shell.exec", "fs.write", "doc.write").
    fn capability(&self) -> &'static str;
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>>;
    /// Chamado quando a tarefa termina, em qualquer estado: a ferramenta que segura recurso
    /// por tarefa (navegador, processo) solta aqui. Padrao: nada a soltar.
    fn finish<'a>(&'a self, _task_id: &'a str) -> BoxFut<'a, ()> {
        Box::pin(async {})
    }
    /// A linha de shell que esta chamada executaria, para as regras por comando
    /// (`permitir/negar/perguntar`) a conferirem no portao do motor ANTES de rodar. Toda
    /// ferramenta que executa texto do modelo num shell responde aqui; as outras, `None`.
    fn comando_de_shell(&self, _args: &Value) -> Option<String> {
        None
    }
}

/// Trunca texto para devolver ao modelo sem estourar o contexto, cortando em fronteira
/// de caractere e dizendo que cortou.
pub fn truncate_for_model(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let cut: String = text.chars().take(max_chars).collect();
    format!(
        "{cut}\n[... truncado: {} caracteres no total]",
        text.chars().count()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncar_respeita_caractere_e_avisa() {
        let t = "ação".repeat(10);
        let r = truncate_for_model(&t, 5);
        assert!(r.starts_with("açãoa"));
        assert!(r.contains("40 caracteres"));
        assert_eq!(truncate_for_model("curto", 10), "curto");
    }

    #[test]
    fn resultado_de_ferramenta_carrega_o_id_da_chamada() {
        let c = ToolCall {
            id: "c1".into(),
            name: "web_search".into(),
            arguments: serde_json::json!({}),
        };
        let m = Message::tool_result(&c, "ok");
        assert_eq!(m.tool_call_id.as_deref(), Some("c1"));
        assert_eq!(m.role, Role::Tool);
    }
}

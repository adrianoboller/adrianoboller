#![forbid(unsafe_code)]
//! Provedores de modelo de linguagem do agente: Ollama, OpenAI (Responses), Anthropic
//! (Messages) e Gemini (generateContent).
//!
//! Cada provedor traduz o contrato de `phxclaw-agent-core` para o formato de fio dele e de
//! volta, inclusive a chamada de ferramenta nos dois sentidos. A seguranca (chave redigida,
//! origem conferida, https, timeout, teto) nao mora nos provedores: mora em `seguranca` e
//! `transporte`, por onde os quatro passam.

mod anthropic;
mod gemini;
mod ollama;
mod openai;
mod seguranca;
mod transporte;

pub use anthropic::AnthropicLlm;
pub use gemini::GeminiLlm;
pub use ollama::OllamaLlm;
pub use openai::OpenAiLlm;
pub use seguranca::{ApiKey, Endpoint};

use phxclaw_agent_core::{Llm, LlmError};
use std::sync::Arc;

/// Host padrao do Ollama quando `OLLAMA_HOST` nao existe.
pub const OLLAMA_PADRAO: &str = "http://127.0.0.1:11434";

/// Monta um provedor a partir de `"provedor:modelo"`. A chave dos de nuvem vem de `chave`,
/// chamada com o nome do provedor (`openai`, `anthropic`, `gemini`): esta crate NAO le
/// chave do ambiente -- o agente a traz do SecretBroker, e so o comando `phxclaw <provedor>
/// chave` le a variavel, para grava-la la. O Ollama nao tem chave e nao chama `chave`.
///
/// Os de nuvem usam SEMPRE a origem oficial: nenhuma variavel de ambiente desvia a chave
/// para outro servidor. Origem customizada so por codigo, com `Endpoint::custom`.
pub fn montar(
    spec: &str,
    chave: impl FnOnce(&str) -> Result<String, LlmError>,
) -> Result<Arc<dyn Llm>, LlmError> {
    let (provedor, modelo) = spec
        .split_once(':')
        .ok_or_else(|| LlmError::Denied(format!("spec sem provedor: {spec:?}")))?;
    Ok(match provedor {
        "ollama" => Arc::new(ollama_do_ambiente(modelo)?),
        "openai" => Arc::new(OpenAiLlm::new(
            chave("openai")?,
            modelo,
            Endpoint::official(),
        )?),
        "anthropic" => Arc::new(AnthropicLlm::new(
            chave("anthropic")?,
            modelo,
            Endpoint::official(),
        )?),
        "gemini" => Arc::new(GeminiLlm::new(
            chave("gemini")?,
            modelo,
            Endpoint::official(),
        )?),
        outro => return Err(LlmError::Denied(format!("provedor desconhecido: {outro}"))),
    })
}

/// O Ollama de `OLLAMA_HOST` com o tipo concreto: quem precisa do que so ele tem (os
/// embeddings) passa pela MESMA leitura do ambiente que o `from_env`.
pub fn ollama_do_ambiente(modelo: &str) -> Result<OllamaLlm, LlmError> {
    let host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| OLLAMA_PADRAO.into());
    OllamaLlm::new(&normalizar_host_ollama(&host), modelo)
}

/// O proprio Ollama aceita `OLLAMA_HOST=127.0.0.1:11434`, sem esquema; sem este ajuste o
/// mesmo ambiente que serve o Ollama quebraria o cliente.
fn normalizar_host_ollama(h: &str) -> String {
    let h = h.trim();
    if h.contains("://") {
        h.to_owned()
    } else {
        format!("http://{h}")
    }
}

/// Id para chamada de ferramenta quando o provedor nao manda um (Ollama antigo, Gemini).
pub(crate) fn novo_id() -> String {
    format!("call_{}", uuid::Uuid::now_v7().simple())
}

/// `max_output_tokens = 0` significaria "sem teto" em alguns provedores e erro em outros;
/// o teto e sempre aplicado, entao zero vira um.
pub(crate) fn teto_tokens(o: &phxclaw_agent_core::LlmOptions) -> u32 {
    o.max_output_tokens.max(1)
}

/// Argumentos que chegam como texto JSON (OpenAI) viram objeto; texto que nao e JSON fica
/// como `Value::String` para a ferramenta recusar com `InvalidArguments` e o modelo ver o
/// motivo, em vez de o turno inteiro morrer num erro de parse.
pub(crate) fn argumentos_de_texto(t: &str) -> serde_json::Value {
    if t.trim().is_empty() {
        return serde_json::json!({});
    }
    serde_json::from_str(t).unwrap_or_else(|_| serde_json::Value::String(t.to_owned()))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn spec_desconhecida_e_recusada_e_nuvem_pede_a_chave_pelo_nome() {
        let sem = |_: &str| Err(LlmError::Credential("nenhuma".into()));
        assert!(matches!(montar("xpto:m", sem), Err(LlmError::Denied(_))));
        assert!(matches!(
            montar("semdoispontos", sem),
            Err(LlmError::Denied(_))
        ));
        let mut pedido = String::new();
        let r = montar("anthropic:m", |p| {
            pedido = p.to_string();
            Ok("chave-de-teste".into())
        });
        assert!(r.is_ok());
        assert_eq!(pedido, "anthropic");
        // Sem chave, nenhum provedor de nuvem nasce; o erro e o de credencial.
        assert!(matches!(
            montar("openai:m", sem),
            Err(LlmError::Credential(_))
        ));
    }

    #[test]
    fn host_do_ollama_sem_esquema_ganha_http() {
        assert_eq!(
            normalizar_host_ollama("127.0.0.1:11434"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(normalizar_host_ollama("https://x:1"), "https://x:1");
    }

    #[test]
    fn argumentos_invalidos_viram_texto() {
        assert_eq!(
            argumentos_de_texto(r#"{"q":1}"#),
            serde_json::json!({"q":1})
        );
        assert_eq!(argumentos_de_texto(""), serde_json::json!({}));
        assert_eq!(
            argumentos_de_texto("{quebrado"),
            serde_json::json!("{quebrado")
        );
    }
}

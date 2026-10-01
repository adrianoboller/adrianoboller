//! Anthropic, Messages API: `POST {base}/v1/messages`.

use crate::seguranca::{ApiKey, Endpoint, validar_modelo};
use crate::transporte::{TIMEOUT_NUVEM, Transporte};
use crate::{novo_id, teto_tokens};
use phxclaw_agent_core::{
    BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, Role, ToolCall, ToolSpec, Usage,
};
use serde_json::{Value, json};
use std::time::Duration;

pub(crate) const ORIGEM_ANTHROPIC: &str = "https://api.anthropic.com";
const VERSAO_API: &str = "2023-06-01";

#[derive(Debug, Clone)]
pub struct AnthropicLlm {
    chave: ApiKey,
    modelo: String,
    url: String,
    transporte: Transporte,
}

impl AnthropicLlm {
    pub fn new(api_key: String, model: &str, endpoint: Endpoint) -> Result<Self, LlmError> {
        Self::with_timeout(api_key, model, endpoint, TIMEOUT_NUVEM)
    }

    pub fn with_timeout(
        api_key: String,
        model: &str,
        endpoint: Endpoint,
        timeout: Duration,
    ) -> Result<Self, LlmError> {
        validar_modelo(model)?;
        let base = endpoint.resolver(ORIGEM_ANTHROPIC)?;
        Ok(Self {
            chave: ApiKey::new(api_key)?,
            modelo: model.to_owned(),
            url: format!("{}/v1/messages", base.texto),
            transporte: Transporte::novo(&base, timeout)?,
        })
    }

    fn corpo(&self, mensagens: &[Message], tools: &[ToolSpec], o: &LlmOptions) -> Value {
        let mut sistema: Vec<&str> = Vec::new();
        let mut fio: Vec<Value> = Vec::new();
        for m in mensagens {
            match m.role {
                // A API nao tem papel system nas mensagens: o texto vai no campo proprio.
                Role::System => sistema.push(&m.content),
                Role::User => {
                    // Imagem antes do texto: e a ordem que a documentacao da Messages API
                    // recomenda, o texto pergunta sobre o que ja foi visto.
                    for i in &m.images {
                        empilhar(
                            &mut fio,
                            "user",
                            json!({"type": "image", "source": {
                                "type": "base64", "media_type": i.media_type, "data": i.base64,
                            }}),
                        );
                    }
                    empilhar(&mut fio, "user", json!({"type": "text", "text": m.content}))
                }
                Role::Assistant => {
                    // Bloco de texto vazio e recusado pela API; so entra se houver texto.
                    if !m.content.is_empty() {
                        empilhar(
                            &mut fio,
                            "assistant",
                            json!({"type": "text", "text": m.content}),
                        );
                    }
                    for c in &m.tool_calls {
                        empilhar(
                            &mut fio,
                            "assistant",
                            json!({"type": "tool_use", "id": c.id, "name": c.name, "input": c.arguments}),
                        );
                    }
                }
                Role::Tool => empilhar(
                    &mut fio,
                    "user",
                    json!({
                        "type": "tool_result",
                        "tool_use_id": m.tool_call_id.clone().unwrap_or_default(),
                        "content": m.content,
                    }),
                ),
            }
        }
        let mut corpo = json!({
            "model": self.modelo,
            "max_tokens": teto_tokens(o),
            "messages": fio,
            // A faixa da Anthropic e 0..1; o contrato admite ate 2 (OpenAI/Gemini).
            "temperature": o.temperature.clamp(0.0, 1.0),
        });
        if !sistema.is_empty() {
            corpo["system"] = json!(sistema.join("\n\n"));
        }
        if !tools.is_empty() {
            corpo["tools"] = tools
                .iter()
                .map(|t| {
                    json!({"name": t.name, "description": t.description, "input_schema": t.parameters})
                })
                .collect();
        }
        corpo
    }
}

/// Mensagens seguidas do mesmo papel viram uma so com varios blocos: os resultados de
/// chamadas paralelas tem de chegar juntos na mesma mensagem user que segue o tool_use.
fn empilhar(fio: &mut Vec<Value>, papel: &str, bloco: Value) {
    if let Some(ultima) = fio.last_mut()
        && ultima["role"] == papel
        && let Some(blocos) = ultima["content"].as_array_mut()
    {
        blocos.push(bloco);
        return;
    }
    fio.push(json!({"role": papel, "content": [bloco]}));
}

fn resposta(v: &Value, modelo: &str) -> Result<LlmReply, LlmError> {
    let blocos = v["content"]
        .as_array()
        .ok_or_else(|| LlmError::Parse("resposta da Anthropic sem 'content'".into()))?;
    let mut texto = String::new();
    let mut chamadas = Vec::new();
    for b in blocos {
        match b["type"].as_str() {
            Some("text") => texto.push_str(b["text"].as_str().unwrap_or_default()),
            Some("tool_use") => {
                let nome = b["name"]
                    .as_str()
                    .ok_or_else(|| LlmError::Parse("tool_use sem name".into()))?;
                chamadas.push(ToolCall {
                    id: b["id"].as_str().map_or_else(novo_id, str::to_owned),
                    name: nome.to_owned(),
                    arguments: if b["input"].is_null() {
                        json!({})
                    } else {
                        b["input"].clone()
                    },
                });
            }
            _ => {}
        }
    }
    Ok(LlmReply {
        content: texto,
        tool_calls: chamadas,
        usage: Usage {
            input_tokens: v["usage"]["input_tokens"].as_u64().unwrap_or(0),
            output_tokens: v["usage"]["output_tokens"].as_u64().unwrap_or(0),
            duracao_geracao_ns: None,
        },
        model: v["model"].as_str().unwrap_or(modelo).to_owned(),
    })
}

impl Llm for AnthropicLlm {
    fn id(&self) -> String {
        format!("anthropic:{}", self.modelo)
    }

    fn chat<'a>(
        &'a self,
        messages: &'a [Message],
        tools: &'a [ToolSpec],
        options: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async move {
            let corpo = self.corpo(messages, tools, options);
            let v = self
                .transporte
                .post_json(
                    &self.url,
                    &[
                        ("x-api-key", self.chave.expor()),
                        ("anthropic-version", VERSAO_API),
                    ],
                    Some(&self.chave),
                    &corpo,
                )
                .await?;
            resposta(&v, &self.modelo)
        })
    }
}

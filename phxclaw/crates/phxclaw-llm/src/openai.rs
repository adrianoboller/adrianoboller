//! OpenAI, Responses API: `POST {base}/v1/responses`.

use crate::seguranca::{ApiKey, Endpoint, validar_modelo};
use crate::transporte::{TIMEOUT_NUVEM, Transporte};
use crate::{argumentos_de_texto, teto_tokens};
use phxclaw_agent_core::{
    BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, Role, ToolCall, ToolSpec, Usage,
};
use serde_json::{Value, json};
use std::time::Duration;

pub(crate) const ORIGEM_OPENAI: &str = "https://api.openai.com";

#[derive(Debug, Clone)]
pub struct OpenAiLlm {
    chave: ApiKey,
    modelo: String,
    url: String,
    transporte: Transporte,
}

impl OpenAiLlm {
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
        let base = endpoint.resolver(ORIGEM_OPENAI)?;
        Ok(Self {
            chave: ApiKey::new(api_key)?,
            modelo: model.to_owned(),
            url: format!("{}/v1/responses", base.texto),
            transporte: Transporte::novo(&base, timeout)?,
        })
    }

    fn corpo(&self, mensagens: &[Message], tools: &[ToolSpec], o: &LlmOptions) -> Value {
        let mut entrada = Vec::new();
        for m in mensagens {
            match m.role {
                Role::System => entrada.push(json!({"role": "system", "content": m.content})),
                Role::User if m.images.is_empty() => {
                    entrada.push(json!({"role": "user", "content": m.content}))
                }
                // Com imagem o conteudo vira lista de partes (`input_text` + `input_image`
                // com URL `data:`): a Responses API nao aceita imagem em texto simples.
                Role::User => {
                    let mut partes = vec![json!({"type": "input_text", "text": m.content})];
                    partes.extend(m.images.iter().map(|i| {
                        json!({"type": "input_image",
                               "image_url": format!("data:{};base64,{}", i.media_type, i.base64)})
                    }));
                    entrada.push(json!({"role": "user", "content": partes}));
                }
                Role::Assistant => {
                    if !m.content.is_empty() {
                        entrada.push(json!({"role": "assistant", "content": m.content}));
                    }
                    // O item function_call tem de voltar no input: sem ele o
                    // function_call_output seguinte nao tem a que responder e a API recusa.
                    for c in &m.tool_calls {
                        entrada.push(json!({
                            "type": "function_call",
                            "call_id": c.id,
                            "name": c.name,
                            "arguments": argumentos_como_texto(&c.arguments),
                        }));
                    }
                }
                Role::Tool => entrada.push(json!({
                    "type": "function_call_output",
                    "call_id": m.tool_call_id.clone().unwrap_or_default(),
                    "output": m.content,
                })),
            }
        }
        let mut corpo = json!({
            "model": self.modelo,
            "input": entrada,
            "max_output_tokens": teto_tokens(o),
            "temperature": o.temperature,
            // O historico viaja inteiro a cada pedido; guardar do lado deles so duplicaria o
            // que as ferramentas leram num servidor que nao e nosso.
            "store": false,
        });
        if !tools.is_empty() {
            corpo["tools"] = tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                        // Na Responses API o strict nasce ligado e recusa esquema que nao
                        // declare additionalProperties=false e todos os campos required; o
                        // contrato aceita JSON Schema comum, entao desliga-se explicitamente.
                        "strict": false,
                    })
                })
                .collect();
        }
        corpo
    }
}

/// O contrato guarda objeto; o fio da OpenAI quer texto JSON. `Value::String` volta como
/// estava (foi o texto que nao era JSON, preservado por `argumentos_de_texto`).
fn argumentos_como_texto(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        outro => outro.to_string(),
    }
}

fn resposta(v: &Value, modelo: &str) -> Result<LlmReply, LlmError> {
    let saida = v["output"]
        .as_array()
        .ok_or_else(|| LlmError::Parse("resposta da OpenAI sem 'output'".into()))?;
    let mut texto = String::new();
    let mut chamadas = Vec::new();
    for item in saida {
        match item["type"].as_str() {
            Some("message") => {
                for c in item["content"].as_array().into_iter().flatten() {
                    let t = match c["type"].as_str() {
                        Some("output_text") => c["text"].as_str(),
                        Some("refusal") => c["refusal"].as_str(),
                        _ => None,
                    };
                    texto.push_str(t.unwrap_or_default());
                }
            }
            Some("function_call") => {
                let id = item["call_id"]
                    .as_str()
                    .ok_or_else(|| LlmError::Parse("function_call sem call_id".into()))?;
                let nome = item["name"]
                    .as_str()
                    .ok_or_else(|| LlmError::Parse("function_call sem name".into()))?;
                chamadas.push(ToolCall {
                    id: id.to_owned(),
                    name: nome.to_owned(),
                    arguments: argumentos_de_texto(item["arguments"].as_str().unwrap_or("")),
                });
            }
            // reasoning e outros itens nao sao texto nem ferramenta para o motor.
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

impl Llm for OpenAiLlm {
    fn id(&self) -> String {
        format!("openai:{}", self.modelo)
    }

    fn chat<'a>(
        &'a self,
        messages: &'a [Message],
        tools: &'a [ToolSpec],
        options: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async move {
            let corpo = self.corpo(messages, tools, options);
            let auth = format!("Bearer {}", self.chave.expor());
            let v = self
                .transporte
                .post_json(
                    &self.url,
                    &[("authorization", &auth)],
                    Some(&self.chave),
                    &corpo,
                )
                .await?;
            resposta(&v, &self.modelo)
        })
    }
}

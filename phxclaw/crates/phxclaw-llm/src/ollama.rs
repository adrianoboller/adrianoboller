//! Ollama: `POST {base}/api/chat`, sem stream.

use crate::seguranca::validar_base;
use crate::transporte::{TIMEOUT_LOCAL, Transporte};
use crate::{argumentos_de_texto, novo_id, teto_tokens};
use phxclaw_agent_core::{
    BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, Role, ToolCall, ToolSpec, Usage,
};
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct OllamaLlm {
    url: String,
    modelo: String,
    transporte: Transporte,
}

impl OllamaLlm {
    /// `http` so em loopback: o servidor do Ollama so fala http, entao a permissao de
    /// loopback vem implicita na propria base; fora da maquina exige https, porque o
    /// prompt do agente carrega o que as ferramentas leram.
    pub fn new(base_url: &str, model: &str) -> Result<Self, LlmError> {
        Self::with_timeout(base_url, model, TIMEOUT_LOCAL)
    }

    pub fn with_timeout(base_url: &str, model: &str, timeout: Duration) -> Result<Self, LlmError> {
        if model.is_empty() || model.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return Err(LlmError::Denied(format!(
                "nome de modelo recusado: {model:?}"
            )));
        }
        let base = validar_base(base_url, true)?;
        Ok(Self {
            url: format!("{}/api/chat", base.texto),
            modelo: model.to_owned(),
            transporte: Transporte::novo(&base, timeout)?,
        })
    }

    fn corpo(&self, mensagens: &[Message], tools: &[ToolSpec], o: &LlmOptions) -> Value {
        let mut corpo = json!({
            "model": self.modelo,
            "messages": mensagens.iter().map(mensagem).collect::<Vec<_>>(),
            "stream": false,
            "options": { "temperature": o.temperature, "num_predict": teto_tokens(o) },
        });
        if !tools.is_empty() {
            corpo["tools"] = tools
                .iter()
                .map(|t| {
                    json!({"type": "function", "function": {
                        "name": t.name, "description": t.description, "parameters": t.parameters,
                    }})
                })
                .collect();
        }
        corpo
    }
}

fn mensagem(m: &Message) -> Value {
    match m.role {
        Role::System => json!({"role": "system", "content": m.content}),
        Role::User => json!({"role": "user", "content": m.content}),
        Role::Assistant => {
            let mut v = json!({"role": "assistant", "content": m.content});
            if !m.tool_calls.is_empty() {
                v["tool_calls"] = m
                    .tool_calls
                    .iter()
                    .map(|c| {
                        json!({"id": c.id, "function": {"name": c.name, "arguments": c.arguments}})
                    })
                    .collect();
            }
            v
        }
        Role::Tool => {
            let mut v = json!({"role": "tool", "content": m.content});
            if let Some(n) = &m.tool_name {
                v["tool_name"] = json!(n);
            }
            v
        }
    }
}

fn resposta(v: &Value, modelo: &str) -> Result<LlmReply, LlmError> {
    let msg = v
        .get("message")
        .ok_or_else(|| LlmError::Parse("resposta do Ollama sem 'message'".into()))?;
    let mut chamadas = Vec::new();
    for c in msg["tool_calls"].as_array().into_iter().flatten() {
        let f = &c["function"];
        let nome = f["name"]
            .as_str()
            .ok_or_else(|| LlmError::Parse("tool_call do Ollama sem nome".into()))?;
        // Versoes antigas nao mandam id; o motor precisa de um para casar o resultado.
        let id = c["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map_or_else(novo_id, str::to_owned);
        let args = match &f["arguments"] {
            Value::String(s) => argumentos_de_texto(s),
            Value::Null => json!({}),
            outro => outro.clone(),
        };
        chamadas.push(ToolCall {
            id,
            name: nome.to_owned(),
            arguments: args,
        });
    }
    Ok(LlmReply {
        content: msg["content"].as_str().unwrap_or_default().to_owned(),
        tool_calls: chamadas,
        usage: Usage {
            input_tokens: v["prompt_eval_count"].as_u64().unwrap_or(0),
            output_tokens: v["eval_count"].as_u64().unwrap_or(0),
        },
        model: v["model"].as_str().unwrap_or(modelo).to_owned(),
    })
}

impl Llm for OllamaLlm {
    fn id(&self) -> String {
        format!("ollama:{}", self.modelo)
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
                .post_json(&self.url, &[], None, &corpo)
                .await?;
            resposta(&v, &self.modelo)
        })
    }
}

//! Gemini: `POST {base}/v1beta/models/{modelo}:generateContent`.

use crate::seguranca::{ApiKey, Endpoint, validar_modelo};
use crate::transporte::{TIMEOUT_NUVEM, Transporte};
use crate::{novo_id, teto_tokens};
use phxclaw_agent_core::{
    BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, Role, ToolCall, ToolSpec, Usage,
};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::Duration;

pub(crate) const ORIGEM_GEMINI: &str = "https://generativelanguage.googleapis.com";
/// Assinaturas guardadas; um agente raramente tem mais que algumas dezenas de chamadas vivas
/// no historico, e o teto impede que uma sessao longa cresca sem limite.
const TETO_ASSINATURAS: usize = 1024;

#[derive(Debug)]
pub struct GeminiLlm {
    chave: ApiKey,
    modelo: String,
    url: String,
    transporte: Transporte,
    /// `thoughtSignature` por id de chamada. Os modelos com raciocinio devolvem a assinatura
    /// junto do functionCall e exigem que ela volte no historico; o contrato nao tem campo
    /// para isso, entao ela fica aqui, casada pelo id que este provedor mesmo gerou.
    assinaturas: Mutex<Assinaturas>,
}

#[derive(Debug, Default)]
struct Assinaturas {
    mapa: HashMap<String, String>,
    ordem: VecDeque<String>,
}

impl Assinaturas {
    fn guardar(&mut self, id: String, sig: String) {
        if self.mapa.insert(id.clone(), sig).is_none() {
            self.ordem.push_back(id);
        }
        while self.ordem.len() > TETO_ASSINATURAS {
            if let Some(velho) = self.ordem.pop_front() {
                self.mapa.remove(&velho);
            }
        }
    }
}

impl GeminiLlm {
    pub fn new(api_key: String, model: &str, endpoint: Endpoint) -> Result<Self, LlmError> {
        Self::with_timeout(api_key, model, endpoint, TIMEOUT_NUVEM)
    }

    pub fn with_timeout(
        api_key: String,
        model: &str,
        endpoint: Endpoint,
        timeout: Duration,
    ) -> Result<Self, LlmError> {
        // O modelo entra no CAMINHO da URL; validar_modelo impede que ele mude o destino.
        validar_modelo(model)?;
        let base = endpoint.resolver(ORIGEM_GEMINI)?;
        Ok(Self {
            chave: ApiKey::new(api_key)?,
            modelo: model.to_owned(),
            url: format!("{}/v1beta/models/{model}:generateContent", base.texto),
            transporte: Transporte::novo(&base, timeout)?,
            assinaturas: Mutex::default(),
        })
    }

    fn corpo(&self, mensagens: &[Message], tools: &[ToolSpec], o: &LlmOptions) -> Value {
        let assinaturas = self.assinaturas.lock().unwrap_or_else(|e| e.into_inner());
        let mut sistema: Vec<Value> = Vec::new();
        let mut conteudos: Vec<Value> = Vec::new();
        for m in mensagens {
            match m.role {
                Role::System => sistema.push(json!({"text": m.content})),
                Role::User => empilhar(&mut conteudos, "user", json!({"text": m.content})),
                Role::Assistant => {
                    if !m.content.is_empty() {
                        empilhar(&mut conteudos, "model", json!({"text": m.content}));
                    }
                    for c in &m.tool_calls {
                        let mut parte =
                            json!({"functionCall": {"name": c.name, "args": c.arguments}});
                        if let Some(sig) = assinaturas.mapa.get(&c.id) {
                            parte["thoughtSignature"] = json!(sig);
                        }
                        empilhar(&mut conteudos, "model", parte);
                    }
                }
                // O Gemini casa resultado com chamada pelo NOME, e nao pelo id.
                Role::Tool => empilhar(
                    &mut conteudos,
                    "user",
                    json!({"functionResponse": {
                        "name": m.tool_name.clone().unwrap_or_default(),
                        "response": {"content": m.content},
                    }}),
                ),
            }
        }
        let mut corpo = json!({
            "contents": conteudos,
            "generationConfig": {
                "temperature": o.temperature,
                "maxOutputTokens": teto_tokens(o),
            },
        });
        if !sistema.is_empty() {
            corpo["systemInstruction"] = json!({"parts": sistema});
        }
        if !tools.is_empty() {
            let decl: Vec<Value> = tools
                .iter()
                .map(|t| {
                    json!({"name": t.name, "description": t.description, "parameters": t.parameters})
                })
                .collect();
            corpo["tools"] = json!([{"functionDeclarations": decl}]);
        }
        corpo
    }

    fn resposta(&self, v: &Value) -> Result<LlmReply, LlmError> {
        let cand = v["candidates"]
            .get(0)
            .ok_or_else(|| LlmError::Parse(motivo_sem_candidato(v)))?;
        let mut texto = String::new();
        let mut chamadas = Vec::new();
        let mut assinaturas = self.assinaturas.lock().unwrap_or_else(|e| e.into_inner());
        for p in cand["content"]["parts"].as_array().into_iter().flatten() {
            if let Some(fc) = p.get("functionCall") {
                let nome = fc["name"]
                    .as_str()
                    .ok_or_else(|| LlmError::Parse("functionCall sem name".into()))?;
                let id = fc["id"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map_or_else(novo_id, str::to_owned);
                if let Some(sig) = p["thoughtSignature"].as_str() {
                    assinaturas.guardar(id.clone(), sig.to_owned());
                }
                chamadas.push(ToolCall {
                    id,
                    name: nome.to_owned(),
                    arguments: if fc["args"].is_null() {
                        json!({})
                    } else {
                        fc["args"].clone()
                    },
                });
            } else if p["thought"].as_bool() != Some(true)
                && let Some(t) = p["text"].as_str()
            {
                texto.push_str(t);
            }
        }
        Ok(LlmReply {
            content: texto,
            tool_calls: chamadas,
            usage: Usage {
                input_tokens: v["usageMetadata"]["promptTokenCount"].as_u64().unwrap_or(0),
                output_tokens: v["usageMetadata"]["candidatesTokenCount"]
                    .as_u64()
                    .unwrap_or(0),
            },
            model: v["modelVersion"]
                .as_str()
                .unwrap_or(&self.modelo)
                .to_owned(),
        })
    }
}

/// Sem candidato o Gemini costuma dizer o porque em `promptFeedback.blockReason`.
fn motivo_sem_candidato(v: &Value) -> String {
    match v["promptFeedback"]["blockReason"].as_str() {
        Some(r) => format!("Gemini sem candidato: bloqueado ({r})"),
        None => "Gemini sem candidato".into(),
    }
}

/// Partes seguidas do mesmo papel viram um so `content`: respostas de chamadas paralelas
/// vao juntas, na ordem das chamadas.
fn empilhar(conteudos: &mut Vec<Value>, papel: &str, parte: Value) {
    if let Some(ultimo) = conteudos.last_mut()
        && ultimo["role"] == papel
        && let Some(partes) = ultimo["parts"].as_array_mut()
    {
        partes.push(parte);
        return;
    }
    conteudos.push(json!({"role": papel, "parts": [parte]}));
}

impl Llm for GeminiLlm {
    fn id(&self) -> String {
        format!("gemini:{}", self.modelo)
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
                    &[("x-goog-api-key", self.chave.expor())],
                    Some(&self.chave),
                    &corpo,
                )
                .await?;
            self.resposta(&v)
        })
    }
}

//! `n8n_workflow`: o agente dispara e acompanha fluxos de um n8n do OPERADOR.
//!
//! O n8n nao se embute (Sustainable Use License, triagem R12 em
//! `docs/absorcao/TRIAGEM_PESQUISAS_2026-10-01.md`): a integracao e pelos padroes abertos
//! que o n8n documenta -- o webhook de entrada de um fluxo (`/webhook/<caminho>`, teste em
//! `/webhook-test/`) e a API publica (`/api/v1/workflows`, `/api/v1/executions`, cabecalho
//! `X-N8N-API-KEY`). Os dois lados estao em `docs/N8N.md`.
//!
//! O que se reaproveita, para nao nascer um segundo jeito de guardar chave nem de falar
//! HTTP: a chave da API e o segredo do webhook moram no SecretBroker da pasta `n8n/` pela
//! regra de `chaves::Servico` (a mesma da xAI e da ElevenLabs), saem por concessao curta
//! (`Credencial::com`, que tira o segredo de todo erro), o pedido sai pelo
//! `canais::http::Http` com politica de destino (so a origem do `n8n.url`; HTTP sem TLS so
//! em loopback) e a assinatura do `run` e a MESMA do canal de webhook
//! (`canais::webhook::assinar`: `sha256=HMAC(segredo, carimbo + "." + corpo)`), para o
//! n8n conferir com um unico no de codigo os dois sentidos.
//!
//! Decisoes que valem saber:
//!
//! - **A ferramenta so existe com `n8n.url` configurado.** Sem n8n declarado nao ha para
//!   onde mandar, e o modelo nunca escolhe a URL: so o caminho do webhook e o corpo.
//! - **`run` nao precisa de chave de API**: webhook e a porta publica do fluxo. `list` e
//!   `status` precisam, e sem ela o erro cita `phxclaw n8n chave`.
//! - **Uma capacidade so, `automacao.n8n`, classificada como escrita**: `run` dispara
//!   trabalho em outro sistema; separar leitura de disparo daria ao Plan Mode uma lista de
//!   fluxos que ele nao pode disparar, sem ganho.

use crate::canais::http::{Credencial, Http, cortar, politica_para, trecho};
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// `n8n.chave`: a chave da API publica do n8n (Settings > n8n API).
pub const SERVICO: crate::chaves::Servico = crate::chaves::Servico {
    espaco: "n8n",
    nome_do_segredo: "n8n-chave",
    chave: "n8n.chave",
    aliases: &[],
    rotulo: "a chave da API do n8n",
    comando: "n8n chave",
};

/// `n8n.webhook_segredo`: o segredo que assina o `run` (e que o Header Auth do n8n
/// confere). Mesma pasta `n8n/` da chave, outro nome de segredo.
pub const SEGREDO_WEBHOOK: crate::chaves::Servico = crate::chaves::Servico {
    espaco: "n8n",
    nome_do_segredo: "n8n-webhook-segredo",
    chave: "n8n.webhook_segredo",
    aliases: &[],
    rotulo: "o segredo do webhook do n8n",
    comando: "n8n segredo",
};

/// Cabecalhos do `run`, os mesmos do canal de webhook (`canais/webhook.rs`).
pub const CABECALHO_CARIMBO: &str = "X-PhxClaw-Carimbo";
pub const CABECALHO_ASSINATURA: &str = "X-PhxClaw-Assinatura";
/// O segredo em claro, para o Header Auth nativo do no Webhook do n8n.
pub const CABECALHO_SEGREDO: &str = "X-PhxClaw-Segredo";

/// Teto do corpo mandado ao webhook: o modelo monta o JSON, e um corpo de megabytes num
/// laco seria o n8n do operador pagando pelo erro do modelo.
const CORPO_MAX: usize = 64 * 1024;
/// Teto de itens pedidos a API (o maximo que a paginacao do n8n aceita e 250).
const LIMITE_MAX: u64 = 250;
const LIMITE_PADRAO: u64 = 20;

pub struct N8nTool {
    http: Arc<Http>,
    raiz_do_agente: PathBuf,
}

impl N8nTool {
    /// `base` e a origem do n8n (`http://127.0.0.1:5678`, `https://n8n.exemplo.com`); a
    /// politica de destino nasce dela e nunca deixa o pedido ir a outra origem.
    pub fn novo(base: &str, raiz_do_agente: &Path) -> Result<Self, String> {
        Ok(Self {
            http: Arc::new(Http::novo(base, politica_para(base)?)?),
            raiz_do_agente: raiz_do_agente.to_path_buf(),
        })
    }

    /// So com `n8n.url` configurado (`PHXCLAW_N8N_URL` ou o config.json).
    pub fn da_configuracao(raiz_do_agente: &Path) -> Option<Self> {
        let base = crate::config::texto_de("n8n.url")?;
        Self::novo(&base, raiz_do_agente)
            .map_err(|e| eprintln!("aviso: n8n_workflow: {e}"))
            .ok()
    }

    fn pedido_bloqueante(&self, acao: &str, args: &Value) -> Result<Value, String> {
        match acao {
            "run" => self.disparar(args),
            "list" => self.listar(args),
            "status" => self.status(args),
            outro => Err(format!("action desconhecida: {outro}")),
        }
    }

    /// `POST /webhook/<caminho>` (ou `/webhook-test/` com `test: true`), corpo JSON,
    /// assinado quando ha segredo guardado e `auth` nao e `none`.
    fn disparar(&self, args: &Value) -> Result<Value, String> {
        let caminho = caminho_do_webhook(
            args.get("path").and_then(Value::as_str).unwrap_or(""),
            args.get("test").and_then(Value::as_bool).unwrap_or(false),
        )?;
        let corpo = args.get("body").cloned().unwrap_or_else(|| json!({}));
        if !corpo.is_object() {
            return Err("body tem de ser um objeto JSON".into());
        }
        let bytes = serde_json::to_vec(&corpo).map_err(|e| e.to_string())?;
        if bytes.len() > CORPO_MAX {
            return Err(format!("body acima de {CORPO_MAX} bytes"));
        }
        let auth = args.get("auth").and_then(Value::as_str).unwrap_or("hmac");
        let segredo = match auth {
            "none" => None,
            "hmac" | "header" => SEGREDO_WEBHOOK.credencial(&self.raiz_do_agente).ok(),
            outro => return Err(format!("auth desconhecida: {outro}")),
        };
        let montar = |pedido: reqwest::blocking::RequestBuilder| {
            pedido
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(bytes.clone())
        };
        let url = self.http.url(&caminho);
        let enviar = |pedido| -> Result<Value, String> {
            let (status, texto) = self.http.texto(pedido)?;
            if !(200..300).contains(&status) {
                return Err(format!("n8n respondeu {status}: {}", cortar(&texto, 300)));
            }
            let resposta = serde_json::from_str::<Value>(&texto)
                .unwrap_or_else(|_| json!(cortar(&texto, 4_000)));
            Ok(json!({"status": status, "response": resposta}))
        };
        let cliente = self.http.cliente()?;
        match segredo {
            None => enviar(montar(cliente.post(&url))),
            Some(c) if auth == "header" => c.com("send", |s| {
                enviar(montar(cliente.post(&url)).header(CABECALHO_SEGREDO, s))
            }),
            Some(c) => c.com("send", |s| {
                let carimbo = chrono::Utc::now().timestamp();
                let assinatura = crate::canais::webhook::assinar(s, carimbo, &bytes);
                enviar(
                    montar(cliente.post(&url))
                        .header(CABECALHO_CARIMBO, carimbo.to_string())
                        .header(CABECALHO_ASSINATURA, assinatura),
                )
            }),
        }
    }

    /// `GET /api/v1/workflows`, com `active` e `name` como filtros da propria API.
    fn listar(&self, args: &Value) -> Result<Value, String> {
        let mut consulta: Vec<(String, String)> = vec![("limit".into(), limite(args).to_string())];
        if let Some(a) = args.get("active").and_then(Value::as_bool) {
            consulta.push(("active".into(), a.to_string()));
        }
        if let Some(n) = args
            .get("name")
            .and_then(Value::as_str)
            .filter(|n| !n.is_empty())
        {
            consulta.push(("name".into(), n.to_string()));
        }
        if let Some(c) = args.get("cursor").and_then(Value::as_str) {
            consulta.push(("cursor".into(), c.to_string()));
        }
        let v = self.api("/api/v1/workflows", &consulta)?;
        let fluxos: Vec<Value> = v["data"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|w| {
                json!({
                    "id": w["id"],
                    "name": w["name"],
                    "active": w["active"],
                    "updatedAt": w["updatedAt"],
                })
            })
            .collect();
        Ok(json!({"workflows": fluxos, "nextCursor": v["nextCursor"]}))
    }

    /// `GET /api/v1/executions/<id>` com `execution_id`; senao a lista filtrada por
    /// `workflow_id` e `status`, a mais recente primeiro, como a API devolve.
    fn status(&self, args: &Value) -> Result<Value, String> {
        let resumir = |e: &Value| {
            json!({
                "id": e["id"],
                "status": e["status"],
                "mode": e["mode"],
                "workflowId": e["workflowId"],
                "startedAt": e["startedAt"],
                "stoppedAt": e["stoppedAt"],
            })
        };
        if let Some(id) = args.get("execution_id").and_then(Value::as_str) {
            let e = self.api(&format!("/api/v1/executions/{}", trecho(id)), &[])?;
            return Ok(json!({"execution": resumir(&e)}));
        }
        let mut consulta: Vec<(String, String)> = vec![("limit".into(), limite(args).to_string())];
        if let Some(w) = args.get("workflow_id").and_then(Value::as_str) {
            consulta.push(("workflowId".into(), w.to_string()));
        }
        if let Some(s) = args.get("status").and_then(Value::as_str) {
            consulta.push(("status".into(), s.to_string()));
        }
        let v = self.api("/api/v1/executions", &consulta)?;
        let lista: Vec<Value> = v["data"]
            .as_array()
            .into_iter()
            .flatten()
            .map(resumir)
            .collect();
        Ok(json!({"executions": lista, "nextCursor": v["nextCursor"]}))
    }

    /// Um GET na API publica com a chave por concessao curta. Sem chave, o erro diz o
    /// comando que a guarda -- e e esse texto que a ferramenta repete ao modelo.
    fn api(&self, caminho: &str, consulta: &[(String, String)]) -> Result<Value, String> {
        let credencial: Credencial = SERVICO.credencial(&self.raiz_do_agente)?;
        let cliente = self.http.cliente()?;
        credencial.com("send", |chave| {
            self.http.json(
                cliente
                    .get(self.http.url(caminho))
                    .query(consulta)
                    .header("X-N8N-API-KEY", chave)
                    .header(reqwest::header::ACCEPT, "application/json"),
            )
        })
    }
}

fn limite(args: &Value) -> u64 {
    args.get("limit")
        .and_then(Value::as_u64)
        .map(|l| l.clamp(1, LIMITE_MAX))
        .unwrap_or(LIMITE_PADRAO)
}

/// O caminho do webhook como o n8n o publica: `<caminho>` ou `/webhook/<caminho>` viram
/// `/webhook/<caminho>` (teste: `/webhook-test/`). O caminho pode ter barras (o n8n aceita
/// rotas com parametro), mas nao `..`, `?` nem `#`: o modelo nao escolhe outra rota.
pub fn caminho_do_webhook(path: &str, teste: bool) -> Result<String, String> {
    let p = path.trim().trim_start_matches('/');
    let p = p
        .strip_prefix("webhook-test/")
        .or_else(|| p.strip_prefix("webhook/"))
        .unwrap_or(p);
    if p.is_empty() || p.len() > 200 {
        return Err("path do webhook vazio ou maior que 200".into());
    }
    if p.split('/').any(|s| s.is_empty() || s == "." || s == "..")
        || p.chars()
            .any(|c| c.is_whitespace() || matches!(c, '?' | '#' | '\\'))
    {
        return Err(format!("path do webhook invalido: {path:?}"));
    }
    let prefixo = if teste { "/webhook-test/" } else { "/webhook/" };
    Ok(format!(
        "{prefixo}{}",
        p.split('/').map(trecho).collect::<Vec<_>>().join("/")
    ))
}

impl Tool for N8nTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "n8n_workflow".into(),
            description: "Run and inspect n8n workflows of the operator. action=run: POST \
JSON 'body' to the workflow webhook 'path' (as shown in the n8n Webhook node; test=true \
uses the test URL); the request is signed with the stored secret (auth=hmac, default; \
auth=header sends it as X-PhxClaw-Segredo for n8n Header Auth; auth=none). action=list: \
workflows (optional active, name, limit, cursor). action=status: executions of \
workflow_id (optional status, limit) or one execution_id."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["run", "list", "status"]},
                    "path": {"type": "string", "description": "webhook path, e.g. 'pedido' or '/webhook/pedido'"},
                    "body": {"type": "object", "description": "JSON body sent to the webhook"},
                    "test": {"type": "boolean", "description": "use the test webhook URL (/webhook-test/)"},
                    "auth": {"type": "string", "enum": ["hmac", "header", "none"]},
                    "active": {"type": "boolean"},
                    "name": {"type": "string"},
                    "workflow_id": {"type": "string"},
                    "execution_id": {"type": "string"},
                    "status": {"type": "string", "enum": ["canceled", "crashed", "error", "new", "running", "success", "waiting"]},
                    "limit": {"type": "integer", "description": "1..250, default 20"},
                    "cursor": {"type": "string"}
                },
                "required": ["action"]
            }),
        }
    }

    /// Dispara trabalho num sistema do operador: escrita, fora do padrao.
    fn capability(&self) -> &'static str {
        "automacao.n8n"
    }

    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let acao = args
                .get("action")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'action'".into()))?
                .to_string();
            let eu = N8nTool {
                http: self.http.clone(),
                raiz_do_agente: self.raiz_do_agente.clone(),
            };
            let trabalho = tokio::task::spawn_blocking(move || eu.pedido_bloqueante(&acao, &args));
            let v = tokio::time::timeout(ctx.timeout, trabalho)
                .await
                .map_err(|_| ToolError::Timeout(ctx.timeout.as_millis() as u64))?
                .map_err(|e| ToolError::Failed(e.to_string()))?
                .map_err(ToolError::Failed)?;
            Ok(ToolOutput::text(
                serde_json::to_string(&v).map_err(|e| ToolError::Failed(e.to_string()))?,
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caminho_do_webhook_normaliza_e_recusa_rota_fora_do_webhook() {
        assert_eq!(
            caminho_do_webhook("pedido", false).unwrap(),
            "/webhook/pedido"
        );
        assert_eq!(
            caminho_do_webhook("/webhook/pedido", false).unwrap(),
            "/webhook/pedido"
        );
        assert_eq!(
            caminho_do_webhook("/webhook/pedido", true).unwrap(),
            "/webhook-test/pedido"
        );
        assert_eq!(
            caminho_do_webhook("a/b c", false).unwrap_err(),
            "path do webhook invalido: \"a/b c\""
        );
        assert!(caminho_do_webhook("../api/v1/workflows", false).is_err());
        assert!(caminho_do_webhook("x?y=1", false).is_err());
        assert!(caminho_do_webhook("", false).is_err());
    }
}

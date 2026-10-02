//! `x_search`: busca em posts do X pela API Responses da xAI (ferramenta `x_search` do
//! lado deles). A API v2 do X e paga por leitura; a da xAI cobra so o modelo.
//!
//! O que se reaproveita, para nao haver um segundo jeito de guardar e usar chave: a
//! chave mora no SecretBroker da pasta `xai/` pela regra de `chaves::Servico` (a mesma da
//! ElevenLabs e da Gemini), e usada por concessao curta pela `Credencial::com` -- que
//! tira a chave de todo erro --, e o pedido sai pelo `canais::http::Http`, com politica de
//! destino e sem seguir redirecionamento.
//!
//! As datas sao conferidas ANTES de qualquer pedido: data invalida custaria uma chamada
//! paga para receber um erro, e data no futuro devolveria «nenhum post» como se fosse
//! resposta.

use crate::canais::http::{Credencial, Http, politica_para};
use chrono::NaiveDate;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;

pub const SERVICO: crate::chaves::Servico = crate::chaves::Servico {
    espaco: "xai",
    nome_do_segredo: "xai-chave",
    chave: "xai.chave",
    aliases: &[],
    rotulo: "a chave da xAI",
    comando: "xai chave",
};
/// Teto de perfis por lista, o da API da xAI.
const MAX_PERFIS: usize = 10;
const MAX_FONTES: usize = 20;

pub fn pasta_da_xai(raiz_do_agente: &Path) -> std::path::PathBuf {
    SERVICO.pasta(raiz_do_agente)
}

/// `phxclaw xai chave`: a chave sai de `PHXCLAW_XAI_API_KEY` direto para o broker.
pub fn guardar_do_ambiente(raiz_do_agente: &Path) -> Result<uuid::Uuid, String> {
    SERVICO.guardar_do_ambiente(raiz_do_agente)
}

/// As datas do pedido, conferidas. `hoje` vem de fora para o teste fixar o dia.
pub fn conferir_datas(
    de: Option<&str>,
    ate: Option<&str>,
    hoje: NaiveDate,
) -> Result<(Option<NaiveDate>, Option<NaiveDate>), String> {
    let ler = |campo: &str, v: Option<&str>| -> Result<Option<NaiveDate>, String> {
        v.map(|s| {
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| format!("{campo} tem de ser AAAA-MM-DD: {s}"))
        })
        .transpose()
    };
    let (de, ate) = (ler("from_date", de)?, ler("to_date", ate)?);
    for (campo, d) in [("from_date", de), ("to_date", ate)] {
        if d.is_some_and(|d| d > hoje) {
            return Err(format!("{campo} no futuro ({hoje} e hoje)"));
        }
    }
    if let (Some(a), Some(b)) = (de, ate)
        && a > b
    {
        return Err(format!("from_date ({a}) depois de to_date ({b})"));
    }
    Ok((de, ate))
}

/// Perfil do X: 1 a 15 letras, digitos ou `_`, com ou sem `@`.
fn perfis(args: &Value, campo: &str) -> Result<Vec<String>, String> {
    let Some(v) = args.get(campo) else {
        return Ok(vec![]);
    };
    let lista = v
        .as_array()
        .ok_or(format!("{campo} tem de ser lista de perfis"))?;
    if lista.len() > MAX_PERFIS {
        return Err(format!("{campo}: no maximo {MAX_PERFIS} perfis"));
    }
    lista
        .iter()
        .map(|p| {
            let p = p.as_str().unwrap_or("").trim_start_matches('@');
            let ok = (1..=15).contains(&p.len())
                && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            ok.then(|| p.to_string())
                .ok_or(format!("{campo}: perfil invalido: {p:?}"))
        })
        .collect()
}

/// O corpo do pedido, ja conferido.
pub fn montar_pedido(args: &Value, modelo: &str, hoje: NaiveDate) -> Result<Value, String> {
    let consulta = args
        .get("query")
        .and_then(Value::as_str)
        .filter(|q| !q.trim().is_empty())
        .ok_or("falta 'query'")?;
    let (de, ate) = conferir_datas(
        args.get("from_date").and_then(Value::as_str),
        args.get("to_date").and_then(Value::as_str),
        hoje,
    )?;
    let (so, menos) = (perfis(args, "handles")?, perfis(args, "excluded_handles")?);
    if !so.is_empty() && !menos.is_empty() {
        return Err("handles e excluded_handles nao podem vir juntos".into());
    }
    let mut ferramenta = json!({"type": "x_search"});
    if let Some(d) = de {
        ferramenta["from_date"] = json!(d.to_string());
    }
    if let Some(d) = ate {
        ferramenta["to_date"] = json!(d.to_string());
    }
    if !so.is_empty() {
        ferramenta["allowed_x_handles"] = json!(so);
    }
    if !menos.is_empty() {
        ferramenta["excluded_x_handles"] = json!(menos);
    }
    Ok(json!({
        "model": modelo,
        "input": [{"role": "user", "content": consulta}],
        "tools": [ferramenta],
    }))
}

/// O texto da resposta e as fontes citadas (sem repetir).
pub fn ler_resposta(v: &Value) -> String {
    let mut texto = Vec::new();
    let mut fontes: Vec<String> = Vec::new();
    for item in v["output"].as_array().into_iter().flatten() {
        if item["type"] != "message" {
            continue;
        }
        for c in item["content"].as_array().into_iter().flatten() {
            if let Some(t) = c["text"].as_str() {
                texto.push(t.to_string());
            }
            for a in c["annotations"].as_array().into_iter().flatten() {
                if let Some(u) = a["url"].as_str()
                    && !fontes.iter().any(|f| f == u)
                    && fontes.len() < MAX_FONTES
                {
                    fontes.push(u.to_string());
                }
            }
        }
    }
    let mut s = texto.join("\n");
    if s.trim().is_empty() {
        s = "nenhum resultado".into();
    }
    if !fontes.is_empty() {
        s.push_str("\n\nfontes:\n");
        s.push_str(&fontes.join("\n"));
    }
    s
}

pub struct XSearchTool {
    http: Arc<Http>,
    credencial: Credencial,
    modelo: String,
}

impl XSearchTool {
    pub fn novo(base: &str, modelo: &str, credencial: Credencial) -> Result<Self, String> {
        Ok(Self {
            http: Arc::new(Http::novo(base, politica_para(base)?)?),
            credencial,
            modelo: modelo.into(),
        })
    }

    /// Sem chave guardada, a ferramenta nem existe (como as forjas). Base e modelo so o
    /// operador escolhe, pelo ambiente.
    pub fn da_pasta(raiz_do_agente: &Path) -> Option<Self> {
        let credencial = SERVICO.credencial(raiz_do_agente).ok()?;
        let base =
            crate::config::texto_de("xai.api").unwrap_or_else(|| "https://api.x.ai/v1".into());
        let modelo = crate::config::texto_de("xai.modelo").unwrap_or_else(|| "grok-4".into());
        Self::novo(&base, &modelo, credencial)
            .map_err(|e| eprintln!("aviso: x_search: {e}"))
            .ok()
    }
}

impl Tool for XSearchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "x_search".into(),
            description: "Search posts on X (Twitter) through xAI and get a cited summary. \
query: what to look for; optional from_date/to_date (YYYY-MM-DD, not in the future), \
handles (only these accounts, max 10) or excluded_handles. Returns text and source URLs."
                .into(),
            parameters: json!({"type":"object","properties":{
                "query":{"type":"string"},
                "from_date":{"type":"string"},
                "to_date":{"type":"string"},
                "handles":{"type":"array","items":{"type":"string"}},
                "excluded_handles":{"type":"array","items":{"type":"string"}}
            },"required":["query"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "x.search"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let hoje = chrono::Utc::now().date_naive();
            let corpo =
                montar_pedido(&args, &self.modelo, hoje).map_err(ToolError::InvalidArguments)?;
            let (http, cred) = (self.http.clone(), self.credencial.clone());
            // Cliente bloqueante do `canais::http`: fora do runtime assincrono.
            let v = tokio::task::spawn_blocking(move || {
                cred.com("send", |chave| {
                    let c = http.cliente()?;
                    http.json(
                        c.post(http.url("/responses"))
                            .bearer_auth(chave)
                            .json(&corpo),
                    )
                })
            })
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?
            .map_err(|e| ToolError::Failed(format!("x_search: {e}")))?;
            Ok(ToolOutput::text(ler_resposta(&v)))
        })
    }
}

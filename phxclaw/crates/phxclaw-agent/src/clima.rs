//! `weather`: previsao do tempo por coordenada, pelo Locationforecast 2.0 do MET Norway
//! (api.met.no), dados sob CC BY 4.0.
//!
//! Por que o MET Norway (pesquisa das ondas 4 e 5): responde sem chave, a licenca permite
//! uso comercial com atribuicao, e cobre o mundo todo. Open-Meteo e so nao comercial; o
//! OpenWeatherMap pede chave.
//!
//! Os termos de uso do MET decidem a forma do cliente, e quebrar qualquer um deles e 403:
//!
//! - **User-Agent identificado** (produto e contato); o generico do reqwest e bloqueado.
//!   O contato padrao e o repositorio; `PHXCLAW_MET_CONTATO` troca pelo do operador.
//! - **No maximo quatro casas decimais** na coordenada: mais que isso e recusado e,
//!   pior, espalha o cache deles. Aqui a coordenada e cortada antes do pedido.
//! - **Respeitar `Expires`**: antes dele, a resposta guardada vale e nada sai pela rede;
//!   depois, o pedido leva `If-Modified-Since` com o `Last-Modified` guardado, e o 304
//!   reaproveita o corpo. O cache e do processo (todas as tarefas), nao da ferramenta: a
//!   montagem cria uma ferramenta por agente, e um cache por agente pediria de novo o que
//!   outra tarefa acabou de pedir.
//! - **Atribuicao**: toda resposta leva a linha do CC BY 4.0. Ela sai daqui, nao do modelo:
//!   e obrigacao da licenca, e o modelo esqueceria.

use crate::canais::http::{Http, politica_para};
use chrono::{DateTime, Utc};
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub const BASE_PADRAO: &str = "https://api.met.no/weatherapi/locationforecast/2.0";
pub const ATRIBUICAO: &str = "Dados meteorologicos: MET Norway (api.met.no), licenca CC BY 4.0 (creativecommons.org/licenses/by/4.0)";
const CONTATO_PADRAO: &str = "https://github.com/adrianoboller/adrianoboller";
const HORAS_PADRAO: usize = 12;
const HORAS_MAX: usize = 48;

/// O que se guarda de uma coordenada: o corpo e as duas datas que decidem o proximo pedido.
#[derive(Clone)]
struct Guardado {
    corpo: Value,
    expira: DateTime<Utc>,
    modificado: Option<String>,
}

fn cache() -> &'static Mutex<HashMap<String, Guardado>> {
    static C: OnceLock<Mutex<HashMap<String, Guardado>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Coordenada com no maximo quatro casas, como os termos do MET exigem. Corta (nao
/// arredonda) para o texto do pedido e a chave do cache serem o mesmo valor.
pub fn coordenada(v: f64, limite: f64, nome: &str) -> Result<String, String> {
    if !v.is_finite() || v.abs() > limite {
        return Err(format!("{nome} fora de -{limite}..{limite}: {v}"));
    }
    let t = (v * 10_000.0).trunc() / 10_000.0;
    let s = format!("{t:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    Ok(if s == "-0" { "0".into() } else { s.to_string() })
}

fn data_http(s: Option<&str>) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc2822(s?.trim())
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

pub struct WeatherTool {
    http: Arc<Http>,
    agente: String,
}

impl WeatherTool {
    pub fn novo(base: &str) -> Result<Self, String> {
        let contato = std::env::var("PHXCLAW_MET_CONTATO")
            .ok()
            .filter(|c| !c.trim().is_empty())
            .unwrap_or_else(|| CONTATO_PADRAO.into());
        Ok(Self {
            http: Arc::new(Http::novo(base, politica_para(base)?)?),
            agente: format!("PhxClaw/{} {}", env!("CARGO_PKG_VERSION"), contato.trim()),
        })
    }

    /// A base sai de `PHXCLAW_MET_API` (o servidor falso dos testes, um espelho do
    /// operador); o modelo nunca escolhe para onde o pedido vai.
    pub fn do_ambiente() -> Result<Self, String> {
        let base = std::env::var("PHXCLAW_MET_API")
            .ok()
            .filter(|b| !b.trim().is_empty())
            .unwrap_or_else(|| BASE_PADRAO.into());
        Self::novo(&base)
    }

    /// O corpo do Locationforecast para a coordenada, pela regra do cache (ver o topo).
    /// Bloqueante: o `Http` dos canais e o cliente bloqueante, chamado em `spawn_blocking`.
    fn previsao(&self, lat: &str, lon: &str, agora: DateTime<Utc>) -> Result<Value, String> {
        let chave = format!("{}|{lat}|{lon}", self.http.base());
        let antes = cache()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&chave)
            .cloned();
        if let Some(g) = &antes
            && agora < g.expira
        {
            return Ok(g.corpo.clone());
        }
        let mut pedido = self
            .http
            .cliente()?
            .get(self.http.url("/compact"))
            .query(&[("lat", lat), ("lon", lon)])
            .header(reqwest::header::USER_AGENT, &self.agente)
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(m) = antes.as_ref().and_then(|g| g.modificado.as_deref()) {
            pedido = pedido.header(reqwest::header::IF_MODIFIED_SINCE, m);
        }
        let r = pedido.send().map_err(|e| e.without_url().to_string())?;
        let status = r.status().as_u16();
        let cab = |n: reqwest::header::HeaderName| {
            r.headers()
                .get(n)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        // Sem `Expires`, nada se guarda alem do agora: pedir de novo e o lado seguro.
        let expira = data_http(cab(reqwest::header::EXPIRES).as_deref()).unwrap_or(agora);
        let modificado = cab(reqwest::header::LAST_MODIFIED);
        let corpo = match (status, antes) {
            (304, Some(g)) => g.corpo,
            (304, None) => return Err("MET respondeu 304 sem nada guardado".into()),
            // 203: o produto vai mudar/sair; os dados continuam validos.
            (200 | 203, _) => {
                let t = r.text().map_err(|e| e.without_url().to_string())?;
                serde_json::from_str(&t).map_err(|e| format!("resposta nao e JSON: {e}"))?
            }
            (s, _) => {
                let t: String = r.text().unwrap_or_default().chars().take(200).collect();
                return Err(format!("MET respondeu {s}: {t}"));
            }
        };
        cache().lock().unwrap_or_else(|e| e.into_inner()).insert(
            chave,
            Guardado {
                corpo: corpo.clone(),
                expira,
                modificado,
            },
        );
        Ok(corpo)
    }
}

/// As proximas `horas` da serie, so o que um humano pergunta: temperatura, vento, umidade,
/// chuva da proxima hora e o simbolo. Unidades saem do proprio `meta.units` da resposta.
pub fn resumir(corpo: &Value, horas: usize, lat: &str, lon: &str) -> Value {
    let serie = corpo
        .pointer("/properties/timeseries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let horas: Vec<Value> = serie
        .iter()
        .take(horas)
        .map(|p| {
            let d = |k: &str| p.pointer(&format!("/data/instant/details/{k}")).cloned();
            json!({
                "hora": p.get("time"),
                "temperatura": d("air_temperature"),
                "vento": d("wind_speed"),
                "umidade": d("relative_humidity"),
                "chuva_1h": p.pointer("/data/next_1_hours/details/precipitation_amount"),
                "simbolo": p.pointer("/data/next_1_hours/summary/symbol_code")
                    .or_else(|| p.pointer("/data/next_6_hours/summary/symbol_code")),
            })
        })
        .collect();
    json!({
        "latitude": lat,
        "longitude": lon,
        "atualizado": corpo.pointer("/properties/meta/updated_at"),
        "unidades": corpo.pointer("/properties/meta/units"),
        "horas": horas,
        "atribuicao": ATRIBUICAO,
    })
}

impl Tool for WeatherTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "weather".into(),
            description: "Previsao do tempo para uma coordenada (MET Norway, CC BY 4.0): \
                          temperatura, vento, umidade, chuva e simbolo hora a hora. Cite a \
                          atribuicao que vem na resposta."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "lat": {"type": "number", "description": "latitude em graus (-90..90)"},
                    "lon": {"type": "number", "description": "longitude em graus (-180..180)"},
                    "hours": {"type": "integer", "description": "horas a devolver (1..48, padrao 12)"}
                },
                "required": ["lat", "lon"]
            }),
        }
    }

    /// So le, de um servico publico, sem conta: entra como leitura (vale no Plan Mode).
    fn capability(&self) -> &'static str {
        "weather.read"
    }

    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let num = |k: &str| {
                args.get(k)
                    .and_then(Value::as_f64)
                    .ok_or_else(|| ToolError::InvalidArguments(format!("falta o numero '{k}'")))
            };
            let lat = coordenada(num("lat")?, 90.0, "lat").map_err(ToolError::InvalidArguments)?;
            let lon = coordenada(num("lon")?, 180.0, "lon").map_err(ToolError::InvalidArguments)?;
            let horas = args
                .get("hours")
                .and_then(Value::as_u64)
                .map(|h| (h as usize).clamp(1, HORAS_MAX))
                .unwrap_or(HORAS_PADRAO);
            let http = self.http.clone();
            let agente = self.agente.clone();
            let (la, lo) = (lat.clone(), lon.clone());
            let trabalho = tokio::task::spawn_blocking(move || {
                WeatherTool { http, agente }.previsao(&la, &lo, Utc::now())
            });
            let corpo = tokio::time::timeout(ctx.timeout, trabalho)
                .await
                .map_err(|_| ToolError::Timeout(ctx.timeout.as_millis() as u64))?
                .map_err(|e| ToolError::Failed(e.to_string()))?
                .map_err(ToolError::Failed)?;
            Ok(ToolOutput::text(
                serde_json::to_string(&resumir(&corpo, horas, &lat, &lon))
                    .map_err(|e| ToolError::Failed(e.to_string()))?,
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordenada_corta_em_quatro_casas_e_recusa_fora_do_globo() {
        assert_eq!(coordenada(59.913_868_9, 90.0, "lat").unwrap(), "59.9138");
        assert_eq!(coordenada(-26.919_99, 90.0, "lat").unwrap(), "-26.9199");
        assert_eq!(coordenada(10.5, 90.0, "lat").unwrap(), "10.5");
        assert_eq!(coordenada(-0.000_01, 90.0, "lat").unwrap(), "0");
        assert!(coordenada(91.0, 90.0, "lat").is_err());
        assert!(coordenada(f64::NAN, 180.0, "lon").is_err());
    }
}

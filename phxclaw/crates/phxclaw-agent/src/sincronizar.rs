//! Sincronizacao do `config.json` entre maquinas pela ponte remota (`remoto.rs`): o que o
//! Settings Sync do VS Code faz, sem servico de fora.
//!
//! - **O que atravessa**: o documento SEM a revisao (`carga::exportar`), e cada valor
//!   passa de novo pela varredura de credencial antes de sair -- o arquivo ja recusa
//!   segredo na leitura, mas o que vai para outra maquina se confere na saida.
//! - **Duas revisoes, de proposito**: o SHA-256 dos BYTES do arquivo (o mesmo do token
//!   de concorrencia da API) e o que quem grava diz ter visto no destino, conferido sob
//!   a trava (`carga::importar`) -- mudou, e 409. E o SHA-256 CANONICO do documento
//!   exportado (sem a revisao, que e da maquina) e o que diz se dois lados tem o mesmo
//!   conteudo: dois arquivos sincronizados nunca tem os mesmos bytes, porque cada um
//!   carrega o proprio contador de revisao.
//! - **Direcao**: a marca `config-sincronia.json` guarda o SHA do conteudo que os dois
//!   lados tinham na ultima sincronizacao. `enviar` so passa se o remoto ainda e aquele
//!   (ou esta vazio); `receber`, se o local ainda e aquele. Os dois mudaram: conflito
//!   com as duas revisoes, e `--forcar` e a escolha escrita do operador.
//! - **Transporte**: a CLI fala HTTP com a ponte (o Bearer da ponte, o mesmo do celular),
//!   e a ponte rele ao agente ligado de saida, no mesmo canal das rotas de tarefa. O
//!   agente atende em `GET`/`PUT /v1/config/sincronizar`, com o Bearer da API.

use crate::api::ApiState;
use crate::config::{self as cfg, arquivo_da_pasta};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use phxclaw_config_runtime::agente::carga::{self, Alvo, Erro, Recusa};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const ROTA: &str = "/v1/config/sincronizar";
/// A marca da ultima sincronizacao, ao lado do `config.json`.
pub const MARCA: &str = "config-sincronia.json";

// ------------------------------------------------------------------ lado do agente

/// O SHA canonico de um documento exportado: o que diz «mesmo conteudo».
pub fn sha_do_conteudo(doc: &Value) -> String {
    phxclaw_config_runtime::canonical_sha256(doc).unwrap_or_default()
}

/// O retrato que o agente entrega: a revisao (SHA do disco, para o CAS), o SHA do
/// conteudo e o documento exportado.
pub fn retrato(pasta: &Path) -> Result<Value, Vec<Erro>> {
    let arq = carga::ler_arquivo(&arquivo_da_pasta(pasta))?;
    let documento = carga::exportar(&arq)?;
    Ok(json!({
        "revisao": arq.sha256,
        "conteudo": sha_do_conteudo(&documento),
        "documento": documento,
    }))
}

/// Aplica o documento vindo de fora se o disco ainda esta em `base`. Devolve o SHA novo.
pub fn aplicar(pasta: &Path, documento: &Value, base: &str) -> Result<String, Recusa> {
    let arquivo = arquivo_da_pasta(pasta);
    let atual = cfg::carregar(pasta).map_err(Recusa::Invalida)?;
    let r = carga::importar(
        Alvo {
            arquivo: &arquivo,
            atual: &atual,
            esperada: None,
            perfil: None,
        },
        documento,
        base,
    );
    cfg::esquecer(pasta);
    r?;
    carga::sha_em_disco(&arquivo).map_err(Recusa::Falha)
}

fn pasta_da_api(s: &ApiState) -> PathBuf {
    let r = s.store.root();
    r.parent().unwrap_or(r).to_path_buf()
}

fn recusa_http(r: Recusa) -> Response {
    match r {
        Recusa::Conflito { atual } => (
            StatusCode::CONFLICT,
            Json(json!({"error": "o destino mudou desde a base enviada", "revisao_atual": atual})),
        )
            .into_response(),
        Recusa::Invalida(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"erros": e.iter().map(|e| json!({"chave": e.chave, "motivo": e.motivo})).collect::<Vec<_>>()})),
        )
            .into_response(),
        Recusa::Falha(m) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": m}))).into_response()
        }
    }
}

async fn obter(State(s): State<ApiState>, h: HeaderMap) -> Response {
    if let Err(e) = crate::api::auth(&s, &h) {
        return e.into_response();
    }
    let pasta = pasta_da_api(&s);
    match tokio::task::spawn_blocking(move || retrato(&pasta)).await {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => recusa_http(Recusa::Invalida(e)),
        Err(e) => recusa_http(Recusa::Falha(e.to_string())),
    }
}

#[derive(Deserialize)]
struct Envio {
    documento: Value,
    base: String,
}

async fn gravar(State(s): State<ApiState>, h: HeaderMap, corpo: Json<Value>) -> Response {
    if let Err(e) = crate::api::auth(&s, &h) {
        return e.into_response();
    }
    let Ok(envio) = serde_json::from_value::<Envio>(corpo.0) else {
        return recusa_http(Recusa::Invalida(vec![Erro {
            chave: "corpo".into(),
            motivo: "esperado {\"documento\": {...}, \"base\": sha}".into(),
        }]));
    };
    let pasta = pasta_da_api(&s);
    match tokio::task::spawn_blocking(move || aplicar(&pasta, &envio.documento, &envio.base)).await
    {
        Ok(Ok(sha)) => Json(json!({"revisao": sha})).into_response(),
        Ok(Err(r)) => recusa_http(r),
        Err(e) => recusa_http(Recusa::Falha(e.to_string())),
    }
}

pub fn rotas() -> Router<ApiState> {
    Router::new().route(ROTA, get(obter).put(gravar))
}

// ------------------------------------------------------------------ a decisao

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marca {
    /// O SHA do conteudo que os dois lados tinham ao fim da ultima sincronizacao.
    pub conteudo: String,
    pub quando: String,
}

impl Marca {
    pub fn ler(pasta: &Path) -> Option<Self> {
        serde_json::from_slice(&std::fs::read(pasta.join(MARCA)).ok()?).ok()
    }

    pub fn gravar(&self, pasta: &Path) -> Result<(), String> {
        std::fs::create_dir_all(pasta).map_err(|e| e.to_string())?;
        std::fs::write(
            pasta.join(MARCA),
            serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sentido {
    Enviar,
    Receber,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decisao {
    /// Os dois lados ja tem o mesmo conteudo.
    Iguais,
    Seguir,
    /// O lado que seria sobreposto mudou desde a ultima sincronizacao.
    Conflito {
        local: String,
        remoto: String,
    },
}

/// O SHA do conteudo de um arquivo ausente (o documento vazio): lado vazio nunca e
/// conflito.
fn sha_vazio() -> String {
    sha_do_conteudo(&json!({}))
}

/// A regra de direcao, pura, sobre os SHAs de CONTEUDO, para o teste repor cada caso.
pub fn decidir(sentido: Sentido, local: &str, remoto: &str, marca: Option<&Marca>) -> Decisao {
    if local == remoto {
        return Decisao::Iguais;
    }
    let vazio = sha_vazio();
    // O lado que sera sobreposto tem de estar como a ultima sincronizacao o deixou.
    let sobreposto = match sentido {
        Sentido::Enviar => remoto,
        Sentido::Receber => local,
    };
    let intacto = match marca {
        Some(m) => sobreposto == m.conteudo || sobreposto == vazio,
        None => sobreposto == vazio,
    };
    if intacto {
        Decisao::Seguir
    } else {
        Decisao::Conflito {
            local: local.to_string(),
            remoto: remoto.to_string(),
        }
    }
}

// ------------------------------------------------------------------ lado do cliente

/// Como a CLI alcanca o agente: a URL HTTP da ponte e o Bearer DELA.
pub struct Ponte {
    pub url: String,
    pub token: String,
}

/// O que a sincronizacao fez, para a CLI imprimir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relato {
    pub decisao: Decisao,
    pub revisao_local: String,
    pub revisao_remota: String,
}

fn url_da_rota(p: &Ponte) -> String {
    format!("{}{ROTA}", p.url.trim_end_matches('/'))
}

/// (SHA do arquivo remoto, SHA do conteudo remoto, documento).
async fn retrato_remoto(c: &reqwest::Client, p: &Ponte) -> Result<(String, String, Value), String> {
    let r = c
        .get(url_da_rota(p))
        .bearer_auth(&p.token)
        .send()
        .await
        .map_err(|e| format!("ponte: {e}"))?;
    let status = r.status();
    let corpo: Value = r
        .json()
        .await
        .map_err(|e| format!("ponte: resposta ilegivel: {e}"))?;
    if !status.is_success() {
        return Err(format!("ponte respondeu {status}: {corpo}"));
    }
    let sha = corpo["revisao"]
        .as_str()
        .ok_or("ponte: retrato sem revisao")?
        .to_string();
    let conteudo = corpo["conteudo"]
        .as_str()
        .ok_or("ponte: retrato sem o sha do conteudo")?
        .to_string();
    Ok((sha, conteudo, corpo["documento"].clone()))
}

/// `phxclaw config sincronizar enviar`: o `config.json` desta pasta vai para o agente do
/// outro lado da ponte. `forcar` passa por cima do conflito de direcao (nao do CAS do
/// destino: se o remoto mudar entre o retrato e a gravacao, continua 409).
pub async fn enviar(pasta: &Path, p: &Ponte, forcar: bool) -> Result<Relato, String> {
    let arquivo = arquivo_da_pasta(pasta);
    let arq = carga::ler_arquivo(&arquivo).map_err(|e| carga::em_texto(&e))?;
    let documento = carga::exportar(&arq).map_err(|e| carga::em_texto(&e))?;
    let local = sha_do_conteudo(&documento);
    let c = reqwest::Client::new();
    let (sha_arquivo_remoto, remoto, _) = retrato_remoto(&c, p).await?;
    let decisao = decidir(Sentido::Enviar, &local, &remoto, Marca::ler(pasta).as_ref());
    match &decisao {
        Decisao::Iguais => {
            return Ok(Relato {
                decisao,
                revisao_local: local,
                revisao_remota: remoto,
            });
        }
        Decisao::Conflito { .. } if !forcar => {
            return Ok(Relato {
                decisao,
                revisao_local: local,
                revisao_remota: remoto,
            });
        }
        _ => {}
    }
    let r = c
        .put(url_da_rota(p))
        .bearer_auth(&p.token)
        .json(&json!({"documento": documento, "base": sha_arquivo_remoto}))
        .send()
        .await
        .map_err(|e| format!("ponte: {e}"))?;
    let status = r.status();
    let corpo: Value = r.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(format!("o agente recusou ({status}): {corpo}"));
    }
    if corpo["revisao"].as_str().is_none() {
        return Err(format!("resposta sem revisao: {corpo}"));
    }
    Marca {
        conteudo: local.clone(),
        quando: chrono::Utc::now().to_rfc3339(),
    }
    .gravar(pasta)?;
    Ok(Relato {
        decisao,
        revisao_local: local.clone(),
        revisao_remota: local,
    })
}

/// `phxclaw config sincronizar receber`: o `config.json` do agente do outro lado vem para
/// esta pasta, gravado pela mesma `carga::importar` (trava, historico, revisao local).
pub async fn receber(pasta: &Path, p: &Ponte, forcar: bool) -> Result<Relato, String> {
    let arquivo = arquivo_da_pasta(pasta);
    let arq = carga::ler_arquivo(&arquivo).map_err(|e| carga::em_texto(&e))?;
    let local = sha_do_conteudo(&carga::exportar(&arq).map_err(|e| carga::em_texto(&e))?);
    let c = reqwest::Client::new();
    let (_, remoto, documento) = retrato_remoto(&c, p).await?;
    let decisao = decidir(
        Sentido::Receber,
        &local,
        &remoto,
        Marca::ler(pasta).as_ref(),
    );
    match &decisao {
        Decisao::Iguais => {
            return Ok(Relato {
                decisao,
                revisao_local: local,
                revisao_remota: remoto,
            });
        }
        Decisao::Conflito { .. } if !forcar => {
            return Ok(Relato {
                decisao,
                revisao_local: local,
                revisao_remota: remoto,
            });
        }
        _ => {}
    }
    aplicar(pasta, &documento, &arq.sha256).map_err(|e| e.to_string())?;
    Marca {
        conteudo: remoto.clone(),
        quando: chrono::Utc::now().to_rfc3339(),
    }
    .gravar(pasta)?;
    Ok(Relato {
        decisao,
        revisao_local: remoto.clone(),
        revisao_remota: remoto,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_direcao_so_segue_se_o_lado_sobreposto_nao_mudou() {
        let vazio = sha_vazio();
        let m = Marca {
            conteudo: "S1".into(),
            quando: String::new(),
        };
        assert_eq!(decidir(Sentido::Enviar, "X", "X", None), Decisao::Iguais);
        // Sem marca: so para um lado vazio.
        assert_eq!(
            decidir(Sentido::Enviar, "L1", &vazio, None),
            Decisao::Seguir
        );
        assert!(matches!(
            decidir(Sentido::Enviar, "L1", "R9", None),
            Decisao::Conflito { .. }
        ));
        assert_eq!(
            decidir(Sentido::Receber, &vazio, "R1", None),
            Decisao::Seguir
        );
        // Com marca: o lado sobreposto intacto (igual a marca) deixa seguir; mudado, conflito.
        assert_eq!(
            decidir(Sentido::Enviar, "L2", "S1", Some(&m)),
            Decisao::Seguir
        );
        assert_eq!(
            decidir(Sentido::Enviar, "L2", "R2", Some(&m)),
            Decisao::Conflito {
                local: "L2".into(),
                remoto: "R2".into()
            }
        );
        assert_eq!(
            decidir(Sentido::Receber, "S1", "R2", Some(&m)),
            Decisao::Seguir
        );
        assert!(matches!(
            decidir(Sentido::Receber, "L2", "R2", Some(&m)),
            Decisao::Conflito { .. }
        ));
    }
}

//! `GET /v1/insights`: o painel de insights das execucoes (o «Insights» do n8n) -- por
//! estado, duracao p50/p95, custo, falhas mais comuns, por fluxo e por dia, num periodo.
//!
//! Decisoes que valem saber:
//! - **Le o disco, nao os contadores do processo.** O `/metrics` zera no reinicio (e e o
//!   certo para um `counter`); um painel que zera no reinicio mente sobre a semana. A fonte e
//!   a mesma do `GET /v1/tasks`: as tarefas da pasta, cortadas pelo projeto do RBAC
//!   (`rbac::visiveis`), e por isso o painel nunca mostra execucao que a lista esconderia.
//! - **Execucao e tarefa de cima** (sem `parent`): o subagente e o passo de agente sao parte
//!   da execucao que os chamou, e conta-los de novo dobraria o total.
//! - **Duracao e da criacao a ultima gravacao**, so de execucao terminada; no fluxo que
//!   esperou, inclui a espera (o que o usuario esperou de fato). p50/p95 pelo posto mais
//!   proximo, a mesma `avaliacao::percentil` das bancadas.
//! - **Falha mais comum agrupa o motivo normalizado** (primeira linha, palavra com digito vira `#`, ate
//!   100 caracteres). Motivo com forma de credencial nao sai: vira `omitido` (o texto de erro
//!   e de uma ferramenta, e uma ferramenta pode ecoar o que recebeu).
//! - **Sem «tempo poupado»**: o n8n o calcula de um minuto por execucao que o dono do fluxo
//!   declara; aqui o fluxo nao tem esse campo, e um numero inventado no painel seria pior que
//!   a ausencia dele.

use crate::api::{ApiState, auth};
use crate::tarefa::{Task, TaskStatus};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const ROTA: &str = "/v1/insights";

/// Os periodos aceitos: o painel e uma janela, nao uma consulta livre.
pub const PERIODOS: [&str; 4] = ["24h", "7d", "30d", "tudo"];

/// Quantas falhas mais comuns o painel mostra.
const MAX_FALHAS: usize = 5;

pub fn rotas() -> Router<ApiState> {
    Router::new().route(ROTA, get(insights))
}

#[derive(Deserialize, Default)]
struct Consulta {
    #[serde(default)]
    periodo: Option<String>,
    #[serde(default)]
    fluxo: Option<String>,
}

async fn insights(
    State(s): State<ApiState>,
    h: HeaderMap,
    acesso: Option<axum::Extension<crate::rbac::Acesso>>,
    Query(q): Query<Consulta>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    auth(&s, &h)?;
    let periodo = q.periodo.as_deref().unwrap_or("7d");
    let agora = Utc::now();
    let desde = desde(periodo, agora).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("periodo: use {}", PERIODOS.join(", "))})),
        )
    })?;
    let store = s.store.clone();
    let l = tokio::task::spawn_blocking(move || store.list())
        .await
        .map_err(|e| erro(e.to_string()))?
        .map_err(|e| erro(e.to_string()))?;
    let l = crate::rbac::visiveis(acesso.as_ref().map(|a| &a.0), l);
    let fluxo = q.fluxo.as_deref().map(str::trim).filter(|f| !f.is_empty());
    let mut v = calcular(&l, desde, fluxo, agora);
    v["periodo"] = json!(periodo);
    Ok(Json(v).into_response())
}

fn erro(e: String) -> (StatusCode, Json<Value>) {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e})))
}

/// O comeco da janela; `Some(None)` = desde sempre; `None` = periodo invalido.
pub fn desde(periodo: &str, agora: DateTime<Utc>) -> Option<Option<DateTime<Utc>>> {
    match periodo {
        "24h" => Some(Some(agora - Duration::hours(24))),
        "7d" => Some(Some(agora - Duration::days(7))),
        "30d" => Some(Some(agora - Duration::days(30))),
        "tudo" => Some(None),
        _ => None,
    }
}

fn nome_do_estado(e: TaskStatus) -> &'static str {
    match e {
        TaskStatus::Pending => "pending",
        TaskStatus::AwaitingApproval => "awaiting_approval",
        TaskStatus::AwaitingInput => "awaiting_input",
        TaskStatus::Running => "running",
        TaskStatus::Completed => "completed",
        TaskStatus::Failed => "failed",
        TaskStatus::Cancelled => "cancelled",
        TaskStatus::BudgetExceeded => "budget_exceeded",
    }
}

/// O motivo agrupavel de uma falha, ou `None` quando ele tem forma de credencial.
pub fn motivo(erro: &str) -> Option<String> {
    if phxclaw_types::segredo::texto_tem_credencial(erro) {
        return None;
    }
    // PII sobrevive a troca de palavra-com-digito por `#` (e-mail nao tem digito): o painel
    // e por projeto, mas dado pessoal no motivo agrupavel nao agrega nada e nao deve aparecer.
    // Mesmo detector do no de politica (CPF/CNPJ com digito, e-mail, telefone com DDD).
    if !crate::fluxo_politica::achar_pii(erro).is_empty() {
        return None;
    }
    let linha = erro.lines().next().unwrap_or("").trim();
    // Palavra com digito (id, contagem, porta, caminho temporario) vira `#`: e o que
    // distingue dois erros iguais, e o que impediria agrupa-los.
    let s = linha
        .split_whitespace()
        .map(|w| {
            if w.chars().any(|c| c.is_ascii_digit()) {
                "#"
            } else {
                w
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    Some(s.chars().take(100).collect())
}

fn duracao_ms(t: &Task) -> Option<f64> {
    t.status
        .is_final()
        .then(|| (t.updated_at - t.created_at).num_milliseconds().max(0) as f64)
}

fn faixa(mut d: Vec<f64>) -> Value {
    if d.is_empty() {
        return json!({"amostras": 0, "p50_ms": null, "p95_ms": null});
    }
    d.sort_by(f64::total_cmp);
    json!({
        "amostras": d.len(),
        "p50_ms": crate::avaliacao::percentil(&d, 50),
        "p95_ms": crate::avaliacao::percentil(&d, 95),
    })
}

/// Custo somado, com a moeda; o nao medido conta a parte, nunca como zero.
fn custo(ts: &[&Task]) -> Value {
    let mut total = 0.0;
    let (mut medidas, mut nao) = (0usize, 0usize);
    let mut moedas: Vec<String> = vec![];
    for t in ts {
        match crate::custo::total(t) {
            Some(c) => {
                total += c;
                medidas += 1;
                let m = t
                    .gasto
                    .as_ref()
                    .and_then(|g| g.moeda.clone())
                    .or_else(|| t.custo.as_ref().and_then(|c| c.moeda.clone()));
                if let Some(m) = m
                    && !moedas.contains(&m)
                {
                    moedas.push(m);
                }
            }
            None => nao += 1,
        }
    }
    json!({
        "total": if medidas > 0 { json!((total * 1e6).round() / 1e6) } else { Value::Null },
        "moeda": if moedas.len() == 1 { json!(moedas[0]) } else { Value::Null },
        "moedas_misturadas": moedas.len() > 1,
        "medidas": medidas,
        "nao_medidas": nao,
    })
}

fn falhou(t: &Task) -> bool {
    matches!(t.status, TaskStatus::Failed | TaskStatus::BudgetExceeded)
}

/// O painel sobre `tarefas` (ja cortadas pelo RBAC).
pub fn calcular(
    tarefas: &[Task],
    desde: Option<DateTime<Utc>>,
    fluxo: Option<&str>,
    agora: DateTime<Utc>,
) -> Value {
    let fluxo_de = |t: &Task| {
        t.objective
            .strip_prefix(crate::fluxos::PREFIXO_TAREFA)
            .map(str::to_string)
    };
    let execucoes: Vec<&Task> = tarefas
        .iter()
        .filter(|t| t.parent.is_none())
        .filter(|t| desde.is_none_or(|d| t.created_at >= d))
        .filter(|t| fluxo.is_none_or(|f| fluxo_de(t).as_deref() == Some(f)))
        .collect();
    let mut por_estado: BTreeMap<&str, usize> = BTreeMap::new();
    for t in &execucoes {
        *por_estado.entry(nome_do_estado(t.status)).or_default() += 1;
    }
    let mut falhas: BTreeMap<Option<String>, usize> = BTreeMap::new();
    for t in execucoes.iter().filter(|t| falhou(t)) {
        *falhas
            .entry(motivo(t.error.as_deref().unwrap_or("")))
            .or_default() += 1;
    }
    let mut falhas: Vec<(Option<String>, usize)> = falhas.into_iter().collect();
    falhas.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let falhas: Vec<Value> = falhas
        .into_iter()
        .take(MAX_FALHAS)
        .map(|(m, n)| match m {
            Some(m) => json!({"motivo": m, "n": n}),
            None => json!({"motivo": null, "omitido": true, "n": n}),
        })
        .collect();
    let mut grupos: BTreeMap<String, Vec<&Task>> = BTreeMap::new();
    for t in &execucoes {
        if let Some(f) = fluxo_de(t) {
            grupos.entry(f).or_default().push(t);
        }
    }
    let mut fluxos: Vec<Value> = grupos
        .into_iter()
        .map(|(nome, ts)| {
            let n_falhas = ts.iter().filter(|t| falhou(t)).count();
            let terminadas = ts.iter().filter(|t| t.status.is_final()).count();
            json!({
                "nome": nome,
                "total": ts.len(),
                "falhas": n_falhas,
                "taxa_falha": if terminadas > 0 { json!(n_falhas as f64 / terminadas as f64) } else { Value::Null },
                "duracao": faixa(ts.iter().filter_map(|t| duracao_ms(t)).collect()),
                "custo": custo(&ts),
            })
        })
        .collect();
    fluxos.sort_by(|a, b| b["total"].as_u64().cmp(&a["total"].as_u64()));
    let mut dias: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for t in &execucoes {
        let d = dias
            .entry(t.created_at.format("%Y-%m-%d").to_string())
            .or_default();
        d.0 += 1;
        if falhou(t) {
            d.1 += 1;
        }
    }
    let terminadas = execucoes.iter().filter(|t| t.status.is_final()).count();
    let n_falhas = execucoes.iter().filter(|t| falhou(t)).count();
    json!({
        "desde": desde.map(|d| d.to_rfc3339()),
        "ate": agora.to_rfc3339(),
        "fluxo": fluxo,
        "total": execucoes.len(),
        "terminadas": terminadas,
        "falhas": n_falhas,
        "taxa_falha": if terminadas > 0 { json!(n_falhas as f64 / terminadas as f64) } else { Value::Null },
        "por_estado": por_estado,
        "duracao": faixa(execucoes.iter().filter_map(|t| duracao_ms(t)).collect()),
        "custo": custo(&execucoes),
        "falhas_comuns": falhas,
        "fluxos": fluxos,
        "dias": dias.into_iter().map(|(dia, (total, falhas))| json!({"dia": dia, "total": total, "falhas": falhas})).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod testes_motivo {
    use super::*;

    #[test]
    fn motivo_omite_credencial_e_pii() {
        // credencial: ja omitido
        assert_eq!(
            motivo("token ghp_A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8"),
            None
        );
        // e-mail (PII que sobrevive a troca de palavra-com-digito): omitido
        assert_eq!(motivo("SMTP rejeitou joao.silva@cliente.com"), None);
        // CPF valido: omitido
        assert_eq!(motivo("falha no cadastro 529.982.247-25"), None);
        // erro comum sem PII: agrupavel, com o numero virando #
        assert_eq!(
            motivo("timeout na porta 8787 do servico").as_deref(),
            Some("timeout na porta # do servico"),
        );
    }
}

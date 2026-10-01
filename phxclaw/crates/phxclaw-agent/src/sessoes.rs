//! O historico das tarefas como fonte: busca textual nas sessoes anteriores e o resumo
//! do dia. Os dois leem o `TaskStore`, o mesmo `task.json` que a API serve -- um indice
//! a parte divergiria do estado gravado no dia em que uma tarefa mudasse depois de
//! indexada. A ferramenta, a CLI e a tarefa agendada chamam estas mesmas funcoes.

use crate::tarefa::{Task, TaskStatus, TaskStore};
use chrono::{NaiveDate, Utc};
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq)]
pub struct Achado {
    pub id: String,
    pub quando: chrono::DateTime<Utc>,
    pub status: TaskStatus,
    pub objetivo: String,
    pub trecho: String,
    pub pontos: usize,
}

fn termos(consulta: &str) -> Vec<String> {
    let mut v: Vec<String> = consulta
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.chars().count() >= 3)
        .map(str::to_string)
        .collect();
    v.sort();
    v.dedup();
    v
}

const PESO_OBJETIVO: usize = 3;

/// Os textos de uma tarefa com o peso de cada um: o objetivo diz do que ela tratava, a
/// resposta o que ela achou; o resumo dos passos e o que menos discrimina.
fn campos(t: &Task) -> Vec<(String, usize)> {
    let mut v = vec![(t.objective.clone(), PESO_OBJETIVO)];
    if let Some(a) = &t.answer {
        v.push((a.clone(), 2));
    }
    v.extend(t.plan.iter().map(|p| (p.clone(), 1)));
    v.extend(t.steps.iter().map(|s| (s.summary.clone(), 1)));
    v
}

fn trecho(texto: &str, termo: &str) -> String {
    let baixo = texto.to_lowercase();
    let Some(pos) = baixo.find(termo) else {
        return texto.chars().take(200).collect();
    };
    // Indices de caractere: `to_lowercase` pode mudar o tamanho em bytes.
    let antes = baixo[..pos].chars().count();
    let ini = antes.saturating_sub(80);
    let s: String = texto.chars().skip(ini).take(240).collect();
    format!(
        "{}{}",
        if ini > 0 { "..." } else { "" },
        s.replace('\n', " ")
    )
}

/// As `n` tarefas que mais citam os termos (3 letras ou mais), mais novas no empate.
/// `excluir` tira a propria tarefa que busca: ela sempre casaria com o proprio objetivo.
pub fn buscar(
    store: &TaskStore,
    consulta: &str,
    n: usize,
    excluir: Option<&str>,
) -> Result<Vec<Achado>, String> {
    let ts = termos(consulta);
    if ts.is_empty() {
        return Err("consulta sem termo de 3 letras ou mais".into());
    }
    let mut achados = Vec::new();
    for t in store.list().map_err(|e| e.to_string())? {
        if Some(t.id.as_str()) == excluir {
            continue;
        }
        let cs = campos(&t);
        let mut pontos = 0;
        let mut melhor: Option<(usize, String)> = None;
        for termo in &ts {
            for (texto, peso) in &cs {
                let k = texto.to_lowercase().matches(termo.as_str()).count();
                pontos += k * peso;
                // O trecho sai do que a tarefa ACHOU (resposta, passos): o objetivo ja vai
                // na linha do achado, e repeti-lo como trecho nao mostraria nada novo.
                if k > 0 && *peso < PESO_OBJETIVO && melhor.as_ref().is_none_or(|(p, _)| *peso > *p)
                {
                    melhor = Some((*peso, trecho(texto, termo)));
                }
            }
        }
        if pontos > 0 {
            achados.push(Achado {
                id: t.id.clone(),
                quando: t.created_at,
                status: t.status,
                objetivo: t.objective.chars().take(200).collect(),
                trecho: melhor.map(|(_, s)| s).unwrap_or_default(),
                pontos,
            });
        }
    }
    // `list` ja vem da mais nova para a mais velha; o sort e estavel.
    achados.sort_by_key(|a| std::cmp::Reverse(a.pontos));
    achados.truncate(n);
    Ok(achados)
}

/// Resumo das tarefas criadas no dia (UTC): contagem por estado, tokens e uma linha por
/// tarefa-mae (subagente entra na conta, nao na lista). Deterministico, sem modelo: o
/// mesmo dia da o mesmo texto, e uma tarefa agendada pode entrega-lo ou resumi-lo.
pub fn resumo_do_dia(store: &TaskStore, dia: NaiveDate) -> Result<String, String> {
    let todas: Vec<Task> = store
        .list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|t| t.created_at.date_naive() == dia)
        .collect();
    let mut s = format!("# Resumo de {dia}\n\n");
    if todas.is_empty() {
        s.push_str("Nenhuma tarefa neste dia.\n");
        return Ok(s);
    }
    let conta = |st: TaskStatus| todas.iter().filter(|t| t.status == st).count();
    let (ent, sai) = todas.iter().fold((0u64, 0u64), |(a, b), t| {
        (a + t.usage.input_tokens, b + t.usage.output_tokens)
    });
    s.push_str(&format!(
        "{} tarefas: {} concluidas, {} falharam, {} canceladas, {} esperando pessoa, {} em andamento.\n\
Tokens: {ent} de entrada, {sai} de saida.\n\n",
        todas.len(),
        conta(TaskStatus::Completed),
        conta(TaskStatus::Failed),
        conta(TaskStatus::Cancelled),
        conta(TaskStatus::AwaitingApproval) + conta(TaskStatus::AwaitingInput),
        conta(TaskStatus::Running) + conta(TaskStatus::Pending),
    ));
    let mut maes: Vec<&Task> = todas.iter().filter(|t| t.parent.is_none()).collect();
    maes.sort_by_key(|t| t.created_at);
    for t in maes {
        let desfecho = match (&t.answer, &t.error) {
            (Some(a), _) if t.status == TaskStatus::Completed => a.clone(),
            (_, Some(e)) => e.clone(),
            _ => String::new(),
        };
        let desfecho: String = desfecho.replace('\n', " ").chars().take(160).collect();
        s.push_str(&format!(
            "- {} [{:?}] {}{}{}\n",
            t.created_at.format("%H:%M"),
            t.status,
            t.objective
                .replace('\n', " ")
                .chars()
                .take(120)
                .collect::<String>(),
            if desfecho.is_empty() { "" } else { " -> " },
            desfecho
        ));
        for a in &t.artifacts {
            s.push_str(&format!("    artefato: {}\n", a.path));
        }
    }
    Ok(s)
}

pub fn dia_do_argumento(v: Option<&str>) -> Result<NaiveDate, String> {
    match v.map(str::trim).filter(|s| !s.is_empty()) {
        None | Some("hoje") | Some("today") => Ok(Utc::now().date_naive()),
        Some("ontem") | Some("yesterday") => Ok(Utc::now().date_naive() - chrono::Days::new(1)),
        Some(d) => NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .map_err(|e| format!("data {d:?} (use AAAA-MM-DD): {e}")),
    }
}

pub struct SessionSearchTool {
    pub store: TaskStore,
}

impl Tool for SessionSearchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "session_search".into(),
            description: "Full-text search over previous tasks of this agent (objective, plan, \
answer and step summaries). Returns the best matches with id, date, status and a snippet. Use \
it to recall what was done or found before."
                .into(),
            parameters: json!({"type":"object","properties":{
                "query":{"type":"string"},
                "limit":{"type":"integer","description":"max results (default 5, up to 20)"}
            },"required":["query"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "session.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let q = args
                .get("query")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'query'".into()))?;
            let n = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(5)
                .clamp(1, 20) as usize;
            let a = buscar(&self.store, q, n, Some(&ctx.task_id))
                .map_err(ToolError::InvalidArguments)?;
            if a.is_empty() {
                return Ok(ToolOutput::text("no previous task matches"));
            }
            let mut s = String::new();
            for x in a {
                s.push_str(&format!(
                    "- task {} ({}, {:?}, score {}): {}\n  {}\n",
                    x.id,
                    x.quando.format("%Y-%m-%d %H:%M UTC"),
                    x.status,
                    x.pontos,
                    x.objetivo.replace('\n', " "),
                    x.trecho
                ));
            }
            Ok(ToolOutput::text(s))
        })
    }
}

pub struct DailySummaryTool {
    pub store: TaskStore,
}

impl Tool for DailySummaryTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "daily_summary".into(),
            description: "Summary of all tasks created on a day (UTC): counts by status, tokens, \
and one line per task with its outcome and files. date: YYYY-MM-DD, 'today' (default) or \
'yesterday'."
                .into(),
            parameters: json!({"type":"object","properties":{"date":{"type":"string"}}}),
        }
    }
    fn capability(&self) -> &'static str {
        "session.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let dia = dia_do_argumento(args.get("date").and_then(Value::as_str))
                .map_err(ToolError::InvalidArguments)?;
            resumo_do_dia(&self.store, dia)
                .map(ToolOutput::text)
                .map_err(ToolError::Failed)
        })
    }
}

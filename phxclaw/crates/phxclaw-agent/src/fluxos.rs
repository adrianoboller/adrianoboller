//! Fluxos declarativos: um arquivo JSON com passos e dependencias (um DAG), cada passo uma
//! tarefa do agente ou uma chamada de ferramenta.
//!
//! Nada aqui e motor novo. O grafo e a fila sao os do `phxclaw-task-graph` (ordem
//! topologica, ciclo recusado, dependencia desconhecida recusada, tentativas com espera);
//! o passo de agente roda pelo `ferramentas::rodar_filhas`, o MESMO laco do
//! `parallel_research` e do `team_delegate`; e o passo de ferramenta passa pelo
//! `Agent::call_tool`, o portao unico de capacidade, regras, hooks e evidencia. Um fluxo
//! com portao proprio seria a segunda copia da politica, e a que alguem esqueceria.
//!
//! Formato:
//! ```json
//! {"nome": "relatorio", "max_paralelo": 4, "passos": [
//!   {"id": "coleta", "tarefa": "liste tres fatos sobre Rust"},
//!   {"id": "grava", "depende": ["coleta"], "ferramenta": "write_file",
//!    "args": {"path": "fatos.md", "content": "{{coleta}}"}}
//! ]}
//! ```
//! `{{id}}` em texto de tarefa ou em argumento vira o resultado do passo `id` -- so de
//! passo DECLARADO em `depende`: referencia a passo que pode nao ter terminado seria uma
//! corrida, e por isso e recusada na leitura, nao descoberta na execucao.

use crate::ferramentas::{config_de_subagente, corpo_do_subagente, rodar_filhas};
use crate::motor::Agent;
use crate::tarefa::{Task, TaskStatus};
use chrono::Utc;
use phxclaw_agent_core::{ToolCall, ToolContext};
use phxclaw_evidence_ledger::EvidenceLedger;
use phxclaw_task_graph::{RetryPolicy, TaskGraph, TaskScheduler, TaskSpec, TaskStatus as Fila};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Teto de passos por fluxo: o arquivo vem do operador, mas um gerado por modelo com mil
/// passos nao pode virar mil subagentes.
pub const MAX_PASSOS: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Passo {
    pub id: String,
    #[serde(default)]
    pub depende: Vec<String>,
    /// Objetivo de um subagente.
    #[serde(default)]
    pub tarefa: Option<String>,
    /// Nome de ferramenta do agente.
    #[serde(default)]
    pub ferramenta: Option<String>,
    #[serde(default)]
    pub args: Value,
    /// Tentativas (1 = sem nova tentativa).
    #[serde(default = "uma")]
    pub tentativas: u16,
}

fn uma() -> u16 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fluxo {
    pub nome: String,
    #[serde(default = "quatro")]
    pub max_paralelo: usize,
    pub passos: Vec<Passo>,
}

fn quatro() -> usize {
    4
}

/// O resultado de um passo, como o fluxo o viu.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Resultado {
    pub id: String,
    /// `ok`, `falhou` ou `bloqueado` (dependencia que nao terminou bem).
    pub estado: String,
    pub saida: String,
    /// Tarefa filha, quando o passo e de agente.
    pub tarefa: Option<String>,
    pub tentativas: u16,
    /// Veio de uma execucao anterior do mesmo fluxo (retomada), sem rodar de novo.
    #[serde(default)]
    pub reaproveitado: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relatorio {
    pub tarefa: String,
    /// sha256 da definicao: retomar com outra definicao aplicaria saidas velhas a passos
    /// novos, e por isso e recusado.
    pub fluxo_sha256: String,
    pub sucesso: bool,
    /// Na ordem em que os passos TERMINARAM (a ordem topologica, por ondas).
    pub passos: Vec<Resultado>,
}

/// Le e valida um fluxo: ids unicos e validos, exatamente um tipo por passo, dependencia
/// conhecida, `{{x}}` so de dependencia declarada e nenhum ciclo (o grafo confere).
pub fn ler(texto: &str) -> Result<Fluxo, String> {
    let f: Fluxo = serde_json::from_str(texto).map_err(|e| format!("fluxo invalido: {e}"))?;
    validar(&f)?;
    Ok(f)
}

pub fn validar(f: &Fluxo) -> Result<(), String> {
    if f.passos.is_empty() || f.passos.len() > MAX_PASSOS {
        return Err(format!("o fluxo precisa de 1 a {MAX_PASSOS} passos"));
    }
    if f.max_paralelo == 0 {
        return Err("max_paralelo precisa ser pelo menos 1".into());
    }
    for p in &f.passos {
        if p.id.is_empty()
            || !p
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(format!("id de passo invalido: {:?}", p.id));
        }
        if p.tarefa.is_some() == p.ferramenta.is_some() {
            return Err(format!(
                "passo {}: diga 'tarefa' OU 'ferramenta' (exatamente um)",
                p.id
            ));
        }
        if p.tentativas == 0 {
            return Err(format!("passo {}: tentativas comeca em 1", p.id));
        }
        let mut textos = Vec::new();
        if let Some(t) = &p.tarefa {
            textos.push(t.clone());
        }
        juntar_textos(&p.args, &mut textos);
        for t in &textos {
            for r in referencias(t) {
                if !p.depende.contains(&r) {
                    return Err(format!(
                        "passo {} usa {{{{{r}}}}} sem declarar '{r}' em depende",
                        p.id
                    ));
                }
            }
        }
    }
    grafo(f).map(|_| ())
}

/// O DAG do `phxclaw-task-graph`, com o mapa id -> uuid de volta.
fn grafo(f: &Fluxo) -> Result<(TaskGraph, BTreeMap<Uuid, usize>), String> {
    let mut uuids: BTreeMap<&str, Uuid> = BTreeMap::new();
    for p in &f.passos {
        if uuids
            .insert(p.id.as_str(), phxclaw_types::new_uuid_v7())
            .is_some()
        {
            return Err(format!("id de passo repetido: {}", p.id));
        }
    }
    let mut specs = Vec::with_capacity(f.passos.len());
    let mut indice = BTreeMap::new();
    for (i, p) in f.passos.iter().enumerate() {
        let mut s = TaskSpec::new(
            p.id.clone(),
            if p.tarefa.is_some() {
                "agente"
            } else {
                "ferramenta"
            },
            Value::Null,
            p.id.clone(),
        );
        s.uuid = uuids[p.id.as_str()];
        s.dependencies = p
            .depende
            .iter()
            .map(|d| {
                uuids
                    .get(d.as_str())
                    .copied()
                    .ok_or_else(|| format!("passo {} depende de '{d}', que nao existe", p.id))
            })
            .collect::<Result<_, _>>()?;
        // Espera curta entre tentativas: o fluxo e interativo, nao uma fila de fundo.
        s.retry = RetryPolicy {
            max_attempts: p.tentativas,
            base_delay_ms: 50,
            max_delay_ms: 2_000,
        };
        indice.insert(s.uuid, i);
        specs.push(s);
    }
    let g = TaskGraph::new(specs).map_err(|e| match e {
        phxclaw_task_graph::TaskGraphError::Cycle => "o fluxo tem ciclo".to_string(),
        outro => outro.to_string(),
    })?;
    Ok((g, indice))
}

fn juntar_textos(v: &Value, saida: &mut Vec<String>) {
    match v {
        Value::String(s) => saida.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|x| juntar_textos(x, saida)),
        Value::Object(o) => o.values().for_each(|x| juntar_textos(x, saida)),
        _ => {}
    }
}

/// Os `x` de cada `{{x}}`.
fn referencias(t: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut resto = t;
    while let Some(i) = resto.find("{{") {
        let depois = &resto[i + 2..];
        let Some(j) = depois.find("}}") else { break };
        v.push(depois[..j].trim().to_string());
        resto = &depois[j + 2..];
    }
    v
}

fn substituir(t: &str, saidas: &BTreeMap<String, String>) -> String {
    let mut r = t.to_string();
    for (id, s) in saidas {
        r = r.replace(&format!("{{{{{id}}}}}"), s);
    }
    r
}

fn substituir_valor(v: &Value, saidas: &BTreeMap<String, String>) -> Value {
    match v {
        Value::String(s) => Value::String(substituir(s, saidas)),
        Value::Array(a) => Value::Array(a.iter().map(|x| substituir_valor(x, saidas)).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| (k.clone(), substituir_valor(x, saidas)))
                .collect(),
        ),
        outro => outro.clone(),
    }
}

/// Roda o fluxo inteiro. O fluxo e ele mesmo uma tarefa (pasta, evidencia, `task.json`):
/// os passos de ferramenta rodam na pasta dela, e os de agente sao tarefas filhas dela.
///
/// Por ondas: cada volta pega da fila tudo o que esta pronto (ate `max_paralelo`) e roda
/// junto. Passo que falha bloqueia os que dependem dele, e eles aparecem como `bloqueado`
/// no relatorio -- nunca somem.
pub async fn rodar(agente: &Agent, fluxo: &Fluxo) -> Result<Relatorio, String> {
    executar(agente, fluxo, None).await
}

/// Retoma um fluxo que parou no meio (passo que falhou, processo que caiu): o progresso
/// gravado a cada onda no `task.json` da tarefa do fluxo e o ponto de retomada. O que
/// terminou bem nao roda de novo -- a saida gravada volta para a fila como sucesso, e os
/// `{{id}}` dos passos seguintes a recebem igual.
pub async fn retomar(agente: &Agent, fluxo: &Fluxo, tarefa: &str) -> Result<Relatorio, String> {
    executar(agente, fluxo, Some(tarefa)).await
}

fn sha256_do_fluxo(f: &Fluxo) -> String {
    use sha2::Digest;
    let t = serde_json::to_string(f).unwrap_or_default();
    sha2::Sha256::digest(t.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

async fn executar(
    agente: &Agent,
    fluxo: &Fluxo,
    retomada: Option<&str>,
) -> Result<Relatorio, String> {
    validar(fluxo)?;
    let (g, indice) = grafo(fluxo)?;
    let mut fila = TaskScheduler::new(g);
    let hash = sha256_do_fluxo(fluxo);
    // passo id -> saida, do que ja terminou bem na execucao anterior
    let mut anteriores: BTreeMap<String, Resultado> = BTreeMap::new();
    let mut mae = match retomada {
        None => Task::new(format!("fluxo: {}", fluxo.nome), agente.llm.id()),
        Some(id) => {
            let t = agente
                .store
                .load(id)
                .map_err(|e| format!("tarefa do fluxo {id}: {e}"))?;
            let r: Relatorio = serde_json::from_str(t.answer.as_deref().unwrap_or(""))
                .map_err(|_| format!("tarefa {id} nao tem progresso de fluxo gravado"))?;
            if r.fluxo_sha256 != hash {
                return Err(format!(
                    "a definicao do fluxo mudou desde a tarefa {id}: retomar aplicaria saidas \
velhas a passos novos; rode de novo"
                ));
            }
            for p in r.passos.into_iter().filter(|p| p.estado == "ok") {
                anteriores.insert(p.id.clone(), p);
            }
            t
        }
    };
    mae.status = TaskStatus::Running;
    mae.error = None;
    agente
        .store
        .save(&mae)
        .map_err(|e| format!("tarefa do fluxo: {e}"))?;
    let ledger = EvidenceLedger::open(agente.store.evidence_path(&mae.id))
        .map_err(|e| format!("evidencia do fluxo: {e}"))?;
    let ctx = ToolContext {
        task_id: mae.id.clone(),
        workdir: agente.store.workdir(&mae.id),
        timeout: agente.config.tool_timeout,
    };
    let _ = std::fs::create_dir_all(&ctx.workdir);
    // O subagente do passo e o do `parallel_research`: menos passos, sem perguntar.
    let sub = Agent::new(
        agente.llm.clone(),
        agente.tools.clone(),
        config_de_subagente(&agente.config),
        agente.store.clone(),
    );
    let mut saidas: BTreeMap<String, String> = BTreeMap::new();
    let mut feitos: Vec<Resultado> = Vec::new();
    let mut tentativas: BTreeMap<String, u16> = BTreeMap::new();
    loop {
        let runs = fila.claim_ready(fluxo.max_paralelo, Utc::now());
        if runs.is_empty() {
            // Nada pronto: ou acabou, ou ha passo esperando a proxima tentativa.
            let esperando = fila
                .graph()
                .tasks()
                .any(|t| fila.state(&t.uuid).map(|s| s.status) == Some(Fila::Pending));
            if esperando {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                continue;
            }
            break;
        }
        // Separa a onda: agentes vao juntos pelo laco unico; ferramentas, pelo portao.
        let mut filhas = Vec::new();
        let mut de_filha = Vec::new();
        let mut chamadas = Vec::new();
        for r in &runs {
            let p = &fluxo.passos[indice[&r.task_uuid]];
            if let Some(antes) = anteriores.remove(&p.id) {
                fila.succeed(r.uuid, json!({"saida": antes.saida}), Utc::now())
                    .map_err(|e| e.to_string())?;
                saidas.insert(p.id.clone(), antes.saida.clone());
                feitos.push(Resultado {
                    reaproveitado: true,
                    ..antes
                });
                continue;
            }
            *tentativas.entry(p.id.clone()).or_default() += 1;
            if let Some(obj) = &p.tarefa {
                let mut t = Task::new(substituir(obj, &saidas), sub.llm.id());
                t.parent = Some(mae.id.clone());
                de_filha.push((r.uuid, p.id.clone(), t.id.clone()));
                filhas.push(t);
            } else if let Some(nome) = &p.ferramenta {
                chamadas.push((
                    r.uuid,
                    p.id.clone(),
                    ToolCall {
                        id: format!("{}-{}", p.id, r.attempt),
                        name: nome.clone(),
                        arguments: substituir_valor(&p.args, &saidas),
                    },
                ));
            }
        }
        let fut_ferramentas = futures_util::future::join_all(
            chamadas
                .iter()
                .map(|(_, _, c)| agente.call_tool(c, &ctx, &ledger, &mae.id)),
        );
        let (terminadas, respostas) = tokio::join!(rodar_filhas(&sub, filhas), fut_ferramentas);
        let agora = Utc::now();
        let mut desfechos: Vec<(Uuid, String, bool, String, Option<String>)> = Vec::new();
        for ((run, id, tid), t) in de_filha.into_iter().zip(terminadas) {
            debug_assert_eq!(tid, t.id);
            let ok = t.status == TaskStatus::Completed;
            desfechos.push((run, id, ok, corpo_do_subagente(&t), Some(t.id)));
        }
        for ((run, id, _), (texto, desfecho, _)) in chamadas.into_iter().zip(respostas) {
            desfechos.push((run, id, desfecho == "ok", texto, None));
        }
        for (run, id, ok, texto, tarefa) in desfechos {
            let n = tentativas[&id];
            if ok {
                fila.succeed(run, json!({"saida": texto}), agora)
                    .map_err(|e| e.to_string())?;
                saidas.insert(id.clone(), texto.clone());
                feitos.push(Resultado {
                    id,
                    estado: "ok".into(),
                    saida: texto,
                    tarefa,
                    tentativas: n,
                    reaproveitado: false,
                });
            } else {
                fila.fail(run, texto.clone(), true, agora)
                    .map_err(|e| e.to_string())?;
                let final_ = fila
                    .run(&run)
                    .and_then(|r| fila.state(&r.task_uuid))
                    .is_some_and(|s| matches!(s.status, Fila::Failed | Fila::DeadLetter));
                if final_ {
                    feitos.push(Resultado {
                        id,
                        estado: "falhou".into(),
                        saida: texto,
                        tarefa,
                        tentativas: n,
                        reaproveitado: false,
                    });
                }
            }
        }
        // O ponto de retomada: o progresso vai para o disco a cada onda, e um processo
        // que cair no meio deixa o que ja terminou bem gravado.
        mae.answer = serde_json::to_string_pretty(&Relatorio {
            tarefa: mae.id.clone(),
            fluxo_sha256: hash.clone(),
            sucesso: false,
            passos: feitos.clone(),
        })
        .ok();
        mae.updated_at = Utc::now();
        let _ = agente.store.save(&mae);
    }
    // O que nunca rodou ficou bloqueado por uma dependencia que nao terminou bem.
    for t in fila.graph().tasks() {
        let p = &fluxo.passos[indice[&t.uuid]];
        if !feitos.iter().any(|r| r.id == p.id) {
            feitos.push(Resultado {
                id: p.id.clone(),
                estado: "bloqueado".into(),
                saida: String::new(),
                tarefa: None,
                tentativas: 0,
                reaproveitado: false,
            });
        }
    }
    let sucesso = feitos.iter().all(|r| r.estado == "ok");
    let relatorio = Relatorio {
        tarefa: mae.id.clone(),
        fluxo_sha256: hash,
        sucesso,
        passos: feitos,
    };
    mae.status = if sucesso {
        TaskStatus::Completed
    } else {
        TaskStatus::Failed
    };
    mae.answer = serde_json::to_string_pretty(&relatorio).ok();
    if !sucesso {
        mae.error = Some("passo falhou ou ficou bloqueado".into());
    }
    mae.updated_at = Utc::now();
    let _ = agente.store.save(&mae);
    Ok(relatorio)
}

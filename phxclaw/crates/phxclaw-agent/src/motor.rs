//! O motor: planeja, executa com ferramentas, registra cada passo e termina.
//!
//! O laco e o de todo agente de ferramenta: o modelo responde; se pediu ferramentas, cada
//! uma passa pela politica (capacidade concedida ou negada), roda com prazo, vira evidencia
//! com hash encadeado e volta ao modelo como resultado; se nao pediu, a resposta e final.
//! Teto de passos e de tokens sempre presente, e cancelamento conferido a cada volta.

use crate::tarefa::{StepRecord, Task, TaskStatus, TaskStore};
use chrono::Utc;
use phxclaw_agent_core::{
    Artifact, Llm, LlmOptions, Message, Tool, ToolCall, ToolContext, ToolError, ToolOutput,
    ToolSpec, truncate_for_model,
};
use phxclaw_evidence_ledger::{EvidenceDraft, EvidenceLedger, EvidenceOutcome};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub max_steps: u32,
    /// Caracteres de resultado de ferramenta devolvidos ao modelo por chamada.
    pub max_tool_output_chars: usize,
    pub tool_timeout: Duration,
    pub options: LlmOptions,
    /// Politica: so as ferramentas cuja capacidade esta aqui rodam. Nega por padrao.
    pub capabilities: BTreeSet<String>,
    /// Instrucao extra do operador, somada ao prompt de sistema.
    pub extra_instructions: Option<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_steps: 16,
            max_tool_output_chars: 6_000,
            tool_timeout: Duration::from_secs(120),
            options: LlmOptions::default(),
            capabilities: BTreeSet::new(),
            extra_instructions: None,
        }
    }
}

impl AgentConfig {
    pub fn grant(mut self, caps: &[&str]) -> Self {
        self.capabilities.extend(caps.iter().map(|c| c.to_string()));
        self
    }
}

/// Sinal de cancelamento compartilhado entre a API e a execucao.
#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Observador de progresso (a API e a tela assinam; o motor nao sabe quem e).
pub trait Observer: Send + Sync {
    fn on_update(&self, task: &Task);
}

pub struct NoObserver;
impl Observer for NoObserver {
    fn on_update(&self, _task: &Task) {}
}

#[derive(Clone)]
pub struct Agent {
    pub llm: Arc<dyn Llm>,
    pub tools: Vec<Arc<dyn Tool>>,
    pub config: AgentConfig,
    pub store: TaskStore,
}

const PROMPT_BASE: &str = "You are PhxClaw, an autonomous agent that completes the user's objective \
by calling tools. Work step by step. Use tools to get real information and to produce files; \
never invent facts, URLs or file contents you did not obtain from a tool. Files you create live \
in the task working directory; mention their relative paths in the final answer. When the \
objective is complete, reply with the final answer in the user's language WITHOUT calling any \
tool. If a tool is denied by policy, do not retry it; work with what is allowed or explain.";

impl Agent {
    pub fn new(
        llm: Arc<dyn Llm>,
        tools: Vec<Arc<dyn Tool>>,
        config: AgentConfig,
        store: TaskStore,
    ) -> Self {
        Self {
            llm,
            tools,
            config,
            store,
        }
    }

    /// Ferramentas que a politica deixa o modelo VER. As negadas nem aparecem: modelo que
    /// ve uma ferramenta proibida tenta usa-la e gasta passos.
    fn visible_specs(&self) -> Vec<ToolSpec> {
        self.tools
            .iter()
            .filter(|t| self.config.capabilities.contains(t.capability()))
            .map(|t| t.spec())
            .collect()
    }

    /// Plan Mode: pede ao modelo um plano curto em JSON. O plano e sugestao editavel, nao
    /// contrato; se o modelo nao devolver JSON valido, o plano vira o proprio objetivo.
    pub async fn plan(&self, task: &mut Task) -> Result<(), String> {
        let ferramentas: Vec<String> = self
            .visible_specs()
            .iter()
            .map(|s| s.name.clone())
            .collect();
        let msgs = vec![
            Message::system(format!(
                "You plan tasks for an autonomous agent. Available tools: {}. \
Reply ONLY with JSON: {{\"steps\": [\"short step\", ...]}} with 2 to 7 steps, in the user's language.",
                ferramentas.join(", ")
            )),
            Message::user(task.objective.clone()),
        ];
        let reply = self
            .llm
            .chat(&msgs, &[], &self.config.options)
            .await
            .map_err(|e| e.to_string())?;
        task.usage.input_tokens += reply.usage.input_tokens;
        task.usage.output_tokens += reply.usage.output_tokens;
        task.plan = parse_plan(&reply.content).unwrap_or_else(|| vec![task.objective.clone()]);
        task.updated_at = Utc::now();
        Ok(())
    }

    /// Executa a tarefa ate a resposta final, o teto de passos, erro ou cancelamento.
    /// Grava o estado a cada passo e devolve a tarefa final.
    pub async fn run(&self, mut task: Task, cancel: &CancelFlag, obs: &dyn Observer) -> Task {
        task.status = TaskStatus::Running;
        task.updated_at = Utc::now();
        self.persist(&task, obs);
        let ledger = match EvidenceLedger::open(self.store.evidence_path(&task.id)) {
            Ok(l) => l,
            Err(e) => {
                return self.finish(
                    task,
                    TaskStatus::Failed,
                    Some(format!("evidencia: {e}")),
                    obs,
                );
            }
        };
        let ctx = ToolContext {
            task_id: task.id.clone(),
            workdir: self.store.workdir(&task.id),
            timeout: self.config.tool_timeout,
        };
        let _ = std::fs::create_dir_all(&ctx.workdir);
        let specs = self.visible_specs();
        let mut sistema = PROMPT_BASE.to_string();
        if !task.plan.is_empty() {
            sistema.push_str("\n\nApproved plan:\n");
            for (i, p) in task.plan.iter().enumerate() {
                sistema.push_str(&format!("{}. {p}\n", i + 1));
            }
        }
        if let Some(x) = &self.config.extra_instructions {
            sistema.push_str("\n\n");
            sistema.push_str(x);
        }
        let mut msgs = vec![
            Message::system(sistema),
            Message::user(task.objective.clone()),
        ];

        for passo in 0..self.config.max_steps {
            if cancel.is_cancelled() {
                return self.finish(task, TaskStatus::Cancelled, None, obs);
            }
            let reply = match self.llm.chat(&msgs, &specs, &self.config.options).await {
                Ok(r) => r,
                Err(e) => {
                    return self.finish(
                        task,
                        TaskStatus::Failed,
                        Some(format!("modelo: {e}")),
                        obs,
                    );
                }
            };
            task.usage.input_tokens += reply.usage.input_tokens;
            task.usage.output_tokens += reply.usage.output_tokens;
            if reply.tool_calls.is_empty() {
                task.answer = Some(reply.content.trim().to_string());
                self.record(
                    &mut task,
                    passo,
                    "pensamento",
                    None,
                    Value::Null,
                    "ok",
                    &reply.content,
                );
                return self.finish(task, TaskStatus::Completed, None, obs);
            }
            if !reply.content.trim().is_empty() {
                self.record(
                    &mut task,
                    passo,
                    "pensamento",
                    None,
                    Value::Null,
                    "ok",
                    &reply.content,
                );
            }
            msgs.push(Message::assistant(
                reply.content.clone(),
                reply.tool_calls.clone(),
            ));
            for call in &reply.tool_calls {
                if cancel.is_cancelled() {
                    return self.finish(task, TaskStatus::Cancelled, None, obs);
                }
                let (texto, outcome, artefatos) =
                    self.call_tool(call, &ctx, &ledger, &task.id).await;
                self.record(
                    &mut task,
                    passo,
                    "ferramenta",
                    Some(call.name.clone()),
                    call.arguments.clone(),
                    outcome,
                    &texto,
                );
                for a in artefatos {
                    if !task.artifacts.iter().any(|x| x.path == a.path) {
                        task.artifacts.push(a);
                    } else if let Some(x) = task.artifacts.iter_mut().find(|x| x.path == a.path) {
                        *x = a;
                    }
                }
                msgs.push(Message::tool_result(
                    call,
                    truncate_for_model(&texto, self.config.max_tool_output_chars),
                ));
            }
            self.persist(&task, obs);
        }
        self.finish(
            task,
            TaskStatus::Failed,
            Some(format!(
                "limite de {} passos sem resposta final",
                self.config.max_steps
            )),
            obs,
        )
    }

    async fn call_tool(
        &self,
        call: &ToolCall,
        ctx: &ToolContext,
        ledger: &EvidenceLedger,
        task_id: &str,
    ) -> (String, &'static str, Vec<Artifact>) {
        let tool = self.tools.iter().find(|t| t.spec().name == call.name);
        let (resultado, capability): (Result<ToolOutput, ToolError>, String) = match tool {
            None => (
                Err(ToolError::InvalidArguments(format!(
                    "ferramenta inexistente: {}",
                    call.name
                ))),
                "desconhecida".into(),
            ),
            // Politica conferida de novo aqui, e nao so na lista: o modelo pode pedir pelo
            // nome uma ferramenta que nao lhe foi mostrada.
            Some(t) if !self.config.capabilities.contains(t.capability()) => (
                Err(ToolError::Denied(format!(
                    "capacidade {} nao concedida",
                    t.capability()
                ))),
                t.capability().to_string(),
            ),
            Some(t) => {
                let r = match tokio::time::timeout(
                    self.config.tool_timeout,
                    t.run(call.arguments.clone(), ctx),
                )
                .await
                {
                    Ok(r) => r,
                    Err(_) => Err(ToolError::Timeout(
                        self.config.tool_timeout.as_millis() as u64
                    )),
                };
                (r, t.capability().to_string())
            }
        };
        let (texto, outcome, ev, artefatos) = match resultado {
            Ok(o) => (o.content, "ok", EvidenceOutcome::Succeeded, o.artifacts),
            Err(ToolError::Denied(m)) => (
                format!("NEGADO pela politica: {m}"),
                "negado",
                EvidenceOutcome::Denied,
                vec![],
            ),
            Err(e) => (
                format!("ERRO: {e}"),
                "erro",
                EvidenceOutcome::Failed,
                vec![],
            ),
        };
        let _ = ledger.append(EvidenceDraft {
            action_uuid: phxclaw_types::new_uuid_v7(),
            correlation_uuid: task_id.parse().ok(),
            actor: "phxclaw-agent".into(),
            capability,
            action: call.name.clone(),
            outcome: ev,
            request_summary: call.arguments.clone(),
            result_summary: json!({
                "chars": texto.chars().count(),
                "sha256": sha256_hex(texto.as_bytes()),
                "artifacts": artefatos.iter().map(|a| json!({"path": a.path, "sha256": a.sha256})).collect::<Vec<_>>(),
            }),
            artifact_uris: artefatos.iter().map(|a| a.path.clone()).collect(),
        });
        (texto, outcome, artefatos)
    }

    #[allow(clippy::too_many_arguments)]
    fn record(
        &self,
        task: &mut Task,
        passo: u32,
        kind: &str,
        tool: Option<String>,
        arguments: Value,
        outcome: &str,
        texto: &str,
    ) {
        task.steps.push(StepRecord {
            n: passo + 1,
            at: Utc::now(),
            kind: kind.into(),
            tool,
            arguments,
            outcome: outcome.into(),
            summary: truncate_for_model(texto.trim(), 400),
        });
    }

    fn persist(&self, task: &Task, obs: &dyn Observer) {
        let _ = self.store.save(task);
        obs.on_update(task);
    }

    fn finish(
        &self,
        mut task: Task,
        status: TaskStatus,
        error: Option<String>,
        obs: &dyn Observer,
    ) -> Task {
        task.status = status;
        task.error = error;
        task.updated_at = Utc::now();
        self.persist(&task, obs);
        task
    }
}

/// Aceita o JSON cercado de texto ou de ``` que modelos pequenos costumam devolver.
pub fn parse_plan(texto: &str) -> Option<Vec<String>> {
    let ini = texto.find('{')?;
    let fim = texto.rfind('}')?;
    let v: Value = serde_json::from_str(&texto[ini..=fim]).ok()?;
    let passos: Vec<String> = v
        .get("steps")
        .or_else(|| v.get("passos"))?
        .as_array()?
        .iter()
        .filter_map(|p| p.as_str().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .take(12)
        .collect();
    (!passos.is_empty()).then_some(passos)
}

pub fn sha256_hex(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

/// Artefato a partir de um arquivo ja gravado na pasta da tarefa.
pub fn artifact_for(workdir: &std::path::Path, relative: &str) -> std::io::Result<Artifact> {
    let bytes = std::fs::read(workdir.join(relative))?;
    Ok(Artifact {
        path: relative.to_string(),
        media_type: media_type(relative).into(),
        bytes: bytes.len() as u64,
        sha256: sha256_hex(&bytes),
    })
}

pub fn media_type(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "md" => "text/markdown",
        "txt" | "log" => "text/plain",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "text/javascript",
        "json" => "application/json",
        "csv" => "text/csv",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plano_aceita_json_cercado() {
        let p = parse_plan("Claro!\n```json\n{\"steps\": [\"buscar\", \" \", \"escrever\"]}\n```")
            .unwrap();
        assert_eq!(p, vec!["buscar", "escrever"]);
        assert_eq!(parse_plan("sem json"), None);
        assert_eq!(parse_plan("{\"passos\": [\"a\"]}").unwrap(), vec!["a"]);
    }
}

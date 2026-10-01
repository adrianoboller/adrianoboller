//! A tarefa como a API a serializa: o estado gravado em `task.json`, o resumo da listagem,
//! o pedido de criacao e a resposta dele.
//!
//! Moram aqui, e nao no `phxclaw-agent`, para que o SDK leia o MESMO tipo que o servidor
//! grava sem depender do agente inteiro (axum, postgres, navegador). Um segundo `Task`
//! no SDK divergiria do primeiro no dia em que alguem acrescentasse um campo so de um lado,
//! e o cliente deixaria de ler a tarefa sem erro de compilacao nenhum.

use crate::{Artifact, Usage};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Criada; o plano ainda nao foi feito.
    Pending,
    /// Plano pronto e esperando alguem aprovar ou editar (o "Plan Mode").
    AwaitingApproval,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn is_final(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepRecord {
    pub n: u32,
    pub at: DateTime<Utc>,
    /// "pensamento" (resposta do modelo) ou "ferramenta".
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default)]
    pub arguments: Value,
    /// ok | erro | negado
    pub outcome: String,
    /// Resumo curto para a tela; o conteudo inteiro foi para o modelo e a evidencia.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub objective: String,
    pub model: String,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub plan: Vec<String>,
    #[serde(default)]
    pub steps: Vec<StepRecord>,
    #[serde(default)]
    pub answer: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
    #[serde(default)]
    pub usage: Usage,
    #[serde(default)]
    pub error: Option<String>,
    /// Tarefa-mae, quando esta e um subagente.
    #[serde(default)]
    pub parent: Option<String>,
    /// URL chamada quando a tarefa termina (opcional).
    #[serde(default)]
    pub webhook: Option<String>,
}

impl Task {
    pub fn new(objective: impl Into<String>, model: impl Into<String>) -> Self {
        let agora = Utc::now();
        Self {
            id: phxclaw_types::new_uuid_v7().to_string(),
            objective: objective.into(),
            model: model.into(),
            status: TaskStatus::Pending,
            created_at: agora,
            updated_at: agora,
            plan: vec![],
            steps: vec![],
            answer: None,
            artifacts: vec![],
            usage: Usage::default(),
            error: None,
            parent: None,
            webhook: None,
        }
    }
}

/// Uma linha da listagem (`GET /v1/tasks`) e o corpo do webhook de fim: `steps` e a
/// CONTAGEM, porque a lista inteira de cada tarefa faria a listagem crescer sem teto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskSummary {
    pub id: String,
    pub objective: String,
    pub status: TaskStatus,
    pub model: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub answer: Option<String>,
    pub error: Option<String>,
    pub steps: usize,
    pub artifacts: Vec<Artifact>,
    pub usage: Usage,
    pub parent: Option<String>,
}

impl From<&Task> for TaskSummary {
    fn from(t: &Task) -> Self {
        Self {
            id: t.id.clone(),
            objective: t.objective.clone(),
            status: t.status,
            model: t.model.clone(),
            created_at: t.created_at,
            updated_at: t.updated_at,
            answer: t.answer.clone(),
            error: t.error.clone(),
            steps: t.steps.len(),
            artifacts: t.artifacts.clone(),
            usage: t.usage.clone(),
            parent: t.parent.clone(),
        }
    }
}

/// Corpo de `POST /v1/tasks`. O canal de mensagens monta o mesmo pedido.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NovaTarefa {
    pub objective: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Plan Mode: gera o plano e espera aprovacao antes de executar.
    #[serde(default)]
    pub plan_first: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook: Option<String>,
}

/// Resposta 202 de `POST /v1/tasks`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TarefaCriada {
    pub id: String,
}

/// Corpo de toda recusa da API. `retry_after` so vem no 429 do limite de criacao.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErroDaApi {
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<u64>,
}

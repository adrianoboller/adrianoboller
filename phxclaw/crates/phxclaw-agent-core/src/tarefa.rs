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
    /// A tarefa perguntou ao usuario (`ask_user`, ou um comando que a regra manda
    /// perguntar) e espera a resposta pela API ou pela CLI; a pergunta esta em `question`.
    AwaitingInput,
    Running,
    Completed,
    Failed,
    Cancelled,
    /// Parou porque bateu o orcamento (tokens ou dinheiro) da tarefa ou de um ancestral
    /// (o fluxo, a tarefa-mae). Estado proprio, e nao `Failed`: quem gastou o teto nao
    /// errou, e quem le a lista tem de distinguir «falhou» de «parou de gastar».
    BudgetExceeded,
}

impl TaskStatus {
    pub fn is_final(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::BudgetExceeded
        )
    }
}

/// Orcamento de uma tarefa ou de um fluxo. Cada dimensao e opcional; `custo` e na moeda da
/// tabela de precos do operador (`custo.precos`), que nao viaja aqui de proposito: a tabela
/// e uma so por instancia, e um pedido nao escolhe a moeda em que o servidor confere.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Orcamento {
    /// Tokens de entrada + saida, somados de todas as chamadas ao modelo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custo: Option<f64>,
}

impl Orcamento {
    pub fn vazio(&self) -> bool {
        self.tokens.is_none() && self.custo.is_none()
    }
}

/// A cotacao de onde saiu um preco: a data e a fonte que o operador escreveu na tabela.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cotacao {
    pub data: String,
    pub fonte: String,
}

/// O custo de UMA chamada ao modelo. `custo` ausente e «nao medido» (modelo sem preco na
/// tabela), nunca zero: zero e o preco que o operador declarou, ausente e o que ninguem sabe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustoDaChamada {
    pub modelo: String,
    pub tokens_entrada: u64,
    pub tokens_saida: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custo: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cotacao: Option<Cotacao>,
}

/// O custo das chamadas da PROPRIA tarefa (as filhas tem o delas; a soma com elas e o
/// `Gasto`). `total` ausente e «nao medido», com o motivo em `nao_medido`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CustoDaTarefa {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moeda: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nao_medido: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chamadas: Vec<CustoDaChamada>,
}

/// O que conta contra o orcamento: a tarefa e as descendentes (subagentes, passos de
/// fluxo, sub-fluxos). `custo` ausente e «nao medido»: basta uma chamada sem preco.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Gasto {
    pub tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custo: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moeda: Option<String>,
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
    /// Pergunta pendente enquanto o estado e `AwaitingInput`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    /// Imagens anexadas ao objetivo, como caminhos relativos a pasta de trabalho
    /// (`entrada/imagem-1.png`). Os bytes ficam no disco e nao no `task.json`: a tarefa
    /// se grava a cada passo, e reescrever megabytes de base64 por passo nao compra nada.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
    /// Comando de verificacao do fim (`--verificar` na CLI, `verificar` na API): roda no MESMO
    /// sandbox dos hooks, na pasta da tarefa, e codigo diferente de 0 recusa a conclusao.
    /// Fica na tarefa, e nao na configuracao, porque o criterio de pronto e de cada pedido.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verificar: Option<String>,
    /// Esquema JSON da resposta final (`--saida-esquema`): o `final_answer` passa a pedir
    /// o objeto, e o MESMO validador das ferramentas o confere antes de aceitar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saida_esquema: Option<Value>,
    /// Projeto da tarefa quando a API tem usuarios (`rbac.rs` do agente): e o que o portao
    /// le para decidir quem a ve. Sai do cabecalho `X-PhxClaw-Projeto` (ou do unico projeto
    /// do usuario), nunca do corpo do pedido. Ausente = tarefa da instancia (so dono e admin).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projeto: Option<String>,
    /// O orcamento em vigor (o do pedido, senao o padrao da configuracao, nunca acima do
    /// teto global). Gravado na tarefa para a retomada conferir o MESMO teto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orcamento: Option<Orcamento>,
    /// O custo das chamadas desta tarefa ao modelo, chamada a chamada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custo: Option<CustoDaTarefa>,
    /// O gasto desta tarefa somado ao das descendentes: e o numero que o orcamento confere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gasto: Option<Gasto>,
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
            question: None,
            images: vec![],
            verificar: None,
            saida_esquema: None,
            projeto: None,
            orcamento: None,
            custo: None,
            gasto: None,
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
    /// Imagens que o modelo ve junto do objetivo, em base64 (`png`, `jpeg`, `gif`,
    /// `webp`). O tipo se confere pelos bytes, nunca pelo que o cliente declara.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
    /// Grava cada pedido e resposta do modelo e cada ferramenta em
    /// `<tarefa>/gravacao.jsonl`, com segredo redigido. O caminho e do servidor, nunca do
    /// pedido: quem chama pela rede nao escolhe onde o servidor escreve.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gravar: bool,
    /// Comando que confere o fim, no sandbox da tarefa (ver `Task::verificar`). Pede a
    /// capacidade `shell.exec`: sem ela, verificar seria a porta lateral do shell negado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verificar: Option<String>,
    /// Esquema JSON da resposta final (ver `Task::saida_esquema`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saida_esquema: Option<Value>,
    /// Orcamento deste pedido. Nunca passa do teto global do operador: acima dele, a
    /// criacao e recusada dizendo o teto, em vez de cortar calado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orcamento: Option<Orcamento>,
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

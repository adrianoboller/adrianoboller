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

/// Uma troca de estado da tarefa, com a hora em que aconteceu. O historico e a trilha que o
/// cartao do Kanban mostra no detalhe: quando a tarefa saiu de Pending, quando comecou a
/// rodar, quando parou. Nasce no `Task::new` com o estado inicial em `created_at`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransicaoEstado {
    pub estado: TaskStatus,
    pub em: DateTime<Utc>,
}

/// Uma resposta do usuario a uma pergunta do agente, guardada com a hora. E dado do usuario,
/// gravado como veio -- o mesmo texto que ja seguia para o modelo, nada novo exposto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ajuste {
    pub texto: String,
    pub em: DateTime<Utc>,
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
    /// A trilha das trocas de estado, empilhada SO por `mudar_estado`: um ponto unico loga a
    /// transicao, para nenhum `status = X` cru escapar sem data.
    #[serde(default)]
    pub historico: Vec<TransicaoEstado>,
    /// As respostas do usuario as perguntas do agente, na ordem em que chegaram.
    #[serde(default)]
    pub ajustes: Vec<Ajuste>,
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
            // O estado inicial entra no historico com `created_at`: a trilha comeca cheia,
            // nao com a primeira transicao.
            historico: vec![TransicaoEstado {
                estado: TaskStatus::Pending,
                em: agora,
            }],
            ajustes: vec![],
        }
    }

    /// O UNICO ponto que troca o estado da tarefa: se mudou, empilha a transicao com a hora
    /// e adianta `updated_at`. Existe para que a trilha seja de um lugar so -- espalhar
    /// `status = X` por api.rs, motor.rs e fila.rs deixaria uma transicao sem data por fora.
    /// Troca para o mesmo estado nao vira linha: nada mudou.
    pub fn mudar_estado(&mut self, novo: TaskStatus) {
        if self.status == novo {
            return;
        }
        let agora = Utc::now();
        self.status = novo;
        self.updated_at = agora;
        self.historico.push(TransicaoEstado {
            estado: novo,
            em: agora,
        });
    }

    /// Guarda a resposta do usuario a uma pergunta do agente. Vem do texto que ja ia ao
    /// modelo: nada de segredo novo entra aqui.
    pub fn registrar_ajuste(&mut self, texto: &str) {
        self.ajustes.push(Ajuste {
            texto: texto.to_string(),
            em: Utc::now(),
        });
    }

    /// O progresso MEDIDO, nunca inventado: 100 no fim, 0 em Pending, e, com plano, a fracao
    /// de passos ja dados sobre os passos do plano (travada em 100). Sem plano e ainda
    /// rodando nao ha fracao honesta -- `None`, e a UI mostra «em andamento» sem numero.
    pub fn progresso(&self) -> Option<u8> {
        match self.status {
            TaskStatus::Completed => Some(100),
            TaskStatus::Pending => Some(0),
            _ if self.plan.is_empty() => None,
            _ => {
                let total = self.plan.len() as u64;
                let feitos = (self.steps.len() as u64).min(total);
                Some((feitos * 100 / total) as u8)
            }
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
    /// O que o cartao do Kanban mostra no detalhe: a trilha de estados, as respostas do
    /// usuario e o progresso medido. Sao pequenos (uma linha por transicao e por resposta),
    /// ao contrario de `steps`, que vira contagem.
    #[serde(default)]
    pub historico: Vec<TransicaoEstado>,
    #[serde(default)]
    pub ajustes: Vec<Ajuste>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progresso: Option<u8>,
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
            historico: t.historico.clone(),
            ajustes: t.ajustes.clone(),
            progresso: t.progresso(),
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

#[cfg(test)]
mod testes {
    use super::*;

    fn passo(n: u32) -> StepRecord {
        StepRecord {
            n,
            at: Utc::now(),
            kind: "ferramenta".into(),
            tool: Some("x".into()),
            arguments: Value::Null,
            outcome: "ok".into(),
            summary: String::new(),
        }
    }

    #[test]
    fn transicao_grava_data() {
        let mut t = Task::new("x", "m");
        // O estado inicial ja nasce no historico, com `created_at`.
        assert_eq!(t.historico.len(), 1);
        assert_eq!(t.historico[0].estado, TaskStatus::Pending);
        assert_eq!(t.historico[0].em, t.created_at);

        t.mudar_estado(TaskStatus::Running);
        t.mudar_estado(TaskStatus::Completed);

        let estados: Vec<TaskStatus> = t.historico.iter().map(|h| h.estado).collect();
        assert_eq!(
            estados,
            vec![
                TaskStatus::Pending,
                TaskStatus::Running,
                TaskStatus::Completed
            ]
        );
        // As datas nao recuam, e `updated_at` acompanha a ultima.
        assert!(t.historico[0].em <= t.historico[1].em);
        assert!(t.historico[1].em <= t.historico[2].em);
        assert_eq!(t.updated_at, t.historico[2].em);

        // Troca para o mesmo estado nao vira linha nova.
        t.mudar_estado(TaskStatus::Completed);
        assert_eq!(t.historico.len(), 3);
    }

    #[test]
    fn progresso_medido() {
        let mut t = Task::new("x", "m");
        // Pending = 0.
        assert_eq!(t.progresso(), Some(0));
        // Sem plano e rodando: nao ha fracao honesta.
        t.mudar_estado(TaskStatus::Running);
        assert_eq!(t.progresso(), None);
        // 2 de 4 passos do plano = 50.
        t.plan = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        t.steps = vec![passo(1), passo(2)];
        assert_eq!(t.progresso(), Some(50));
        // Completed = 100 mesmo com plano so pela metade.
        t.mudar_estado(TaskStatus::Completed);
        assert_eq!(t.progresso(), Some(100));
    }

    #[test]
    fn ajuste_guarda_texto_e_data() {
        let mut t = Task::new("x", "m");
        assert!(t.ajustes.is_empty());
        t.registrar_ajuste("Maria Ltda");
        assert_eq!(t.ajustes.len(), 1);
        assert_eq!(t.ajustes[0].texto, "Maria Ltda");
        assert!(t.ajustes[0].em >= t.created_at);
    }
}

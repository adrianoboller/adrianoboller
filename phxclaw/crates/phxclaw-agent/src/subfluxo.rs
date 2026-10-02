//! Sub-fluxo: a ferramenta `fluxo` roda um fluxo gravado -- por `caminho` ou por `nome`
//! na pasta de fluxos do agente -- com itens de entrada, em `once` (a lista inteira numa
//! execucao) ou `each` (uma execucao por item). A saida sao os itens do ultimo passo que
//! terminou bem, como JSON, para o passo seguinte do fluxo de cima os ler como itens.
//!
//! E ferramenta comum: capacidade `flow.run`, esquema, regra e hook pelo portao de sempre,
//! e a execucao e o MESMO `fluxos::rodar_com` da CLI e do gatilho. O teto de profundidade
//! e a recusa de ciclo (A -> B -> A) sao do `fluxos::conferir_cadeia`, pela cadeia de
//! `parent` das tarefas no disco: a tarefa que chamou a ferramenta vira o `pai` do
//! sub-fluxo, e a cadeia continua inteira mesmo quando o chamador e um subagente.
//!
//! Inspiracao no `ExecuteWorkflow` do n8n (SP000035), nao copia: la o sub-fluxo recebe o
//! `workflowId` e roda no mesmo processo com as credenciais decifradas; aqui o modelo so
//! nomeia um arquivo que o operador gravou, e cada passo do sub-fluxo paga o portao de
//! novo com as capacidades do agente que o chamou -- sub-fluxo nao amplia poder.

use crate::fluxos::{self, Execucao, Fluxo};
use crate::motor::Agent;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// Teto de itens numa execucao `each`: cada item e um fluxo inteiro.
pub const ITENS_MAX: usize = 100;

pub struct FluxoTool {
    /// O agente com que o sub-fluxo roda. Preenchido pela montagem DEPOIS de a propria
    /// ferramenta entrar na lista, para um fluxo poder chamar outro fluxo (o teto de
    /// profundidade e quem segura a recursao, nao a ausencia da ferramenta).
    pub base: Arc<OnceLock<Agent>>,
    /// Onde `nome` e procurado: `<raiz do agente>/fluxos/<nome>.json`.
    pub pasta: PathBuf,
}

impl FluxoTool {
    pub fn nova(pasta: impl Into<PathBuf>) -> Self {
        Self {
            base: Arc::new(OnceLock::new()),
            pasta: pasta.into(),
        }
    }

    /// O arquivo do fluxo: `caminho` como veio, ou `nome` (so letra, digito, `-`, `_`)
    /// dentro da pasta de fluxos -- nome com `/` ou `..` nao vira caminho pelas costas.
    fn arquivo(&self, args: &Value) -> Result<PathBuf, ToolError> {
        let texto = |k: &str| {
            args.get(k)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
        };
        match (texto("caminho"), texto("nome")) {
            (Some(c), None) => Ok(PathBuf::from(c)),
            (None, Some(n)) => {
                if !n
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                {
                    return Err(ToolError::Denied(format!(
                        "nome de fluxo invalido: {n:?} (so letra, digito, '-' e '_')"
                    )));
                }
                Ok(self.pasta.join(format!("{n}.json")))
            }
            _ => Err(ToolError::InvalidArguments(
                "informe 'caminho' OU 'nome' do fluxo".into(),
            )),
        }
    }
}

fn ler_fluxo(arq: &Path) -> Result<Fluxo, ToolError> {
    let texto = std::fs::read_to_string(arq)
        .map_err(|e| ToolError::InvalidArguments(format!("fluxo {}: {e}", arq.display())))?;
    fluxos::ler(&texto).map_err(|e| ToolError::InvalidArguments(format!("fluxo {}: {e}", arq.display())))
}

/// Os itens do ultimo passo que terminou bem, na ordem em que os passos terminaram.
fn saida_do_ultimo(r: &fluxos::Relatorio) -> Vec<Value> {
    r.passos
        .iter()
        .rev()
        .find(|p| p.estado == "ok" || p.estado == "continuou")
        .map(|p| p.itens.clone())
        .unwrap_or_default()
}

impl Tool for FluxoTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "fluxo".into(),
            description: "Run a saved PhxClaw flow (DAG, JSON file) as a sub-flow and return the \
items produced by its last step as a JSON array. Give 'caminho' (path to the .json) or 'nome' \
(file in the agent's flows folder). 'entrada' are the input items the flow reads as {{entrada}}; \
'modo' 'once' (default) runs the flow once with the whole list, 'each' runs it once per item. \
Every step of the sub-flow goes through the same policy as your own tool calls."
                .into(),
            parameters: json!({"type":"object","properties":{
                "caminho":{"type":"string","description":"path of the flow JSON file"},
                "nome":{"type":"string","description":"name of a flow saved in the agent's flows folder"},
                "entrada":{"type":"array","description":"input items"},
                "modo":{"type":"string","enum":["once","each"]}
            }}),
        }
    }
    fn capability(&self) -> &'static str {
        "flow.run"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let arq = self.arquivo(&args)?;
            let f = ler_fluxo(&arq)?;
            let agente = self.base.get().ok_or_else(|| {
                ToolError::Failed("ferramenta fluxo sem agente montado (defeito de montagem)".into())
            })?;
            let entrada: Vec<Value> = args
                .get("entrada")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let cada = args.get("modo").and_then(Value::as_str) == Some("each");
            if cada && entrada.len() > ITENS_MAX {
                return Err(ToolError::InvalidArguments(format!(
                    "modo each com {} itens passa do teto de {ITENS_MAX} execucoes",
                    entrada.len()
                )));
            }
            let lotes: Vec<Vec<Value>> = if cada {
                entrada.into_iter().map(|i| vec![i]).collect()
            } else {
                vec![entrada]
            };
            let mut itens = Vec::new();
            for lote in lotes {
                let r = fluxos::rodar_com(
                    agente,
                    &f,
                    Execucao {
                        entrada: lote,
                        pai: Some(&ctx.task_id),
                        ..Execucao::default()
                    },
                )
                .await
                .map_err(ToolError::Failed)?;
                if !r.sucesso {
                    let falhos: Vec<String> = r
                        .passos
                        .iter()
                        .filter(|p| p.estado == "falhou")
                        .map(|p| format!("{}: {}", p.id, p.saida))
                        .collect();
                    return Err(ToolError::Failed(format!(
                        "sub-fluxo {} falhou (tarefa {}): {}",
                        f.nome,
                        r.tarefa,
                        falhos.join("; ")
                    )));
                }
                itens.extend(saida_do_ultimo(&r));
            }
            Ok(ToolOutput::text(Value::Array(itens).to_string()))
        })
    }
}

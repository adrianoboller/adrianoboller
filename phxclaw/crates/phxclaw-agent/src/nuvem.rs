//! Tarefas paralelas em ambientes isolados (o "cloud tasks" do Codex, local): N objetivos
//! sobre o mesmo repositorio, cada um num subagente com a PROPRIA worktree git como pasta
//! de trabalho e o proprio sandbox; no fim, cada worktree vira um commit no seu ramo.
//!
//! Nada de motor novo:
//! - a worktree sai do `git_worktree` (`git::WorktreeTool`), o mesmo que o modelo chama;
//! - os subagentes rodam pelo `ferramentas::rodar_filhas`, o laco unico dos subagentes;
//! - o commit e a lista de arquivos saem do `git_write` (`git::GitTool`).
//!
//! O isolamento e o do sandbox de sempre: a pasta de trabalho da filha (`<tarefa>/work`)
//! e um link para a worktree, e o bwrap monta o caminho CANONICO dela em `/work`. A filha
//! nao enxerga o repositorio principal nem a worktree da irma -- e por isso tambem nao
//! roda git (o `.git` da worktree aponta para fora do que ela ve); quem grava o commit e
//! esta ferramenta, na pasta da mae, depois que todas terminam.

use crate::ferramentas::{config_de_subagente, corpo_do_subagente, rodar_filhas};
use crate::git::{GitTool, WorktreeTool};
use crate::motor::{Agent, AgentConfig};
use crate::tarefa::{Task, TaskStatus, TaskStore};
use phxclaw_agent_core::{BoxFut, Llm, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub struct ParallelTasksTool {
    pub llm: Arc<dyn Llm>,
    /// Ferramentas das filhas (sem esta, para nao haver recursao).
    pub tools: Vec<Arc<dyn Tool>>,
    pub config: AgentConfig,
    pub store: TaskStore,
    pub bwrap: PathBuf,
    pub max_parallel: usize,
}

struct Item {
    nome: String,
    /// A tarefa pedida; difere de `nome` nas tentativas de best-of-N (`nome-1`, `nome-2`).
    de: String,
    objetivo: String,
}

/// Teto de tentativas por tarefa no best-of-N: cada uma e um subagente inteiro.
pub const MAX_TENTATIVAS: u64 = 4;

fn itens(args: &Value, max: usize) -> Result<Vec<Item>, ToolError> {
    let v = args
        .get("tasks")
        .and_then(Value::as_array)
        .ok_or_else(|| ToolError::InvalidArguments("falta 'tasks' (lista)".into()))?;
    let mut out: Vec<Item> = Vec::new();
    for x in v {
        let nome = x.get("name").and_then(Value::as_str).unwrap_or("").trim();
        let objetivo = x
            .get("objective")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if nome.is_empty() || objetivo.is_empty() {
            return Err(ToolError::InvalidArguments(
                "cada tarefa precisa de 'name' e 'objective'".into(),
            ));
        }
        let n = x.get("attempts").and_then(Value::as_u64).unwrap_or(1);
        if !(1..=MAX_TENTATIVAS).contains(&n) {
            return Err(ToolError::InvalidArguments(format!(
                "attempts de 1 a {MAX_TENTATIVAS}"
            )));
        }
        for k in 1..=n {
            let nome_k = if n == 1 {
                nome.to_string()
            } else {
                format!("{nome}-{k}")
            };
            if out.iter().any(|i| i.nome == nome_k) {
                return Err(ToolError::InvalidArguments(format!(
                    "nome repetido: {nome_k}"
                )));
            }
            out.push(Item {
                nome: nome_k,
                de: nome.into(),
                objetivo: objetivo.into(),
            });
        }
    }
    // o teto conta TENTATIVAS: cada uma e um subagente e uma worktree
    if out.is_empty() || out.len() > max {
        return Err(ToolError::InvalidArguments(format!(
            "de 1 a {max} tarefas (somando as tentativas)"
        )));
    }
    Ok(out)
}

async fn json_de(t: &dyn Tool, args: Value, ctx: &ToolContext) -> Result<Value, ToolError> {
    let o = t.run(args, ctx).await?;
    serde_json::from_str(&o.content).map_err(|e| ToolError::Failed(format!("git: {e}")))
}

/// A pasta de trabalho da tarefa passa a ser outra (link simbolico): a worktree da filha
/// aqui, o projeto aberto no editor no `acp`.
pub(crate) fn ligar_pasta(
    store: &TaskStore,
    id: &str,
    alvo: &std::path::Path,
) -> Result<(), ToolError> {
    let erro = |e: std::io::Error| ToolError::Failed(format!("pasta da tarefa {id}: {e}"));
    std::fs::create_dir_all(store.dir(id)).map_err(erro)?;
    let alvo = std::fs::canonicalize(alvo).map_err(erro)?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&alvo, store.workdir(id)).map_err(erro)
    }
    #[cfg(not(unix))]
    {
        let _ = alvo;
        Err(ToolError::Failed(
            "tarefas em worktree exigem Linux (bwrap)".into(),
        ))
    }
}

impl Tool for ParallelTasksTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "parallel_tasks".into(),
            description: format!(
                "Run up to {} coding tasks in parallel over a git repo in the working directory. \
Each task gets its own git worktree (branch phxclaw/<name>) as an isolated sandboxed workspace \
and a sub-agent; when all finish, each worktree's changes are committed on its branch. Returns \
per task: answer, branch, commit and changed files. 'attempts' (best-of-N, up to 4) runs the \
same objective N times in separate worktrees (<name>-1..N) so you can compare and keep the best. \
Review/merge the branches afterwards with git/git_write.",
                self.max_parallel
            ),
            parameters: json!({"type":"object","properties":{
                "path":{"type":"string","description":"repo path inside the working directory ('' = root)"},
                "base":{"type":"string","description":"base revision (default HEAD)"},
                "tasks":{"type":"array","items":{"type":"object","properties":{
                    "name":{"type":"string","description":"worktree/branch name: letters, digits, . _ -"},
                    "objective":{"type":"string"},
                    "attempts":{"type":"integer","minimum":1,"maximum":4}},"required":["name","objective"]}}
            },"required":["tasks"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "agent.parallel"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let lista = itens(&args, self.max_parallel)?;
            let repo = args.get("path").and_then(Value::as_str).unwrap_or("");
            let arvore = WorktreeTool {
                bwrap: self.bwrap.clone(),
                timeout: Duration::from_secs(60),
            };
            let git = GitTool::escrita(self.bwrap.clone());
            // Em sequencia: dois `worktree add` juntos disputam o lock do repositorio.
            let mut caminhos = Vec::new();
            for i in &lista {
                let mut a = json!({"action":"add","path":repo,"name":i.nome});
                if let Some(b) = args.get("base").and_then(Value::as_str) {
                    a["base"] = json!(b);
                }
                let v = json_de(&arvore, a, ctx).await?;
                let p = v["path"].as_str().unwrap_or_default().to_string();
                caminhos.push((p, v["ramo"].as_str().unwrap_or_default().to_string()));
            }
            let mut filhas = Vec::new();
            for (i, (p, ramo)) in lista.iter().zip(&caminhos) {
                let mut t = Task::new(
                    format!(
                        "{}\n\n(Your working directory is an isolated checkout of the project on \
branch {ramo}. Edit the files there; do not run git -- your changes are committed for you when \
you finish.)",
                        i.objetivo
                    ),
                    self.llm.id(),
                );
                t.parent = Some(ctx.task_id.clone());
                ligar_pasta(&self.store, &t.id, &ctx.workdir.join(p))?;
                filhas.push(t);
            }
            let sub = Agent::new(
                self.llm.clone(),
                self.tools.clone(),
                config_de_subagente(&self.config),
                self.store.clone(),
            );
            let feitas = rodar_filhas(&sub, filhas).await;
            let mut saida = Vec::new();
            for ((i, (p, ramo)), t) in lista.iter().zip(&caminhos).zip(&feitas) {
                let st = json_de(&git, json!({"action":"add","path":p,"paths":["."]}), ctx).await?;
                let commit = if st["limpo"].as_bool() == Some(true) {
                    Value::Null
                } else {
                    let primeira = i.objetivo.lines().next().unwrap_or("");
                    let msg: String = format!("phxclaw {}: {primeira}", i.nome)
                        .chars()
                        .take(200)
                        .collect();
                    json_de(&git, json!({"action":"commit","path":p,"message":msg}), ctx)
                            .await?["commit"]["commit"]
                            .clone()
                };
                let arquivos: Vec<Value> = st["arquivos"]
                    .as_array()
                    .map(|a| a.iter().map(|f| f["caminho"].clone()).collect())
                    .unwrap_or_default();
                saida.push(json!({
                    "name": i.nome, "de": i.de, "tarefa": t.id,
                    "estado": format!("{:?}", t.status),
                    "concluida": t.status == TaskStatus::Completed,
                    "resposta": corpo_do_subagente(t),
                    "ramo": ramo, "path": p, "commit": commit, "arquivos": arquivos,
                }));
            }
            Ok(ToolOutput::text(json!({ "tarefas": saida }).to_string()))
        })
    }
}

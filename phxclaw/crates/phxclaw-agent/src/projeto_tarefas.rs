//! `project_task`: as tarefas que o PROJETO declara em `.phxclaw/tarefas.json` (o
//! `tasks.json` do VS Code), rodadas no mesmo bwrap do `shell` e pelas mesmas regras de
//! comando. `rust_project` e `python_project` continuam como estao: sao o padrao de quem
//! nao escreveu o arquivo.
//!
//! Um motor so: a ferramenta do agente e a CLI (`phxclaw tarefa listar|rodar`) chamam
//! `carregar` e `rodar` daqui. A linha que as regras conferem (`comando_de_shell`) e a
//! MESMA que o sandbox executa, montada por `linha_de_shell` -- regra sobre `cargo` ou
//! `npm` alcanca a tarefa como alcanca o `shell`.

use crate::python::aspas;
use crate::sistema::caminho_do_projeto;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_sandbox::{WorkdirCommand, WorkdirOutput, run_in_workdir};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Onde o projeto declara as tarefas, debaixo de `.phxclaw/`.
pub const ARQUIVO: &str = "tarefas.json";
pub const GRUPOS: &[&str] = &["build", "test", "run"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tarefa {
    pub nome: String,
    pub comando: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Pasta, relativa a raiz do projeto, onde o comando roda (padrao: a raiz).
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default = "grupo_padrao")]
    pub grupo: String,
}

fn grupo_padrao() -> String {
    "build".into()
}

/// Le `<raiz>/.phxclaw/tarefas.json`. Arquivo ausente e lista vazia (vale o padrao);
/// arquivo ilegivel ou tarefa invalida e ERRO com o motivo, nunca «vale o padrao» calado.
pub fn carregar(raiz: &Path) -> Result<Vec<Tarefa>, String> {
    let p = raiz.join(".phxclaw").join(ARQUIVO);
    let texto = match std::fs::read_to_string(&p) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(format!("{}: {e}", p.display())),
    };
    let lista: Vec<Tarefa> =
        serde_json::from_str(&texto).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut nomes = std::collections::HashSet::new();
    for t in &lista {
        let nome_ok = !t.nome.is_empty()
            && t.nome
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._:-".contains(c));
        if !nome_ok {
            return Err(format!(
                "{}: nome de tarefa invalido {:?} (letras, digitos e . _ : -)",
                p.display(),
                t.nome
            ));
        }
        if !nomes.insert(t.nome.as_str()) {
            return Err(format!("{}: tarefa {:?} repetida", p.display(), t.nome));
        }
        if t.comando.trim().is_empty() {
            return Err(format!("{}: tarefa {:?} sem comando", p.display(), t.nome));
        }
        if !GRUPOS.contains(&t.grupo.as_str()) {
            return Err(format!(
                "{}: tarefa {:?} com grupo {:?} (use {})",
                p.display(),
                t.nome,
                t.grupo,
                GRUPOS.join(", ")
            ));
        }
    }
    Ok(lista)
}

/// A linha que roda e que as regras conferem: comando e argumentos entre aspas simples.
pub fn linha_de_shell(t: &Tarefa) -> String {
    let mut l = aspas(t.comando.trim());
    for a in &t.args {
        l.push(' ');
        l.push_str(&aspas(a));
    }
    l
}

/// Roda a tarefa no sandbox do `shell`, com `/work` = `workdir` e o `cwd` da tarefa
/// confinado a ele.
pub async fn rodar(
    bwrap: &Path,
    workdir: &Path,
    t: &Tarefa,
    timeout: Duration,
) -> Result<WorkdirOutput, ToolError> {
    let rel = caminho_do_projeto(workdir, t.cwd.as_deref().unwrap_or("."))?;
    let guest = if rel.is_empty() {
        "/work".to_string()
    } else {
        format!("/work/{rel}")
    };
    let cmd = WorkdirCommand {
        workdir: workdir.to_path_buf(),
        script: format!("cd '{guest}' && exec {}", linha_de_shell(t)),
        timeout,
        network: false,
        max_output_bytes: 256 * 1024,
    };
    let b = bwrap.to_path_buf();
    tokio::task::spawn_blocking(move || run_in_workdir(&b, &cmd))
        .await
        .map_err(|e| ToolError::Failed(e.to_string()))?
        .map_err(|e| ToolError::Failed(e.to_string()))
}

/// Saida estruturada de uma corrida, a mesma na ferramenta e na CLI.
pub fn resultado(t: &Tarefa, s: &WorkdirOutput) -> Value {
    json!({
        "tarefa": t.nome,
        "grupo": t.grupo,
        "comando": linha_de_shell(t),
        "exit_code": s.exit_code,
        "sucesso": s.exit_code == Some(0),
        "stdout": s.stdout,
        "stderr": s.stderr,
        "truncado": s.truncated,
    })
}

pub struct ProjectTaskTool {
    pub bwrap: PathBuf,
    pub timeout: Duration,
    /// A raiz do projeto do agente (`montagem::raiz_do_projeto`), de onde as regras de
    /// comando leem a linha de verdade da tarefa ANTES de rodar: o portao do motor so tem
    /// os argumentos, nao a pasta da tarefa. A corrida le o `.phxclaw/tarefas.json` da
    /// pasta da tarefa, que e a unica que o sandbox ve.
    pub projeto: Option<PathBuf>,
}

impl ProjectTaskTool {
    fn raiz(ctx: &ToolContext, args: &Value) -> Result<PathBuf, ToolError> {
        let rel = caminho_do_projeto(
            &ctx.workdir,
            args.get("path").and_then(Value::as_str).unwrap_or("."),
        )?;
        Ok(if rel.is_empty() {
            ctx.workdir.clone()
        } else {
            ctx.workdir.join(rel)
        })
    }

    fn tarefa_pedida(raiz: &Path, args: &Value) -> Result<Tarefa, ToolError> {
        let nome = args
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ToolError::InvalidArguments("falta 'name' (veja action=list)".into()))?;
        let lista = carregar(raiz).map_err(ToolError::Failed)?;
        lista.into_iter().find(|t| t.nome == nome).ok_or_else(|| {
            ToolError::InvalidArguments(format!(
                "tarefa {nome:?} nao esta em .phxclaw/{ARQUIVO} (sem arquivo valem rust_project \
                 e python_project)"
            ))
        })
    }
}

impl Tool for ProjectTaskTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "project_task".into(),
            description: "Tasks the project declares in .phxclaw/tarefas.json ([{nome, comando, \
args?, cwd?, grupo: build|test|run}]). action=list shows them; action=run {name} runs one in \
the task sandbox (no network) and returns exit code, stdout and stderr. Without the file, use \
rust_project / python_project. 'path' selects the project folder (default: task root)."
                .into(),
            parameters: json!({"type":"object","properties":{
                "action":{"type":"string","enum":["list","run"]},
                "name":{"type":"string"},
                "path":{"type":"string"}
            },"required":["action"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "shell.exec"
    }
    /// A linha de verdade da tarefa pedida: as regras de comando a conferem antes de rodar.
    /// Tarefa que nao se acha devolve o nome, para a regra ver ao menos o pedido.
    fn comando_de_shell(&self, args: &Value) -> Option<String> {
        if args.get("action").and_then(Value::as_str) != Some("run") {
            return None;
        }
        let nome = args.get("name").and_then(Value::as_str).unwrap_or("");
        let linha = self
            .projeto
            .as_deref()
            .and_then(|raiz| carregar(raiz).ok())
            .and_then(|l| l.into_iter().find(|t| t.nome == nome))
            .map(|t| linha_de_shell(&t))
            .unwrap_or_else(|| format!("project_task {}", aspas(nome)));
        Some(linha)
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let acao = args.get("action").and_then(Value::as_str).unwrap_or("");
            let raiz = Self::raiz(ctx, &args)?;
            match acao {
                "list" => {
                    let lista = carregar(&raiz).map_err(ToolError::Failed)?;
                    let padrao = lista.is_empty();
                    Ok(ToolOutput::text(
                        json!({
                            "arquivo": format!(".phxclaw/{ARQUIVO}"),
                            "tarefas": lista,
                            "padrao": if padrao { json!(["rust_project", "python_project"]) } else { Value::Null },
                        })
                        .to_string(),
                    ))
                }
                "run" => {
                    let t = Self::tarefa_pedida(&raiz, &args)?;
                    let s = rodar(&self.bwrap, &raiz, &t, self.timeout.min(ctx.timeout)).await?;
                    Ok(ToolOutput::text(resultado(&t, &s).to_string()))
                }
                outra => Err(ToolError::InvalidArguments(format!(
                    "action desconhecida: {outra} (list, run)"
                ))),
            }
        })
    }
}

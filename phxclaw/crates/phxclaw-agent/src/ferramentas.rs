//! Ferramentas nativas do motor: shell no sandbox, arquivos da tarefa e subagentes.
//!
//! As ferramentas de navegador, busca e documentos vivem nas proprias crates e entram por
//! adaptadores (ver `adaptadores.rs`); aqui fica so o que depende do motor em si.

use crate::motor::{Agent, AgentConfig, CancelFlag, NoObserver, artifact_for};
use crate::tarefa::{Task, TaskStatus, confine};
use phxclaw_agent_core::{
    BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, Tool, ToolContext, ToolError, ToolOutput,
    ToolSpec, Usage,
};
use phxclaw_sandbox::{WorkdirCommand, run_in_workdir};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn arg_str<'a>(args: &'a Value, nome: &str) -> Result<&'a str, ToolError> {
    args.get(nome)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidArguments(format!("falta o campo texto '{nome}'")))
}

/// Comando de shell na pasta persistente da tarefa, dentro do bwrap, sem rede por padrao.
pub struct ShellTool {
    pub bwrap: PathBuf,
    pub network: bool,
    pub timeout: Duration,
}

impl Tool for ShellTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "shell".into(),
            description: "Run a shell command (sh -c) in the task's isolated working directory \
(/work, persists between calls; python3, coreutils available; no network unless granted). \
Returns exit code, stdout and stderr."
                .into(),
            parameters: json!({"type":"object","properties":{"command":{"type":"string","description":"shell command line"}},"required":["command"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "shell.exec"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let script = arg_str(&args, "command")?.to_string();
            let cmd = WorkdirCommand {
                workdir: ctx.workdir.clone(),
                script,
                timeout: self.timeout.min(ctx.timeout),
                network: self.network,
                max_output_bytes: 64 * 1024,
            };
            let bwrap = self.bwrap.clone();
            let antes = listar(&ctx.workdir);
            let r = tokio::task::spawn_blocking(move || run_in_workdir(&bwrap, &cmd))
                .await
                .map_err(|e| ToolError::Failed(e.to_string()))?
                .map_err(|e| ToolError::Failed(e.to_string()))?;
            // Arquivo que o comando criou ou mudou vira artefato, com hash.
            let depois = listar(&ctx.workdir);
            let artifacts = depois
                .iter()
                .filter(|(p, m)| antes.iter().all(|(q, n)| q != p || n != m))
                .filter_map(|(p, _)| artifact_for(&ctx.workdir, p).ok())
                .collect();
            Ok(ToolOutput {
                content: format!(
                    "exit_code: {}\nstdout:\n{}\nstderr:\n{}{}",
                    r.exit_code
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "sinal".into()),
                    r.stdout,
                    r.stderr,
                    if r.truncated {
                        "\n[saida truncada]"
                    } else {
                        ""
                    }
                ),
                artifacts,
            })
        })
    }
}

/// Arquivos da pasta (caminho relativo, (tamanho, modificacao)) ate 500 entradas.
fn listar(dir: &std::path::Path) -> Vec<(String, (u64, std::time::SystemTime))> {
    let mut v = Vec::new();
    let mut pilha = vec![dir.to_path_buf()];
    while let Some(d) = pilha.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(md) = e.metadata() else { continue };
            if md.is_dir() {
                pilha.push(p);
            } else if let Ok(rel) = p.strip_prefix(dir) {
                v.push((
                    rel.to_string_lossy().into_owned(),
                    (md.len(), md.modified().unwrap_or(std::time::UNIX_EPOCH)),
                ));
            }
            if v.len() >= 500 {
                return v;
            }
        }
    }
    v
}

pub struct WriteFileTool;
impl Tool for WriteFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "write_file".into(),
            description: "Create or overwrite a text file (markdown, html, csv, code...) in the task working directory.".into(),
            parameters: json!({"type":"object","properties":{"path":{"type":"string","description":"relative path, e.g. report.md"},"content":{"type":"string"}},"required":["path","content"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.write"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let rel = arg_str(&args, "path")?;
            let conteudo = arg_str(&args, "content")?;
            let alvo = confine(&ctx.workdir, rel).map_err(ToolError::Denied)?;
            if let Some(p) = alvo.parent() {
                std::fs::create_dir_all(p).map_err(|e| ToolError::Failed(e.to_string()))?;
            }
            std::fs::write(&alvo, conteudo).map_err(|e| ToolError::Failed(e.to_string()))?;
            let a =
                artifact_for(&ctx.workdir, rel).map_err(|e| ToolError::Failed(e.to_string()))?;
            Ok(ToolOutput {
                content: format!("gravado {rel} ({} bytes)", a.bytes),
                artifacts: vec![a],
            })
        })
    }
}

pub struct ReadFileTool;
impl Tool for ReadFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_file".into(),
            description:
                "Read a text file from the task working directory. With start_line/end_line \
(1-based, inclusive) returns only that range, each line prefixed by its number."
                    .into(),
            parameters: json!({"type":"object","properties":{"path":{"type":"string"},"start_line":{"type":"integer"},"end_line":{"type":"integer"}},"required":["path"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let rel = arg_str(&args, "path")?;
            let alvo = confine(&ctx.workdir, rel).map_err(ToolError::Denied)?;
            let b = std::fs::read(&alvo).map_err(|e| ToolError::Failed(e.to_string()))?;
            let texto = String::from_utf8_lossy(&b).into_owned();
            let ini = args.get("start_line").and_then(Value::as_u64);
            let fim = args.get("end_line").and_then(Value::as_u64);
            if ini.is_none() && fim.is_none() {
                return Ok(ToolOutput::text(texto));
            }
            // Faixa numerada: e o que deixa o modelo citar a linha certa no edit_file
            // sem reler o arquivo inteiro a cada passo.
            let total = texto.lines().count() as u64;
            let ini = ini.unwrap_or(1).max(1);
            let fim = fim.unwrap_or(total).min(total);
            if ini > fim {
                return Err(ToolError::InvalidArguments(format!(
                    "faixa vazia: {ini}..{fim} (o arquivo tem {total} linhas)"
                )));
            }
            let faixa: Vec<String> = texto
                .lines()
                .enumerate()
                .skip(ini as usize - 1)
                .take((fim - ini + 1) as usize)
                .map(|(i, l)| format!("{:>5}\t{l}", i + 1))
                .collect();
            Ok(ToolOutput::text(faixa.join("\n")))
        })
    }
}

/// Edicao precisa: troca UM trecho exato por outro. Reescrever o arquivo inteiro para
/// mudar uma linha e onde o modelo pequeno perde o resto do conteudo; aqui o resto nao
/// passa pela mao dele.
pub struct EditFileTool;
impl Tool for EditFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "edit_file".into(),
            description:
                "Replace one exact occurrence of old_text with new_text in a file of the task \
directory. old_text must appear exactly once (include surrounding lines to make it unique). \
Empty old_text appends new_text at the end of the file."
                    .into(),
            parameters: json!({"type":"object","properties":{"path":{"type":"string"},"old_text":{"type":"string"},"new_text":{"type":"string"}},"required":["path","old_text","new_text"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.write"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let rel = arg_str(&args, "path")?;
            let velho = arg_str(&args, "old_text")?;
            let novo = arg_str(&args, "new_text")?;
            let alvo = confine(&ctx.workdir, rel).map_err(ToolError::Denied)?;
            let atual =
                std::fs::read_to_string(&alvo).map_err(|e| ToolError::Failed(e.to_string()))?;
            let resultado = if velho.is_empty() {
                format!("{atual}{novo}")
            } else {
                // Ambiguidade recusa em vez de adivinhar: trocar a ocorrencia errada
                // e um defeito que ninguem ve no resumo do passo.
                match atual.matches(velho).count() {
                    1 => atual.replacen(velho, novo, 1),
                    0 => {
                        return Err(ToolError::InvalidArguments(format!(
                            "old_text nao aparece em {rel}; leia o arquivo e copie o trecho exato"
                        )));
                    }
                    n => {
                        return Err(ToolError::InvalidArguments(format!(
                            "old_text aparece {n} vezes em {rel}; inclua linhas vizinhas para ficar unico"
                        )));
                    }
                }
            };
            std::fs::write(&alvo, &resultado).map_err(|e| ToolError::Failed(e.to_string()))?;
            let a =
                artifact_for(&ctx.workdir, rel).map_err(|e| ToolError::Failed(e.to_string()))?;
            // Linha onde o trecho comecava: posicao no texto ANTIGO, nao busca do novo
            // (o novo pode existir antes, e a conta de linhas() perde o fim de linha).
            let pos = if velho.is_empty() {
                atual.len()
            } else {
                atual.find(velho).unwrap_or(0)
            };
            let linha = atual[..pos].matches('\n').count() + 1;
            Ok(ToolOutput {
                content: format!("editado {rel} perto da linha {linha} ({} bytes)", a.bytes),
                artifacts: vec![a],
            })
        })
    }
}

pub struct ListFilesTool;
impl Tool for ListFilesTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "list_files".into(),
            description: "List files in the task working directory with sizes.".into(),
            parameters: json!({"type":"object","properties":{}}),
        }
    }
    fn capability(&self) -> &'static str {
        "fs.read"
    }
    fn run<'a>(
        &'a self,
        _args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let mut l = listar(&ctx.workdir);
            l.sort();
            let t: Vec<String> = l
                .iter()
                .map(|(p, (n, _))| format!("{p}\t{n} bytes"))
                .collect();
            Ok(ToolOutput::text(if t.is_empty() {
                "(vazio)".into()
            } else {
                t.join("\n")
            }))
        })
    }
}

/// Pesquisa em paralelo: cada item vira um subagente com as mesmas ferramentas (menos
/// esta, para nao haver recursao), rodando ao mesmo tempo. E a "pesquisa ampla" do Manus.
pub struct ParallelAgentsTool {
    pub llm: Arc<dyn Llm>,
    pub tools: Vec<Arc<dyn Tool>>,
    pub config: AgentConfig,
    pub store: crate::tarefa::TaskStore,
    pub max_parallel: usize,
}

impl Tool for ParallelAgentsTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "parallel_research".into(),
            description: format!(
                "Run up to {} independent sub-agents in parallel, one per sub-task, and return each \
answer. Use for research over many items (compare products, gather facts about several topics).",
                self.max_parallel
            ),
            parameters: json!({"type":"object","properties":{"subtasks":{"type":"array","items":{"type":"string"},"description":"one self-contained instruction per sub-agent"}},"required":["subtasks"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "agent.spawn"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let itens: Vec<String> = args
                .get("subtasks")
                .and_then(Value::as_array)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'subtasks' (lista)".into()))?
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            if itens.is_empty() || itens.len() > self.max_parallel {
                return Err(ToolError::InvalidArguments(format!(
                    "de 1 a {} subtarefas",
                    self.max_parallel
                )));
            }
            let sub = Agent::new(
                self.llm.clone(),
                self.tools.clone(),
                AgentConfig {
                    max_steps: self.config.max_steps.min(8),
                    ..self.config.clone()
                },
                self.store.clone(),
            );
            let futuros = itens.iter().map(|obj| {
                let mut t = Task::new(obj.clone(), sub.llm.id());
                t.parent = Some(ctx.task_id.clone());
                let sub = sub.clone();
                async move {
                    let cancel = CancelFlag::default();
                    sub.run(t, &cancel, &NoObserver).await
                }
            });
            let feitos = futures_util::future::join_all(futuros).await;
            let mut texto = String::new();
            for (i, t) in feitos.iter().enumerate() {
                let corpo = match t.status {
                    TaskStatus::Completed => t.answer.clone().unwrap_or_default(),
                    s => format!("[{s:?}] {}", t.error.clone().unwrap_or_default()),
                };
                texto.push_str(&format!(
                    "### {}. {}\n(subtarefa {})\n{}\n\n",
                    i + 1,
                    itens[i],
                    t.id,
                    corpo
                ));
            }
            Ok(ToolOutput::text(texto))
        })
    }
}

/// LLM roteirizado: devolve respostas prontas em ordem. Para testes deterministicos do motor
/// e para demonstracao sem modelo; registra o que recebeu para o teste conferir.
pub struct ScriptedLlm {
    pub replies: Mutex<Vec<LlmReply>>,
    pub seen: Mutex<Vec<(Vec<Message>, Vec<String>)>>,
}

impl ScriptedLlm {
    pub fn new(mut replies: Vec<LlmReply>) -> Self {
        replies.reverse();
        Self {
            replies: Mutex::new(replies),
            seen: Mutex::new(vec![]),
        }
    }
    pub fn text(t: &str) -> LlmReply {
        LlmReply {
            content: t.into(),
            tool_calls: vec![],
            usage: Usage {
                input_tokens: 10,
                output_tokens: 5,
            },
            model: "roteiro".into(),
        }
    }
    pub fn call(id: &str, name: &str, args: Value) -> LlmReply {
        LlmReply {
            content: String::new(),
            tool_calls: vec![phxclaw_agent_core::ToolCall {
                id: id.into(),
                name: name.into(),
                arguments: args,
            }],
            usage: Usage {
                input_tokens: 10,
                output_tokens: 5,
            },
            model: "roteiro".into(),
        }
    }
}

impl Llm for ScriptedLlm {
    fn id(&self) -> String {
        "roteiro".into()
    }
    fn chat<'a>(
        &'a self,
        messages: &'a [Message],
        tools: &'a [ToolSpec],
        _options: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async move {
            self.seen.lock().unwrap().push((
                messages.to_vec(),
                tools.iter().map(|t| t.name.clone()).collect(),
            ));
            self.replies
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| LlmError::Parse("roteiro acabou".into()))
        })
    }
}

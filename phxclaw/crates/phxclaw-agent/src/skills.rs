//! Skills: instrucoes guardadas numa pasta (`<pasta>/<nome>/SKILL.md`) que o modelo carrega
//! quando precisa. O prompt leva so nome e descricao de cada uma; o corpo entra pela
//! ferramenta `skill_load`, para dez skills nao custarem dez corpos em toda tarefa.
//!
//! Ler e validar e do `phxclaw-skill-runtime` (`SkillFolder`): o que o prompt lista e o que
//! a ferramenta carrega saem do mesmo leitor, entao skill que nao carrega nao aparece.

use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_skill_runtime::SkillFolder;
use serde_json::{Value, json};
use std::path::Path;

/// Teto de skills listadas no prompt: a lista vai em toda tarefa.
pub const SKILLS_NO_PROMPT: usize = 50;

/// `PHXCLAW_SKILLS_DIR`, ou `<raiz do TaskStore>/_skills`. O `_` tira a pasta da lista de
/// tarefas, do mesmo jeito que a da memoria.
pub fn pasta_do_ambiente(raiz: &Path) -> SkillFolder {
    match crate::config::texto_de("agente.skills_dir") {
        Some(p) => SkillFolder::new(p.trim()),
        None => SkillFolder::new(raiz.join("_skills")),
    }
}

/// O bloco do prompt de sistema: nome e descricao, nada do corpo. Pasta vazia ou ausente
/// nao acrescenta nada.
pub fn bloco_para_o_prompt(pasta: &SkillFolder) -> Option<String> {
    let scan = pasta.scan();
    if scan.skills.is_empty() {
        return None;
    }
    let mut s = String::from(
        "Available skills (instructions saved for recurring jobs). When one fits the objective, \
call skill_load with its name BEFORE starting, then follow it:\n",
    );
    for k in scan.skills.iter().take(SKILLS_NO_PROMPT) {
        s.push_str(&format!("- {}: {}\n", k.name, k.description));
    }
    if scan.skills.len() > SKILLS_NO_PROMPT {
        s.push_str(&format!(
            "(+{} more not listed)\n",
            scan.skills.len() - SKILLS_NO_PROMPT
        ));
    }
    Some(s)
}

pub struct SkillLoadTool {
    pub pasta: SkillFolder,
}

impl Tool for SkillLoadTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "skill_load".into(),
            description:
                "Load the full instructions of a skill listed in the system prompt, by name.".into(),
            parameters: json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "skill.read"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let nome = args
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'name'".into()))?
                .trim();
            use phxclaw_skill_runtime::SkillError;
            let doc = self.pasta.load(nome).map_err(|e| match e {
                // Nome com caminho e fuga por symlink sao recusa de politica, nao erro de
                // uso: o modelo nao deve tentar de novo com outra grafia do mesmo caminho.
                SkillError::InvalidName(_) | SkillError::PathEscape(_) => {
                    ToolError::Denied(e.to_string())
                }
                SkillError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
                    ToolError::InvalidArguments(format!("skill inexistente: {nome}"))
                }
                outro => ToolError::Failed(outro.to_string()),
            })?;
            Ok(ToolOutput::text(format!(
                "# skill {}\n{}\n\n{}",
                doc.name, doc.description, doc.body
            )))
        })
    }
}

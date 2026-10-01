//! Ponto de restauracao antes de cada escrita do agente, e as ferramentas que listam e
//! restauram.
//!
//! O ponto nasce no portao do motor (`Agent::call_tool`), e nao dentro de cada ferramenta:
//! as que escrevem sao muitas (`write_file`, `edit_file`, `notebook_edit`, os conversores,
//! `git_write`), e a que alguem esquecesse de chamar seria a escrita sem volta. O portao ve
//! a capacidade antes de rodar, e a lista do que escreve e UMA (`CAPACIDADES_QUE_ESCREVEM`).
//!
//! Os pontos moram na pasta da TAREFA (`<tarefa>/checkpoints`), fora da pasta de trabalho:
//! o sandbox monta so a pasta de trabalho em /work, entao o comando do modelo nao alcanca
//! os pontos para adultera-los.

use crate::tarefa::TaskStore;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_checkpoint::{CheckpointError, CheckpointManager};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Capacidades cujas ferramentas mudam arquivos da pasta. `shell.exec` fica de fora de
/// proposito: todo comando seria um ponto, inclusive `ls`, e o custo de varrer a pasta
/// passaria a ser o de cada passo do agente.
pub const CAPACIDADES_QUE_ESCREVEM: &[&str] = &["fs.write", "doc.write", "git.write"];

pub fn escreve(capacidade: &str) -> bool {
    CAPACIDADES_QUE_ESCREVEM.contains(&capacidade)
}

pub fn pasta_de_pontos(store: &TaskStore, task_id: &str) -> PathBuf {
    store.dir(task_id).join("checkpoints")
}

/// Tira o ponto antes da escrita. Pasta igual ao ultimo ponto nao gera outro: o modelo que
/// escreve tres vezes seguidas o mesmo arquivo sem mudar nada nao enche a lista de pontos
/// identicos. Devolve o id do ponto novo (ou `None` se nao precisou).
pub fn antes_de_escrever(
    pontos: &Path,
    workdir: &Path,
    motivo: &str,
) -> Result<Option<String>, CheckpointError> {
    let m = CheckpointManager::new(pontos);
    let (_, agora) = m.scan(workdir)?;
    // So o ultimo manifesto: ler todos a cada escrita faria o custo crescer com os pontos.
    if let Some(ultimo) = m.ultimo()
        && ultimo.files == agora
    {
        return Ok(None);
    }
    let p = m.create_labeled(workdir, Some(motivo.to_string()))?;
    Ok(Some(p.uuid.to_string()))
}

/// O que o portao chama: falha ao tirar o ponto NAO impede a escrita (pasta acima do teto
/// travaria todo `write_file`), mas aparece no stderr dizendo qual tarefa ficou sem ponto.
pub fn no_portao(store: &TaskStore, ctx: &ToolContext, ferramenta: &str) {
    let pontos = pasta_de_pontos(store, &ctx.task_id);
    if let Err(e) = antes_de_escrever(&pontos, &ctx.workdir, &format!("antes de {ferramenta}")) {
        eprintln!(
            "aviso: tarefa {} sem ponto de restauracao antes de {ferramenta}: {e}",
            ctx.task_id
        );
    }
}

/// `checkpoint_list` (fs.read) e `checkpoint_restore` (fs.write): a mesma ferramenta
/// registrada duas vezes, uma por capacidade, como as de sistema.
pub struct CheckpointTool {
    pub store: TaskStore,
    pub restaurar: bool,
}

impl Tool for CheckpointTool {
    fn spec(&self) -> ToolSpec {
        if self.restaurar {
            ToolSpec {
                name: "checkpoint_restore".into(),
                description: "Restore the task working directory to a checkpoint (one is taken \
automatically before every file write). Give 'id' from checkpoint_list. Files created after \
the checkpoint are kept unless remove_new=true. The restore itself is checkpointed, so it can \
be undone."
                    .into(),
                parameters: json!({"type":"object","properties":{
                    "id":{"type":"string"},
                    "remove_new":{"type":"boolean"}
                },"required":["id"]}),
            }
        } else {
            ToolSpec {
                name: "checkpoint_list".into(),
                description: "List the restore points of the task working directory (taken \
before each file write), newest last, with what changed since each one."
                    .into(),
                parameters: json!({"type":"object","properties":{}}),
            }
        }
    }
    fn capability(&self) -> &'static str {
        if self.restaurar {
            "fs.write"
        } else {
            "fs.read"
        }
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let m = CheckpointManager::new(pasta_de_pontos(&self.store, &ctx.task_id));
            let falha = |e: CheckpointError| ToolError::Failed(e.to_string());
            if !self.restaurar {
                let mut l = Vec::new();
                let (pontos, ilegiveis) = m.list_relatando();
                for p in pontos {
                    let v = m.verify(&p).map_err(falha)?;
                    l.push(json!({
                        "id": p.uuid.to_string(),
                        "quando": p.created_at.to_rfc3339(),
                        "motivo": p.label,
                        "arquivos": p.files.len(),
                        "mudaram_desde": v.changed,
                        "sumiram_desde": v.missing,
                        "nasceram_desde": v.added,
                    }));
                }
                let ilegiveis: Vec<Value> = ilegiveis
                    .into_iter()
                    .map(|(id, motivo)| json!({"id": id, "motivo": motivo}))
                    .collect();
                return Ok(ToolOutput::text(
                    json!({"pontos": l, "ilegiveis": ilegiveis}).to_string(),
                ));
            }
            let id = args
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'id'".into()))?;
            let uuid = id
                .trim()
                .parse()
                .map_err(|_| ToolError::InvalidArguments(format!("id invalido: {id}")))?;
            let p = m.load(uuid).map_err(|_| {
                ToolError::InvalidArguments(format!(
                    "nao ha ponto {id} nesta tarefa; veja checkpoint_list"
                ))
            })?;
            // O ponto guarda o caminho de quando nasceu; restaurar so vale na pasta desta
            // tarefa, nunca num caminho que o manifesto diga.
            let aqui = std::fs::canonicalize(&ctx.workdir)
                .map_err(|e| ToolError::Failed(e.to_string()))?;
            if p.workspace != aqui {
                return Err(ToolError::Denied(format!(
                    "o ponto {id} e de outra pasta ({})",
                    p.workspace.display()
                )));
            }
            let remover = args
                .get("remove_new")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let r = m.restore(&p, remover).map_err(falha)?;
            Ok(ToolOutput::text(
                json!({
                    "ponto": id,
                    "restaurados": r.restored,
                    "removidos": r.removed,
                    "novos_mantidos": r.kept_new,
                })
                .to_string(),
            ))
        })
    }
}

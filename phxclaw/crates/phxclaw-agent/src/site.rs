//! Publicar site (o "Web Builder" do Manus, local): o agente escreve HTML/CSS/JS na pasta da
//! tarefa e publica uma subpasta; a API a serve em /sites/<tarefa>/<pasta>/.
//!
//! A marca de publicacao (`sites.json`) fica na pasta da TAREFA, fora de `work/`: o shell do
//! agente so enxerga `work/`, entao nao consegue forjar uma publicacao. E o site e servido
//! com `CSP: sandbox` -- origem opaca --, para o JavaScript publicado nao alcancar a API.

use crate::tarefa::confine;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};
use std::path::Path;

pub struct PublishSiteTool {
    /// Base publica da API, ex.: http://127.0.0.1:8787
    pub base_url: String,
}

/// Pastas publicadas da tarefa.
pub fn published(task_dir: &Path) -> Vec<String> {
    std::fs::read(task_dir.join("sites.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

impl Tool for PublishSiteTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "publish_site".into(),
            description: "Publish a folder of the task directory that contains index.html as a website and get its URL. \
Write the files first (write_file), e.g. site/index.html and site/style.css."
                .into(),
            parameters: json!({"type":"object","properties":{"folder":{"type":"string","description":"relative folder containing index.html"}},"required":["folder"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "site.publish"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let pasta = args
                .get("folder")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'folder'".into()))?
                .trim_matches('/')
                .to_string();
            let dir = confine(&ctx.workdir, &pasta).map_err(ToolError::Denied)?;
            if !dir.join("index.html").is_file() {
                return Err(ToolError::InvalidArguments(format!(
                    "{pasta}/index.html nao existe"
                )));
            }
            let tarefa = ctx
                .workdir
                .parent()
                .ok_or_else(|| ToolError::Failed("pasta da tarefa".into()))?;
            let mut l = published(tarefa);
            if !l.contains(&pasta) {
                l.push(pasta.clone());
            }
            std::fs::write(
                tarefa.join("sites.json"),
                serde_json::to_vec(&l).unwrap_or_default(),
            )
            .map_err(|e| ToolError::Failed(e.to_string()))?;
            Ok(ToolOutput::text(format!(
                "publicado: {}/sites/{}/{}/index.html",
                self.base_url.trim_end_matches('/'),
                ctx.task_id,
                pasta
            )))
        })
    }
}

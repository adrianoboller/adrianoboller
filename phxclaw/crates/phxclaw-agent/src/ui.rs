//! Designer de telas ERP como ferramenta do agente: SQL -> UI-IR -> HTML.
//!
//! O modelo so escolhe a entrada (DDL no argumento ou num arquivo da tarefa); a analise e o
//! desenho sao deterministicos, em `phxclaw-ui-ir`. Assim o mesmo SQL da a mesma tela, e o
//! `ui-ir.json` gravado ao lado e o contrato que os outros renderizadores vao ler.

use crate::motor::artifact_for;
use crate::tarefa::confine;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};

pub struct DesignErpUiTool;

impl Tool for DesignErpUiTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "design_erp_ui".into(),
            description:
                "Generate ERP screens (list, form, master-detail with totals, lookups) from SQL \
CREATE TABLE statements. Give the DDL in 'sql' or a file of the task in 'sql_path'. Writes \
<folder>/ui-ir.json and <folder>/index.html; publish the folder with publish_site to show it."
                    .into(),
            parameters: json!({"type":"object","properties":{
                "sql":{"type":"string"},
                "sql_path":{"type":"string"},
                "app_name":{"type":"string"},
                "folder":{"type":"string","description":"output folder, default erp"},
                "react":{"type":"boolean","description":"also write a React project in <folder>/react (npm install && npm run build)"}
            }}),
        }
    }
    fn capability(&self) -> &'static str {
        "doc.write"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let texto = |k: &str| args.get(k).and_then(Value::as_str).map(str::trim);
            let sql = match (texto("sql"), texto("sql_path")) {
                (Some(s), _) if !s.is_empty() => s.to_string(),
                (_, Some(p)) if !p.is_empty() => {
                    let arq = confine(&ctx.workdir, p).map_err(ToolError::Denied)?;
                    std::fs::read_to_string(arq).map_err(|e| ToolError::Failed(e.to_string()))?
                }
                _ => {
                    return Err(ToolError::InvalidArguments(
                        "informe 'sql' (CREATE TABLE ...) ou 'sql_path'".into(),
                    ));
                }
            };
            let nome = texto("app_name")
                .filter(|s| !s.is_empty())
                .unwrap_or("Sistema");
            let pasta = texto("folder")
                .filter(|s| !s.is_empty())
                .unwrap_or("erp")
                .trim_matches('/')
                .to_string();
            let (app, avisos) = phxclaw_ui_ir::from_sql(nome, &sql);
            if app.entities.is_empty() {
                // Tela vazia publicada como sucesso seria a pior resposta: diz por que.
                return Err(ToolError::InvalidArguments(format!(
                    "nenhuma tabela reconhecida no SQL; avisos: {}",
                    if avisos.is_empty() {
                        "nenhum".into()
                    } else {
                        avisos.join("; ")
                    }
                )));
            }
            let dir = confine(&ctx.workdir, &pasta).map_err(ToolError::Denied)?;
            std::fs::create_dir_all(&dir).map_err(|e| ToolError::Failed(e.to_string()))?;
            let ir =
                serde_json::to_vec_pretty(&app).map_err(|e| ToolError::Failed(e.to_string()))?;
            std::fs::write(dir.join("ui-ir.json"), ir)
                .map_err(|e| ToolError::Failed(e.to_string()))?;
            std::fs::write(dir.join("index.html"), phxclaw_ui_ir::html::render(&app))
                .map_err(|e| ToolError::Failed(e.to_string()))?;
            let mut artefatos = vec![];
            let react = args.get("react").and_then(Value::as_bool) == Some(true);
            if react {
                for (p, c) in phxclaw_ui_ir::react::render(&app) {
                    let rel = format!("{pasta}/react/{p}");
                    let alvo = confine(&ctx.workdir, &rel).map_err(ToolError::Denied)?;
                    if let Some(d) = alvo.parent() {
                        std::fs::create_dir_all(d).map_err(|e| ToolError::Failed(e.to_string()))?;
                    }
                    std::fs::write(&alvo, c).map_err(|e| ToolError::Failed(e.to_string()))?;
                    artefatos.push(
                        artifact_for(&ctx.workdir, &rel)
                            .map_err(|e| ToolError::Failed(e.to_string()))?,
                    );
                }
            }
            for f in ["ui-ir.json", "index.html"] {
                artefatos.push(
                    artifact_for(&ctx.workdir, &format!("{pasta}/{f}"))
                        .map_err(|e| ToolError::Failed(e.to_string()))?,
                );
            }
            let telas: Vec<String> = app.screens.iter().map(|s| s.id().to_string()).collect();
            let mut r = format!(
                "{} entidades, {} telas ({}) gravadas em {pasta}/index.html e {pasta}/ui-ir.json",
                app.entities.len(),
                telas.len(),
                telas.join(", ")
            );
            if react {
                r.push_str(&format!(
                    "\nprojeto React em {pasta}/react (npm install && npm run build)"
                ));
            }
            if !avisos.is_empty() {
                r.push_str(&format!("\navisos: {}", avisos.join("; ")));
            }
            Ok(ToolOutput {
                content: r,
                artifacts: artefatos,
            })
        })
    }
}

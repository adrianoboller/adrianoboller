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
                "react":{"type":"boolean","description":"also write a React project in <folder>/react (npm install && npm run build)"},
                "flutter":{"type":"boolean","description":"also write a Flutter project in <folder>/flutter"},
                "rust":{"type":"boolean","description":"also write the business rules as a Rust crate in <folder>/rust (cargo test)"},
                "wlanguage":{"type":"boolean","description":"also write the same business rules as WLanguage procedures in <folder>/wlanguage (for WinDev/WebDev)"}
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
            gravar_telas(nome, &sql, &pasta, &args, ctx)
        })
    }
}

/// SQL -> UI-IR -> arquivos: o motor unico das ferramentas de tela. Quem chega por SQL
/// (`design_erp_ui`) e quem chega por print (`screenshot_to_erp_ui`) grava por aqui, com
/// as mesmas saidas opcionais e a mesma recusa de tela vazia.
pub fn gravar_telas(
    nome: &str,
    sql: &str,
    pasta: &str,
    args: &Value,
    ctx: &ToolContext,
) -> Result<ToolOutput, ToolError> {
    let (app, avisos) = phxclaw_ui_ir::from_sql(nome, sql);
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
    let dir = confine(&ctx.workdir, pasta).map_err(ToolError::Denied)?;
    std::fs::create_dir_all(&dir).map_err(|e| ToolError::Failed(e.to_string()))?;
    let ir = serde_json::to_vec_pretty(&app).map_err(|e| ToolError::Failed(e.to_string()))?;
    std::fs::write(dir.join("ui-ir.json"), ir).map_err(|e| ToolError::Failed(e.to_string()))?;
    std::fs::write(dir.join("index.html"), phxclaw_ui_ir::html::render(&app))
        .map_err(|e| ToolError::Failed(e.to_string()))?;
    let mut artefatos = vec![];
    let react = args.get("react").and_then(Value::as_bool) == Some(true);
    let quer = |k: &str| args.get(k).and_then(Value::as_bool) == Some(true);
    let mut extras = vec![];
    if react {
        extras.push(("react", phxclaw_ui_ir::react::render(&app)));
    }
    if quer("rust") {
        extras.push(("rust", phxclaw_ui_ir::rust::render(&app)));
    }
    if quer("wlanguage") {
        extras.push(("wlanguage", phxclaw_ui_ir::wlanguage::render(&app)));
    }
    for (sub, lista) in extras {
        for (p, c) in lista {
            let rel = format!("{pasta}/{sub}/{p}");
            let alvo = confine(&ctx.workdir, &rel).map_err(ToolError::Denied)?;
            if let Some(d) = alvo.parent() {
                std::fs::create_dir_all(d).map_err(|e| ToolError::Failed(e.to_string()))?;
            }
            std::fs::write(&alvo, c).map_err(|e| ToolError::Failed(e.to_string()))?;
            artefatos.push(
                artifact_for(&ctx.workdir, &rel).map_err(|e| ToolError::Failed(e.to_string()))?,
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
    if quer("rust") {
        r.push_str(&format!("\nregras em Rust em {pasta}/rust (cargo test)"));
    }
    if quer("wlanguage") {
        r.push_str(&format!(
            "\nregras em WLanguage em {pasta}/wlanguage (nao compiladas aqui: conferir no WinDev)"
        ));
    }
    if !avisos.is_empty() {
        r.push_str(&format!("\navisos: {}", avisos.join("; ")));
    }
    Ok(ToolOutput {
        content: r,
        artifacts: artefatos,
    })
}

/// Print de tela -> telas ERP. A borda le a imagem (OCR com tesseract e tres perguntas a um
/// modelo de visao local); o nucleo deterministico (`phxclaw_ui_ir::imagem`) confirma cada
/// rotulo contra o OCR e escreve o SQL. O que o modelo inventar nao passa: rotulo que nao
/// esta escrito na tela e descartado.
pub struct ScreenshotToErpUiTool;

/// Perguntas medidas em 30/09 (qwen2.5vl:3b): campos 8/8 no cadastro; a das listas achou
/// a "Situação" que a primeira perdeu; a da grade so acerta sem formato JSON forcado.
const PERGUNTA_CAMPOS: &str = "This is a screenshot of a business application form. List the LABELS of the data entry fields (text boxes, dropdowns, checkboxes, date and money fields) in the main form area, exactly as written, including the * mark when present. Do not include the side menu, screen title, section titles, buttons, placeholders or values. Answer only a JSON array of strings.";
const PERGUNTA_LISTAS: &str = "List the labels of the dropdown (select) fields in the main form area, exactly as written. Answer only a JSON array of strings.";
const PERGUNTA_GRADE: &str = "What are the column headers of the items table in this screenshot? Answer only a JSON array of strings.";

async fn ocr(png: &std::path::Path, prazo: std::time::Duration) -> Result<Vec<String>, ToolError> {
    // psm 11 (texto esparso): rotulo de tela nao e paragrafo
    Ok(crate::visao::tesseract(png, "por+eng", Some(11), prazo)
        .await?
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect())
}

impl Tool for ScreenshotToErpUiTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "screenshot_to_erp_ui".into(),
            description:
                "Rebuild ERP screens from a SCREENSHOT (png/jpg in the task folder) of an existing \
system: reads the field labels (OCR + local vision model), writes <folder>/tela.sql and the same \
outputs as design_erp_ui. Labels not actually written on the screenshot are discarded."
                    .into(),
            parameters: json!({"type":"object","properties":{
                "image":{"type":"string","description":"image file in the task folder"},
                "table":{"type":"string","description":"table/entity name, e.g. pedido"},
                "app_name":{"type":"string"},
                "folder":{"type":"string","description":"output folder, default erp"},
                "flutter":{"type":"boolean"},
                "react":{"type":"boolean"},
                "rust":{"type":"boolean"},
                "wlanguage":{"type":"boolean"}
            },"required":["image","table"]}),
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
            use phxclaw_ui_ir::imagem;
            let texto = |k: &str| {
                args.get(k)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            };
            let img = texto("image")
                .ok_or_else(|| ToolError::InvalidArguments("falta 'image'".into()))?;
            let tabela = texto("table")
                .ok_or_else(|| ToolError::InvalidArguments("falta 'table'".into()))?;
            let nome = texto("app_name").unwrap_or("Sistema");
            let pasta = texto("folder")
                .unwrap_or("erp")
                .trim_matches('/')
                .to_string();
            let caminho = confine(&ctx.workdir, img).map_err(ToolError::Denied)?;
            let bytes = std::fs::read(&caminho).map_err(|e| ToolError::Failed(e.to_string()))?;

            let linhas = ocr(&caminho, ctx.timeout).await?;
            let modelo =
                std::env::var("PHXCLAW_MODELO_VISAO").unwrap_or_else(|_| "qwen2.5vl:3b".into());
            let base =
                std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://127.0.0.1:11434".into());
            let base = if base.starts_with("http") {
                base
            } else {
                format!("http://{base}")
            };
            let visao = phxclaw_llm::OllamaLlm::with_timeout(&base, &modelo, ctx.timeout)
                .map_err(|e| ToolError::Failed(e.to_string()))?;
            let mut respostas = vec![];
            for p in [PERGUNTA_CAMPOS, PERGUNTA_LISTAS, PERGUNTA_GRADE] {
                let r = visao
                    .perguntar_com_imagem(p, &bytes)
                    .await
                    .map_err(|e| ToolError::Failed(format!("modelo de visao {modelo}: {e}")))?;
                respostas.push(r);
            }
            let itens = imagem::confirmar(&imagem::lista_da_resposta(&respostas[2]), &[], &linhas);
            let campos = imagem::confirmar(
                &imagem::lista_da_resposta(&respostas[0]),
                &imagem::lista_da_resposta(&respostas[1]),
                &linhas,
            );
            let campos = imagem::sem_itens(&campos, &itens);
            let dir = confine(&ctx.workdir, &pasta).map_err(ToolError::Denied)?;
            std::fs::create_dir_all(&dir).map_err(|e| ToolError::Failed(e.to_string()))?;
            // a trilha da leitura: o que o OCR viu, o que o modelo disse, o que ficou
            let leitura = json!({
                "imagem": img, "modelo": modelo, "ocr": linhas, "respostas": respostas,
                "campos": campos.iter().map(|r| json!({"rotulo": r.texto, "obrigatorio": r.obrigatorio, "lista": r.lista})).collect::<Vec<_>>(),
                "itens": itens.iter().map(|r| r.texto.clone()).collect::<Vec<_>>(),
            });
            std::fs::write(
                dir.join("leitura.json"),
                serde_json::to_vec_pretty(&leitura).unwrap_or_default(),
            )
            .map_err(|e| ToolError::Failed(e.to_string()))?;
            if campos.is_empty() {
                return Err(ToolError::Failed(format!(
                    "nenhum rotulo confirmado na tela (o OCR leu {} linhas; o modelo disse {:?}); veja {pasta}/leitura.json",
                    linhas.len(),
                    respostas[0].chars().take(200).collect::<String>()
                )));
            }
            let sql = imagem::sql(tabela, &campos, &itens);
            std::fs::write(dir.join("tela.sql"), &sql)
                .map_err(|e| ToolError::Failed(e.to_string()))?;
            let mut saida = gravar_telas(nome, &sql, &pasta, &args, ctx)?;
            for f in ["tela.sql", "leitura.json"] {
                saida.artifacts.push(
                    artifact_for(&ctx.workdir, &format!("{pasta}/{f}"))
                        .map_err(|e| ToolError::Failed(e.to_string()))?,
                );
            }
            saida.content = format!(
                "lidos na tela: {} campos ({} obrigatorios){}; SQL em {pasta}/tela.sql\n{}",
                campos.len(),
                campos.iter().filter(|c| c.obrigatorio).count(),
                if itens.is_empty() {
                    String::new()
                } else {
                    format!(" e grade de itens com {} colunas", itens.len())
                },
                saida.content
            );
            Ok(saida)
        })
    }
}

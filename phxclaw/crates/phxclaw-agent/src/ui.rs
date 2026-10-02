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
                "bootstrap":{"type":"boolean","description":"also write <folder>/bootstrap/index.html with Bootstrap 5.3 component classes (same layout engine; the CSS file is LOCAL, never a CDN)"},
                "bootstrap_css":{"type":"string","description":"local path of bootstrap.min.css relative to <folder>/bootstrap/index.html; default config ui.bootstrap_css or vendor/bootstrap-5.3.3/bootstrap.min.css"},
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
    gravar_app(&app, &avisos, pasta, args, ctx)
}

/// O UI-IR ja montado -> arquivos. Quem chega por print passa por aqui com o layout lido
/// dentro do app; quem chega por SQL, sem.
pub fn gravar_app(
    app: &phxclaw_ui_ir::App,
    avisos: &[String],
    pasta: &str,
    args: &Value,
    ctx: &ToolContext,
) -> Result<ToolOutput, ToolError> {
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
    let ir = serde_json::to_vec_pretty(app).map_err(|e| ToolError::Failed(e.to_string()))?;
    std::fs::write(dir.join("ui-ir.json"), ir).map_err(|e| ToolError::Failed(e.to_string()))?;
    std::fs::write(dir.join("index.html"), phxclaw_ui_ir::html::render(app))
        .map_err(|e| ToolError::Failed(e.to_string()))?;
    let mut artefatos = vec![];
    let react = args.get("react").and_then(Value::as_bool) == Some(true);
    let quer = |k: &str| args.get(k).and_then(Value::as_bool) == Some(true);
    let mut extras = vec![];
    if react {
        extras.push(("react", phxclaw_ui_ir::react::render(app)));
    }
    if quer("rust") {
        extras.push(("rust", phxclaw_ui_ir::rust::render(app)));
    }
    if quer("wlanguage") {
        extras.push(("wlanguage", phxclaw_ui_ir::wlanguage::render(app)));
    }
    // a folha do Bootstrap: argumento, depois `ui.bootstrap_css` do config, depois o caminho
    // padrao com a versao fixada. O adaptador recusa CDN; o arquivo quem poe e o projeto.
    let css_bootstrap = args
        .get("bootstrap_css")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| crate::config::texto("ui.bootstrap_css").ok().flatten())
        .unwrap_or_else(|| phxclaw_ui_ir::bootstrap::CSS_PADRAO.to_string());
    if quer("bootstrap") {
        let h = phxclaw_ui_ir::bootstrap::render(app, &css_bootstrap)
            .map_err(ToolError::InvalidArguments)?;
        extras.push(("bootstrap", vec![("index.html".to_string(), h)]));
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
    if quer("bootstrap") {
        r.push_str(&format!(
            "\nversao Bootstrap {} em {pasta}/bootstrap/index.html; a folha e LOCAL: ponha o bootstrap.min.css em {pasta}/bootstrap/{css_bootstrap}",
            phxclaw_ui_ir::bootstrap::VERSAO
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
/// rotulo contra o OCR e escreve o SQL, e o layout (`phxclaw_ui_ir::layout`) grava no UI-IR
/// a posicao, o grupo e a ordem de cada rotulo pelas caixas do mesmo OCR. O que o modelo inventar nao passa: rotulo que nao
/// esta escrito na tela e descartado.
pub struct ScreenshotToErpUiTool;

/// Perguntas medidas em 30/09 (qwen2.5vl:3b): campos 8/8 no cadastro; a das listas achou
/// a "Situação" que a primeira perdeu; a da grade so acerta sem formato JSON forcado.
const PERGUNTA_CAMPOS: &str = "This is a screenshot of a business application form. List the LABELS of the data entry fields (text boxes, dropdowns, checkboxes, date and money fields) in the main form area, exactly as written, including the * mark when present. Do not include the side menu, screen title, section titles, buttons, placeholders or values. Answer only a JSON array of strings.";
const PERGUNTA_LISTAS: &str = "List the labels of the dropdown (select) fields in the main form area, exactly as written. Answer only a JSON array of strings.";
const PERGUNTA_GRADE: &str = "What are the column headers of the items table in this screenshot? Answer only a JSON array of strings.";

/// O que se leu de uma captura: o OCR (linhas e layout), o que o modelo disse quando foi
/// perguntado, e os rotulos que ficaram.
pub struct LeituraDeTela {
    pub linhas: Vec<String>,
    pub layout: phxclaw_ui_ir::layout::Layout,
    /// Modelo de visao perguntado; `None` = so OCR + layout.
    pub modelo: Option<String>,
    pub respostas: Vec<String>,
    pub campos: Vec<phxclaw_ui_ir::imagem::Rotulo>,
    pub itens: Vec<phxclaw_ui_ir::imagem::Rotulo>,
}

/// Servidor e modelo de visao configurados: a UNICA leitura deles, para a ferramenta e a
/// prova de fidelidade perguntarem ao mesmo modelo.
pub fn modelo_de_visao() -> (String, String) {
    let modelo = crate::config::texto_de("modelo.visao").unwrap_or_else(|| "qwen2.5vl:3b".into());
    let base = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://127.0.0.1:11434".into());
    let base = if base.starts_with("http") {
        base
    } else {
        format!("http://{base}")
    };
    (base, modelo)
}

/// Le uma captura. O OCR roda UMA vez, em TSV, no bwrap: dele saem as linhas (a regua da
/// confirmacao) e o layout (posicao, grupo, ordem). Com `visao`, as tres perguntas ao
/// modelo, confirmadas pelo OCR; sem, os rotulos vem do proprio layout -- o modo que a
/// prova de fidelidade mede sem esperar ~90 s de modelo por tela.
pub async fn ler_tela(
    png: &std::path::Path,
    visao: Option<(&str, &str)>,
    prazo: std::time::Duration,
) -> Result<LeituraDeTela, ToolError> {
    use phxclaw_ui_ir::{imagem, layout};
    // psm 11 (texto esparso): rotulo de tela nao e paragrafo
    let tsv = crate::visao::tesseract_tsv(png, "por+eng", Some(11), prazo).await?;
    let (palavras, largura, altura) = layout::ler_tsv(&tsv);
    let linhas = layout::linhas(&palavras);
    let lay = layout::analisar(&palavras, largura, altura);
    let Some((base, modelo)) = visao else {
        return Ok(LeituraDeTela {
            linhas,
            campos: lay.campos(),
            itens: lay.colunas(),
            layout: lay,
            modelo: None,
            respostas: vec![],
        });
    };
    let bytes = std::fs::read(png).map_err(|e| ToolError::Failed(e.to_string()))?;
    let llm = phxclaw_llm::OllamaLlm::with_timeout(base, modelo, prazo)
        .map_err(|e| ToolError::Failed(e.to_string()))?;
    let mut respostas = vec![];
    for p in [PERGUNTA_CAMPOS, PERGUNTA_LISTAS, PERGUNTA_GRADE] {
        let r = llm
            .perguntar_com_imagem(p, &bytes)
            .await
            .map_err(|e| ToolError::Failed(format!("modelo de visao {modelo}: {e}")))?;
        respostas.push(r);
    }
    let itens = lay.confirmar_colunas(&imagem::confirmar(
        &imagem::lista_da_resposta(&respostas[2]),
        &[],
        &linhas,
    ));
    let campos = imagem::confirmar(
        &imagem::lista_da_resposta(&respostas[0]),
        &imagem::lista_da_resposta(&respostas[1]),
        &linhas,
    );
    let campos = imagem::sem_itens(&campos, &itens);
    Ok(LeituraDeTela {
        linhas,
        layout: lay,
        modelo: Some(modelo.to_string()),
        respostas,
        campos,
        itens,
    })
}

/// Leitura -> SQL -> UI-IR, com o layout lido gravado na tela principal da tabela. Cada
/// rotulo vira o campo pelo MESMO `imagem::coluna` que escreveu o SQL; o "Codigo" e a chave.
pub fn app_da_leitura(
    nome: &str,
    tabela: &str,
    l: &LeituraDeTela,
) -> (phxclaw_ui_ir::App, Vec<String>, String) {
    use phxclaw_ui_ir::{Screen, imagem};
    let sql = imagem::sql(tabela, &l.campos, &l.itens);
    let (mut app, avisos) = phxclaw_ui_ir::from_sql(nome, &sql);
    let t = imagem::coluna(tabela);
    let ti = format!("{t}_item");
    let tela = app.screens.iter().find_map(|s| match s {
        Screen::Form { id, entity, .. } if *entity == t => Some(id.clone()),
        Screen::MasterDetail { id, master, .. } if *master == t => Some(id.clone()),
        _ => None,
    });
    let existe = |app: &phxclaw_ui_ir::App, e: &str, c: &str| {
        app.entities
            .iter()
            .any(|x| x.name == e && x.fields.iter().any(|f| f.name == c))
    };
    if let Some(tela) = tela {
        let mut escolhidos = vec![];
        for (rotulos, ent) in [(&l.campos, &t), (&l.itens, &ti)] {
            for r in rotulos {
                let c = match imagem::coluna(&r.texto).as_str() {
                    "codigo" => "id".to_string(),
                    c => c.to_string(),
                };
                if let Some(i) = l.layout.achar(&r.texto)
                    && existe(&app, ent, &c)
                    && !escolhidos.iter().any(|(j, _, _)| *j == i)
                {
                    escolhidos.push((i, ent.clone(), c));
                }
            }
        }
        if !escolhidos.is_empty() {
            app.layouts.push(l.layout.para_ir(&tela, &escolhidos));
        }
    }
    (app, avisos, sql)
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
                "bootstrap":{"type":"boolean"},
                "bootstrap_css":{"type":"string"},
                "rust":{"type":"boolean"},
                "wlanguage":{"type":"boolean"},
                "vision":{"type":"boolean","description":"ask the local vision model (default true); false = OCR + layout only"}
            },"required":["image","table"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "doc.write"
    }
    /// Cria processo (tesseract, pelo `ler_tela`): a regra de comando a alcanca pela imagem.
    fn comando_de_shell(&self, args: &Value) -> Option<String> {
        Some(crate::motor::linha_sintetica(
            "screenshot_to_erp_ui",
            args,
            &["image"],
        ))
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
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
            // vision:false = so OCR + layout, sem modelo (o modo da prova de fidelidade)
            let (base, modelo) = modelo_de_visao();
            let com_modelo = args.get("vision").and_then(Value::as_bool) != Some(false);
            let l = ler_tela(
                &caminho,
                com_modelo.then_some((base.as_str(), modelo.as_str())),
                ctx.timeout,
            )
            .await?;
            let (campos, itens) = (&l.campos, &l.itens);
            let dir = confine(&ctx.workdir, &pasta).map_err(ToolError::Denied)?;
            std::fs::create_dir_all(&dir).map_err(|e| ToolError::Failed(e.to_string()))?;
            // a trilha da leitura: o que o OCR viu, o que o modelo disse, o que ficou
            let leitura = json!({
                "imagem": img, "modelo": l.modelo, "ocr": l.linhas, "respostas": l.respostas,
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
                    l.linhas.len(),
                    l.respostas
                        .first()
                        .map(|r| r.chars().take(200).collect::<String>())
                        .unwrap_or_default()
                )));
            }
            let (app, avisos, sql) = app_da_leitura(nome, tabela, &l);
            std::fs::write(dir.join("tela.sql"), &sql)
                .map_err(|e| ToolError::Failed(e.to_string()))?;
            let mut saida = gravar_app(&app, &avisos, &pasta, &args, ctx)?;
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

/// `phxclaw ui importar ARQ.phx.json`: o PHX JSON do Phoenix vira UI-IR e as duas telas web
/// (adaptador «phoenix» e Bootstrap), pelo mesmo motor de `design_erp_ui`. Grava tambem o
/// PHX JSON escrito DE VOLTA a partir do IR, e diz se a ida e volta saiu identica: e a
/// prova de que nada do arquivo se perdeu na traducao.
pub fn importar_phx(
    arq: &std::path::Path,
    saida: &std::path::Path,
    css_bootstrap: Option<&str>,
) -> Result<String, String> {
    let texto = std::fs::read_to_string(arq).map_err(|e| format!("{}: {e}", arq.display()))?;
    let (env, app) = phxclaw_ui_ir::phx_json::ler(&texto)?;
    let volta = phxclaw_ui_ir::phx_json::escrever(&env, &app)?;
    let css = css_bootstrap.map(String::from).unwrap_or_else(|| {
        format!(
            "vendor/bootstrap-{}/bootstrap.min.css",
            env.versao_do_adaptador
        )
    });
    let boot = phxclaw_ui_ir::bootstrap::render_com_versao(&app, &css, &env.versao_do_adaptador)?;
    let gravar = |rel: &str, c: &str| -> Result<(), String> {
        let p = saida.join(rel);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(&p, c).map_err(|e| format!("{}: {e}", p.display()))
    };
    gravar(
        "ui-ir.json",
        &serde_json::to_string_pretty(&app).map_err(|e| e.to_string())?,
    )?;
    gravar("index.html", &phxclaw_ui_ir::html::render(&app))?;
    gravar("bootstrap/index.html", &boot)?;
    gravar("app.phx.json", &volta)?;
    let dir = saida.display();
    let ida = if volta == texto {
        "identica"
    } else {
        "DIFERENTE (veja app.phx.json)"
    };
    Ok(format!(
        "{}: {} tela(s), UI-IR v{} em {dir}/ui-ir.json; telas em {dir}/index.html e \
{dir}/bootstrap/index.html (Bootstrap {} LOCAL: ponha a folha em {dir}/bootstrap/{css}); \
ida e volta do PHX JSON: {ida}",
        arq.display(),
        app.screens.len(),
        app.ir_version,
        env.versao_do_adaptador,
    ))
}

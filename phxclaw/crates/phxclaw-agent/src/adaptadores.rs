//! Adaptadores: navegador, busca e documentos viram ferramentas do agente.
//!
//! A leitura de paginas passa pelo navegador, e nao pelo EgressBroker, de proposito: o
//! agente precisa ler qualquer pagina publica que a busca devolver, e o navegador ja tem
//! essa politica (publico liberado, rede interna recusada pelo IP resolvido, toda
//! requisicao da pagina interceptada). A busca passa pelo broker liberando so as origens
//! do buscador.

use crate::motor::artifact_for;
use crate::tarefa::confine;
use phxclaw_agent_core::{
    BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec, truncate_for_model,
};
use phxclaw_browser::{Browser, BrowserPolicy, LaunchOptions, Page};
use phxclaw_web_search::SearchBackend;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

fn arg<'a>(a: &'a Value, n: &str) -> Result<&'a str, ToolError> {
    a.get(n)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidArguments(format!("falta o campo texto '{n}'")))
}

// ---------------------------------------------------------------- busca

pub struct WebSearchTool {
    pub backend: Arc<dyn SearchBackend>,
}

impl Tool for WebSearchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "web_search".into(),
            description: "Search the web. Returns titles, URLs and snippets. Then open promising URLs with browser_open.".into(),
            parameters: json!({"type":"object","properties":{"query":{"type":"string"},"max_results":{"type":"integer","minimum":1,"maximum":10}},"required":["query"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "web.search"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let q = arg(&args, "query")?;
            let max = args
                .get("max_results")
                .and_then(Value::as_u64)
                .unwrap_or(6)
                .clamp(1, 10) as usize;
            let hits = self
                .backend
                .search(q, max)
                .await
                .map_err(|e| ToolError::Failed(e.to_string()))?;
            if hits.is_empty() {
                return Ok(ToolOutput::text("nenhum resultado"));
            }
            let t: Vec<String> = hits
                .iter()
                .enumerate()
                .map(|(i, h)| format!("{}. {}\n   {}\n   {}", i + 1, h.title, h.url, h.snippet))
                .collect();
            Ok(ToolOutput::text(t.join("\n")))
        })
    }
}

// ---------------------------------------------------------------- navegador

/// Uma sessao de navegador por tarefa, aberta na primeira chamada e fechada no fim da tarefa.
pub struct BrowserSessions {
    pub policy: BrowserPolicy,
    sessions: Mutex<HashMap<String, (Browser, Page)>>,
}

impl BrowserSessions {
    pub fn new(policy: BrowserPolicy) -> Arc<Self> {
        Arc::new(Self {
            policy,
            sessions: Mutex::new(HashMap::new()),
        })
    }

    async fn with_page<T>(
        &self,
        task: &str,
        f: impl for<'p> FnOnce(&'p Page) -> BoxFut<'p, Result<T, ToolError>>,
    ) -> Result<T, ToolError> {
        let mut g = self.sessions.lock().await;
        if !g.contains_key(task) {
            let b = Browser::launch(LaunchOptions::with_policy(self.policy.clone()))
                .await
                .map_err(|e| ToolError::Failed(format!("navegador: {e}")))?;
            let p = b
                .new_page()
                .await
                .map_err(|e| ToolError::Failed(e.to_string()))?;
            g.insert(task.to_string(), (b, p));
        }
        let (_, page) = g.get(task).expect("acabou de inserir");
        f(page).await
    }

    /// Abre `url` na sessao da tarefa e devolve (url final, titulo, texto legivel), o MESMO
    /// texto que o `browser_open` mostra ao modelo. A pesquisa profunda confere citacao
    /// contra este texto: conferir contra outro leitor aprovaria trecho que o modelo nunca
    /// viu, ou recusaria o que ele viu.
    pub async fn ler_pagina(
        &self,
        task: &str,
        url: &str,
    ) -> Result<(String, String, String), ToolError> {
        let url = url.to_string();
        self.with_page(task, move |p| {
            Box::pin(async move {
                let f = |e: phxclaw_browser::BrowserError| match e {
                    phxclaw_browser::BrowserError::PolicyDenied { .. } => {
                        ToolError::Denied(e.to_string())
                    }
                    outro => ToolError::Failed(outro.to_string()),
                };
                p.goto(&url).await.map_err(f)?;
                Ok((
                    p.url().await.map_err(f)?,
                    p.title().await.map_err(f)?,
                    p.markdown_like().await.map_err(f)?,
                ))
            })
        })
        .await
    }

    pub async fn close(&self, task: &str) {
        if let Some((b, p)) = self.sessions.lock().await.remove(task) {
            let _ = p.close().await;
            let _ = b.close().await;
        }
    }

    pub async fn open_sessions(&self) -> usize {
        self.sessions.lock().await.len()
    }
}

#[derive(Clone, Copy)]
pub enum BrowserAction {
    Open,
    Read,
    Click,
    Type,
    Screenshot,
}

pub struct BrowserTool {
    pub sessions: Arc<BrowserSessions>,
    pub action: BrowserAction,
    pub max_chars: usize,
}

/// As cinco ferramentas de navegador, dividindo a mesma sessao por tarefa.
pub fn browser_tools(sessions: Arc<BrowserSessions>) -> Vec<Arc<dyn Tool>> {
    [
        BrowserAction::Open,
        BrowserAction::Read,
        BrowserAction::Click,
        BrowserAction::Type,
        BrowserAction::Screenshot,
    ]
    .into_iter()
    .map(|action| {
        Arc::new(BrowserTool {
            sessions: sessions.clone(),
            action,
            max_chars: 12_000,
        }) as Arc<dyn Tool>
    })
    .collect()
}

async fn leitura(p: &Page, max: usize) -> Result<String, ToolError> {
    let f = |e: phxclaw_browser::BrowserError| ToolError::Failed(e.to_string());
    let titulo = p.title().await.map_err(f)?;
    let url = p.url().await.map_err(f)?;
    let corpo = p.markdown_like().await.map_err(f)?;
    Ok(format!(
        "URL: {url}\nTitle: {titulo}\n\n{}",
        truncate_for_model(&corpo, max)
    ))
}

impl Tool for BrowserTool {
    fn spec(&self) -> ToolSpec {
        let (name, desc, params) = match self.action {
            BrowserAction::Open => (
                "browser_open",
                "Open a URL in the headless browser and return the page as readable text with links.",
                json!({"type":"object","properties":{"url":{"type":"string"}},"required":["url"]}),
            ),
            BrowserAction::Read => (
                "browser_read",
                "Read the current browser page again (after clicking or typing).",
                json!({"type":"object","properties":{}}),
            ),
            BrowserAction::Click => (
                "browser_click",
                "Click the element matching a CSS selector in the current page and wait for navigation.",
                json!({"type":"object","properties":{"selector":{"type":"string"}},"required":["selector"]}),
            ),
            BrowserAction::Type => (
                "browser_type",
                "Type text into the input matching a CSS selector; optionally press Enter to submit.",
                json!({"type":"object","properties":{"selector":{"type":"string"},"text":{"type":"string"},"submit":{"type":"boolean"}},"required":["selector","text"]}),
            ),
            BrowserAction::Screenshot => (
                "browser_screenshot",
                "Save a PNG screenshot of the current page into the task directory.",
                json!({"type":"object","properties":{"path":{"type":"string","description":"relative .png path"}}}),
            ),
        };
        ToolSpec {
            name: name.into(),
            description: desc.into(),
            parameters: params,
        }
    }
    fn capability(&self) -> &'static str {
        "web.browse"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let max = self.max_chars;
            let bf = |e: phxclaw_browser::BrowserError| match e {
                phxclaw_browser::BrowserError::PolicyDenied { .. } => {
                    ToolError::Denied(e.to_string())
                }
                outro => ToolError::Failed(outro.to_string()),
            };
            match self.action {
                BrowserAction::Open => {
                    let url = arg(&args, "url")?.to_string();
                    let t = self
                        .sessions
                        .with_page(&ctx.task_id, move |p| {
                            Box::pin(async move {
                                p.goto(&url).await.map_err(bf)?;
                                leitura(p, max).await
                            })
                        })
                        .await?;
                    Ok(ToolOutput::text(t))
                }
                BrowserAction::Read => Ok(ToolOutput::text(
                    self.sessions
                        .with_page(&ctx.task_id, move |p| Box::pin(leitura(p, max)))
                        .await?,
                )),
                BrowserAction::Click => {
                    let sel = arg(&args, "selector")?.to_string();
                    let t = self
                        .sessions
                        .with_page(&ctx.task_id, move |p| {
                            Box::pin(async move {
                                p.click_and_wait(&sel).await.map_err(bf)?;
                                leitura(p, max).await
                            })
                        })
                        .await?;
                    Ok(ToolOutput::text(t))
                }
                BrowserAction::Type => {
                    let sel = arg(&args, "selector")?.to_string();
                    let texto = arg(&args, "text")?.to_string();
                    let enviar = args.get("submit").and_then(Value::as_bool).unwrap_or(false);
                    let t = self
                        .sessions
                        .with_page(&ctx.task_id, move |p| {
                            Box::pin(async move {
                                p.type_text(&sel, &texto).await.map_err(bf)?;
                                if enviar {
                                    p.press_enter().await.map_err(bf)?;
                                    let _ = p.wait_for_load().await;
                                }
                                leitura(p, max).await
                            })
                        })
                        .await?;
                    Ok(ToolOutput::text(t))
                }
                BrowserAction::Screenshot => {
                    let rel = args
                        .get("path")
                        .and_then(Value::as_str)
                        .unwrap_or("captura.png")
                        .to_string();
                    if !rel.ends_with(".png") {
                        return Err(ToolError::InvalidArguments(
                            "path tem de terminar em .png".into(),
                        ));
                    }
                    let alvo = confine(&ctx.workdir, &rel).map_err(ToolError::Denied)?;
                    let png = self
                        .sessions
                        .with_page(&ctx.task_id, move |p| {
                            Box::pin(async move { p.screenshot_png().await.map_err(bf) })
                        })
                        .await?;
                    if let Some(d) = alvo.parent() {
                        std::fs::create_dir_all(d).map_err(|e| ToolError::Failed(e.to_string()))?;
                    }
                    std::fs::write(&alvo, &png).map_err(|e| ToolError::Failed(e.to_string()))?;
                    let a = artifact_for(&ctx.workdir, &rel)
                        .map_err(|e| ToolError::Failed(e.to_string()))?;
                    Ok(ToolOutput {
                        content: format!("captura salva em {rel} ({} bytes)", a.bytes),
                        artifacts: vec![a],
                    })
                }
            }
        })
    }
    fn finish<'a>(&'a self, task_id: &'a str) -> BoxFut<'a, ()> {
        // So a primeira ferramenta do grupo precisa fechar; fechar de novo e no-op.
        Box::pin(async move { self.sessions.close(task_id).await })
    }
}

// ---------------------------------------------------------------- documentos

#[derive(Clone, Copy)]
pub enum OfficeKind {
    Document,
    Spreadsheet,
    Presentation,
    Read,
}

pub struct OfficeTool {
    pub kind: OfficeKind,
}

pub fn office_tools() -> Vec<Arc<dyn Tool>> {
    [
        OfficeKind::Document,
        OfficeKind::Spreadsheet,
        OfficeKind::Presentation,
        OfficeKind::Read,
    ]
    .into_iter()
    .map(|kind| Arc::new(OfficeTool { kind }) as Arc<dyn Tool>)
    .collect()
}

/// Celula a partir de JSON simples: modelo pequeno erra o formato etiquetado do serde, entao
/// a ferramenta aceita o valor cru -- numero e numero, "=..." e formula, null e vazio.
fn celula(v: &Value) -> phxclaw_office::Cell {
    use phxclaw_office::Cell;
    match v {
        Value::Null => Cell::Empty,
        Value::Bool(b) => Cell::Bool(*b),
        Value::Number(n) => Cell::Number(n.as_f64().unwrap_or(0.0)),
        Value::String(s) if s.starts_with('=') && s.len() > 1 => Cell::Formula(s[1..].to_string()),
        Value::String(s) => Cell::Text(s.clone()),
        outro => Cell::Text(outro.to_string()),
    }
}

fn caminho_com_extensao(
    ctx: &ToolContext,
    args: &Value,
    ext: &str,
) -> Result<(String, std::path::PathBuf), ToolError> {
    let rel = arg(args, "path")?.to_string();
    if !rel.to_ascii_lowercase().ends_with(ext) {
        return Err(ToolError::InvalidArguments(format!(
            "path tem de terminar em {ext}"
        )));
    }
    let alvo = confine(&ctx.workdir, &rel).map_err(ToolError::Denied)?;
    if let Some(d) = alvo.parent() {
        std::fs::create_dir_all(d).map_err(|e| ToolError::Failed(e.to_string()))?;
    }
    Ok((rel, alvo))
}

impl Tool for OfficeTool {
    fn spec(&self) -> ToolSpec {
        match self.kind {
            OfficeKind::Document => ToolSpec {
                name: "create_document".into(),
                description: "Create a Word .docx file. blocks: [{type:'heading',level:1,text}, {type:'paragraph',text,bold?}, {type:'bullets',items:[..]}, {type:'table',header:[..],rows:[[..]]}].".into(),
                parameters: json!({"type":"object","properties":{"path":{"type":"string","description":"relative path ending in .docx"},"title":{"type":"string"},"blocks":{"type":"array","items":{"type":"object"}}},"required":["path","blocks"]}),
            },
            OfficeKind::Spreadsheet => ToolSpec {
                name: "create_spreadsheet".into(),
                description: "Create an Excel .xlsx file. Each sheet has rows of plain values: numbers stay numbers, strings starting with '=' are formulas (e.g. '=SUM(B2:B4)'), null is an empty cell.".into(),
                parameters: json!({"type":"object","properties":{"path":{"type":"string","description":"relative path ending in .xlsx"},"sheets":{"type":"array","items":{"type":"object","properties":{"name":{"type":"string"},"rows":{"type":"array","items":{"type":"array"}},"bold_header":{"type":"boolean"}},"required":["name","rows"]}}},"required":["path","sheets"]}),
            },
            OfficeKind::Presentation => ToolSpec {
                name: "create_presentation".into(),
                description: "Create a PowerPoint .pptx file with a title slide and content slides (title + bullets + optional speaker notes).".into(),
                parameters: json!({"type":"object","properties":{"path":{"type":"string","description":"relative path ending in .pptx"},"title":{"type":"string"},"subtitle":{"type":"string"},"slides":{"type":"array","items":{"type":"object","properties":{"title":{"type":"string"},"bullets":{"type":"array","items":{"type":"string"}},"notes":{"type":"string"}},"required":["title"]}}},"required":["path","title","slides"]}),
            },
            OfficeKind::Read => ToolSpec {
                name: "read_document".into(),
                description: "Read a document of the task directory as text: .docx, .pptx (slides and notes), .pdf, .xlsx (cell values), and plain text (.txt .md .csv .html .css .js .json .xml .svg; .html also comes without markup). Long output is truncated.".into(),
                parameters: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
            },
        }
    }
    fn capability(&self) -> &'static str {
        match self.kind {
            OfficeKind::Read => "fs.read",
            _ => "doc.write",
        }
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let of = |e: phxclaw_office::OfficeError| ToolError::Failed(e.to_string());
            let invalido = |e: serde_json::Error| ToolError::InvalidArguments(e.to_string());
            let rel = match self.kind {
                OfficeKind::Document => {
                    let (rel, alvo) = caminho_com_extensao(ctx, &args, ".docx")?;
                    let doc = phxclaw_office::Document {
                        title: args
                            .get("title")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        blocks: serde_json::from_value(
                            args.get("blocks").cloned().unwrap_or(json!([])),
                        )
                        .map_err(invalido)?,
                    };
                    phxclaw_office::write_docx(&doc, &alvo).map_err(of)?;
                    rel
                }
                OfficeKind::Spreadsheet => {
                    let (rel, alvo) = caminho_com_extensao(ctx, &args, ".xlsx")?;
                    let mut sheets = vec![];
                    for s in args
                        .get("sheets")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default()
                    {
                        let rows = s
                            .get("rows")
                            .and_then(Value::as_array)
                            .map(|r| {
                                r.iter()
                                    .map(|linha| {
                                        linha
                                            .as_array()
                                            .map(|c| c.iter().map(celula).collect())
                                            .unwrap_or_default()
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        sheets.push(phxclaw_office::Sheet {
                            name: s
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("Planilha1")
                                .to_string(),
                            rows,
                            bold_header: s
                                .get("bold_header")
                                .and_then(Value::as_bool)
                                .unwrap_or(true),
                        });
                    }
                    if sheets.is_empty() {
                        return Err(ToolError::InvalidArguments("sheets vazio".into()));
                    }
                    phxclaw_office::write_xlsx(&phxclaw_office::Workbook { sheets }, &alvo)
                        .map_err(of)?;
                    rel
                }
                OfficeKind::Presentation => {
                    let (rel, alvo) = caminho_com_extensao(ctx, &args, ".pptx")?;
                    let deck = phxclaw_office::Deck {
                        title: arg(&args, "title")?.to_string(),
                        subtitle: args
                            .get("subtitle")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        slides: serde_json::from_value(
                            args.get("slides").cloned().unwrap_or(json!([])),
                        )
                        .map_err(invalido)?,
                    };
                    phxclaw_office::write_pptx(&deck, &alvo).map_err(of)?;
                    rel
                }
                OfficeKind::Read => {
                    // O formato decide-se num lugar so (`arquivos::ler_documento`), que a
                    // ferramenta `pdf` tambem usa para ler PDF.
                    let texto = crate::arquivos::ler_documento(ctx, arg(&args, "path")?).await?;
                    return Ok(ToolOutput::text(texto));
                }
            };
            let a =
                artifact_for(&ctx.workdir, &rel).map_err(|e| ToolError::Failed(e.to_string()))?;
            Ok(ToolOutput {
                content: format!("criado {rel} ({} bytes)", a.bytes),
                artifacts: vec![a],
            })
        })
    }
}

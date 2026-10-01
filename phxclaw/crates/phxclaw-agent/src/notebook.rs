//! `notebook_read` e `notebook_edit`: o `.ipynb` lido e editado como o JSON que ele e.
//!
//! Por que nao `edit_file`: o fonte de uma celula e uma LISTA de linhas dentro do JSON, com
//! `\n` escapado; o modelo que troca texto ali erra a aspa ou o escape e o Jupyter nao abre
//! mais o caderno. Aqui o modelo fala em celula (indice ou id) e texto corrido, e a
//! gravacao refaz o JSON.
//!
//! Duas decisoes:
//! - **Celula de codigo editada perde as saidas e o `execution_count`.** Saida que ficou
//!   de um codigo que nao existe mais e mentira sobre o caderno -- o Jupyter faz igual.
//! - **Gravacao no formato do nbformat**: chaves em ordem, indentacao de 1, `\n` no fim.
//!   Diff de caderno ja e ruim; reformatar o arquivo inteiro a cada celula seria pior.

use crate::tarefa::confine;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::path::Path;

const MAX_CHARS_SAIDA: usize = 2000;

fn texto_de(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
        _ => String::new(),
    }
}

/// Texto corrido -> lista de linhas, cada uma com o `\n` dela (o formato do nbformat).
fn linhas_de(t: &str) -> Value {
    Value::Array(t.split_inclusive('\n').map(|l| json!(l)).collect())
}

fn ler_caderno(p: &Path) -> Result<Value, ToolError> {
    let b = std::fs::read(p).map_err(|e| ToolError::Failed(e.to_string()))?;
    let nb: Value = serde_json::from_slice(&b)
        .map_err(|e| ToolError::InvalidArguments(format!("nao e JSON de caderno: {e}")))?;
    let versao = nb.get("nbformat").and_then(Value::as_u64).unwrap_or(0);
    if versao != 4 || !nb.get("cells").is_some_and(Value::is_array) {
        return Err(ToolError::InvalidArguments(format!(
            "caderno nbformat {versao}: so o 4 (com 'cells') e editado aqui"
        )));
    }
    Ok(nb)
}

pub fn gravar_caderno(p: &Path, nb: &Value) -> Result<(), ToolError> {
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    nb.serialize(&mut ser)
        .map_err(|e| ToolError::Failed(e.to_string()))?;
    buf.push(b'\n');
    std::fs::write(p, buf).map_err(|e| ToolError::Failed(e.to_string()))
}

/// Uma saida em texto curto: stream e texto, imagem vira so o tipo e o tamanho.
fn resumir_saida(o: &Value) -> Value {
    let tipo = o.get("output_type").and_then(Value::as_str).unwrap_or("");
    let corta = |s: String| -> String {
        if s.chars().count() > MAX_CHARS_SAIDA {
            format!(
                "{}[... {} caracteres]",
                s.chars().take(MAX_CHARS_SAIDA).collect::<String>(),
                s.chars().count()
            )
        } else {
            s
        }
    };
    match tipo {
        "stream" => {
            json!({"tipo": "stream", "nome": o["name"], "texto": corta(texto_de(&o["text"]))})
        }
        "error" => json!({
            "tipo": "erro",
            "nome": o["ename"],
            "valor": o["evalue"],
        }),
        _ => {
            let dados = o.get("data").and_then(Value::as_object);
            let texto = dados
                .and_then(|d| d.get("text/plain"))
                .map(texto_de)
                .map(corta);
            let mimes: Vec<Value> = dados
                .map(|d| {
                    d.iter()
                        .filter(|(k, _)| *k != "text/plain")
                        .map(|(k, v)| json!({"mime": k, "bytes": texto_de(v).len()}))
                        .collect()
                })
                .unwrap_or_default();
            json!({"tipo": tipo, "texto": texto, "outros": mimes})
        }
    }
}

/// Indice da celula por `index` ou por `cell_id`.
fn achar_celula(cells: &[Value], args: &Value) -> Result<usize, ToolError> {
    if let Some(id) = args.get("cell_id").and_then(Value::as_str) {
        return cells
            .iter()
            .position(|c| c.get("id").and_then(Value::as_str) == Some(id))
            .ok_or_else(|| ToolError::InvalidArguments(format!("nao ha celula com id {id}")));
    }
    let i = args
        .get("index")
        .and_then(Value::as_u64)
        .ok_or_else(|| ToolError::InvalidArguments("falta 'index' ou 'cell_id'".into()))?
        as usize;
    if i >= cells.len() {
        return Err(ToolError::InvalidArguments(format!(
            "index {i} fora do caderno ({} celulas, de 0 a {})",
            cells.len(),
            cells.len().saturating_sub(1)
        )));
    }
    Ok(i)
}

pub struct NotebookReadTool;

impl Tool for NotebookReadTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "notebook_read".into(),
            description: "Read a Jupyter notebook (.ipynb) of the task directory as cells: \
index, id, type, source and a summary of the outputs (text, errors, image types)."
                .into(),
            parameters: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
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
            let rel = args
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'path'".into()))?;
            let nb = ler_caderno(&confine(&ctx.workdir, rel).map_err(ToolError::Denied)?)?;
            let celulas: Vec<Value> = nb["cells"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
                .map(|(i, c)| {
                    json!({
                        "indice": i,
                        "id": c.get("id"),
                        "tipo": c.get("cell_type"),
                        "fonte": texto_de(&c["source"]),
                        "execution_count": c.get("execution_count"),
                        "saidas": c.get("outputs").and_then(Value::as_array)
                            .map(|o| o.iter().map(resumir_saida).collect::<Vec<_>>())
                            .unwrap_or_default(),
                    })
                })
                .collect();
            let linguagem = nb
                .pointer("/metadata/kernelspec/language")
                .or_else(|| nb.pointer("/metadata/language_info/name"))
                .cloned();
            Ok(ToolOutput::text(
                json!({
                    "nbformat": format!("{}.{}", nb["nbformat"], nb["nbformat_minor"].as_u64().unwrap_or(0)),
                    "linguagem": linguagem,
                    "celulas": celulas,
                })
                .to_string(),
            ))
        })
    }
}

pub struct NotebookEditTool;

impl Tool for NotebookEditTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "notebook_edit".into(),
            description: "Edit a Jupyter notebook (.ipynb) by cell. action=replace {index or \
cell_id, source, cell_type?} (code cells lose stale outputs); insert {index (position, = cell \
count appends), source, cell_type: code|markdown|raw}; delete {index or cell_id}."
                .into(),
            parameters: json!({"type":"object","properties":{
                "path":{"type":"string"},
                "action":{"type":"string","enum":["replace","insert","delete"]},
                "index":{"type":"integer"},
                "cell_id":{"type":"string"},
                "source":{"type":"string"},
                "cell_type":{"type":"string","enum":["code","markdown","raw"]}
            },"required":["path","action"]}),
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
            let rel = args
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidArguments("falta 'path'".into()))?;
            let p = confine(&ctx.workdir, rel).map_err(ToolError::Denied)?;
            let mut nb = ler_caderno(&p)?;
            let minor = nb["nbformat_minor"].as_u64().unwrap_or(0);
            let tipo = args.get("cell_type").and_then(Value::as_str);
            if let Some(t) = tipo
                && !matches!(t, "code" | "markdown" | "raw")
            {
                return Err(ToolError::InvalidArguments(format!(
                    "cell_type invalido: {t} (code, markdown, raw)"
                )));
            }
            let fonte = || {
                args.get("source")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ToolError::InvalidArguments("falta 'source'".into()))
            };
            let acao = args.get("action").and_then(Value::as_str).unwrap_or("");
            let cells = nb["cells"]
                .as_array_mut()
                .expect("conferido em ler_caderno");
            let feito = match acao {
                "replace" => {
                    let i = achar_celula(cells, &args)?;
                    let c = cells[i]
                        .as_object_mut()
                        .ok_or_else(|| ToolError::Failed(format!("celula {i} nao e objeto")))?;
                    c.insert("source".into(), linhas_de(fonte()?));
                    if let Some(t) = tipo {
                        c.insert("cell_type".into(), json!(t));
                    }
                    normalizar_celula(c);
                    json!({"substituida": i})
                }
                "insert" => {
                    let i = args
                        .get("index")
                        .and_then(Value::as_u64)
                        .map(|i| i as usize)
                        .unwrap_or(cells.len());
                    if i > cells.len() {
                        return Err(ToolError::InvalidArguments(format!(
                            "index {i} alem do fim ({} celulas)",
                            cells.len()
                        )));
                    }
                    let mut c = Map::new();
                    c.insert("cell_type".into(), json!(tipo.unwrap_or("code")));
                    c.insert("metadata".into(), json!({}));
                    c.insert("source".into(), linhas_de(fonte()?));
                    // `id` e obrigatorio a partir do nbformat 4.5.
                    if minor >= 5 {
                        let id: String = phxclaw_types::new_uuid_v7()
                            .simple()
                            .to_string()
                            .chars()
                            .rev()
                            .take(8)
                            .collect();
                        c.insert("id".into(), json!(id));
                    }
                    normalizar_celula(&mut c);
                    cells.insert(i, Value::Object(c));
                    json!({"inserida": i, "id": cells[i].get("id")})
                }
                "delete" => {
                    let i = achar_celula(cells, &args)?;
                    cells.remove(i);
                    json!({"removida": i})
                }
                outra => {
                    return Err(ToolError::InvalidArguments(format!(
                        "action desconhecida: {outra} (replace, insert, delete)"
                    )));
                }
            };
            let total = cells.len();
            gravar_caderno(&p, &nb)?;
            let a = crate::motor::artifact_for(&ctx.workdir, rel)
                .map_err(|e| ToolError::Failed(e.to_string()))?;
            let mut v = feito;
            v["celulas"] = json!(total);
            Ok(ToolOutput {
                content: v.to_string(),
                artifacts: vec![a],
            })
        })
    }
}

/// Celula de codigo tem `outputs` e `execution_count` (zerados: o codigo mudou); as outras
/// nao podem ter -- o validador do nbformat recusa.
fn normalizar_celula(c: &mut Map<String, Value>) {
    if c.get("cell_type").and_then(Value::as_str) == Some("code") {
        c.insert("outputs".into(), json!([]));
        c.insert("execution_count".into(), Value::Null);
    } else {
        c.remove("outputs");
        c.remove("execution_count");
    }
}

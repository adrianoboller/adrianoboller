#![forbid(unsafe_code)]
//! Entregaveis de escritorio (.docx, .xlsx, .pptx) escritos direto em OOXML.
//!
//! Sem LibreOffice nem Python em tempo de execucao: o agente manda o modelo em
//! JSON e recebe o arquivo. A saida e deterministica (mesma entrada, mesmos
//! bytes) para que o hash do entregavel sirva de prova em auditoria.

use serde::{Deserialize, Serialize};
use std::path::Path;

mod docx;
mod opc;
mod pptx;
mod xlsx;
pub mod xml;
pub mod zip;

pub use docx::{docx_bytes, read_docx_text, read_docx_text_bytes, write_docx};
pub use pptx::{pptx_bytes, read_pptx_text, read_pptx_text_bytes, write_pptx};
pub use xlsx::{read_xlsx_values, read_xlsx_values_bytes, write_xlsx, xlsx_bytes};

#[derive(Debug, thiserror::Error)]
pub enum OfficeError {
    #[error("erro de E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("entrada invalida: {0}")]
    Invalid(String),
    #[error("arquivo corrompido ou nao suportado: {0}")]
    Corrupt(String),
}

pub type Result<T> = std::result::Result<T, OfficeError>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub blocks: Vec<Block>,
}

/// JSON: `{"type":"heading","level":1,"text":"..."}`,
/// `{"type":"paragraph","text":"...","bold":true}`,
/// `{"type":"bullets","items":[...]}`, `{"type":"table","header":[...],"rows":[[...]]}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Heading {
        level: u8,
        text: String,
    },
    Paragraph {
        text: String,
        #[serde(default)]
        bold: bool,
    },
    Bullets {
        items: Vec<String>,
    },
    Table {
        #[serde(default)]
        header: Vec<String>,
        #[serde(default)]
        rows: Vec<Vec<String>>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workbook {
    pub sheets: Vec<Sheet>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    pub name: String,
    #[serde(default)]
    pub rows: Vec<Vec<Cell>>,
    /// Primeira linha em negrito (cabecalho).
    #[serde(default)]
    pub bold_header: bool,
}

/// JSON: `{"text":"..."}`, `{"number":1.5}`, `{"bool":true}`,
/// `{"formula":"SUM(A1:A3)"}` (com ou sem `=`), `"empty"`.
///
/// `Empty` existe porque a leitura precisa representar o buraco entre duas
/// celulas preenchidas sem inventar texto vazio, que e outro valor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cell {
    Empty,
    Text(String),
    Number(f64),
    Bool(bool),
    Formula(String),
}

impl From<&str> for Cell {
    fn from(s: &str) -> Self {
        Cell::Text(s.to_string())
    }
}
impl From<String> for Cell {
    fn from(s: String) -> Self {
        Cell::Text(s)
    }
}
impl From<f64> for Cell {
    fn from(n: f64) -> Self {
        Cell::Number(n)
    }
}
impl From<i64> for Cell {
    fn from(n: i64) -> Self {
        Cell::Number(n as f64)
    }
}
impl From<bool> for Cell {
    fn from(b: bool) -> Self {
        Cell::Bool(b)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deck {
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub slides: Vec<Slide>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slide {
    pub title: String,
    #[serde(default)]
    pub bullets: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes)?;
    Ok(())
}

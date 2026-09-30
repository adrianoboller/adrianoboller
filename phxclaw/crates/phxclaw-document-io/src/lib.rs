use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentFormat {
    Pdf,
    Txt,
    Csv,
    Json,
    Xml,
    Python,
    Xlsx,
    Docx,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentReadResult {
    pub path: PathBuf,
    pub format: DocumentFormat,
    pub text: Option<String>,
    pub structured: Option<Value>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentWriteRequest {
    pub path: PathBuf,
    pub format: DocumentFormat,
    pub text: Option<String>,
    pub structured: Option<Value>,
}

#[derive(Debug, Error)]
pub enum DocumentIoError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
    #[error("unsupported in native facade, use signed office adapter: {0:?}")]
    ExternalAdapterRequired(DocumentFormat),
    #[error("invalid document payload: {0}")]
    Invalid(String),
}

pub fn detect_format(path: &Path) -> Option<DocumentFormat> {
    match path
        .extension()?
        .to_string_lossy()
        .to_ascii_lowercase()
        .as_str()
    {
        "pdf" => Some(DocumentFormat::Pdf),
        "txt" | "md" | "log" => Some(DocumentFormat::Txt),
        "csv" => Some(DocumentFormat::Csv),
        "json" => Some(DocumentFormat::Json),
        "xml" => Some(DocumentFormat::Xml),
        "py" => Some(DocumentFormat::Python),
        "xlsx" | "xlsm" => Some(DocumentFormat::Xlsx),
        "docx" => Some(DocumentFormat::Docx),
        _ => None,
    }
}

pub fn read_native(
    path: &Path,
    format: DocumentFormat,
) -> Result<DocumentReadResult, DocumentIoError> {
    match format {
        DocumentFormat::Txt | DocumentFormat::Python | DocumentFormat::Xml => {
            let text = fs::read_to_string(path)?;
            Ok(DocumentReadResult {
                path: path.into(),
                format,
                text: Some(text),
                structured: None,
                metadata: Value::Null,
            })
        }
        DocumentFormat::Json => {
            let text = fs::read_to_string(path)?;
            let value: Value = serde_json::from_str(&text)?;
            Ok(DocumentReadResult {
                path: path.into(),
                format,
                text: Some(text),
                structured: Some(value),
                metadata: Value::Null,
            })
        }
        DocumentFormat::Csv => {
            let mut reader = csv::Reader::from_path(path)?;
            let headers = reader
                .headers()?
                .iter()
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            let mut rows = Vec::<Value>::new();
            for record in reader.records() {
                let record = record?;
                let mut object = serde_json::Map::new();
                for (idx, value) in record.iter().enumerate() {
                    let key = headers
                        .get(idx)
                        .cloned()
                        .unwrap_or_else(|| format!("column_{idx}"));
                    object.insert(key, Value::String(value.to_owned()));
                }
                rows.push(Value::Object(object));
            }
            Ok(DocumentReadResult {
                path: path.into(),
                format,
                text: None,
                structured: Some(Value::Array(rows)),
                metadata: serde_json::json!({"headers": headers}),
            })
        }
        DocumentFormat::Pdf | DocumentFormat::Xlsx | DocumentFormat::Docx => {
            Err(DocumentIoError::ExternalAdapterRequired(format))
        }
    }
}

pub fn write_native(request: &DocumentWriteRequest) -> Result<(), DocumentIoError> {
    match request.format {
        DocumentFormat::Txt | DocumentFormat::Python | DocumentFormat::Xml => {
            fs::write(&request.path, request.text.as_deref().unwrap_or_default())?;
        }
        DocumentFormat::Json => {
            let value = request.structured.as_ref().ok_or_else(|| {
                DocumentIoError::Invalid("JSON requires structured payload".into())
            })?;
            fs::write(&request.path, serde_json::to_vec_pretty(value)?)?;
        }
        DocumentFormat::Csv => {
            let rows = request
                .structured
                .as_ref()
                .and_then(Value::as_array)
                .ok_or_else(|| DocumentIoError::Invalid("CSV requires array of objects".into()))?;
            let mut writer = csv::Writer::from_path(&request.path)?;
            let headers = rows
                .first()
                .and_then(Value::as_object)
                .map(|o| o.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            if !headers.is_empty() {
                writer.write_record(&headers)?;
            }
            for row in rows {
                let object = row
                    .as_object()
                    .ok_or_else(|| DocumentIoError::Invalid("CSV row must be object".into()))?;
                let values = headers
                    .iter()
                    .map(|h| value_as_text(object.get(h)))
                    .collect::<Vec<_>>();
                writer.write_record(values)?;
            }
            writer.flush()?;
        }
        DocumentFormat::Pdf | DocumentFormat::Xlsx | DocumentFormat::Docx => {
            return Err(DocumentIoError::ExternalAdapterRequired(request.format));
        }
    }
    Ok(())
}

fn value_as_text(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(v)) => v.clone(),
        Some(v) => v.to_string(),
    }
}

/// Contract implemented by the signed office adapter plugin. The v0.5 reference provider
/// handles PDF/XLSX/DOCX while the microkernel remains dependency-light.
pub trait OfficeDocumentAdapter {
    fn read(&self, path: &Path, format: DocumentFormat) -> Result<DocumentReadResult, String>;
    fn write(&self, request: &DocumentWriteRequest) -> Result<(), String>;
}

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ObsidianBridgeError {
    #[error("unsafe vault path")]
    UnsafePath,
    #[error("note exceeds configured size")]
    NoteTooLarge,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultNote {
    pub path: String,
    pub title: String,
    pub markdown: String,
    pub source_uuid: Uuid,
    pub source_sha256: String,
    pub trust: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportedNote {
    pub path: String,
    pub content_sha256: String,
    pub epistemic_state: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CanvasNode {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanvasEdge {
    pub id: String,
    // O formato JSON Canvas do Obsidian grava camelCase; o nome Rust segue a convencao.
    #[serde(rename = "fromNode")]
    pub from_node: String,
    #[serde(rename = "toNode")]
    pub to_node: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct JsonCanvas {
    pub nodes: Vec<CanvasNode>,
    pub edges: Vec<CanvasEdge>,
}

pub fn note_sha256(markdown: &str) -> String {
    format!("{:x}", Sha256::digest(markdown.as_bytes()))
}
fn safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.split(['/', '\\']).any(|p| p == ".." || p.is_empty())
}
pub fn stage_import(
    path: &str,
    markdown: &str,
    max_bytes: usize,
) -> Result<ImportedNote, ObsidianBridgeError> {
    if !safe_relative_path(path) {
        return Err(ObsidianBridgeError::UnsafePath);
    }
    if markdown.len() > max_bytes {
        return Err(ObsidianBridgeError::NoteTooLarge);
    }
    Ok(ImportedNote {
        path: path.into(),
        content_sha256: note_sha256(markdown),
        epistemic_state: "unverified_context".into(),
    })
}
pub fn render_note(title: &str, source_uuid: Uuid, body: &str, trust: &str) -> VaultNote {
    let md = format!(
        "---\nphxclaw_uuid: {}\nphxclaw_trust: {}\n---\n\n# {}\n\n{}\n",
        source_uuid, trust, title, body
    );
    VaultNote {
        path: format!("PhxClaw/{}.md", source_uuid),
        title: title.into(),
        source_uuid,
        source_sha256: note_sha256(&md),
        markdown: md,
        trust: trust.into(),
    }
}
pub fn canvas_from_graph(nodes: &[(Uuid, String)], edges: &[(Uuid, Uuid, String)]) -> JsonCanvas {
    let cn = nodes
        .iter()
        .enumerate()
        .map(|(i, (id, label))| CanvasNode {
            id: id.to_string(),
            kind: "text".into(),
            x: ((i % 4) as i64) * 340,
            y: ((i / 4) as i64) * 220,
            width: 300,
            height: 160,
            text: Some(label.clone()),
            file: None,
        })
        .collect();
    let ce = edges
        .iter()
        .enumerate()
        .map(|(i, (a, b, label))| CanvasEdge {
            id: format!("e{}", i),
            from_node: a.to_string(),
            to_node: b.to_string(),
            label: Some(label.clone()),
        })
        .collect();
    JsonCanvas {
        nodes: cn,
        edges: ce,
    }
}
pub fn imported_trust() -> &'static str {
    "unverified_context"
}

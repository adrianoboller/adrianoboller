#![forbid(unsafe_code)]
use chrono::{DateTime, Utc};
use phxclaw_types::new_uuid_v7;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointManifest {
    pub uuid: Uuid,
    pub workspace: PathBuf,
    pub files: Vec<CheckpointFile>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointVerifyReport {
    pub checkpoint_uuid: Uuid,
    pub valid: bool,
    pub changed: Vec<String>,
    pub missing: Vec<String>,
}

#[derive(Debug, Error)]
pub enum CheckpointError {
    #[error("workspace does not exist: {0}")]
    MissingWorkspace(String),
    #[error("checkpoint path escapes workspace: {0}")]
    EscapesWorkspace(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub struct CheckpointManager {
    store_root: PathBuf,
}

impl CheckpointManager {
    pub fn new(store_root: impl Into<PathBuf>) -> Self {
        Self {
            store_root: store_root.into(),
        }
    }

    pub fn create(&self, workspace: &Path) -> Result<CheckpointManifest, CheckpointError> {
        let workspace = fs::canonicalize(workspace)
            .map_err(|_| CheckpointError::MissingWorkspace(workspace.display().to_string()))?;
        let uuid = new_uuid_v7();
        let checkpoint_root = self.store_root.join(uuid.to_string());
        let files_root = checkpoint_root.join("files");
        fs::create_dir_all(&files_root)?;
        let mut files = Vec::new();
        for entry in WalkDir::new(&workspace).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if !entry.file_type().is_file() || ignored(path, &workspace, &self.store_root) {
                continue;
            }
            let relative = path
                .strip_prefix(&workspace)
                .map_err(|_| CheckpointError::EscapesWorkspace(path.display().to_string()))?;
            let bytes = fs::read(path)?;
            let digest = hex_sha256(&bytes);
            let dest = files_root.join(relative);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&dest, &bytes)?;
            files.push(CheckpointFile {
                path: relative.to_string_lossy().replace('\\', "/"),
                sha256: digest,
                bytes: bytes.len() as u64,
            });
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let manifest = CheckpointManifest {
            uuid,
            workspace,
            files,
            created_at: Utc::now(),
        };
        fs::write(
            checkpoint_root.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest)?,
        )?;
        Ok(manifest)
    }

    pub fn load(&self, uuid: Uuid) -> Result<CheckpointManifest, CheckpointError> {
        Ok(serde_json::from_slice(&fs::read(
            self.store_root.join(uuid.to_string()).join("manifest.json"),
        )?)?)
    }

    pub fn verify(
        &self,
        manifest: &CheckpointManifest,
    ) -> Result<CheckpointVerifyReport, CheckpointError> {
        let mut changed = Vec::new();
        let mut missing = Vec::new();
        for file in &manifest.files {
            let path = confined_join(&manifest.workspace, &file.path)?;
            if !path.exists() {
                missing.push(file.path.clone());
                continue;
            }
            if hex_sha256(&fs::read(path)?) != file.sha256 {
                changed.push(file.path.clone());
            }
        }
        Ok(CheckpointVerifyReport {
            checkpoint_uuid: manifest.uuid,
            valid: changed.is_empty() && missing.is_empty(),
            changed,
            missing,
        })
    }

    pub fn restore_files(&self, manifest: &CheckpointManifest) -> Result<usize, CheckpointError> {
        let source_root = self
            .store_root
            .join(manifest.uuid.to_string())
            .join("files");
        let mut restored = 0usize;
        for file in &manifest.files {
            let source = confined_join(&source_root, &file.path)?;
            let dest = confined_join(&manifest.workspace, &file.path)?;
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(source, dest)?;
            restored += 1;
        }
        Ok(restored)
    }
}

fn ignored(path: &Path, workspace: &Path, store_root: &Path) -> bool {
    let rel = path.strip_prefix(workspace).ok();
    if let Some(rel) = rel {
        if rel.components().next().is_some_and(|c| {
            matches!(
                c.as_os_str().to_str(),
                Some(".git" | "target" | "node_modules" | "var")
            )
        }) {
            return true;
        }
    }
    path.starts_with(store_root)
}

fn confined_join(root: &Path, relative: &str) -> Result<PathBuf, CheckpointError> {
    let rel = Path::new(relative);
    if rel.is_absolute()
        || rel
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(CheckpointError::EscapesWorkspace(relative.into()));
    }
    Ok(root.join(rel))
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

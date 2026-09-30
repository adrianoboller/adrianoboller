use chrono::{DateTime, Utc};
use phxclaw_types::{is_uuid_v7, new_uuid_v7};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceMode {
    Online,
    Offline,
    Hybrid,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceFreshness {
    Stable,
    Current,
    Latest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEndpoint {
    pub name: String,
    pub url: String,
    pub authority: String,
    pub purpose: String,
    pub offline_relative_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeSource {
    pub uuid: Uuid,
    pub name: String,
    pub ecosystem: String,
    pub mode: SourceMode,
    pub authoritative: bool,
    pub license_note: String,
    pub local_root: String,
    pub sync_strategy: String,
    pub verified_version: Option<String>,
    pub allowed_agents: BTreeSet<String>,
    pub required_capabilities: BTreeSet<String>,
    pub endpoints: Vec<SourceEndpoint>,
    pub last_synced_at: Option<DateTime<Utc>>,
}

impl KnowledgeSource {
    pub fn can_read(&self, agent_name: &str, capabilities: &BTreeSet<String>) -> bool {
        self.allowed_agents.contains(agent_name)
            && self
                .required_capabilities
                .iter()
                .all(|cap| capabilities.contains(cap))
    }

    pub fn local_root_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(&self.local_root)
    }

    pub fn offline_index_path(&self, project_root: &Path) -> PathBuf {
        self.local_root_path(project_root)
            .join("index")
            .join("documents.jsonl")
    }

    pub fn snapshot_path(&self, project_root: &Path) -> PathBuf {
        self.local_root_path(project_root).join("snapshot.json")
    }
}

#[derive(Debug, Default, Clone)]
pub struct SourceRegistry {
    sources: BTreeMap<String, KnowledgeSource>,
}

impl SourceRegistry {
    pub fn load_dir(path: impl AsRef<Path>) -> Result<Self, SourceRegistryError> {
        let mut registry = Self::default();
        for entry in fs::read_dir(path)? {
            let path = entry?.path();
            if path.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let source: KnowledgeSource = serde_json::from_slice(&fs::read(&path)?)?;
            if !is_uuid_v7(&source.uuid) {
                return Err(SourceRegistryError::UuidVersion(source.name));
            }
            if registry.sources.contains_key(&source.name) {
                return Err(SourceRegistryError::Duplicate(source.name));
            }
            registry.sources.insert(source.name.clone(), source);
        }
        Ok(registry)
    }

    pub fn get(&self, name: &str) -> Option<&KnowledgeSource> {
        self.sources.get(name)
    }

    pub fn resolve_local_first(
        &self,
        name: &str,
        project_root: &Path,
        agent_name: &str,
        capabilities: &BTreeSet<String>,
    ) -> Result<ResolvedSource, SourceRegistryError> {
        self.resolve_for_query(
            name,
            project_root,
            agent_name,
            capabilities,
            SourceFreshness::Stable,
            "",
        )
    }

    pub fn resolve_for_query(
        &self,
        name: &str,
        project_root: &Path,
        agent_name: &str,
        capabilities: &BTreeSet<String>,
        freshness: SourceFreshness,
        query: &str,
    ) -> Result<ResolvedSource, SourceRegistryError> {
        let source = self
            .sources
            .get(name)
            .ok_or_else(|| SourceRegistryError::Unknown(name.into()))?;
        if !source.can_read(agent_name, capabilities) {
            return Err(SourceRegistryError::Denied {
                source_id: name.into(),
                agent: agent_name.into(),
            });
        }

        let index = source.offline_index_path(project_root);
        let offline_ready = index.is_file() && index.metadata()?.len() > 0;
        let prefer_online = matches!(
            freshness,
            SourceFreshness::Current | SourceFreshness::Latest
        );

        if !prefer_online
            && matches!(source.mode, SourceMode::Offline | SourceMode::Hybrid)
            && offline_ready
        {
            return Ok(ResolvedSource::Offline {
                root: source.local_root_path(project_root).join("html"),
                index,
                verified_version: source.verified_version.clone(),
            });
        }

        if matches!(source.mode, SourceMode::Online | SourceMode::Hybrid) {
            let endpoint = select_endpoint(source, query)
                .ok_or_else(|| SourceRegistryError::NoEndpoint(name.into()))?;
            return Ok(ResolvedSource::Online {
                name: endpoint.name.clone(),
                url: Url::parse(&endpoint.url)?,
                authority: endpoint.authority.clone(),
            });
        }

        if offline_ready {
            return Ok(ResolvedSource::Offline {
                root: source.local_root_path(project_root).join("html"),
                index,
                verified_version: source.verified_version.clone(),
            });
        }

        Err(SourceRegistryError::OfflineUnavailable(name.into()))
    }

    pub fn search_offline(
        &self,
        name: &str,
        project_root: &Path,
        agent_name: &str,
        capabilities: &BTreeSet<String>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<OfflineSearchHit>, SourceRegistryError> {
        let source = self
            .sources
            .get(name)
            .ok_or_else(|| SourceRegistryError::Unknown(name.into()))?;
        if !source.can_read(agent_name, capabilities) {
            return Err(SourceRegistryError::Denied {
                source_id: name.into(),
                agent: agent_name.into(),
            });
        }
        let index = source.offline_index_path(project_root);
        if !index.is_file() {
            return Err(SourceRegistryError::OfflineUnavailable(name.into()));
        }
        search_jsonl_index(name, &index, query, limit)
    }

    pub fn default_rust_source() -> KnowledgeSource {
        KnowledgeSource {
            uuid: new_uuid_v7(),
            name: "rust-official".into(),
            ecosystem: "rust".into(),
            mode: SourceMode::Hybrid,
            authoritative: true,
            license_note:
                "Respect each upstream Rust documentation license and preserve attribution.".into(),
            local_root: "knowledge/offline/rust".into(),
            sync_strategy: "rustup_component:rust-docs".into(),
            verified_version: None,
            allowed_agents: BTreeSet::from([
                "Research Agent".into(),
                "Research Lead / Pesquisador PDCA".into(),
                "Documentador".into(),
                "Perséfone".into(),
            ]),
            required_capabilities: BTreeSet::from(["knowledge.rust.read".into()]),
            endpoints: vec![],
            last_synced_at: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ResolvedSource {
    Offline {
        root: PathBuf,
        index: PathBuf,
        verified_version: Option<String>,
    },
    Online {
        name: String,
        url: Url,
        authority: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineDocumentRecord {
    pub path: String,
    pub title: String,
    pub sha256: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineSearchHit {
    pub source: String,
    pub uri: String,
    pub path: String,
    pub title: String,
    pub sha256: String,
    pub score: i64,
    pub excerpt: String,
}

pub fn search_jsonl_index(
    source_name: &str,
    index_path: &Path,
    query: &str,
    limit: usize,
) -> Result<Vec<OfflineSearchHit>, SourceRegistryError> {
    let query_terms = tokenize(query);
    if query_terms.is_empty() {
        return Ok(vec![]);
    }
    let phrase = query.trim().to_ascii_lowercase();
    let file = File::open(index_path)?;
    let mut hits = Vec::<OfflineSearchHit>::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record: OfflineDocumentRecord = serde_json::from_str(&line)?;
        let score = document_score(&record, &query_terms, &phrase);
        if score <= 0 {
            continue;
        }
        hits.push(OfflineSearchHit {
            source: source_name.to_string(),
            uri: format!(
                "phxclaw://knowledge/{}/{}",
                source_name,
                record.path.trim_start_matches('/')
            ),
            path: record.path.clone(),
            title: record.title.clone(),
            sha256: record.sha256.clone(),
            score,
            excerpt: make_excerpt(&record.text, &query_terms, 700),
        });
    }
    hits.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.path.cmp(&right.path))
    });
    hits.truncate(limit.max(1));
    Ok(hits)
}

fn select_endpoint<'a>(source: &'a KnowledgeSource, query: &str) -> Option<&'a SourceEndpoint> {
    let terms = tokenize(query);
    let mut endpoints = source.endpoints.iter().collect::<Vec<_>>();
    endpoints.sort_by(|left, right| {
        endpoint_score(right, &terms)
            .cmp(&endpoint_score(left, &terms))
            .then_with(|| left.name.cmp(&right.name))
    });
    endpoints.first().copied()
}

fn endpoint_score(endpoint: &SourceEndpoint, query_terms: &BTreeSet<String>) -> i64 {
    let name = endpoint.name.to_ascii_lowercase();
    let purpose = endpoint.purpose.to_ascii_lowercase();
    let url = endpoint.url.to_ascii_lowercase();
    let mut score = if endpoint.authority == "official" {
        10
    } else {
        5
    };
    for term in query_terms {
        if name.contains(term) {
            score += 8;
        }
        if purpose.contains(term) {
            score += 4;
        }
        if url.contains(term) {
            score += 2;
        }
    }
    score
}

fn document_score(
    record: &OfflineDocumentRecord,
    query_terms: &BTreeSet<String>,
    phrase: &str,
) -> i64 {
    let title = record.title.to_ascii_lowercase();
    let path = record.path.to_ascii_lowercase();
    let text = record.text.to_ascii_lowercase();
    let mut score = 0i64;
    if !phrase.is_empty() && text.contains(phrase) {
        score += 40;
    }
    for term in query_terms {
        if title.contains(term) {
            score += 12;
        }
        if path.contains(term) {
            score += 6;
        }
        score += (text.match_indices(term).count().min(24) as i64) * 2;
    }
    score
}

fn make_excerpt(text: &str, query_terms: &BTreeSet<String>, max_chars: usize) -> String {
    let lowered = text.to_ascii_lowercase();
    let first = query_terms
        .iter()
        .filter_map(|term| lowered.find(term))
        .min()
        .unwrap_or(0);
    let chars = text.char_indices().collect::<Vec<_>>();
    if chars.is_empty() {
        return String::new();
    }
    let center_char = chars
        .iter()
        .position(|(byte, _)| *byte >= first)
        .unwrap_or(0);
    let half = max_chars / 2;
    let start_char = center_char.saturating_sub(half);
    let end_char = (start_char + max_chars).min(chars.len());
    let start_byte = chars[start_char].0;
    let end_byte = if end_char >= chars.len() {
        text.len()
    } else {
        chars[end_char].0
    };
    let mut excerpt = text[start_byte..end_byte].trim().to_string();
    if start_byte > 0 {
        excerpt.insert_str(0, "…");
    }
    if end_byte < text.len() {
        excerpt.push('…');
    }
    excerpt
}

fn tokenize(value: &str) -> BTreeSet<String> {
    value
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|part| part.len() >= 2)
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

#[derive(Debug, Error)]
pub enum SourceRegistryError {
    #[error("unknown knowledge source {0}")]
    Unknown(String),
    #[error("duplicate knowledge source {0}")]
    Duplicate(String),
    #[error("knowledge source {source_id} denied for agent {agent}")]
    Denied { source_id: String, agent: String },
    #[error("offline copy unavailable for {0}")]
    OfflineUnavailable(String),
    #[error("knowledge source {0} has no endpoint")]
    NoEndpoint(String),
    #[error("knowledge source {0} does not use UUIDv7")]
    UuidVersion(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Url(#[from] url::ParseError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn offline_search_is_deterministic() {
        let root = std::env::temp_dir().join(format!("phoenix-source-{}", new_uuid_v7()));
        fs::create_dir_all(&root).unwrap();
        let index = root.join("documents.jsonl");
        let mut file = File::create(&index).unwrap();
        for record in [
            OfflineDocumentRecord {
                path: "book/ch09.html".into(),
                title: "Recoverable Errors with Result".into(),
                sha256: "a".repeat(64),
                text:
                    "Rust uses Result<T, E> and the question mark operator for recoverable errors."
                        .into(),
            },
            OfflineDocumentRecord {
                path: "std/string.html".into(),
                title: "String".into(),
                sha256: "b".repeat(64),
                text: "A UTF-8 encoded growable string.".into(),
            },
        ] {
            writeln!(file, "{}", serde_json::to_string(&record).unwrap()).unwrap();
        }
        let hits = search_jsonl_index("rust-official", &index, "Rust Result errors", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].path.contains("ch09"));
        let _ = fs::remove_dir_all(root);
    }
}

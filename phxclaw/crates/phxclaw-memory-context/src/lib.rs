use chrono::{DateTime, Utc};
use phxclaw_types::{EvidenceRef, new_uuid_v7};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DataClassification {
    Public,
    Internal,
    Confidential,
    Restricted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    Session(Uuid),
    Agent(Uuid),
    Project(String),
    Organization(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub uuid: Uuid,
    pub namespace: String,
    pub key: String,
    pub value: Value,
    pub scope: MemoryScope,
    pub classification: DataClassification,
    pub evidence: Vec<EvidenceRef>,
    pub confidence_millis: u16,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub sha256: String,
}

impl MemoryRecord {
    pub fn new(
        namespace: impl Into<String>,
        key: impl Into<String>,
        value: Value,
        scope: MemoryScope,
        classification: DataClassification,
        evidence: Vec<EvidenceRef>,
    ) -> Result<Self, MemoryError> {
        let now = Utc::now();
        let mut record = Self {
            uuid: new_uuid_v7(),
            namespace: namespace.into(),
            key: key.into(),
            value,
            scope,
            classification,
            evidence,
            confidence_millis: 1000,
            created_at: now,
            updated_at: now,
            expires_at: None,
            sha256: String::new(),
        };
        record.refresh_hash()?;
        Ok(record)
    }

    pub fn refresh_hash(&mut self) -> Result<(), MemoryError> {
        let mut clone = self.clone();
        clone.sha256.clear();
        let bytes = serde_json::to_vec(&clone)?;
        self.sha256 = format!("{:x}", Sha256::digest(bytes));
        Ok(())
    }
}

pub trait MemoryStore {
    fn put(&mut self, record: MemoryRecord) -> Result<(), MemoryError>;
    fn get(&self, namespace: &str, key: &str) -> Option<&MemoryRecord>;
    fn all(&self) -> Vec<&MemoryRecord>;
}

#[derive(Debug, Default)]
pub struct InMemoryStore {
    items: BTreeMap<(String, String), MemoryRecord>,
}

impl MemoryStore for InMemoryStore {
    fn put(&mut self, record: MemoryRecord) -> Result<(), MemoryError> {
        self.items
            .insert((record.namespace.clone(), record.key.clone()), record);
        Ok(())
    }

    fn get(&self, namespace: &str, key: &str) -> Option<&MemoryRecord> {
        self.items.get(&(namespace.to_string(), key.to_string()))
    }

    fn all(&self) -> Vec<&MemoryRecord> {
        self.items.values().collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPolicy {
    pub max_items: usize,
    pub max_bytes: usize,
    pub max_classification: DataClassification,
    pub include_namespaces: Vec<String>,
}

impl Default for ContextPolicy {
    fn default() -> Self {
        Self {
            max_items: 64,
            max_bytes: 256 * 1024,
            max_classification: DataClassification::Internal,
            include_namespaces: vec![],
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextScopeFilter {
    pub session_uuid: Option<Uuid>,
    pub agent_uuid: Option<Uuid>,
    pub project: Option<String>,
    pub organization: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextItemRef {
    pub memory_uuid: Uuid,
    pub namespace: String,
    pub key: String,
    pub reason: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPack {
    pub uuid: Uuid,
    pub correlation_uuid: Uuid,
    pub items: Vec<ContextItemRef>,
    pub bytes_estimate: usize,
    pub compiled_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMaterializedItem {
    pub memory_uuid: Uuid,
    pub namespace: String,
    pub key: String,
    pub value: Value,
    pub classification: DataClassification,
    pub sha256: String,
    pub relevance_score: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBundle {
    pub pack: ContextPack,
    pub query_sha256: String,
    pub items: Vec<ContextMaterializedItem>,
}

#[derive(Debug, Default)]
pub struct ContextCompiler;

impl ContextCompiler {
    pub fn compile<S: MemoryStore>(
        &self,
        store: &S,
        correlation_uuid: Uuid,
        policy: &ContextPolicy,
    ) -> Result<ContextPack, MemoryError> {
        self.compile_for_query(
            store,
            correlation_uuid,
            "",
            policy,
            &ContextScopeFilter::default(),
        )
        .map(|bundle| bundle.pack)
    }

    pub fn compile_for_query<S: MemoryStore>(
        &self,
        store: &S,
        correlation_uuid: Uuid,
        query: &str,
        policy: &ContextPolicy,
        scope_filter: &ContextScopeFilter,
    ) -> Result<ContextBundle, MemoryError> {
        let now = Utc::now();
        let query_terms = tokenize(query);
        let mut candidates = Vec::<(&MemoryRecord, i64, usize)>::new();

        for record in store.all() {
            if record.expires_at.is_some_and(|expires| expires <= now) {
                continue;
            }
            if record.classification > policy.max_classification {
                continue;
            }
            if !policy.include_namespaces.is_empty()
                && !policy
                    .include_namespaces
                    .iter()
                    .any(|namespace| namespace == &record.namespace)
            {
                continue;
            }
            if !scope_matches(&record.scope, scope_filter) {
                continue;
            }

            let serialized = serde_json::to_vec(&record.value)?;
            let score = relevance_score(record, &query_terms);
            candidates.push((record, score, serialized.len()));
        }

        candidates.sort_by(|(left, left_score, _), (right, right_score, _)| {
            right_score
                .cmp(left_score)
                .then_with(|| right.confidence_millis.cmp(&left.confidence_millis))
                .then_with(|| left.namespace.cmp(&right.namespace))
                .then_with(|| left.key.cmp(&right.key))
                .then_with(|| left.uuid.cmp(&right.uuid))
        });

        let mut refs = Vec::new();
        let mut materialized = Vec::new();
        let mut bytes = 0usize;
        for (record, score, estimate) in candidates {
            if refs.len() >= policy.max_items || bytes.saturating_add(estimate) > policy.max_bytes {
                continue;
            }
            bytes += estimate;
            let reason = if query_terms.is_empty() {
                "policy_match".to_string()
            } else {
                format!("query_relevance:{score}")
            };
            refs.push(ContextItemRef {
                memory_uuid: record.uuid,
                namespace: record.namespace.clone(),
                key: record.key.clone(),
                reason,
                sha256: record.sha256.clone(),
            });
            materialized.push(ContextMaterializedItem {
                memory_uuid: record.uuid,
                namespace: record.namespace.clone(),
                key: record.key.clone(),
                value: record.value.clone(),
                classification: record.classification,
                sha256: record.sha256.clone(),
                relevance_score: score,
            });
        }

        let query_sha256 = format!("{:x}", Sha256::digest(query.as_bytes()));
        Ok(ContextBundle {
            pack: ContextPack {
                uuid: new_uuid_v7(),
                correlation_uuid,
                items: refs,
                bytes_estimate: bytes,
                compiled_at: now,
            },
            query_sha256,
            items: materialized,
        })
    }
}

fn scope_matches(scope: &MemoryScope, filter: &ContextScopeFilter) -> bool {
    match scope {
        MemoryScope::Session(uuid) => filter
            .session_uuid
            .map_or(true, |candidate| candidate == *uuid),
        MemoryScope::Agent(uuid) => filter
            .agent_uuid
            .map_or(true, |candidate| candidate == *uuid),
        MemoryScope::Project(project) => filter
            .project
            .as_ref()
            .map_or(true, |candidate| candidate == project),
        MemoryScope::Organization(org) => filter
            .organization
            .as_ref()
            .map_or(true, |candidate| candidate == org),
    }
}

fn relevance_score(record: &MemoryRecord, query_terms: &BTreeSet<String>) -> i64 {
    if query_terms.is_empty() {
        return 0;
    }
    let namespace = record.namespace.to_ascii_lowercase();
    let key = record.key.to_ascii_lowercase();
    let value = serde_json::to_string(&record.value)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut score = 0i64;
    for term in query_terms {
        if namespace.contains(term) {
            score += 8;
        }
        if key.contains(term) {
            score += 12;
        }
        let occurrences = value.match_indices(term).count().min(16) as i64;
        score += occurrences * 2;
    }
    score + i64::from(record.confidence_millis / 100)
}

fn tokenize(value: &str) -> BTreeSet<String> {
    value
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|part| part.len() >= 2)
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_excludes_restricted_by_default() {
        let mut store = InMemoryStore::default();
        store
            .put(
                MemoryRecord::new(
                    "project",
                    "public",
                    serde_json::json!({"v": 1}),
                    MemoryScope::Project("phoenix".into()),
                    DataClassification::Internal,
                    vec![],
                )
                .unwrap(),
            )
            .unwrap();
        store
            .put(
                MemoryRecord::new(
                    "project",
                    "secret",
                    serde_json::json!({"v": 2}),
                    MemoryScope::Project("phoenix".into()),
                    DataClassification::Restricted,
                    vec![],
                )
                .unwrap(),
            )
            .unwrap();
        let pack = ContextCompiler
            .compile_for_query(
                &store,
                new_uuid_v7(),
                "public",
                &ContextPolicy::default(),
                &ContextScopeFilter {
                    project: Some("phoenix".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(pack.items.len(), 1);
    }

    #[test]
    fn query_ranks_matching_memory_first() {
        let mut store = InMemoryStore::default();
        for (key, value) in [
            (
                "rust",
                serde_json::json!({"note": "Use Result and the ? operator in Rust"}),
            ),
            ("other", serde_json::json!({"note": "CSS layout"})),
        ] {
            store
                .put(
                    MemoryRecord::new(
                        "project",
                        key,
                        value,
                        MemoryScope::Project("phoenix".into()),
                        DataClassification::Internal,
                        vec![],
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        let bundle = ContextCompiler
            .compile_for_query(
                &store,
                new_uuid_v7(),
                "Rust Result error handling",
                &ContextPolicy::default(),
                &ContextScopeFilter {
                    project: Some("phoenix".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(bundle.items[0].key, "rust");
    }
}

use chrono::{DateTime, Utc};
use phxclaw_types::new_uuid_v7;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceOutcome {
    Requested,
    Succeeded,
    Failed,
    Denied,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceDraft {
    pub action_uuid: Uuid,
    pub correlation_uuid: Option<Uuid>,
    pub actor: String,
    pub capability: String,
    pub action: String,
    pub outcome: EvidenceOutcome,
    pub request_summary: Value,
    pub result_summary: Value,
    pub artifact_uris: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub uuid: Uuid,
    pub action_uuid: Uuid,
    pub correlation_uuid: Option<Uuid>,
    pub actor: String,
    pub capability: String,
    pub action: String,
    pub outcome: EvidenceOutcome,
    pub request_summary: Value,
    pub result_summary: Value,
    pub artifact_uris: Vec<String>,
    pub occurred_at: DateTime<Utc>,
    pub previous_hash: Option<String>,
    pub record_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyReport {
    pub records: usize,
    pub valid: bool,
    pub first_invalid_line: Option<usize>,
    pub final_hash: Option<String>,
}

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ledger lock poisoned")]
    Poisoned,
    #[error("invalid record at line {0}")]
    InvalidRecord(usize),
}

#[derive(Clone)]
pub struct EvidenceLedger {
    path: PathBuf,
    last_hash: Arc<Mutex<Option<String>>>,
}

impl EvidenceLedger {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, LedgerError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if !path.exists() {
            File::create(&path)?;
        }
        let report = verify_path(&path)?;
        if !report.valid {
            return Err(LedgerError::InvalidRecord(report.first_invalid_line.unwrap_or(0)));
        }
        Ok(Self { path, last_hash: Arc::new(Mutex::new(report.final_hash)) })
    }

    pub fn path(&self) -> &Path { &self.path }

    pub fn append(&self, draft: EvidenceDraft) -> Result<EvidenceRecord, LedgerError> {
        let mut last_hash = self.last_hash.lock().map_err(|_| LedgerError::Poisoned)?;
        let occurred_at = Utc::now();
        let uuid = new_uuid_v7();
        let payload = canonical_payload(
            uuid,
            &draft,
            occurred_at,
            last_hash.clone(),
        );
        let record_hash = sha256_hex(&serde_json::to_vec(&payload)?);
        let record = EvidenceRecord {
            uuid,
            action_uuid: draft.action_uuid,
            correlation_uuid: draft.correlation_uuid,
            actor: draft.actor,
            capability: draft.capability,
            action: draft.action,
            outcome: draft.outcome,
            request_summary: canonical_value(draft.request_summary),
            result_summary: canonical_value(draft.result_summary),
            artifact_uris: draft.artifact_uris,
            occurred_at,
            previous_hash: last_hash.clone(),
            record_hash: record_hash.clone(),
        };

        let mut file = OpenOptions::new().create(true).append(true).open(&self.path)?;
        serde_json::to_writer(&mut file, &record)?;
        file.write_all(b"\n")?;
        file.flush()?;
        file.sync_data()?;
        *last_hash = Some(record_hash);
        Ok(record)
    }

    pub fn verify(&self) -> Result<VerifyReport, LedgerError> {
        verify_path(&self.path)
    }

    pub fn tail(&self, limit: usize) -> Result<Vec<EvidenceRecord>, LedgerError> {
        let file = File::open(&self.path)?;
        let mut records = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() { continue; }
            records.push(serde_json::from_str(&line)?);
            if records.len() > limit.max(1) {
                records.remove(0);
            }
        }
        Ok(records)
    }
}

fn verify_path(path: &Path) -> Result<VerifyReport, LedgerError> {
    let file = File::open(path)?;
    let mut previous: Option<String> = None;
    let mut count = 0usize;
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() { continue; }
        let record: EvidenceRecord = serde_json::from_str(&line)?;
        let draft = EvidenceDraft {
            action_uuid: record.action_uuid,
            correlation_uuid: record.correlation_uuid,
            actor: record.actor.clone(),
            capability: record.capability.clone(),
            action: record.action.clone(),
            outcome: record.outcome.clone(),
            request_summary: record.request_summary.clone(),
            result_summary: record.result_summary.clone(),
            artifact_uris: record.artifact_uris.clone(),
        };
        let payload = canonical_payload(record.uuid, &draft, record.occurred_at, previous.clone());
        let expected = sha256_hex(&serde_json::to_vec(&payload)?);
        if record.previous_hash != previous || record.record_hash != expected {
            return Ok(VerifyReport {
                records: count,
                valid: false,
                first_invalid_line: Some(index + 1),
                final_hash: previous,
            });
        }
        previous = Some(record.record_hash);
        count += 1;
    }
    Ok(VerifyReport { records: count, valid: true, first_invalid_line: None, final_hash: previous })
}

fn canonical_payload(
    uuid: Uuid,
    draft: &EvidenceDraft,
    occurred_at: DateTime<Utc>,
    previous_hash: Option<String>,
) -> Value {
    let mut map = BTreeMap::<String, Value>::new();
    map.insert("uuid".into(), Value::String(uuid.to_string()));
    map.insert("action_uuid".into(), Value::String(draft.action_uuid.to_string()));
    map.insert("correlation_uuid".into(), draft.correlation_uuid.map(|u| Value::String(u.to_string())).unwrap_or(Value::Null));
    map.insert("actor".into(), Value::String(draft.actor.clone()));
    map.insert("capability".into(), Value::String(draft.capability.clone()));
    map.insert("action".into(), Value::String(draft.action.clone()));
    map.insert("outcome".into(), serde_json::to_value(&draft.outcome).expect("serializable outcome"));
    map.insert("request_summary".into(), canonical_value(draft.request_summary.clone()));
    map.insert("result_summary".into(), canonical_value(draft.result_summary.clone()));
    map.insert("artifact_uris".into(), serde_json::to_value(&draft.artifact_uris).expect("serializable artifacts"));
    map.insert("occurred_at".into(), Value::String(occurred_at.to_rfc3339()));
    map.insert("previous_hash".into(), previous_hash.map(Value::String).unwrap_or(Value::Null));
    serde_json::to_value(map).expect("serializable canonical payload")
}

fn canonical_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted = map.into_iter()
                .map(|(k, v)| (k, canonical_value(v)))
                .collect::<BTreeMap<_, _>>();
            serde_json::to_value(sorted).expect("canonical object")
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonical_value).collect()),
        other => other,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn append_and_verify_chain() {
        let path = std::env::temp_dir().join(format!("phoenix-evidence-{}.jsonl", new_uuid_v7()));
        let ledger = EvidenceLedger::open(&path).unwrap();
        for n in 0..3 {
            ledger.append(EvidenceDraft {
                action_uuid: new_uuid_v7(),
                correlation_uuid: None,
                actor: "test".into(),
                capability: "test.action".into(),
                action: "test".into(),
                outcome: EvidenceOutcome::Succeeded,
                request_summary: json!({"n": n}),
                result_summary: json!({"ok": true}),
                artifact_uris: vec![],
            }).unwrap();
        }
        let report = ledger.verify().unwrap();
        assert!(report.valid);
        assert_eq!(report.records, 3);
        let _ = std::fs::remove_file(path);
    }
}

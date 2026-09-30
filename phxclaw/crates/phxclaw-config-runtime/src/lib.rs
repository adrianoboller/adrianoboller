use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid configuration: {0}")]
    Invalid(String),
    #[error("revision conflict: expected {expected}, actual {actual}")]
    RevisionConflict { expected: u64, actual: u64 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnifiedConfig {
    pub schema_version: String,
    pub config_uuid: Uuid,
    pub revision: u64,
    #[serde(flatten)]
    pub sections: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub revision: u64,
    pub sha256: String,
    pub value: Value,
}

pub fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(m) => {
            let mut sorted = BTreeMap::<String, Value>::new();
            for (k, v) in m {
                sorted.insert(k.clone(), canonicalize(v));
            }
            let mut out = Map::new();
            for (k, v) in sorted {
                out.insert(k, v);
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(canonicalize).collect()),
        _ => value.clone(),
    }
}

pub fn canonical_sha256(value: &Value) -> Result<String, ConfigError> {
    let bytes = serde_json::to_vec(&canonicalize(value))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn suspicious_key(k: &str) -> bool {
    let s = k.to_ascii_lowercase();
    [
        "password",
        "api_key",
        "apikey",
        "token",
        "private_key",
        "client_secret",
    ]
    .iter()
    .any(|x| s.contains(x))
}
fn allowed_secret_reference(k: &str) -> bool {
    let s = k.to_ascii_lowercase();
    s.ends_with("_secret_uuid") || s.ends_with("_secret_ref") || s == "password_secret_uuid"
}
fn scan_secrets(v: &Value, path: &str) -> Result<(), ConfigError> {
    match v {
        Value::Object(m) => {
            for (k, val) in m {
                let p = format!("{path}/{k}");
                if suspicious_key(k) && !allowed_secret_reference(k) {
                    if matches!(val, Value::String(s) if !s.is_empty()) {
                        return Err(ConfigError::Invalid(format!(
                            "plaintext secret-like value forbidden at {p}"
                        )));
                    }
                }
                if allowed_secret_reference(k) {
                    if let Value::String(s) = val {
                        Uuid::parse_str(s).map_err(|_| {
                            ConfigError::Invalid(format!("secret reference must be UUID at {p}"))
                        })?;
                    }
                }
                scan_secrets(val, &p)?;
            }
        }
        Value::Array(a) => {
            for (i, val) in a.iter().enumerate() {
                scan_secrets(val, &format!("{path}/{i}"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn validate(value: &Value) -> Result<UnifiedConfig, ConfigError> {
    scan_secrets(value, "")?;
    let cfg: UnifiedConfig = serde_json::from_value(value.clone())?;
    if cfg.schema_version != "0.40.0" {
        return Err(ConfigError::Invalid("schema_version must be 0.40.0".into()));
    }
    if cfg.revision == 0 {
        return Err(ConfigError::Invalid("revision must be >= 1".into()));
    }
    if cfg.config_uuid.get_version_num() != 7 {
        return Err(ConfigError::Invalid("config_uuid must be UUIDv7".into()));
    }
    let sec = cfg
        .sections
        .get("security")
        .ok_or_else(|| ConfigError::Invalid("security section missing".into()))?;
    if sec.get("deny_by_default") != Some(&Value::Bool(true))
        || sec.get("secrets_plaintext_forbidden") != Some(&Value::Bool(true))
    {
        return Err(ConfigError::Invalid(
            "security deny-by-default/plaintext-secret guards must remain enabled".into(),
        ));
    }
    Ok(cfg)
}

pub struct ConfigStore {
    path: PathBuf,
    history_dir: PathBuf,
}
impl ConfigStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let history_dir = path.parent().unwrap_or(Path::new(".")).join("history");
        Self { path, history_dir }
    }
    pub fn load(&self) -> Result<ConfigSnapshot, ConfigError> {
        let value: Value = serde_json::from_slice(&fs::read(&self.path)?)?;
        let cfg = validate(&value)?;
        Ok(ConfigSnapshot {
            revision: cfg.revision,
            sha256: canonical_sha256(&value)?,
            value,
        })
    }
    pub fn save(
        &self,
        mut next: Value,
        expected_revision: u64,
    ) -> Result<ConfigSnapshot, ConfigError> {
        let current = self.load()?;
        if current.revision != expected_revision {
            return Err(ConfigError::RevisionConflict {
                expected: expected_revision,
                actual: current.revision,
            });
        }
        let obj = next
            .as_object_mut()
            .ok_or_else(|| ConfigError::Invalid("root must be object".into()))?;
        obj.insert("revision".into(), Value::from(expected_revision + 1));
        validate(&next)?;
        fs::create_dir_all(&self.history_dir)?;
        fs::write(
            self.history_dir.join(format!(
                "revision-{:08}-{}.json",
                current.revision, current.sha256
            )),
            serde_json::to_vec_pretty(&current.value)?,
        )?;
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(&serde_json::to_vec_pretty(&next)?)?;
            f.write_all(b"\n")?;
            f.sync_all()?;
        }
        fs::rename(tmp, &self.path)?;
        self.load()
    }
}

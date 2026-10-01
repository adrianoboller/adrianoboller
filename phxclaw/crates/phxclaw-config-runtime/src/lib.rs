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

/// O `config.json` do agente: catalogo das chaves, carga com origem e geradores.
pub mod agente;

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
                if suspicious_key(k)
                    && !allowed_secret_reference(k)
                    && matches!(val, Value::String(s) if !s.is_empty())
                {
                    return Err(ConfigError::Invalid(format!(
                        "plaintext secret-like value forbidden at {p}"
                    )));
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
    pub fn save(&self, next: Value, expected_revision: u64) -> Result<ConfigSnapshot, ConfigError> {
        let current = self.load()?;
        gravar_revisado(
            &self.path,
            &self.history_dir,
            Some((current.revision, &current.value)),
            expected_revision,
            next,
            "revision",
            &|v| validate(v).map(|_| ()),
        )?;
        self.load()
    }
}

/// A gravacao com revisao, UMA so para o `ConfigStore` e para o `config.json` do agente
/// (`agente::carga`): conferir a revisao esperada, carimbar a seguinte, validar, guardar a
/// anterior no historico e trocar o arquivo por renomeacao. Duas copias desta sequencia
/// divergiriam justamente no ponto que importa -- uma esqueceria o `sync_all` ou o
/// historico, e o conflito de revisao deixaria de valer num dos caminhos.
///
/// `atual` e a revisao e o documento em disco (`None`: arquivo ainda nao existe, revisao 0).
/// Devolve a revisao gravada.
pub fn gravar_revisado(
    caminho: &Path,
    historico: &Path,
    atual: Option<(u64, &Value)>,
    esperada: u64,
    mut proximo: Value,
    campo_revisao: &str,
    validar: &dyn Fn(&Value) -> Result<(), ConfigError>,
) -> Result<u64, ConfigError> {
    let revisao_atual = atual.map(|(r, _)| r).unwrap_or(0);
    if revisao_atual != esperada {
        return Err(ConfigError::RevisionConflict {
            expected: esperada,
            actual: revisao_atual,
        });
    }
    let obj = proximo
        .as_object_mut()
        .ok_or_else(|| ConfigError::Invalid("root must be object".into()))?;
    let nova = esperada + 1;
    obj.insert(campo_revisao.into(), Value::from(nova));
    validar(&proximo)?;
    if let Some((r, valor)) = atual {
        fs::create_dir_all(historico)?;
        fs::write(
            historico.join(format!(
                "revision-{:08}-{}.json",
                r,
                canonical_sha256(valor)?
            )),
            serde_json::to_vec_pretty(valor)?,
        )?;
    }
    if let Some(pai) = caminho.parent() {
        fs::create_dir_all(pai)?;
    }
    let tmp = caminho.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&serde_json::to_vec_pretty(&proximo)?)?;
        f.write_all(b"\n")?;
        f.sync_all()?;
    }
    fs::rename(tmp, caminho)?;
    Ok(nova)
}

/// Distancia de edicao (Levenshtein), para sugerir o nome proximo de comando ou de chave
/// digitada errado. Uma so na base: a ajuda da CLI e a carga do `config.json` perguntam a
/// mesma coisa.
pub fn distancia(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut linha: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut ant = linha[0];
        linha[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let atual = linha[j + 1];
            linha[j + 1] = (ant + usize::from(ca != *cb))
                .min(linha[j] + 1)
                .min(atual + 1);
            ant = atual;
        }
    }
    linha[b.len()]
}

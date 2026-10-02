use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
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
    /// O token de revisao (SHA-256 do conteudo) que o cliente viu nao e o do disco.
    #[error("revision token conflict: actual {atual}")]
    ConflitoDeToken { atual: String },
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
        gravar_revisado(
            &self.path,
            &self.history_dir,
            "revision",
            &|v| validate(v).map(|_| ()),
            &mut |em_disco| {
                let atual = revisao_de(em_disco, "revision");
                if atual != expected_revision {
                    return Err(ConfigError::RevisionConflict {
                        expected: expected_revision,
                        actual: atual,
                    });
                }
                Ok(next.clone())
            },
        )?;
        self.load()
    }
}

/// A revisao carimbada num documento em disco (`None`: arquivo ausente, revisao 0).
pub fn revisao_de(doc: Option<&Value>, campo_revisao: &str) -> u64 {
    doc.and_then(|d| d.get(campo_revisao))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

/// A gravacao com revisao, UMA so para o `ConfigStore` e para o `config.json` do agente
/// (`agente::carga`): travar entre processos, ler o que esta em disco, montar o proximo
/// documento a partir DELE, carimbar a revisao seguinte, validar, guardar a anterior no
/// historico e trocar o arquivo por renomeacao. Duas copias desta sequencia divergiriam
/// justamente no ponto que importa -- uma esqueceria o `sync_all` ou o historico, e o
/// conflito de revisao deixaria de valer num dos caminhos.
///
/// `montar` roda COM a trava tomada e recebe o documento que esta no disco agora (`None`:
/// arquivo ainda nao existe). E nele que quem chama confere a revisao ou o token que o
/// cliente viu e aplica a mudanca: conferir fora da trava, contra o que o processo leu
/// antes, deixava a CLI e o servidor gravarem um por cima do outro -- e a conferencia que
/// existia aqui comparava a revisao lida com ela mesma. Devolve a revisao gravada.
pub fn gravar_revisado(
    caminho: &Path,
    historico: &Path,
    campo_revisao: &str,
    validar: &dyn Fn(&Value) -> Result<(), ConfigError>,
    montar: &mut dyn FnMut(Option<&Value>) -> Result<Value, ConfigError>,
) -> Result<u64, ConfigError> {
    let _trava = phxclaw_types::arquivo::travar(&phxclaw_types::arquivo::trava_de(caminho))?;
    let em_disco: Option<Value> = match fs::read(caminho) {
        Ok(b) => Some(serde_json::from_slice(&b)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let mut proximo = montar(em_disco.as_ref())?;
    let obj = proximo
        .as_object_mut()
        .ok_or_else(|| ConfigError::Invalid("root must be object".into()))?;
    let nova = revisao_de(em_disco.as_ref(), campo_revisao) + 1;
    obj.insert(campo_revisao.into(), Value::from(nova));
    validar(&proximo)?;
    if let Some(valor) = &em_disco {
        fs::create_dir_all(historico)?;
        fs::write(
            historico.join(format!(
                "revision-{:08}-{}.json",
                nova - 1,
                canonical_sha256(valor)?
            )),
            serde_json::to_vec_pretty(valor)?,
        )?;
    }
    let mut bytes = serde_json::to_vec_pretty(&proximo)?;
    bytes.push(b'\n');
    phxclaw_types::arquivo::gravar_atomico(caminho, &bytes)?;
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

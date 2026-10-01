use chrono::{DateTime, Utc};
use phxclaw_types::{EvidenceRef, new_uuid_v7};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillState {
    Candidate,
    Validated,
    Promoted,
    Disabled,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillStep {
    pub order: u32,
    pub instruction: String,
    #[serde(default)]
    pub capability: Option<String>,
    #[serde(default)]
    pub requires_human_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub uuid: Uuid,
    pub name: String,
    pub version: String,
    pub description: String,
    pub state: SkillState,
    pub required_capabilities: Vec<String>,
    pub steps: Vec<SkillStep>,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub knowledge_sources: Vec<String>,
    #[serde(default)]
    pub context_namespaces: Vec<String>,
    pub input_schema: Value,
    pub output_schema: Value,
    pub provenance: Vec<String>,
    pub evidence: Vec<EvidenceRef>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub sha256: String,
}

impl SkillManifest {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
        required_capabilities: Vec<String>,
        steps: Vec<SkillStep>,
    ) -> Result<Self, SkillError> {
        let name = name.into();
        let version = version.into();
        Version::parse(&version).map_err(|source| SkillError::InvalidVersion {
            version: version.clone(),
            source,
        })?;
        let now = Utc::now();
        let mut manifest = Self {
            uuid: new_uuid_v7(),
            name,
            version,
            description: description.into(),
            state: SkillState::Candidate,
            required_capabilities,
            steps,
            triggers: vec![],
            knowledge_sources: vec![],
            context_namespaces: vec![],
            input_schema: Value::Object(Default::default()),
            output_schema: Value::Object(Default::default()),
            provenance: vec![],
            evidence: vec![],
            created_at: now,
            updated_at: now,
            sha256: String::new(),
        };
        manifest.refresh_hash()?;
        Ok(manifest)
    }

    pub fn refresh_hash(&mut self) -> Result<(), SkillError> {
        self.sha256 = canonical_skill_hash(self)?;
        Ok(())
    }

    pub fn verify_hash(&self) -> Result<bool, SkillError> {
        Ok(self
            .sha256
            .eq_ignore_ascii_case(&canonical_skill_hash(self)?))
    }

    pub fn required_capability_set(&self) -> BTreeSet<String> {
        self.required_capabilities.iter().cloned().collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPromotionProof {
    pub uuid: Uuid,
    pub skill_uuid: Uuid,
    pub objective_proof_uuid: Uuid,
    pub qa_evidence: Vec<EvidenceRef>,
    pub approved_by: String,
    pub approved_at: DateTime<Utc>,
}

impl SkillPromotionProof {
    pub fn new(
        skill_uuid: Uuid,
        objective_proof_uuid: Uuid,
        qa_evidence: Vec<EvidenceRef>,
        approved_by: impl Into<String>,
    ) -> Self {
        Self {
            uuid: new_uuid_v7(),
            skill_uuid,
            objective_proof_uuid,
            qa_evidence,
            approved_by: approved_by.into(),
            approved_at: Utc::now(),
        }
    }
}

#[derive(Debug, Default)]
pub struct SkillRegistry {
    by_name: BTreeMap<String, Vec<SkillManifest>>,
}

impl SkillRegistry {
    pub fn register(&mut self, mut manifest: SkillManifest) -> Result<(), SkillError> {
        manifest.refresh_hash()?;
        let versions = self.by_name.entry(manifest.name.clone()).or_default();
        if versions.iter().any(|x| x.version == manifest.version) {
            return Err(SkillError::Duplicate {
                name: manifest.name,
                version: manifest.version,
            });
        }
        versions.push(manifest);
        versions.sort_by(|a, b| {
            let av = Version::parse(&a.version).expect("validated semver");
            let bv = Version::parse(&b.version).expect("validated semver");
            bv.cmp(&av)
        });
        Ok(())
    }

    pub fn resolve(
        &self,
        name: &str,
        requirement: &str,
        promoted_only: bool,
    ) -> Result<&SkillManifest, SkillError> {
        let req =
            VersionReq::parse(requirement).map_err(|source| SkillError::InvalidRequirement {
                requirement: requirement.to_string(),
                source,
            })?;
        self.by_name
            .get(name)
            .into_iter()
            .flatten()
            .filter(|skill| !promoted_only || skill.state == SkillState::Promoted)
            .find(|skill| {
                Version::parse(&skill.version)
                    .map(|v| req.matches(&v))
                    .unwrap_or(false)
            })
            .ok_or_else(|| SkillError::NotFound {
                name: name.to_string(),
                requirement: requirement.to_string(),
            })
    }

    pub fn promote(
        &mut self,
        skill_uuid: Uuid,
        proof: &SkillPromotionProof,
    ) -> Result<&SkillManifest, SkillError> {
        if proof.skill_uuid != skill_uuid || proof.qa_evidence.is_empty() {
            return Err(SkillError::PromotionProofRejected(skill_uuid));
        }
        for versions in self.by_name.values_mut() {
            if let Some(skill) = versions.iter_mut().find(|x| x.uuid == skill_uuid) {
                if !matches!(skill.state, SkillState::Validated | SkillState::Candidate) {
                    return Err(SkillError::InvalidState(skill.state));
                }
                skill.state = SkillState::Promoted;
                skill.updated_at = Utc::now();
                skill.evidence.extend(proof.qa_evidence.clone());
                skill.refresh_hash()?;
                return Ok(skill);
            }
        }
        Err(SkillError::UnknownUuid(skill_uuid))
    }

    pub fn validate(&mut self, skill_uuid: Uuid, evidence: EvidenceRef) -> Result<(), SkillError> {
        for versions in self.by_name.values_mut() {
            if let Some(skill) = versions.iter_mut().find(|x| x.uuid == skill_uuid) {
                if skill.state != SkillState::Candidate {
                    return Err(SkillError::InvalidState(skill.state));
                }
                skill.state = SkillState::Validated;
                skill.evidence.push(evidence);
                skill.updated_at = Utc::now();
                skill.refresh_hash()?;
                return Ok(());
            }
        }
        Err(SkillError::UnknownUuid(skill_uuid))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillIndexEntry {
    pub name: String,
    pub version: String,
    pub state: SkillState,
    pub path: String,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    #[serde(default)]
    pub knowledge_sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillIndex {
    pub version: String,
    pub skills: Vec<SkillIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResolutionPolicy {
    pub require_promoted: bool,
    pub allow_validated: bool,
}

impl Default for SkillResolutionPolicy {
    fn default() -> Self {
        Self {
            require_promoted: true,
            allow_validated: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResolutionRequest {
    pub query: String,
    #[serde(default)]
    pub preferred_name: Option<String>,
    #[serde(default)]
    pub version_requirement: Option<String>,
    #[serde(default)]
    pub knowledge_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResolution {
    pub skill: SkillManifest,
    pub score: i64,
    pub matched_triggers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LazySkillResolver {
    root: PathBuf,
    index: SkillIndex,
}

impl LazySkillResolver {
    pub fn load(index_path: impl AsRef<Path>) -> Result<Self, SkillError> {
        let index_path = index_path.as_ref();
        let index: SkillIndex = serde_json::from_slice(&fs::read(index_path)?)?;
        let root = index_path
            .parent()
            .ok_or_else(|| SkillError::InvalidIndexPath(index_path.display().to_string()))?
            .to_path_buf();
        Ok(Self { root, index })
    }

    pub fn entries(&self) -> &[SkillIndexEntry] {
        &self.index.skills
    }

    pub fn resolve(
        &self,
        request: &SkillResolutionRequest,
        agent_capabilities: &BTreeSet<String>,
        policy: &SkillResolutionPolicy,
    ) -> Result<SkillResolution, SkillError> {
        let version_req = if let Some(requirement) = &request.version_requirement {
            Some(VersionReq::parse(requirement).map_err(|source| {
                SkillError::InvalidRequirement {
                    requirement: requirement.clone(),
                    source,
                }
            })?)
        } else {
            None
        };
        let query_terms = tokenize(&request.query);
        let mut candidates = Vec::<(&SkillIndexEntry, i64, Vec<String>)>::new();

        for entry in &self.index.skills {
            if !state_allowed(entry.state, policy) {
                continue;
            }
            if request
                .preferred_name
                .as_ref()
                .is_some_and(|name| name != &entry.name)
            {
                continue;
            }
            if let Some(req) = &version_req {
                let version = Version::parse(&entry.version).map_err(|source| {
                    SkillError::InvalidVersion {
                        version: entry.version.clone(),
                        source,
                    }
                })?;
                if !req.matches(&version) {
                    continue;
                }
            }
            if !entry
                .required_capabilities
                .iter()
                .all(|capability| agent_capabilities.contains(capability))
            {
                continue;
            }
            if request.knowledge_source.as_ref().is_some_and(|source| {
                !entry.knowledge_sources.is_empty()
                    && !entry
                        .knowledge_sources
                        .iter()
                        .any(|candidate| candidate == source)
            }) {
                continue;
            }

            let mut score = if request.preferred_name.as_ref() == Some(&entry.name) {
                100
            } else {
                0
            };
            let mut matched = Vec::new();
            for trigger in &entry.triggers {
                let normalized = trigger.to_ascii_lowercase();
                if query_terms.contains(&normalized)
                    || request.query.to_ascii_lowercase().contains(&normalized)
                {
                    score += 10;
                    matched.push(trigger.clone());
                }
            }
            if request.knowledge_source.as_ref().is_some_and(|source| {
                entry
                    .knowledge_sources
                    .iter()
                    .any(|candidate| candidate == source)
            }) {
                score += 20;
            }
            candidates.push((entry, score, matched));
        }

        candidates.sort_by(|(left, left_score, _), (right, right_score, _)| {
            right_score
                .cmp(left_score)
                .then_with(|| {
                    let lv = Version::parse(&left.version).expect("validated semver");
                    let rv = Version::parse(&right.version).expect("validated semver");
                    rv.cmp(&lv)
                })
                .then_with(|| left.name.cmp(&right.name))
        });

        let (entry, score, matched_triggers) =
            candidates
                .first()
                .cloned()
                .ok_or_else(|| SkillError::NoResolution {
                    query: request.query.clone(),
                    preferred_name: request.preferred_name.clone(),
                })?;
        let manifest = self.load_manifest(entry)?;
        Ok(SkillResolution {
            skill: manifest,
            score,
            matched_triggers,
        })
    }

    fn load_manifest(&self, entry: &SkillIndexEntry) -> Result<SkillManifest, SkillError> {
        let root = self.root.canonicalize()?;
        let path = self.root.join(&entry.path).canonicalize()?;
        if !path.starts_with(&root) {
            return Err(SkillError::PathEscape(entry.path.clone()));
        }
        let manifest: SkillManifest = serde_json::from_slice(&fs::read(&path)?)?;
        if manifest.name != entry.name
            || manifest.version != entry.version
            || manifest.state != entry.state
        {
            return Err(SkillError::IndexMismatch(entry.name.clone()));
        }
        if !manifest.verify_hash()? {
            return Err(SkillError::HashMismatch(manifest.name));
        }
        Ok(manifest)
    }
}

/// Uma skill em texto: o `SKILL.md` com cabecalho `---` (nome, descricao) e o corpo em
/// Markdown. E o formato que o agente le da pasta de skills; o manifesto JSON acima e o do
/// registro com versao e promocao, outra pergunta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDoc {
    pub name: String,
    pub description: String,
    pub body: String,
}

/// Teto do `SKILL.md` lido: o corpo vai inteiro para o contexto do modelo.
pub const SKILL_DOC_MAX_BYTES: u64 = 64 * 1024;
/// Teto da descricao: ela vai para o prompt de TODA tarefa, uma linha por skill.
pub const SKILL_DESCRIPTION_MAX_CHARS: usize = 300;

/// Nome de skill: letras ASCII, digitos, `-` e `_`, ate 64. Sem ponto e sem barra, para
/// o nome nunca virar caminho -- nem `..`, nem `a/b`, nem absoluto.
pub fn validate_skill_name(name: &str) -> Result<(), SkillError> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'));
    if ok {
        Ok(())
    } else {
        Err(SkillError::InvalidName(name.chars().take(80).collect()))
    }
}

/// Le o texto de um `SKILL.md`. O cabecalho e `chave: valor` por linha entre duas linhas
/// `---`; so `name` e `description` sao obrigatorias, o resto se ignora.
pub fn parse_skill_doc(text: &str) -> Result<SkillDoc, SkillError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    // `split_inclusive` e nao `lines`: o tamanho de cada linha conta o `\r\n` inteiro, e o
    // corpo comeca no byte certo tambem em arquivo salvo no Windows.
    let mut linhas = text.split_inclusive('\n');
    if linhas.next().map(str::trim_end) != Some("---") {
        return Err(SkillError::InvalidDoc("missing --- header".into()));
    }
    let mut name = None;
    let mut description = None;
    let mut fechou = false;
    let mut consumido = text.find('\n').map_or(text.len(), |i| i + 1);
    for linha in linhas.by_ref() {
        consumido += linha.len();
        let linha = linha.trim_end();
        if linha.trim_end() == "---" {
            fechou = true;
            break;
        }
        if let Some((k, v)) = linha.split_once(':') {
            let v = v.trim().trim_matches(|c| c == '"' || c == '\'').to_string();
            match k.trim() {
                "name" => name = Some(v),
                "description" => description = Some(v),
                _ => {}
            }
        }
    }
    if !fechou {
        return Err(SkillError::InvalidDoc("unterminated --- header".into()));
    }
    let name = name.ok_or_else(|| SkillError::InvalidDoc("missing name".into()))?;
    validate_skill_name(&name)?;
    let description =
        description.ok_or_else(|| SkillError::InvalidDoc("missing description".into()))?;
    if description.is_empty() || description.chars().count() > SKILL_DESCRIPTION_MAX_CHARS {
        return Err(SkillError::InvalidDoc(format!(
            "description must have 1 to {SKILL_DESCRIPTION_MAX_CHARS} characters"
        )));
    }
    let body = text
        .get(consumido.min(text.len())..)
        .unwrap_or("")
        .trim()
        .to_string();
    Ok(SkillDoc {
        name,
        description,
        body,
    })
}

/// A pasta de skills: `<raiz>/<nome>/SKILL.md`, uma pasta por skill.
#[derive(Debug, Clone)]
pub struct SkillFolder {
    root: PathBuf,
}

impl SkillFolder {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Le uma skill pelo nome. O nome se valida ANTES de virar caminho, e o caminho
    /// canonico tem de continuar dentro da raiz: um symlink na pasta nao leva para fora.
    /// A skill cujo cabecalho diz outro nome que a pasta e recusada, para o nome do
    /// prompt e o nome do pedido serem sempre o mesmo.
    pub fn load(&self, name: &str) -> Result<SkillDoc, SkillError> {
        validate_skill_name(name)?;
        let raiz = self.root.canonicalize()?;
        let caminho = self.root.join(name).join("SKILL.md").canonicalize()?;
        if !caminho.starts_with(&raiz) {
            return Err(SkillError::PathEscape(name.to_string()));
        }
        let tamanho = fs::metadata(&caminho)?.len();
        if tamanho > SKILL_DOC_MAX_BYTES {
            return Err(SkillError::InvalidDoc(format!(
                "SKILL.md has {tamanho} bytes, above {SKILL_DOC_MAX_BYTES}"
            )));
        }
        let doc = parse_skill_doc(&fs::read_to_string(&caminho)?)?;
        if doc.name != name {
            return Err(SkillError::IndexMismatch(name.to_string()));
        }
        Ok(doc)
    }

    /// Todas as skills validas, por nome. A invalida fica de fora e volta em `rejected`
    /// com o motivo: uma skill quebrada nao derruba a lista das outras, e nao some calada.
    pub fn scan(&self) -> SkillScan {
        let mut scan = SkillScan::default();
        let Ok(entradas) = fs::read_dir(&self.root) else {
            return scan;
        };
        let mut nomes: Vec<String> = entradas
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        nomes.sort();
        for nome in nomes {
            match self.load(&nome) {
                Ok(doc) => scan.skills.push(doc),
                Err(e) => scan.rejected.push((nome, e.to_string())),
            }
        }
        scan
    }
}

#[derive(Debug, Default)]
pub struct SkillScan {
    pub skills: Vec<SkillDoc>,
    pub rejected: Vec<(String, String)>,
}

fn canonical_skill_hash(manifest: &SkillManifest) -> Result<String, SkillError> {
    let mut clone = manifest.clone();
    clone.sha256.clear();
    let canonical = canonical_value(serde_json::to_value(&clone)?);
    let bytes = serde_json::to_vec(&canonical)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn canonical_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted = map
                .into_iter()
                .map(|(key, value)| (key, canonical_value(value)))
                .collect::<BTreeMap<_, _>>();
            serde_json::to_value(sorted).expect("canonical skill object")
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonical_value).collect()),
        other => other,
    }
}

fn state_allowed(state: SkillState, policy: &SkillResolutionPolicy) -> bool {
    if policy.require_promoted {
        state == SkillState::Promoted
    } else {
        state == SkillState::Promoted || (policy.allow_validated && state == SkillState::Validated)
    }
}

fn tokenize(value: &str) -> BTreeSet<String> {
    value
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|part| part.len() >= 2)
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

#[derive(Debug, Error)]
pub enum SkillError {
    #[error("invalid skill version {version}: {source}")]
    InvalidVersion {
        version: String,
        source: semver::Error,
    },
    #[error("invalid version requirement {requirement}: {source}")]
    InvalidRequirement {
        requirement: String,
        source: semver::Error,
    },
    #[error("duplicate skill {name}@{version}")]
    Duplicate { name: String, version: String },
    #[error("skill {name} matching {requirement} was not found")]
    NotFound { name: String, requirement: String },
    #[error("unknown skill uuid {0}")]
    UnknownUuid(Uuid),
    #[error("skill is in invalid state {0:?}")]
    InvalidState(SkillState),
    #[error("promotion proof rejected for skill {0}")]
    PromotionProofRejected(Uuid),
    #[error("no skill resolution for query {query} preferred={preferred_name:?}")]
    NoResolution {
        query: String,
        preferred_name: Option<String>,
    },
    #[error("skill index path is invalid: {0}")]
    InvalidIndexPath(String),
    #[error("skill path escaped skill root: {0}")]
    PathEscape(String),
    #[error("skill index does not match manifest: {0}")]
    IndexMismatch(String),
    #[error("skill manifest hash mismatch: {0}")]
    HashMismatch(String),
    #[error("invalid skill name: {0:?}")]
    InvalidName(String),
    #[error("invalid SKILL.md: {0}")]
    InvalidDoc(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_highest_matching_version() {
        let mut registry = SkillRegistry::default();
        let a = SkillManifest::new("rust.lookup", "1.0.0", "lookup", vec![], vec![]).unwrap();
        let b = SkillManifest::new("rust.lookup", "1.2.0", "lookup", vec![], vec![]).unwrap();
        registry.register(a).unwrap();
        registry.register(b).unwrap();
        let found = registry.resolve("rust.lookup", "^1", false).unwrap();
        assert_eq!(found.version, "1.2.0");
    }

    #[test]
    fn production_policy_rejects_validated_skill() {
        assert!(!state_allowed(
            SkillState::Validated,
            &SkillResolutionPolicy::default()
        ));
    }
    fn pasta() -> PathBuf {
        let d = std::env::temp_dir().join(format!("phx-skills-{}", new_uuid_v7().simple()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn grava(raiz: &Path, pasta: &str, texto: &str) {
        fs::create_dir_all(raiz.join(pasta)).unwrap();
        fs::write(raiz.join(pasta).join("SKILL.md"), texto).unwrap();
    }

    #[test]
    fn skill_md_le_cabecalho_e_corpo() {
        let d = parse_skill_doc(
            "---\nname: relatorio-pdf\ndescription: \"Gera relatorio\"\nversion: 1\n---\n\n# Passos\n1. a\n",
        )
        .unwrap();
        assert_eq!(d.name, "relatorio-pdf");
        assert_eq!(d.description, "Gera relatorio");
        assert_eq!(d.body, "# Passos\n1. a");
        let w = parse_skill_doc("---\r\nname: w\r\ndescription: W\r\n---\r\ncorpo\r\n").unwrap();
        assert_eq!((w.description.as_str(), w.body.as_str()), ("W", "corpo"));
        assert!(parse_skill_doc("sem cabecalho").is_err());
        assert!(parse_skill_doc("---\nname: a\n").is_err());
        assert!(
            parse_skill_doc("---\nname: a\n---\ncorpo").is_err(),
            "sem descricao"
        );
    }

    #[test]
    fn nome_com_caminho_e_recusado_antes_de_tocar_o_disco() {
        for hostil in ["../x", "a/b", "/etc", "..", ".", "a.b", "", "a b"] {
            assert!(
                matches!(validate_skill_name(hostil), Err(SkillError::InvalidName(_))),
                "{hostil:?} passou"
            );
        }
        assert!(validate_skill_name("relatorio_pdf-2").is_ok());
        let raiz = pasta();
        let fora = raiz.with_extension("fora");
        grava(
            &fora,
            "segredo",
            "---\nname: segredo\ndescription: x\n---\nvazou",
        );
        let f = SkillFolder::new(raiz.join("skills"));
        fs::create_dir_all(f.root()).unwrap();
        assert!(matches!(
            f.load("../../segredo"),
            Err(SkillError::InvalidName(_))
        ));
        let _ = fs::remove_dir_all(raiz);
        let _ = fs::remove_dir_all(fora);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_para_fora_da_pasta_e_recusado() {
        let raiz = pasta();
        let fora = pasta();
        grava(&fora, "x", "---\nname: fuga\ndescription: x\n---\nvazou");
        std::os::unix::fs::symlink(fora.join("x"), raiz.join("fuga")).unwrap();
        let e = SkillFolder::new(&raiz).load("fuga").unwrap_err();
        assert!(matches!(e, SkillError::PathEscape(_)), "{e}");
        let _ = fs::remove_dir_all(raiz);
        let _ = fs::remove_dir_all(fora);
    }

    #[test]
    fn varredura_lista_as_validas_e_diz_quais_recusou() {
        let raiz = pasta();
        grava(
            &raiz,
            "b-skill",
            "---\nname: b-skill\ndescription: B\n---\ncorpo b",
        );
        grava(
            &raiz,
            "a-skill",
            "---\nname: a-skill\ndescription: A\n---\ncorpo a",
        );
        grava(&raiz, "troca", "---\nname: outro\ndescription: T\n---\nx");
        grava(
            &raiz,
            "com.ponto",
            "---\nname: com.ponto\ndescription: P\n---\nx",
        );
        let scan = SkillFolder::new(&raiz).scan();
        let nomes: Vec<_> = scan.skills.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(nomes, ["a-skill", "b-skill"]);
        let recusadas: Vec<_> = scan.rejected.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(recusadas, ["com.ponto", "troca"]);
        assert!(
            SkillFolder::new(raiz.join("nao-existe"))
                .scan()
                .skills
                .is_empty()
        );
        let _ = fs::remove_dir_all(raiz);
    }
}

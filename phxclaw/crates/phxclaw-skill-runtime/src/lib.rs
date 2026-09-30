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
}

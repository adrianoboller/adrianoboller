use phxclaw_types::{PermissionClaim, PluginPermission, is_uuid_v7};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentDependencyRef {
    pub uuid: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSourceRef {
    pub workbook: String,
    pub sheet: String,
    pub row: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    pub manifest_version: String,
    pub uuid: Uuid,
    pub agent_id: u32,
    pub name: String,
    #[serde(default)]
    pub macroarea: String,
    #[serde(default)]
    pub nucleus: String,
    #[serde(default)]
    pub role_type: String,
    pub mission: String,
    #[serde(default)]
    pub responsibilities: String,
    #[serde(default)]
    pub resources: String,
    #[serde(default)]
    pub authorized_actions: String,
    #[serde(default)]
    pub trigger: String,
    #[serde(default)]
    pub declared_dependencies: String,
    #[serde(default)]
    pub resolved_agent_dependencies: Vec<AgentDependencyRef>,
    #[serde(default)]
    pub deliverables: String,
    #[serde(default)]
    pub limits: String,
    #[serde(default)]
    pub next_gate: String,
    #[serde(default)]
    pub execution: String,
    #[serde(default)]
    pub models_allowed: Vec<String>,
    #[serde(default)]
    pub criticality: String,
    #[serde(default)]
    pub modules: Vec<String>,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    #[serde(default)]
    pub lifecycle: Vec<String>,
    #[serde(default)]
    pub memory_skill_state: String,
    #[serde(default)]
    pub event_topics: String,
    pub execution_policy: String,
    #[serde(default)]
    pub references: String,
    #[serde(default)]
    pub knowledge_sources: Vec<String>,
    pub source: AgentSourceRef,
}

impl AgentManifest {
    pub fn capability_set(&self) -> BTreeSet<String> {
        self.capabilities.iter().cloned().collect()
    }

    pub fn provides(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|item| item == capability)
    }

    pub fn can_use_source(&self, source: &str) -> bool {
        self.knowledge_sources.iter().any(|item| item == source)
    }

    pub fn authorize(&self, claims: &[PermissionClaim]) -> Result<(), AgentCatalogError> {
        for claim in claims {
            let allowed = self.permissions.iter().any(|permission| {
                permission.name == claim.name
                    && permission
                        .scopes
                        .iter()
                        .any(|scope| scope_matches(scope, &claim.scope))
            });
            if !allowed {
                return Err(AgentCatalogError::PermissionDenied {
                    agent: self.name.clone(),
                    permission: claim.name.clone(),
                    scope: claim.scope.clone(),
                });
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), AgentCatalogError> {
        if self.manifest_version != "1.0.0" {
            return Err(AgentCatalogError::ManifestVersion {
                agent: self.name.clone(),
                version: self.manifest_version.clone(),
            });
        }
        if !is_uuid_v7(&self.uuid) {
            return Err(AgentCatalogError::UuidVersion(self.name.clone()));
        }
        if self.name.trim().is_empty() || self.mission.trim().is_empty() {
            return Err(AgentCatalogError::InvalidManifest(self.name.clone()));
        }
        if self.capabilities.is_empty() {
            return Err(AgentCatalogError::NoCapabilities(self.name.clone()));
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct AgentCatalog {
    by_uuid: BTreeMap<Uuid, AgentManifest>,
    by_name: BTreeMap<String, Uuid>,
}

impl AgentCatalog {
    pub fn load_dir(path: impl AsRef<Path>) -> Result<Self, AgentCatalogError> {
        let mut catalog = Self::default();
        for entry in fs::read_dir(path)? {
            let path = entry?.path();
            if !path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.ends_with(".agent.json"))
            {
                continue;
            }
            let manifest: AgentManifest = serde_json::from_slice(&fs::read(&path)?)?;
            catalog.register(manifest)?;
        }
        Ok(catalog)
    }

    pub fn register(&mut self, manifest: AgentManifest) -> Result<(), AgentCatalogError> {
        manifest.validate()?;
        if self.by_uuid.contains_key(&manifest.uuid) {
            return Err(AgentCatalogError::DuplicateUuid(manifest.uuid));
        }
        if self.by_name.contains_key(&manifest.name) {
            return Err(AgentCatalogError::DuplicateName(manifest.name));
        }
        self.by_name.insert(manifest.name.clone(), manifest.uuid);
        self.by_uuid.insert(manifest.uuid, manifest);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.by_uuid.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_uuid.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Uuid, &AgentManifest)> {
        self.by_uuid.iter()
    }

    pub fn get(&self, uuid: &Uuid) -> Option<&AgentManifest> {
        self.by_uuid.get(uuid)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&AgentManifest> {
        self.by_name
            .get(name)
            .and_then(|uuid| self.by_uuid.get(uuid))
    }

    pub fn candidates_for_capability(&self, capability: &str) -> Vec<&AgentManifest> {
        let mut candidates = self
            .by_uuid
            .values()
            .filter(|agent| agent.provides(capability))
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            criticality_rank(&right.criticality)
                .cmp(&criticality_rank(&left.criticality))
                .then_with(|| left.agent_id.cmp(&right.agent_id))
                .then_with(|| left.uuid.cmp(&right.uuid))
        });
        candidates
    }
}

fn criticality_rank(value: &str) -> u8 {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.contains("crít") || normalized.contains("crit") {
        3
    } else if normalized.contains("alta") || normalized.contains("high") {
        2
    } else if normalized.contains("média")
        || normalized.contains("media")
        || normalized.contains("medium")
    {
        1
    } else {
        0
    }
}

fn scope_matches(granted: &str, requested: &str) -> bool {
    if granted == "*" || granted == requested {
        return true;
    }
    granted
        .strip_suffix("/*")
        .is_some_and(|prefix| requested.starts_with(&format!("{prefix}/")))
}

#[derive(Debug, Error)]
pub enum AgentCatalogError {
    #[error("agent {agent} uses unsupported manifest version {version}")]
    ManifestVersion { agent: String, version: String },
    #[error("agent {0} does not use UUIDv7")]
    UuidVersion(String),
    #[error("agent manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("agent {0} declares no capabilities")]
    NoCapabilities(String),
    #[error("duplicate agent UUID {0}")]
    DuplicateUuid(Uuid),
    #[error("duplicate agent name {0}")]
    DuplicateName(String),
    #[error("permission denied for agent {agent}: {permission}:{scope}")]
    PermissionDenied {
        agent: String,
        permission: String,
        scope: String,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_official_agent_catalog() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/agents");
        let catalog = AgentCatalog::load_dir(root).unwrap();
        let indice: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../config/agents/registry.index.json"),
            )
            .unwrap(),
        )
        .unwrap();
        // O total vem do indice, que lista arquivo por arquivo; o catalogo tem de bater com ele.
        assert_eq!(catalog.len() as u64, indice["count"].as_u64().unwrap());
        assert_eq!(indice["agents"].as_array().unwrap().len(), catalog.len());
        assert!(catalog.get_by_name("Integrador").is_some());
        let research = catalog.get_by_name("Research Agent").unwrap();
        assert!(research.provides("research.collect"));
        assert!(research.can_use_source("rust-official"));
    }
}

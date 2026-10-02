use chrono::{DateTime, Utc};
use phxclaw_types::is_uuid_v7;
pub use semver::Version;
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TrustTier {
    Builtin,
    Verified,
    Community,
    LocalDevelopment,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublisherIdentity {
    pub id: String,
    pub display_name: String,
    pub website: Option<String>,
    pub repository: Option<String>,
    pub signer: String,
    pub trust_tier: TrustTier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommunityPluginRelease {
    pub plugin_uuid: Uuid,
    pub name: String,
    pub version: String,
    pub core_api: String,
    pub publisher_id: String,
    pub license: String,
    pub source_repository: String,
    pub documentation_url: Option<String>,
    pub package_url: String,
    pub package_sha256: String,
    pub manifest_sha256: String,
    pub signature_algorithm: String,
    pub signature: String,
    pub signer: String,
    pub provenance: String,
    pub categories: Vec<String>,
    pub capabilities: Vec<String>,
    pub extension_points: Vec<String>,
    pub published_at: DateTime<Utc>,
    pub yanked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityRegistryIndex {
    pub registry_version: String,
    pub generated_at: DateTime<Utc>,
    pub publishers: Vec<PublisherIdentity>,
    pub releases: Vec<CommunityPluginRelease>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryValidationReport {
    pub publishers: usize,
    pub releases: usize,
    pub valid: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Error)]
pub enum CommunityRegistryError {
    #[error("registry JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("registry validation failed: {0}")]
    Invalid(String),
}

impl CommunityRegistryIndex {
    pub fn from_json(input: &str) -> Result<Self, CommunityRegistryError> {
        Ok(serde_json::from_str(input)?)
    }

    pub fn validate(&self) -> RegistryValidationReport {
        let mut errors = Vec::new();
        if Version::parse(&self.registry_version).is_err() {
            errors.push(format!(
                "invalid registry_version: {}",
                self.registry_version
            ));
        }

        let mut publishers = BTreeMap::new();
        for publisher in &self.publishers {
            if publisher.id.trim().is_empty() {
                errors.push("publisher id cannot be empty".into());
                continue;
            }
            if publisher.signer.trim().is_empty() {
                errors.push(format!("publisher {} is missing signer", publisher.id));
            }
            if let Some(url) = &publisher.website
                && Url::parse(url).is_err()
            {
                errors.push(format!("publisher {} has invalid website", publisher.id));
            }
            if let Some(url) = &publisher.repository
                && Url::parse(url).is_err()
            {
                errors.push(format!("publisher {} has invalid repository", publisher.id));
            }
            if publishers.insert(publisher.id.clone(), publisher).is_some() {
                errors.push(format!("duplicate publisher id: {}", publisher.id));
            }
        }

        let mut release_keys = BTreeSet::new();
        let mut name_versions = BTreeSet::new();
        for release in &self.releases {
            if !is_uuid_v7(&release.plugin_uuid) {
                errors.push(format!("{} uses a non-UUIDv7 plugin id", release.name));
            }
            if Version::parse(&release.version).is_err() {
                errors.push(format!(
                    "{} has invalid semver {}",
                    release.name, release.version
                ));
            }
            if VersionReq::parse(&release.core_api).is_err() {
                errors.push(format!(
                    "{} has invalid core_api {}",
                    release.name, release.core_api
                ));
            }
            if !publishers.contains_key(&release.publisher_id) {
                errors.push(format!(
                    "{} references unknown publisher {}",
                    release.name, release.publisher_id
                ));
            }
            if release.license.trim().is_empty() {
                errors.push(format!("{} is missing license metadata", release.name));
            }
            for (label, value) in [
                ("package_sha256", release.package_sha256.as_str()),
                ("manifest_sha256", release.manifest_sha256.as_str()),
            ] {
                if value.len() != 64 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
                    errors.push(format!("{} has invalid {label}", release.name));
                }
            }
            if release.signature_algorithm != "ed25519" || release.signature.trim().is_empty() {
                errors.push(format!(
                    "{} must be published with an Ed25519 signature",
                    release.name
                ));
            }
            if release.signer.trim().is_empty() || release.provenance.trim().is_empty() {
                errors.push(format!("{} is missing signer/provenance", release.name));
            }
            if release.capabilities.is_empty() {
                errors.push(format!("{} declares no capabilities", release.name));
            }
            if Url::parse(&release.source_repository).is_err() {
                errors.push(format!("{} has invalid source_repository", release.name));
            }
            if let Ok(url) = Url::parse(&release.package_url) {
                if url.scheme() != "https"
                    && publishers
                        .get(&release.publisher_id)
                        .is_some_and(|p| p.trust_tier != TrustTier::LocalDevelopment)
                {
                    errors.push(format!("{} package_url must use https", release.name));
                }
            } else {
                errors.push(format!("{} has invalid package_url", release.name));
            }
            if !release_keys.insert((release.plugin_uuid, release.version.clone())) {
                errors.push(format!(
                    "duplicate release {} {}",
                    release.plugin_uuid, release.version
                ));
            }
            if !name_versions.insert((release.name.clone(), release.version.clone())) {
                errors.push(format!(
                    "duplicate name/version {} {}",
                    release.name, release.version
                ));
            }
        }

        RegistryValidationReport {
            publishers: self.publishers.len(),
            releases: self.releases.len(),
            valid: errors.is_empty(),
            errors,
        }
    }

    pub fn compatible_releases<'a>(
        &'a self,
        core_version: &Version,
    ) -> Vec<&'a CommunityPluginRelease> {
        let mut releases = self
            .releases
            .iter()
            .filter(|release| !release.yanked)
            .filter(|release| {
                VersionReq::parse(&release.core_api).is_ok_and(|req| req.matches(core_version))
            })
            .collect::<Vec<_>>();
        releases.sort_by(|left, right| {
            left.name.cmp(&right.name).then_with(|| {
                Version::parse(&right.version)
                    .ok()
                    .cmp(&Version::parse(&left.version).ok())
            })
        });
        releases
    }
}

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
pub mod assinatura;

use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use phxclaw_types::{PluginManifest, PluginState, SandboxNetworkMode, is_uuid_v7};
use postgres::Client;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedSigner {
    pub id: String,
    pub algorithm: String,
    pub public_key_base64: String,
    pub status: String,
    pub allowed_name_prefixes: Vec<String>,
    /// 1 = a assinatura cobre uuid, nome, versao e o hash do artefato (legado); 2 = cobre o
    /// manifesto INTEIRO (permissoes, sandbox, entrypoint, dependencias). Por signatario:
    /// quem e V2 nunca aceita V1, senao uma assinatura V1 antiga deixaria trocar permissoes.
    #[serde(default = "formato_legado")]
    pub signature_format: u8,
}

fn formato_legado() -> u8 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustStore {
    pub version: String,
    pub signers: Vec<TrustedSigner>,
}

impl TrustStore {
    pub fn from_json(input: &str) -> Result<Self, RegistryError> {
        Ok(serde_json::from_str(input)?)
    }

    pub fn signer(&self, id: &str) -> Option<&TrustedSigner> {
        self.signers
            .iter()
            .find(|signer| signer.id == id && signer.status == "active")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineRecord {
    pub plugin_uuid: Option<Uuid>,
    pub plugin_name: Option<String>,
    pub source: String,
    pub reason: String,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryReport {
    pub accepted: usize,
    pub quarantined: usize,
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("manifest JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("PostgreSQL error: {0}")]
    Postgres(#[from] postgres::Error),
    #[error("invalid plugin {plugin}: {reason}")]
    Invalid { plugin: String, reason: String },
    #[error("duplicate plugin UUID: {0}")]
    Duplicate(Uuid),
    #[error("duplicate plugin name: {0}")]
    DuplicateName(String),
    #[error("missing required dependency {dependency} for plugin {plugin}")]
    MissingDependency { plugin: String, dependency: Uuid },
    #[error("dependency version mismatch: plugin {plugin} requires {requirement}, found {found}")]
    DependencyVersion {
        plugin: String,
        requirement: String,
        found: String,
    },
    #[error("untrusted signer {signer} for plugin {plugin}")]
    UntrustedSigner { plugin: String, signer: String },
    #[error("integrity failure for plugin {plugin}: {reason}")]
    Integrity { plugin: String, reason: String },
}

#[derive(Debug)]
pub struct PluginRegistry {
    core_api_version: String,
    package_root: PathBuf,
    trust_store: TrustStore,
    manifests: BTreeMap<Uuid, PluginManifest>,
    states: BTreeMap<Uuid, PluginState>,
    quarantine: Vec<QuarantineRecord>,
}

impl PluginRegistry {
    pub fn new(
        core_api_version: impl Into<String>,
        package_root: impl Into<PathBuf>,
        trust_store: TrustStore,
    ) -> Self {
        Self {
            core_api_version: core_api_version.into(),
            package_root: package_root.into(),
            trust_store,
            manifests: BTreeMap::new(),
            states: BTreeMap::new(),
            quarantine: Vec::new(),
        }
    }

    pub fn manifests(&self) -> impl Iterator<Item = &PluginManifest> {
        self.manifests.values()
    }

    pub fn quarantine(&self) -> &[QuarantineRecord] {
        &self.quarantine
    }

    pub fn state(&self, plugin_uuid: &Uuid) -> Option<PluginState> {
        self.states.get(plugin_uuid).copied()
    }

    pub fn manifest(&self, plugin_uuid: &Uuid) -> Option<&PluginManifest> {
        self.manifests.get(plugin_uuid)
    }

    pub fn enable(&mut self, plugin_uuid: Uuid) -> Result<(), RegistryError> {
        if !self.manifests.contains_key(&plugin_uuid) {
            return Err(RegistryError::Invalid {
                plugin: plugin_uuid.to_string(),
                reason: "cannot enable an unknown plugin".into(),
            });
        }
        self.states.insert(plugin_uuid, PluginState::Enabled);
        Ok(())
    }

    pub fn disable(&mut self, plugin_uuid: Uuid) -> Result<(), RegistryError> {
        if !self.manifests.contains_key(&plugin_uuid) {
            return Err(RegistryError::Invalid {
                plugin: plugin_uuid.to_string(),
                reason: "cannot disable an unknown plugin".into(),
            });
        }
        self.states.insert(plugin_uuid, PluginState::Disabled);
        Ok(())
    }

    pub fn discover_tree(&mut self, dir: &Path) -> Result<DiscoveryReport, RegistryError> {
        let mut paths = Vec::new();
        collect_manifest_paths(dir, &mut paths)?;
        paths.sort();

        let before_manifests = self.manifests.len();
        let before_quarantine = self.quarantine.len();
        for path in paths {
            if let Err(error) = self.load_path(&path) {
                self.quarantine_path(&path, &error);
            }
        }

        self.resolve_dependencies_fail_closed();
        Ok(DiscoveryReport {
            accepted: self.manifests.len().saturating_sub(before_manifests),
            quarantined: self.quarantine.len() - before_quarantine,
        })
    }

    pub fn load_json(&mut self, input: &str, source: &Path) -> Result<Uuid, RegistryError> {
        let manifest: PluginManifest = serde_json::from_str(input)?;
        let id = manifest.uuid;
        self.register(manifest, source)?;
        Ok(id)
    }

    pub fn load_path(&mut self, path: &Path) -> Result<Uuid, RegistryError> {
        let input = fs::read_to_string(path)?;
        self.load_json(&input, path)
    }

    pub fn register(
        &mut self,
        manifest: PluginManifest,
        source: &Path,
    ) -> Result<(), RegistryError> {
        self.validate_manifest(&manifest)?;
        self.verify_integrity(&manifest)?;
        if self.manifests.contains_key(&manifest.uuid) {
            return Err(RegistryError::Duplicate(manifest.uuid));
        }
        if self
            .manifests
            .values()
            .any(|item| item.name == manifest.name)
        {
            return Err(RegistryError::DuplicateName(manifest.name));
        }
        self.states.insert(manifest.uuid, PluginState::Validated);
        self.manifests.insert(manifest.uuid, manifest);
        let _ = source;
        Ok(())
    }

    pub fn resolve_dependencies(&self) -> Result<(), RegistryError> {
        for plugin in self.manifests.values() {
            for dep in &plugin.dependencies {
                match self.manifests.get(&dep.uuid) {
                    Some(found) => {
                        let req = VersionReq::parse(&dep.version).map_err(|error| {
                            RegistryError::Invalid {
                                plugin: plugin.name.clone(),
                                reason: format!("invalid dependency semver requirement: {error}"),
                            }
                        })?;
                        let found_version = Version::parse(&found.version).map_err(|error| {
                            RegistryError::Invalid {
                                plugin: found.name.clone(),
                                reason: format!("invalid semver: {error}"),
                            }
                        })?;
                        if !req.matches(&found_version) {
                            return Err(RegistryError::DependencyVersion {
                                plugin: plugin.name.clone(),
                                requirement: dep.version.clone(),
                                found: found.version.clone(),
                            });
                        }
                    }
                    None if dep.optional => {}
                    None => {
                        return Err(RegistryError::MissingDependency {
                            plugin: plugin.name.clone(),
                            dependency: dep.uuid,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    pub fn persist_postgres(&self, client: &mut Client) -> Result<(), RegistryError> {
        let mut transaction = client.transaction()?;
        for manifest in self.manifests.values() {
            let state = self
                .states
                .get(&manifest.uuid)
                .copied()
                .unwrap_or(PluginState::Validated);
            let manifest_json = serde_json::to_value(manifest)?;
            transaction.execute(
                "INSERT INTO phoenix_plugin_manifests \
                 (uuid, name, version, core_api, status, manifest, digest_sha256, signature, signer, provenance, updated_at) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,now()) \
                 ON CONFLICT (uuid) DO UPDATE SET \
                 name=EXCLUDED.name, version=EXCLUDED.version, core_api=EXCLUDED.core_api, \
                 status=EXCLUDED.status, manifest=EXCLUDED.manifest, digest_sha256=EXCLUDED.digest_sha256, \
                 signature=EXCLUDED.signature, signer=EXCLUDED.signer, provenance=EXCLUDED.provenance, updated_at=now()",
                &[
                    &manifest.uuid,
                    &manifest.name,
                    &manifest.version,
                    &manifest.core_api,
                    &plugin_state_str(state),
                    &manifest_json,
                    &manifest.integrity.digest,
                    &manifest.integrity.signature,
                    &manifest.integrity.signer,
                    &manifest.integrity.provenance,
                ],
            )?;
        }
        for item in &self.quarantine {
            transaction.execute(
                "INSERT INTO phoenix_plugin_quarantine \
                 (plugin_uuid, plugin_name, source, reason, observed_at) VALUES ($1,$2,$3,$4,$5)",
                &[
                    &item.plugin_uuid,
                    &item.plugin_name,
                    &item.source,
                    &item.reason,
                    &item.observed_at,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn resolve_dependencies_fail_closed(&mut self) {
        loop {
            match self.resolve_dependencies() {
                Ok(()) => break,
                Err(error) => {
                    let plugin_name = match &error {
                        RegistryError::MissingDependency { plugin, .. } => Some(plugin.clone()),
                        RegistryError::DependencyVersion { plugin, .. } => Some(plugin.clone()),
                        RegistryError::Invalid { plugin, .. } => Some(plugin.clone()),
                        _ => None,
                    };
                    let Some(plugin_name) = plugin_name else {
                        break;
                    };
                    let Some(plugin_uuid) = self
                        .manifests
                        .values()
                        .find(|manifest| manifest.name == plugin_name)
                        .map(|manifest| manifest.uuid)
                    else {
                        break;
                    };
                    let removed = self.manifests.remove(&plugin_uuid);
                    self.states.insert(plugin_uuid, PluginState::Quarantined);
                    self.quarantine.push(QuarantineRecord {
                        plugin_uuid: Some(plugin_uuid),
                        plugin_name: removed.as_ref().map(|manifest| manifest.name.clone()),
                        source: "dependency-resolution".into(),
                        reason: error.to_string(),
                        observed_at: Utc::now(),
                    });
                }
            }
        }
    }

    fn quarantine_path(&mut self, path: &Path, error: &RegistryError) {
        let mut plugin_uuid = None;
        let mut plugin_name = None;
        if let Ok(input) = fs::read_to_string(path)
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&input)
        {
            plugin_uuid = value
                .get("uuid")
                .and_then(|value| value.as_str())
                .and_then(|value| Uuid::parse_str(value).ok());
            plugin_name = value
                .get("name")
                .and_then(|value| value.as_str())
                .map(str::to_owned);
        }
        if let Some(plugin_uuid) = plugin_uuid {
            self.states.insert(plugin_uuid, PluginState::Quarantined);
        }
        self.quarantine.push(QuarantineRecord {
            plugin_uuid,
            plugin_name,
            source: path.display().to_string(),
            reason: error.to_string(),
            observed_at: Utc::now(),
        });
    }

    fn validate_manifest(&self, manifest: &PluginManifest) -> Result<(), RegistryError> {
        let invalid = |reason: String| RegistryError::Invalid {
            plugin: manifest.name.clone(),
            reason,
        };

        if !is_uuid_v7(&manifest.uuid) {
            return Err(invalid("plugin UUID must be UUIDv7".into()));
        }
        Version::parse(&manifest.version)
            .map_err(|error| invalid(format!("invalid semver: {error}")))?;
        Version::parse(&manifest.manifest_version)
            .map_err(|error| invalid(format!("invalid manifest version: {error}")))?;
        let api_req = VersionReq::parse(&manifest.core_api)
            .map_err(|error| invalid(format!("invalid core_api requirement: {error}")))?;
        let core = Version::parse(&self.core_api_version)
            .map_err(|error| invalid(format!("invalid core API version: {error}")))?;
        if !api_req.matches(&core) {
            return Err(invalid(format!(
                "core API {} does not satisfy {}",
                self.core_api_version, manifest.core_api
            )));
        }
        if manifest.capabilities.is_empty() {
            return Err(invalid("at least one capability is required".into()));
        }
        let mut extension_points = std::collections::BTreeSet::new();
        for extension in &manifest.extension_points {
            if extension.point.trim().is_empty()
                || !extension_points.insert(extension.point.clone())
            {
                return Err(invalid(
                    "extension point names must be non-empty and unique".into(),
                ));
            }
            VersionReq::parse(&extension.contract_version).map_err(|error| {
                invalid(format!(
                    "invalid extension contract version for {}: {error}",
                    extension.point
                ))
            })?;
        }
        if manifest.kind == "agent" && manifest.agent.is_none() {
            return Err(invalid("kind=agent requires an agent profile".into()));
        }
        if manifest.kind != "agent" && manifest.agent.is_some() {
            return Err(invalid("agent profile is only valid for kind=agent".into()));
        }
        if manifest.kind == "model_provider" && manifest.model_provider.is_none() {
            return Err(invalid(
                "kind=model_provider requires a model_provider profile".into(),
            ));
        }
        if manifest.kind != "model_provider" && manifest.model_provider.is_some() {
            return Err(invalid(
                "model_provider profile is only valid for kind=model_provider".into(),
            ));
        }
        if let Some(provider) = &manifest.model_provider {
            if provider.models.is_empty() {
                return Err(invalid(
                    "model_provider must declare at least one model".into(),
                ));
            }
            let mut model_ids = std::collections::BTreeSet::new();
            for model in &provider.models {
                if model.id.trim().is_empty() || !model_ids.insert(model.id.clone()) {
                    return Err(invalid("model ids must be non-empty and unique".into()));
                }
                if model.capabilities.is_empty() || model.context_window == 0 {
                    return Err(invalid(
                        "every model requires capabilities and a positive context window".into(),
                    ));
                }
            }
        }
        if manifest.integrity.hash_algorithm != "sha256" {
            return Err(invalid("only sha256 is accepted".into()));
        }
        if manifest.integrity.signature_algorithm != "ed25519" {
            return Err(invalid("only ed25519 signatures are accepted".into()));
        }
        if manifest.integrity.digest.len() != 64
            || !manifest
                .integrity
                .digest
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err(invalid("digest must be 64 hex chars".into()));
        }
        if manifest.integrity.signature.trim().is_empty()
            || manifest.integrity.signer.trim().is_empty()
            || manifest.integrity.provenance.trim().is_empty()
            || manifest.integrity.artifact.trim().is_empty()
        {
            return Err(invalid(
                "artifact, signature, signer and provenance are required".into(),
            ));
        }
        if manifest.tests.is_empty() || !manifest.tests.iter().any(|test| test.required) {
            return Err(invalid("at least one required test is required".into()));
        }
        if manifest.rollback.steps.is_empty() {
            return Err(invalid("rollback steps are required".into()));
        }
        if manifest.sandbox.timeout_ms == 0 || manifest.sandbox.memory_mb == 0 {
            return Err(invalid(
                "sandbox timeout and memory must be greater than zero".into(),
            ));
        }
        if !(1..=100).contains(&manifest.sandbox.cpu_quota_percent) {
            return Err(invalid(
                "sandbox cpu_quota_percent must be between 1 and 100".into(),
            ));
        }
        if manifest.sandbox.network == SandboxNetworkMode::Deny
            && !manifest.sandbox.network_allowlist.is_empty()
        {
            return Err(invalid(
                "network_allowlist must be empty when sandbox network=deny".into(),
            ));
        }
        Ok(())
    }

    fn verify_integrity(&self, manifest: &PluginManifest) -> Result<(), RegistryError> {
        let plugin = manifest.name.clone();
        let signer = self
            .trust_store
            .signer(&manifest.integrity.signer)
            .ok_or_else(|| RegistryError::UntrustedSigner {
                plugin: plugin.clone(),
                signer: manifest.integrity.signer.clone(),
            })?;

        if signer.algorithm != "ed25519"
            || !signer
                .allowed_name_prefixes
                .iter()
                .any(|prefix| manifest.name.starts_with(prefix))
        {
            return Err(RegistryError::UntrustedSigner {
                plugin,
                signer: signer.id.clone(),
            });
        }

        let root = fs::canonicalize(&self.package_root)?;
        let artifact = fs::canonicalize(self.package_root.join(&manifest.integrity.artifact))?;
        if !artifact.starts_with(&root) {
            return Err(RegistryError::Integrity {
                plugin: manifest.name.clone(),
                reason: "artifact escaped the package root".into(),
            });
        }

        let bytes = fs::read(&artifact)?;
        let digest = hex_sha256(&bytes);
        if !digest.eq_ignore_ascii_case(&manifest.integrity.digest) {
            return Err(RegistryError::Integrity {
                plugin: manifest.name.clone(),
                reason: format!(
                    "sha256 mismatch: manifest={}, computed={digest}",
                    manifest.integrity.digest
                ),
            });
        }

        let public_key =
            BASE64
                .decode(&signer.public_key_base64)
                .map_err(|error| RegistryError::Integrity {
                    plugin: manifest.name.clone(),
                    reason: format!("invalid trusted public key encoding: {error}"),
                })?;
        let public_key: [u8; 32] = public_key
            .try_into()
            .map_err(|_| RegistryError::Integrity {
                plugin: manifest.name.clone(),
                reason: "trusted public key must be exactly 32 bytes".into(),
            })?;
        let verifying_key =
            VerifyingKey::from_bytes(&public_key).map_err(|error| RegistryError::Integrity {
                plugin: manifest.name.clone(),
                reason: format!("invalid Ed25519 public key: {error}"),
            })?;

        let signature = BASE64
            .decode(&manifest.integrity.signature)
            .map_err(|error| RegistryError::Integrity {
                plugin: manifest.name.clone(),
                reason: format!("invalid signature encoding: {error}"),
            })?;
        let signature =
            Signature::from_slice(&signature).map_err(|error| RegistryError::Integrity {
                plugin: manifest.name.clone(),
                reason: format!("invalid Ed25519 signature: {error}"),
            })?;

        verifying_key
            .verify(
                mensagem_do_formato(manifest, signer.signature_format).as_bytes(),
                &signature,
            )
            .map_err(|error| RegistryError::Integrity {
                plugin: manifest.name.clone(),
                reason: format!("Ed25519 verification failed: {error}"),
            })?;
        Ok(())
    }
}

/// A mensagem assinada no formato do signatario: um lugar so para o assinador e para a
/// verificacao, para os dois nunca divergirem.
pub fn mensagem_do_formato(manifest: &PluginManifest, formato: u8) -> String {
    if formato >= 2 {
        signing_message_v2(manifest)
    } else {
        signing_message(manifest)
    }
}

/// Forma canonica do manifesto: JSON compacto de chaves ordenadas, com a propria
/// assinatura vazia. Os verificadores em Python reproduzem a mesma forma.
pub fn manifesto_canonico(manifest: &PluginManifest) -> String {
    let mut m = manifest.clone();
    m.integrity.signature = String::new();
    serde_json::to_value(&m)
        .map(|v| v.to_string())
        .unwrap_or_default()
}

/// V2: a V1 mais o sha256 do manifesto inteiro. Na V1, editar `permissions`, `sandbox`
/// ou `entrypoint` nao quebrava a assinatura (achado em 30/09, ao reassinar os builtin).
pub fn signing_message_v2(manifest: &PluginManifest) -> String {
    format!(
        "PHXCLAW-PLUGIN-V2\nuuid={}\nname={}\nversion={}\nsha256={}\nmanifesto_sha256={}\n",
        manifest.uuid,
        manifest.name,
        manifest.version,
        manifest.integrity.digest,
        hex_sha256(manifesto_canonico(manifest).as_bytes())
    )
}

pub fn signing_message(manifest: &PluginManifest) -> String {
    format!(
        "PHXCLAW-PLUGIN-V1\nuuid={}\nname={}\nversion={}\nsha256={}\n",
        manifest.uuid, manifest.name, manifest.version, manifest.integrity.digest
    )
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn plugin_state_str(state: PluginState) -> &'static str {
    match state {
        PluginState::Discovered => "discovered",
        PluginState::Validated => "validated",
        PluginState::Enabled => "enabled",
        PluginState::Disabled => "disabled",
        PluginState::Quarantined => "quarantined",
        PluginState::Failed => "failed",
    }
}

fn collect_manifest_paths(dir: &Path, output: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_manifest_paths(&path, output)?;
        } else if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.ends_with(".plugin.json"))
        {
            output.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn registry() -> PluginRegistry {
        let trust_store =
            TrustStore::from_json(include_str!("../../../config/trust/plugin-signers.json"))
                .unwrap();
        // A versao sai da constituicao: cravada aqui, envelheceu em 0.3.0 enquanto os manifestos pediam 0.5.
        let constitution: serde_json::Value =
            serde_json::from_str(include_str!("../../../config/constitution.json")).unwrap();
        let api = constitution["plugin_api_version"]
            .as_str()
            .unwrap()
            .to_owned();
        PluginRegistry::new(api, root(), trust_store)
    }

    #[test]
    fn signed_builtin_agents_load() {
        let mut registry = registry();
        let report = registry
            .discover_tree(&root().join("plugins/builtin/manifests"))
            .unwrap();
        assert!(report.accepted >= 3);
        // o motivo da quarentena aparece na falha: sem ele, "1 != 0" nao diz o que olhar
        assert_eq!(report.quarantined, 0, "{:#?}", registry.quarantine());
        registry.resolve_dependencies().unwrap();
    }

    #[test]
    fn reassinar_com_a_chave_do_signatario_e_aceito_e_com_outra_e_recusado() {
        use crate::assinatura::{chave_do_signatario, reassinar};
        use base64::Engine as _;
        let semente = [42u8; 32];
        let publica = base64::engine::general_purpose::STANDARD.encode(
            ed25519_dalek::SigningKey::from_bytes(&semente)
                .verifying_key()
                .as_bytes(),
        );
        let trust = TrustStore::from_json(&format!(
            r#"{{"version":"1","signers":[{{"id":"teste","algorithm":"ed25519","public_key_base64":"{publica}","status":"active","allowed_name_prefixes":["com.phxclaw."]}}]}}"#
        ))
        .unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(semente);
        // chave de outro signatario: recusada antes de assinar
        let outra = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
        assert!(chave_do_signatario(&outra, "teste", &trust).is_err());
        let chave = chave_do_signatario(&b64, "teste", &trust).unwrap();
        let mut v: serde_json::Value = serde_json::from_str(include_str!(
            "../../../plugins/builtin/manifests/office-document.tool.plugin.json"
        ))
        .unwrap();
        v["integrity"]["signer"] = "teste".into();
        let novo = reassinar(&v.to_string(), &root(), &chave, 1).unwrap();
        let constitution: serde_json::Value =
            serde_json::from_str(include_str!("../../../config/constitution.json")).unwrap();
        let api = constitution["plugin_api_version"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut reg = PluginRegistry::new(api, root(), trust);
        let m: PluginManifest = serde_json::from_str(&novo).unwrap();
        reg.register(m, Path::new("reassinado.plugin.json"))
            .unwrap();
    }

    #[test]
    fn permissao_ou_rede_adulteradas_quebram_a_assinatura_v2() {
        // com a V1 este teste passava a manifesto adulterado: permissoes e rede ficavam
        // fora da mensagem assinada
        for adulterar in ["permissao", "rede", "entrypoint"] {
            let mut reg = registry();
            let input =
                include_str!("../../../plugins/builtin/manifests/morpheus.agent.plugin.json");
            let mut v: serde_json::Value = serde_json::from_str(input).unwrap();
            match adulterar {
                "permissao" => v["permissions"][0]["scopes"] = serde_json::json!(["*"]),
                "rede" => {
                    v["sandbox"]["network"] = "allowlist".into();
                    v["sandbox"]["network_allowlist"] = serde_json::json!(["exfiltra.exemplo"]);
                }
                _ => v["entrypoint"]["value"] = "/bin/sh".into(),
            }
            let m: PluginManifest = serde_json::from_value(v).unwrap();
            let e = reg
                .register(m, Path::new("adulterado.plugin.json"))
                .unwrap_err();
            assert!(e.to_string().contains("Ed25519"), "{adulterar}: {e}");
        }
    }

    #[test]
    fn tampered_artifact_is_rejected() {
        let mut registry = registry();
        let input = include_str!("../../../plugins/builtin/manifests/morpheus.agent.plugin.json");
        let mut manifest: PluginManifest = serde_json::from_str(input).unwrap();
        manifest.integrity.digest = "00".repeat(32);
        let error = registry
            .register(manifest, Path::new("tampered.plugin.json"))
            .unwrap_err();
        assert!(error.to_string().contains("sha256 mismatch"));
    }

    #[test]
    fn non_v7_plugin_is_rejected() {
        let mut registry = registry();
        let input = include_str!("../../../plugins/builtin/manifests/morpheus.agent.plugin.json");
        let mut value: serde_json::Value = serde_json::from_str(input).unwrap();
        value["uuid"] = serde_json::Value::String("550e8400-e29b-41d4-a716-446655440000".into());
        let manifest: PluginManifest = serde_json::from_value(value).unwrap();
        let error = registry
            .register(manifest, Path::new("invalid.plugin.json"))
            .unwrap_err();
        assert!(error.to_string().contains("UUIDv7"));
    }
}

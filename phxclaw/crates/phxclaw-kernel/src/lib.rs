use chrono::{DateTime, Utc};
use phxclaw_types::{is_uuid_v7, new_uuid_v7};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constitution {
    pub phxclaw_constitution: String,
    pub core_version: String,
    pub plugin_api_version: String,
    pub core_language: String,
    pub declarative_format: String,
    pub persistent_identity: String,
    pub official_state_store: String,
    pub native_core_capabilities: Vec<String>,
    pub plugin_first: bool,
    pub microkernel: bool,
    pub default_permission_effect: String,
    pub deterministic_gates_required: bool,
    pub required_plugin_controls: Vec<String>,
}

impl Constitution {
    pub fn from_json(input: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(input)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.core_language != "Rust" {
            return Err("core_language must be Rust".into());
        }
        if self.declarative_format != "JSON" {
            return Err("declarative_format must be JSON".into());
        }
        if self.persistent_identity != "UUIDv7" {
            return Err("persistent_identity must be UUIDv7".into());
        }
        if self.official_state_store != "PostgreSQL" {
            return Err("official_state_store must be PostgreSQL".into());
        }
        for required in ["research", "hypothesis", "installer"] {
            if !self.native_core_capabilities.iter().any(|v| v == required) {
                return Err(format!("missing native core capability: {required}"));
            }
        }
        if self.default_permission_effect != "deny" {
            return Err("default permission effect must be deny".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Kernel {
    pub session_uuid: Uuid,
    pub started_at: DateTime<Utc>,
    pub constitution: Constitution,
}

impl Kernel {
    pub fn boot(constitution: Constitution) -> Result<Self, String> {
        constitution.validate()?;
        let session_uuid = new_uuid_v7();
        if !is_uuid_v7(&session_uuid) {
            return Err("kernel session UUID is not v7".into());
        }
        Ok(Self {
            session_uuid,
            started_at: Utc::now(),
            constitution,
        })
    }
}

pub trait NativeCapability {
    fn name(&self) -> &'static str;
    fn health(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constitution_boots_kernel() {
        let c = Constitution::from_json(include_str!("../../../config/constitution.json")).unwrap();
        let kernel = Kernel::boot(c).unwrap();
        assert!(is_uuid_v7(&kernel.session_uuid));
    }
}

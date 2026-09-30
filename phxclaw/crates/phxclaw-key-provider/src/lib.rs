//! PhxClaw production key-provider boundary.
//! Key bytes never implement Debug/Display and are zeroized on drop.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use std::fmt;
use thiserror::Error;
use zeroize::Zeroize;

#[derive(Error, Debug)]
pub enum KeyProviderError {
    #[error("invalid key identifier")]
    InvalidKeyId,
    #[error("key not found")]
    NotFound,
    #[error("key material is shorter than policy minimum")]
    WeakKey,
    #[error("provider is not allowed in release mode")]
    ProviderNotReleaseSafe,
    #[error("provider error: {0}")]
    Provider(String),
    #[error("invalid encoded key material")]
    Decode,
}

pub struct KeyMaterial(Vec<u8>);

impl KeyMaterial {
    pub fn new(bytes: Vec<u8>) -> Result<Self, KeyProviderError> {
        if bytes.len() < 32 {
            return Err(KeyProviderError::WeakKey);
        }
        Ok(Self(bytes))
    }

    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for KeyMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KeyMaterial([REDACTED])")
    }
}

impl Drop for KeyMaterial {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

pub trait KeyProvider: Send + Sync {
    fn provider_id(&self) -> &str;
    fn is_release_safe(&self) -> bool;
    fn load(&self, key_id: &str) -> Result<KeyMaterial, KeyProviderError>;
    fn store(&self, key_id: &str, material: &KeyMaterial) -> Result<(), KeyProviderError>;
    fn delete(&self, key_id: &str) -> Result<(), KeyProviderError>;
}

#[derive(Clone, Debug)]
pub struct KeyProviderPolicy {
    pub minimum_key_bytes: usize,
    pub require_release_safe: bool,
}

impl Default for KeyProviderPolicy {
    fn default() -> Self {
        Self {
            minimum_key_bytes: 32,
            require_release_safe: true,
        }
    }
}

pub fn validate_provider_for_release(
    provider: &dyn KeyProvider,
    policy: &KeyProviderPolicy,
) -> Result<(), KeyProviderError> {
    if policy.require_release_safe && !provider.is_release_safe() {
        return Err(KeyProviderError::ProviderNotReleaseSafe);
    }
    let _ = policy.minimum_key_bytes;
    Ok(())
}

fn validate_key_id(key_id: &str) -> Result<(), KeyProviderError> {
    let ok = !key_id.is_empty()
        && key_id.len() <= 160
        && key_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b':' | b'/'));
    if ok {
        Ok(())
    } else {
        Err(KeyProviderError::InvalidKeyId)
    }
}

#[cfg(feature = "os-keyring")]
#[derive(Clone, Debug)]
pub struct OsKeyringProvider {
    service: String,
    namespace: String,
}

#[cfg(feature = "os-keyring")]
impl OsKeyringProvider {
    pub fn new(service: impl Into<String>, namespace: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            namespace: namespace.into(),
        }
    }

    fn entry(&self, key_id: &str) -> Result<keyring::Entry, KeyProviderError> {
        validate_key_id(key_id)?;
        let account = format!("{}:{}", self.namespace, key_id);
        keyring::Entry::new(&self.service, &account)
            .map_err(|e| KeyProviderError::Provider(e.to_string()))
    }
}

#[cfg(feature = "os-keyring")]
impl KeyProvider for OsKeyringProvider {
    fn provider_id(&self) -> &str {
        "os_keyring"
    }

    fn is_release_safe(&self) -> bool {
        true
    }

    fn load(&self, key_id: &str) -> Result<KeyMaterial, KeyProviderError> {
        let encoded = self.entry(key_id)?.get_password().map_err(|e| match e {
            keyring::Error::NoEntry => KeyProviderError::NotFound,
            other => KeyProviderError::Provider(other.to_string()),
        })?;
        let bytes = B64.decode(encoded).map_err(|_| KeyProviderError::Decode)?;
        KeyMaterial::new(bytes)
    }

    fn store(&self, key_id: &str, material: &KeyMaterial) -> Result<(), KeyProviderError> {
        let encoded = B64.encode(material.expose());
        self.entry(key_id)?
            .set_password(&encoded)
            .map_err(|e| KeyProviderError::Provider(e.to_string()))
    }

    fn delete(&self, key_id: &str) -> Result<(), KeyProviderError> {
        self.entry(key_id)?
            .delete_credential()
            .map_err(|e| match e {
                keyring::Error::NoEntry => KeyProviderError::NotFound,
                other => KeyProviderError::Provider(other.to_string()),
            })
    }
}

#[cfg(feature = "dev-env")]
#[derive(Clone, Debug)]
pub struct DevEnvKeyProvider {
    prefix: String,
}

#[cfg(feature = "dev-env")]
impl DevEnvKeyProvider {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }
}

#[cfg(feature = "dev-env")]
impl KeyProvider for DevEnvKeyProvider {
    fn provider_id(&self) -> &str {
        "dev_env"
    }

    fn is_release_safe(&self) -> bool {
        false
    }

    fn load(&self, key_id: &str) -> Result<KeyMaterial, KeyProviderError> {
        validate_key_id(key_id)?;
        let env_name = format!(
            "{}_{}",
            self.prefix,
            key_id.replace([':', '/', '.', '-'], "_")
        );
        let encoded = std::env::var(env_name).map_err(|_| KeyProviderError::NotFound)?;
        let bytes = B64.decode(encoded).map_err(|_| KeyProviderError::Decode)?;
        KeyMaterial::new(bytes)
    }

    fn store(&self, _key_id: &str, _material: &KeyMaterial) -> Result<(), KeyProviderError> {
        Err(KeyProviderError::Provider(
            "environment provider is read-only".into(),
        ))
    }

    fn delete(&self, _key_id: &str) -> Result<(), KeyProviderError> {
        Err(KeyProviderError::Provider(
            "environment provider is read-only".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct UnsafeProvider;
    impl KeyProvider for UnsafeProvider {
        fn provider_id(&self) -> &str {
            "unsafe-test"
        }
        fn is_release_safe(&self) -> bool {
            false
        }
        fn load(&self, _key_id: &str) -> Result<KeyMaterial, KeyProviderError> {
            Err(KeyProviderError::NotFound)
        }
        fn store(&self, _key_id: &str, _material: &KeyMaterial) -> Result<(), KeyProviderError> {
            Ok(())
        }
        fn delete(&self, _key_id: &str) -> Result<(), KeyProviderError> {
            Ok(())
        }
    }

    #[test]
    fn weak_key_is_rejected() {
        assert!(matches!(
            KeyMaterial::new(vec![0_u8; 16]),
            Err(KeyProviderError::WeakKey)
        ));
    }

    #[test]
    fn release_rejects_unsafe_provider() {
        let p = UnsafeProvider;
        assert!(matches!(
            validate_provider_for_release(&p, &KeyProviderPolicy::default()),
            Err(KeyProviderError::ProviderNotReleaseSafe)
        ));
    }
}

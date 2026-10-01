//! PhxClaw production key-provider boundary.
//! Key bytes never implement Debug/Display and are zeroized on drop.

// So os backends com armazenamento usam base64 e a validacao do id: sem nenhum deles
// ligado, o crate compilava com avisos no binario que nao liga feature nenhuma.
#[cfg(any(feature = "os-keyring", feature = "arquivo", feature = "dev-env"))]
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

#[cfg(any(feature = "os-keyring", feature = "arquivo", feature = "dev-env"))]
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
impl OsKeyringProvider {
    /// O chaveiro desta plataforma guarda a chave entre execucoes? Falso quando o crate
    /// caiu no armazenamento simulado (plataforma sem backend compilado).
    pub fn persistente() -> bool {
        !matches!(
            keyring::default::default_credential_builder().persistence(),
            keyring::credential::CredentialPersistence::ProcessOnly
                | keyring::credential::CredentialPersistence::EntryOnly
        )
    }
}

#[cfg(feature = "os-keyring")]
impl KeyProvider for OsKeyringProvider {
    fn provider_id(&self) -> &str {
        "os_keyring"
    }

    /// Seguro para producao so se a chave sobrevive ao processo.
    fn is_release_safe(&self) -> bool {
        Self::persistente()
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

/// Cofre em arquivo, para maquina sem chaveiro do sistema (Linux sem sessao grafica,
/// Raspberry Pi, servidor). E o que OpenSSH, WireGuard e Tailscale fazem com a identidade
/// da maquina: arquivo so do dono (0600) em pasta so do dono (0700). Entra PEDIDO pelo
/// operador, nunca como queda silenciosa do chaveiro.
#[cfg(feature = "arquivo")]
#[derive(Clone, Debug)]
pub struct ArquivoKeyProvider {
    pasta: std::path::PathBuf,
}

#[cfg(feature = "arquivo")]
impl ArquivoKeyProvider {
    pub fn new(pasta: impl Into<std::path::PathBuf>) -> Result<Self, KeyProviderError> {
        let pasta = pasta.into();
        std::fs::create_dir_all(&pasta).map_err(|e| KeyProviderError::Provider(e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&pasta, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| KeyProviderError::Provider(e.to_string()))?;
        }
        Ok(Self { pasta })
    }

    fn caminho(&self, key_id: &str) -> Result<std::path::PathBuf, KeyProviderError> {
        validate_key_id(key_id)?;
        // "/" e ":" viram "_": o id nunca vira subpasta nem caminho fora do cofre
        Ok(self.pasta.join(key_id.replace(['/', ':'], "_") + ".key"))
    }
}

#[cfg(feature = "arquivo")]
impl KeyProvider for ArquivoKeyProvider {
    fn provider_id(&self) -> &str {
        "arquivo_0600"
    }

    fn is_release_safe(&self) -> bool {
        cfg!(unix)
    }

    fn load(&self, key_id: &str) -> Result<KeyMaterial, KeyProviderError> {
        let c = self.caminho(key_id)?;
        let t = std::fs::read_to_string(&c).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => KeyProviderError::NotFound,
            _ => KeyProviderError::Provider(e.to_string()),
        })?;
        let bytes = B64.decode(t.trim()).map_err(|_| KeyProviderError::Decode)?;
        KeyMaterial::new(bytes)
    }

    fn store(&self, key_id: &str, material: &KeyMaterial) -> Result<(), KeyProviderError> {
        let c = self.caminho(key_id)?;
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // nasce 0600: criar e depois apertar deixaria uma janela legivel
            o.mode(0o600);
        }
        use std::io::Write;
        o.open(&c)
            .and_then(|mut f| f.write_all(B64.encode(material.expose()).as_bytes()))
            .map_err(|e| KeyProviderError::Provider(e.to_string()))
    }

    fn delete(&self, key_id: &str) -> Result<(), KeyProviderError> {
        std::fs::remove_file(self.caminho(key_id)?).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => KeyProviderError::NotFound,
            _ => KeyProviderError::Provider(e.to_string()),
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

    #[cfg(all(feature = "arquivo", unix))]
    #[test]
    fn cofre_em_arquivo_sobrevive_nasce_0600_e_nao_sai_da_pasta() {
        use std::os::unix::fs::PermissionsExt;
        let d = std::env::temp_dir().join(format!("phx-cofre-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let a = ArquivoKeyProvider::new(&d).unwrap();
        a.store(
            "device/abc/ed25519",
            &KeyMaterial::new(vec![7; 32]).unwrap(),
        )
        .unwrap();
        // outra instancia (como outro processo) acha a chave
        let b = ArquivoKeyProvider::new(&d).unwrap();
        assert_eq!(b.load("device/abc/ed25519").unwrap().expose(), &[7; 32]);
        let modo = std::fs::metadata(d.join("device_abc_ed25519.key"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(modo & 0o777, 0o600);
        assert_eq!(
            std::fs::metadata(&d).unwrap().permissions().mode() & 0o777,
            0o700
        );
        // id com ".." e "/" grava DENTRO do cofre: a barra vira "_" e nao ha subpasta
        b.store("../../fora", &KeyMaterial::new(vec![1; 32]).unwrap())
            .unwrap();
        assert!(d.join(".._.._fora.key").is_file());
        assert!(!d
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fora.key")
            .exists());
        b.delete("device/abc/ed25519").unwrap();
        assert!(matches!(
            b.load("device/abc/ed25519"),
            Err(KeyProviderError::NotFound)
        ));
    }

    #[cfg(feature = "os-keyring")]
    #[test]
    fn chaveiro_simulado_nao_se_declara_seguro() {
        // o que o provedor diz tem de bater com o backend que o crate escolheu
        let p = OsKeyringProvider::new("PhxClaw-teste", "t");
        assert_eq!(p.is_release_safe(), OsKeyringProvider::persistente());
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        assert!(
            OsKeyringProvider::persistente(),
            "backend de plataforma nao compilado: o chaveiro seria o simulado"
        );
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

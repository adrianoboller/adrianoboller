//! Reassinatura de manifestos: recalcula o sha256 do artefato e assina a mensagem
//! canonica (`signing_message`) com a chave do signatario. O mesmo `signing_message` que a
//! verificacao usa: assinar por outra conta seria a copia que diverge no dia em que o
//! formato mudar.

use crate::{PluginManifest, RegistryError, TrustStore, signing_message};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Chave privada Ed25519 (semente de 32 bytes em base64) que so assina se a publica dela
/// for a do signatario no `plugin-signers.json`: chave errada nao produz manifesto que o
/// registro recusaria depois.
pub fn chave_do_signatario(
    semente_b64: &str,
    signer_id: &str,
    trust: &TrustStore,
) -> Result<SigningKey, RegistryError> {
    let erro = |r: &str| RegistryError::Integrity {
        plugin: signer_id.into(),
        reason: r.into(),
    };
    let bytes = BASE64
        .decode(semente_b64.trim())
        .map_err(|_| erro("chave: base64 invalido"))?;
    let semente: [u8; 32] = bytes
        .try_into()
        .map_err(|_| erro("chave: a semente Ed25519 tem 32 bytes"))?;
    let chave = SigningKey::from_bytes(&semente);
    let publica = BASE64.encode(chave.verifying_key().as_bytes());
    let confiavel = trust
        .signer(signer_id)
        .ok_or_else(|| erro("signatario ausente do trust store"))?;
    if confiavel.public_key_base64 != publica {
        return Err(erro(
            "a chave dada nao e a do signatario: a publica derivada difere da do trust store",
        ));
    }
    Ok(chave)
}

/// Reassina um manifesto (JSON) em relacao a raiz do pacote; devolve o JSON novo com
/// `integrity.digest` e `integrity.signature` atualizados e o resto intacto.
pub fn reassinar(
    manifesto_json: &str,
    raiz: &Path,
    chave: &SigningKey,
) -> Result<String, RegistryError> {
    let mut valor: serde_json::Value =
        serde_json::from_str(manifesto_json).map_err(|e| RegistryError::Integrity {
            plugin: "?".into(),
            reason: e.to_string(),
        })?;
    let mut m: PluginManifest =
        serde_json::from_value(valor.clone()).map_err(|e| RegistryError::Integrity {
            plugin: "?".into(),
            reason: e.to_string(),
        })?;
    let bytes = std::fs::read(raiz.join(&m.integrity.artifact))?;
    m.integrity.digest = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let assinatura = BASE64.encode(chave.sign(signing_message(&m).as_bytes()).to_bytes());
    valor["integrity"]["digest"] = m.integrity.digest.clone().into();
    valor["integrity"]["signature"] = assinatura.into();
    let mut s = serde_json::to_string_pretty(&valor).map_err(|e| RegistryError::Integrity {
        plugin: m.name.clone(),
        reason: e.to_string(),
    })?;
    s.push('\n');
    Ok(s)
}

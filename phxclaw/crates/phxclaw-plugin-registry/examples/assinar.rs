//! Reassina os manifestos builtin. A chave NUNCA vem pela linha de comando (fica no
//! historico do shell e no /proc de qualquer um): vem de PHXCLAW_PLUGIN_SIGNING_KEY ou do
//! arquivo em PHXCLAW_PLUGIN_SIGNING_KEY_FILE (semente Ed25519 de 32 bytes, base64).
//!
//!   PHXCLAW_PLUGIN_SIGNING_KEY_FILE=~/chave-dev-root.b64 \
//!     cargo run -p phxclaw-plugin-registry --example assinar -- plugins/builtin/manifests
//!
//! Recusa se a chave nao for a do signatario de cada manifesto no trust store.
use phxclaw_plugin_registry::TrustStore;
use phxclaw_plugin_registry::assinatura::{chave_do_signatario, reassinar};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pasta = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| raiz.join("plugins/builtin/manifests"));
    let semente = match std::env::var("PHXCLAW_PLUGIN_SIGNING_KEY") {
        Ok(k) => k,
        Err(_) => {
            std::fs::read_to_string(std::env::var("PHXCLAW_PLUGIN_SIGNING_KEY_FILE").map_err(
                |_| "defina PHXCLAW_PLUGIN_SIGNING_KEY ou PHXCLAW_PLUGIN_SIGNING_KEY_FILE",
            )?)?
        }
    };
    let trust = TrustStore::from_json(&std::fs::read_to_string(
        raiz.join("config/trust/plugin-signers.json"),
    )?)?;
    let mut n = 0;
    for e in std::fs::read_dir(&pasta)? {
        let p = e?.path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let texto = std::fs::read_to_string(&p)?;
        let v: serde_json::Value = serde_json::from_str(&texto)?;
        let signer = v["integrity"]["signer"].as_str().unwrap_or_default();
        let chave = chave_do_signatario(&semente, signer, &trust)?;
        std::fs::write(&p, reassinar(&texto, &raiz, &chave)?)?;
        println!("reassinado {}", p.display());
        n += 1;
    }
    println!("{n} manifesto(s)");
    Ok(())
}

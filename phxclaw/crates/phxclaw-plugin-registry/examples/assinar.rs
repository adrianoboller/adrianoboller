//! Reassina os manifestos builtin. A chave NUNCA vem pela linha de comando (fica no
//! historico do shell e no /proc de qualquer um): vem de PHXCLAW_PLUGIN_SIGNING_KEY ou do
//! arquivo em PHXCLAW_PLUGIN_SIGNING_KEY_FILE (semente Ed25519 de 32 bytes, base64).
//! `phxclaw plugins assinar` faz o mesmo com a semente do broker (`phxclaw plugins chave`),
//! pelo mesmo `reassinar_pasta`.
//!
//!   PHXCLAW_PLUGIN_SIGNING_KEY_FILE=~/chave-dev-root.b64 \
//!     cargo run -p phxclaw-plugin-registry --example assinar -- plugins/builtin/manifests
//!
//! Recusa se a chave nao for a do signatario de cada manifesto no trust store.
use phxclaw_plugin_registry::assinatura::reassinar_pasta;
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
    let feitos = reassinar_pasta(&raiz, &pasta, &semente)?;
    for p in &feitos {
        println!("reassinado {}", p.display());
    }
    println!("{} manifesto(s)", feitos.len());
    Ok(())
}

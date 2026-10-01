//! Pasta assinada: o pacote de plugin no formato do Claude Code (`.claude-plugin/`) ou do
//! Codex (`.codex-plugin/`) nao tem artefato unico nem manifesto nosso -- e uma pasta de
//! skills, hooks, servidores MCP e subagentes. A assinatura cobre a PASTA INTEIRA: o
//! sha256 de cada arquivo, em ordem, com o caminho relativo. Mexer em qualquer um
//! (o script de um hook, o corpo de uma skill, a linha de comando de um servidor MCP)
//! quebra a assinatura.
//!
//! O assinador e o verificador usam o MESMO `digest_da_pasta` e a MESMA mensagem, e a
//! conferencia Ed25519 e a do manifesto (`verificar_ed25519`), contra o mesmo trust store.

use crate::{RegistryError, TrustStore, signatario_para, verificar_ed25519};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Onde a assinatura mora, na raiz do pacote (fora do proprio digest).
pub const ARQUIVO_ASSINATURA: &str = ".phxclaw-assinatura.json";

/// Teto de arquivos por pacote: pasta gigante assinada e pasta que ninguem revisou.
pub const MAX_ARQUIVOS: usize = 5_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssinaturaDePasta {
    pub nome: String,
    pub signer: String,
    pub digest: String,
    pub signature: String,
}

/// sha256 de "caminho\0sha256-do-arquivo\n" por arquivo, em ordem de caminho. Fica de
/// fora so a propria assinatura e `.git/`. Link simbolico e recusado: ele poderia apontar
/// para fora da pasta e trocar de alvo depois de assinado.
pub fn digest_da_pasta(raiz: &Path) -> Result<String, RegistryError> {
    let mut itens = Vec::new();
    juntar(raiz, raiz, &mut itens)?;
    itens.sort();
    let mut h = Sha256::new();
    for (rel, d) in &itens {
        h.update(rel.as_bytes());
        h.update([0]);
        h.update(d.as_bytes());
        h.update(b"\n");
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn juntar(raiz: &Path, dir: &Path, out: &mut Vec<(String, String)>) -> Result<(), RegistryError> {
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        let p = e.path();
        let tipo = e.file_type()?;
        let rel = p
            .strip_prefix(raiz)
            .map_err(|_| integridade("?", "caminho fora da raiz"))?
            .to_string_lossy()
            .replace('\\', "/");
        if rel == ARQUIVO_ASSINATURA || rel == ".git" {
            continue;
        }
        if tipo.is_symlink() {
            return Err(integridade(&rel, "link simbolico no pacote assinado"));
        }
        if tipo.is_dir() {
            juntar(raiz, &p, out)?;
        } else {
            if out.len() >= MAX_ARQUIVOS {
                return Err(integridade(&rel, "pacote com arquivos demais"));
            }
            let d: String = Sha256::digest(std::fs::read(&p)?)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            out.push((rel, d));
        }
    }
    Ok(())
}

fn integridade(plugin: &str, r: &str) -> RegistryError {
    RegistryError::Integrity {
        plugin: plugin.into(),
        reason: r.into(),
    }
}

pub fn mensagem(nome: &str, digest: &str) -> String {
    format!("PHXCLAW-PACOTE-V1\nnome={nome}\nsha256={digest}\n")
}

/// Assina a pasta e grava `ARQUIVO_ASSINATURA` nela.
pub fn assinar_pasta(
    raiz: &Path,
    nome: &str,
    signer: &str,
    chave: &SigningKey,
) -> Result<AssinaturaDePasta, RegistryError> {
    let digest = digest_da_pasta(raiz)?;
    let a = AssinaturaDePasta {
        nome: nome.into(),
        signer: signer.into(),
        signature: BASE64.encode(chave.sign(mensagem(nome, &digest).as_bytes()).to_bytes()),
        digest,
    };
    std::fs::write(
        raiz.join(ARQUIVO_ASSINATURA),
        serde_json::to_string_pretty(&a)?,
    )?;
    Ok(a)
}

/// Confere a pasta: assinatura presente, do nome esperado, de signatario ativo e
/// autorizado para o nome, digest recalculado igual ao assinado, e Ed25519 valida.
pub fn verificar_pasta(
    raiz: &Path,
    nome: &str,
    trust: &TrustStore,
) -> Result<AssinaturaDePasta, RegistryError> {
    let texto = std::fs::read_to_string(raiz.join(ARQUIVO_ASSINATURA))
        .map_err(|_| integridade(nome, "pacote sem assinatura (.phxclaw-assinatura.json)"))?;
    let a: AssinaturaDePasta = serde_json::from_str(&texto)?;
    if a.nome != nome {
        return Err(integridade(
            nome,
            &format!("assinatura e do pacote {:?}, nao deste", a.nome),
        ));
    }
    let signer = signatario_para(trust, &a.signer, nome)?;
    let digest = digest_da_pasta(raiz)?;
    if digest != a.digest {
        return Err(integridade(
            nome,
            "o conteudo da pasta mudou depois de assinado (sha256 difere)",
        ));
    }
    verificar_ed25519(
        signer,
        nome,
        mensagem(nome, &digest).as_bytes(),
        &a.signature,
    )?;
    Ok(a)
}

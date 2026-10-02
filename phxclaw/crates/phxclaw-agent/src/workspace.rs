//! As raizes do workspace alem da pasta da tarefa: o `.phxclaw/workspace.json` do projeto
//! (`phxclaw-workspace`, o mesmo leitor do IDE de mesa) mais a chave `ide.raizes` da
//! configuracao. Quem as usa: o `confine` (a unica porta de disco das ferramentas), o
//! LSP (um `workspaceFolder` por raiz) e o IDE (abre a primeira, expoe as outras).
//!
//! Lidas de fora do `work/` da tarefa, como os hooks e as regras: raiz extra e permissao
//! de disco, e o shell do modelo nao pode conceder permissao a si mesmo.

use std::path::{Path, PathBuf};

/// As raizes extras, canonicas, na ordem (arquivo primeiro, configuracao depois). Erro
/// quando o arquivo ou a configuracao estao invalidos: raiz que some calada e o modelo
/// recebendo «fora da pasta» sem saber por que.
pub fn raizes() -> Result<Vec<PathBuf>, String> {
    let mut v = match crate::montagem::pasta_do_projeto() {
        Some(p) => phxclaw_workspace::raizes(&p)?,
        None => Vec::new(),
    };
    if let Some(serde_json::Value::Array(lista)) = crate::config::valor("ide.raizes")? {
        for r in lista.iter().filter_map(serde_json::Value::as_str) {
            let canon = std::fs::canonicalize(r).map_err(|e| format!("ide.raizes: {r}: {e}"))?;
            if !v.contains(&canon) {
                v.push(canon);
            }
        }
    }
    Ok(v)
}

/// A raiz da lista que contem o caminho (ja canonico), se alguma.
pub fn raiz_de<'a>(raizes: &'a [PathBuf], canon: &Path) -> Option<&'a PathBuf> {
    raizes.iter().find(|r| canon.starts_with(r))
}

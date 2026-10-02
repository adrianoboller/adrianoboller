#![forbid(unsafe_code)]
//! O workspace de varias raizes: `.phxclaw/workspace.json` (`{"raizes": [...]}`), lido num
//! lugar so para o agente (confine, LSP, busca) e para o IDE (o terminal que abre o hx)
//! nunca divergirem sobre quais pastas fazem parte do projeto.
//!
//! O arquivo mora na pasta de CONFIGURACAO do projeto (`$PHXCLAW_PROJETO/.phxclaw`), fora
//! do `work/` das tarefas: raiz extra e permissao de disco, e permissao que o shell do
//! modelo pudesse reescrever nao seria permissao.

use std::path::{Path, PathBuf};

pub const ARQUIVO: &str = "workspace.json";

/// As raizes declaradas, canonicas e existentes, na ordem do arquivo. Raiz relativa e
/// resolvida contra a pasta que contem o `.phxclaw`. `Ok(vec![])` sem arquivo; arquivo
/// ilegivel ou raiz inexistente e erro dito, nunca lista encolhida calada.
pub fn raizes(pasta_phxclaw: &Path) -> Result<Vec<PathBuf>, String> {
    let arq = pasta_phxclaw.join(ARQUIVO);
    let texto = match std::fs::read_to_string(&arq) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", arq.display())),
    };
    let v: serde_json::Value =
        serde_json::from_str(&texto).map_err(|e| format!("{}: {e}", arq.display()))?;
    let lista = v
        .get("raizes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("{}: falta a lista \"raizes\"", arq.display()))?;
    let base = pasta_phxclaw.parent().unwrap_or(pasta_phxclaw);
    let mut saida = Vec::new();
    for r in lista {
        let s = r
            .as_str()
            .ok_or_else(|| format!("{}: raiz que nao e texto: {r}", arq.display()))?;
        let p = base.join(s);
        let canon =
            std::fs::canonicalize(&p).map_err(|e| format!("{}: raiz {s}: {e}", arq.display()))?;
        if !canon.is_dir() {
            return Err(format!("{}: raiz {s} nao e pasta", arq.display()));
        }
        if !saida.contains(&canon) {
            saida.push(canon);
        }
    }
    Ok(saida)
}

/// Separador da variavel de raizes que o IDE expoe ao terminal (`ide.raizes` no
/// catalogo): o do `PATH`.
pub const SEPARADOR: char = ':';

pub fn variavel(raizes: &[PathBuf]) -> String {
    raizes
        .iter()
        .map(|r| r.display().to_string())
        .collect::<Vec<_>>()
        .join(&SEPARADOR.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pasta(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("phx-ws-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join(".phxclaw")).unwrap();
        d
    }

    #[test]
    fn sem_arquivo_e_lista_vazia_e_arquivo_ruim_e_erro() {
        let d = pasta("vazio");
        assert_eq!(raizes(&d.join(".phxclaw")).unwrap(), Vec::<PathBuf>::new());
        std::fs::write(d.join(".phxclaw/workspace.json"), "{").unwrap();
        assert!(raizes(&d.join(".phxclaw")).is_err());
        std::fs::write(
            d.join(".phxclaw/workspace.json"),
            r#"{"raizes":["nao-existe"]}"#,
        )
        .unwrap();
        let e = raizes(&d.join(".phxclaw")).unwrap_err();
        assert!(e.contains("nao-existe"), "{e}");
    }

    #[test]
    fn raiz_relativa_resolve_contra_o_projeto_e_repetida_entra_uma_vez() {
        let d = pasta("rel");
        std::fs::create_dir_all(d.join("outra")).unwrap();
        let abs = std::fs::canonicalize(d.join("outra")).unwrap();
        std::fs::write(
            d.join(".phxclaw/workspace.json"),
            format!(r#"{{"raizes":["outra", "{}", "./outra"]}}"#, abs.display()),
        )
        .unwrap();
        assert_eq!(raizes(&d.join(".phxclaw")).unwrap(), vec![abs.clone()]);
        assert_eq!(
            variavel(&[abs.clone(), abs.clone()]),
            format!("{0}:{0}", abs.display())
        );
    }
}

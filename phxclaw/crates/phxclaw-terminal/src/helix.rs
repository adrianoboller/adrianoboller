//! Onde esta o Helix e como ele sobe: UM lugar para o IDE de mesa (Tauri) e para o IDE no
//! navegador (websocket do agente). Duas montagens divergiriam no dia em que uma ganhasse
//! uma variavel (o runtime, as raizes do workspace) e a outra nao.

use crate::Programa;
use std::path::{Path, PathBuf};

/// O hx instalado pelo `tools/instalar_helix.sh` fica em /opt/helix com o runtime ao lado;
/// `pedido` (a chave `desktop.hx` do catalogo, lida por quem chama) aponta outro. Sem
/// nenhum dos dois, o do PATH.
pub fn achar(pedido: Option<PathBuf>) -> Option<(PathBuf, Option<PathBuf>)> {
    if let Some(p) = pedido {
        return p.is_file().then_some((p, None));
    }
    let opt = Path::new("/opt/helix/hx");
    if opt.is_file() {
        let runtime = Path::new("/opt/helix/runtime");
        return Some((opt.into(), runtime.is_dir().then(|| runtime.into())));
    }
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|d| d.join("hx"))
            .find(|p| p.is_file())
            .map(|p| (p, None))
    })
}

/// O texto que a tela mostra quando nao ha hx: diz o que fazer, nao so que faltou.
pub const SEM_HELIX: &str = "hx nao encontrado: rode phxclaw/tools/instalar_helix.sh (instala em /opt/helix) \
                             ou aponte a chave desktop.hx (PHXCLAW_HX) para o executavel";

/// `hx .` na pasta `cwd`, com o runtime na variavel quando ele esta ao lado do binario.
/// `env` entra por cima (as raizes do workspace, a URL da completacao por IA).
/// `pedido` e o executavel escolhido na configuracao, se houver.
pub fn programa(
    cwd: &Path,
    pedido: Option<PathBuf>,
    env: Vec<(String, String)>,
) -> Result<Programa, String> {
    let (hx, runtime) = achar(pedido).ok_or(SEM_HELIX)?;
    let mut todas = Vec::with_capacity(env.len() + 1);
    if let Some(r) = runtime {
        todas.push(("HELIX_RUNTIME".to_string(), r.display().to_string()));
    }
    todas.extend(env);
    Ok(Programa {
        programa: hx.display().to_string(),
        args: vec![".".into()],
        cwd: Some(cwd.into()),
        env: todas,
    })
}

/// A linha de estado do Helix (a de baixo, por padrao): `NOR   src/main.rs[+]  ...`. Devolve
/// o arquivo corrente, ou `None` quando nenhuma linha da tela tem a forma do modo + arquivo.
/// E a mesma leitura que a tela faz da grade (app.js, `arquivoDaGrade`): o Helix 25.07 nao
/// expoe o documento aberto por nenhum outro canal (nem titulo, nem variavel), medido no
/// fonte dele -- so a statusline o diz.
pub fn arquivo_corrente(texto_da_tela: &str) -> Option<String> {
    texto_da_tela.lines().rev().find_map(|l| {
        let mut partes = l.split_whitespace();
        let modo = partes.next()?;
        if !matches!(modo, "NOR" | "INS" | "SEL") {
            return None;
        }
        // O spinner do LSP (braille U+2800..U+28FF) fica entre o modo e o arquivo enquanto o
        // servidor indexa: nao e nome de arquivo.
        let arquivo =
            partes.find(|t| !t.chars().all(|c| ('\u{2800}'..='\u{28ff}').contains(&c)))?;
        let arquivo = arquivo.trim_end_matches("[+]");
        // «[scratch]» e o buffer sem arquivo; nome com separador ou extensao e arquivo.
        (!arquivo.starts_with('[') && arquivo != "sel").then(|| arquivo.to_string())
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_statusline_do_helix_diz_o_arquivo() {
        let tela = "fn main() {}\n~\n~\nNOR   src/main.rs[+]                    1 sel  1:1\n";
        assert_eq!(arquivo_corrente(tela).as_deref(), Some("src/main.rs"));
        assert_eq!(
            arquivo_corrente("INS   Cargo.toml   1 sel  3:4").as_deref(),
            Some("Cargo.toml")
        );
        // O spinner do LSP indexando (medido no agente real: «NOR ⣻ src/main.rs»).
        assert_eq!(
            arquivo_corrente("NOR ⣻ src/main.rs   1 sel  1:1").as_deref(),
            Some("src/main.rs")
        );
        // Buffer sem arquivo e tela sem statusline: nada, em vez de um nome inventado.
        assert_eq!(arquivo_corrente("NOR   [scratch]   1 sel  1:1"), None);
        assert_eq!(arquivo_corrente("phxclaw$ ls\nCargo.toml"), None);
    }

    #[test]
    fn o_programa_do_helix_leva_o_cwd_e_o_ambiente_pedido() {
        if achar(None).is_none() {
            return; // hospedeiro sem hx: nada a provar aqui
        }
        let p = programa(Path::new("/tmp"), None, vec![("X".into(), "1".into())]).unwrap();
        assert_eq!(p.args, vec![".".to_string()]);
        assert_eq!(p.cwd.as_deref(), Some(Path::new("/tmp")));
        assert!(p.env.iter().any(|(k, v)| k == "X" && v == "1"));
    }
}

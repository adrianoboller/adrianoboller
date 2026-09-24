//! O nome de entrada que pode virar caminho no disco -- a recusa de zip-slip
//! mora AQUI, no motor, e nao em quem extrai.
//!
//! O terminal e a web extraem arquivos. Se cada um conferisse o nome por
//! conta propria, o dia em que um esquecesse seria a porta dos fundos: uma
//! entrada `../../.bashrc` escrita fora da pasta de destino. Um motor so, uma
//! decisao so (lei «funcao e comando nao se duplicam»).

use alloc::string::String;

use crate::erro::{Erro, Resultado};

/// Normaliza o nome gravado no 7z para um caminho relativo seguro, com `/`.
///
/// Recusa: vazio, absoluto (`/x`, `\x`), letra de unidade (`C:`), qualquer
/// componente `..`, NUL e caractere de controle. `\` conta como separador,
/// porque um 7z gravado no Windows pode trazer, e porque tratar `a\..\..\x`
/// como um nome so de arquivo abriria a porta no Windows, onde ele E caminho.
/// Componentes `.` e vazios (`a//b`) somem.
pub fn caminho_seguro(nome: &str) -> Resultado<String> {
    if nome.is_empty() || nome.chars().any(|c| c == '\0' || c.is_control()) {
        return Err(Erro::CaminhoInseguro);
    }
    let n = nome.replace('\\', "/");
    if n.starts_with('/') {
        return Err(Erro::CaminhoInseguro);
    }
    let b = n.as_bytes();
    if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
        return Err(Erro::CaminhoInseguro);
    }
    let mut saida = String::with_capacity(n.len());
    for parte in n.split('/') {
        match parte {
            "" | "." => continue,
            ".." => return Err(Erro::CaminhoInseguro),
            _ => {
                if !saida.is_empty() {
                    saida.push('/');
                }
                saida.push_str(parte);
            }
        }
    }
    if saida.is_empty() {
        return Err(Erro::CaminhoInseguro);
    }
    Ok(saida)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn recusa_os_que_saem_da_pasta() {
        for ruim in [
            "../x",
            "a/../../x",
            "..",
            "/etc/passwd",
            "\\x",
            "C:\\x",
            "c:x",
            "a\\..\\..\\x",
            "",
            "a\0b",
            "./..",
        ] {
            assert_eq!(caminho_seguro(ruim), Err(Erro::CaminhoInseguro), "{ruim:?}");
        }
    }

    #[test]
    fn aceita_e_normaliza_os_bons() {
        assert_eq!(caminho_seguro("a/b.txt").unwrap(), "a/b.txt");
        assert_eq!(caminho_seguro("a\\b.txt").unwrap(), "a/b.txt");
        assert_eq!(caminho_seguro("./a//b/").unwrap(), "a/b");
        assert_eq!(caminho_seguro("..a/b..").unwrap(), "..a/b..");
        assert_eq!(
            caminho_seguro("configuração.json").unwrap(),
            "configuração.json"
        );
    }
}

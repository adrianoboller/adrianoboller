//! Base64 (RFC 4648), sem dependencias externas.
//!
//! # Base64 NAO e criptografia
//!
//! Isto e CODIFICACAO, nao cifra. Quem capturar o trafego decodifica com um
//! comando:
//!
//! ```text
//! $ echo 'YWRyaWFubzpzZW5oYTEyMw==' | base64 -d
//! adriano:senha123
//! ```
//!
//! O que ele resolve de verdade, e por isso esta aqui:
//!
//! * senha some do `grep` casual e do olho de quem passa atras da cadeira;
//! * senha com aspas, barra invertida ou byte estranho atravessa o JSON sem
//!   escape nenhum;
//! * o campo fica de tamanho previsivel no log e no dump de pacote.
//!
//! O que **nao** resolve: confidencialidade na rede. Para isso ha o
//! desafio-resposta (`phxsql_core::desafio`), onde a senha nunca atravessa o
//! fio, e o tunel (IPSec, WireGuard) para o resto do trafego.

use crate::error::{PhxError, Result};

const ALFABETO: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Codifica em Base64 com preenchimento (`=`).
pub fn codificar(dados: &[u8]) -> String {
    let mut saida = String::with_capacity(dados.len().div_ceil(3) * 4);
    for pedaco in dados.chunks(3) {
        let b0 = pedaco[0] as u32;
        let b1 = *pedaco.get(1).unwrap_or(&0) as u32;
        let b2 = *pedaco.get(2).unwrap_or(&0) as u32;
        let junto = (b0 << 16) | (b1 << 8) | b2;

        saida.push(ALFABETO[(junto >> 18) as usize & 0x3F] as char);
        saida.push(ALFABETO[(junto >> 12) as usize & 0x3F] as char);
        saida.push(if pedaco.len() > 1 {
            ALFABETO[(junto >> 6) as usize & 0x3F] as char
        } else {
            '='
        });
        saida.push(if pedaco.len() > 2 {
            ALFABETO[junto as usize & 0x3F] as char
        } else {
            '='
        });
    }
    saida
}

fn valor(c: u8) -> Option<u32> {
    Some(match c {
        b'A'..=b'Z' => (c - b'A') as u32,
        b'a'..=b'z' => (c - b'a') as u32 + 26,
        b'0'..=b'9' => (c - b'0') as u32 + 52,
        // Aceita tambem o alfabeto "URL-safe" da RFC 4648, secao 5.
        b'+' | b'-' => 62,
        b'/' | b'_' => 63,
        _ => return None,
    })
}

/// Decodifica Base64. Ignora espaco e quebra de linha; recusa o resto.
pub fn decodificar(texto: &str) -> Result<Vec<u8>> {
    let limpo: Vec<u8> = texto.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    let sem_pad: Vec<u8> = limpo.iter().copied().take_while(|c| *c != b'=').collect();
    // Depois do primeiro '=' so pode vir '='.
    if limpo[sem_pad.len()..].iter().any(|c| *c != b'=') {
        return Err(PhxError::Tipo("base64 com dado depois do padding".into()));
    }
    // Quantos bytes de dado sobram no ultimo grupo decide o padding possivel.
    let padding_esperado = match sem_pad.len() % 4 {
        0 => 0,
        2 => 2,
        3 => 1,
        _ => return Err(PhxError::Tipo("base64 com comprimento invalido".into())),
    };
    // Entrada sem padding e aceita; com padding, ele tem de estar certo.
    let padding = limpo.len() - sem_pad.len();
    if padding != 0 && (padding != padding_esperado || limpo.len() % 4 != 0) {
        return Err(PhxError::Tipo("base64 com padding invalido".into()));
    }

    let mut saida = Vec::with_capacity(sem_pad.len() / 4 * 3);
    for grupo in sem_pad.chunks(4) {
        let mut junto = 0u32;
        for (i, c) in grupo.iter().enumerate() {
            let v = valor(*c)
                .ok_or_else(|| PhxError::Tipo(format!("base64 invalido: {:?}", *c as char)))?;
            junto |= v << (18 - 6 * i);
        }
        saida.push((junto >> 16) as u8);
        if grupo.len() > 2 {
            saida.push((junto >> 8) as u8);
        }
        if grupo.len() > 3 {
            saida.push(junto as u8);
        }
    }
    Ok(saida)
}

/// Decodifica e interpreta como UTF-8.
pub fn decodificar_texto(texto: &str) -> Result<String> {
    String::from_utf8(decodificar(texto)?)
        .map_err(|e| PhxError::Tipo(format!("base64 nao contem UTF-8 valido: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vetores da RFC 4648, secao 10.
    #[test]
    fn vetores_rfc4648() {
        for (cru, codificado) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(codificar(cru.as_bytes()), codificado, "codificando {cru:?}");
            assert_eq!(
                decodificar_texto(codificado).unwrap(),
                cru,
                "decodificando {codificado:?}"
            );
        }
    }

    #[test]
    fn ida_e_volta_com_todos_os_bytes() {
        let todos: Vec<u8> = (0..=255u8).collect();
        assert_eq!(decodificar(&codificar(&todos)).unwrap(), todos);
    }

    #[test]
    fn credencial_de_verdade() {
        let c = codificar(b"adriano:senha123");
        assert_eq!(c, "YWRyaWFubzpzZW5oYTEyMw==");
        assert_eq!(decodificar_texto(&c).unwrap(), "adriano:senha123");
    }

    #[test]
    fn senha_com_acento_e_caractere_dificil() {
        for s in [
            "Ação do José 2026!",
            "senha\"com'aspas\\e barra",
            "quebra\nde linha",
            "",
        ] {
            let c = codificar(s.as_bytes());
            assert_eq!(decodificar_texto(&c).unwrap(), s, "falhou em {s:?}");
        }
    }

    #[test]
    fn aceita_sem_padding_mas_exige_padding_correto() {
        // Sem padding e aceito (aparece muito em API e em URL).
        assert_eq!(decodificar_texto("Zm9vYmE").unwrap(), "fooba");
        assert_eq!(decodificar_texto("Zg").unwrap(), "f");
        // Com padding, ele tem de estar certo.
        assert_eq!(decodificar_texto("Zm9vYmE=").unwrap(), "fooba");
        assert!(decodificar("Zm9vYmE==").is_err());
    }

    #[test]
    fn ignora_espaco_e_quebra_de_linha() {
        assert_eq!(decodificar_texto("Zm9v YmFy").unwrap(), "foobar");
        assert_eq!(decodificar_texto("Zm9v\nYmFy\n").unwrap(), "foobar");
    }

    #[test]
    fn aceita_o_alfabeto_url_safe() {
        // 0xFB 0xFF codifica como "+/8=" no alfabeto padrao e "-_8=" no URL-safe.
        assert_eq!(decodificar("-_8=").unwrap(), decodificar("+/8=").unwrap());
    }

    #[test]
    fn entradas_invalidas_sao_recusadas() {
        for ruim in [
            "Z",             // grupo de 1 byte nao existe
            "Zm9vYmFy!",     // caractere fora do alfabeto
            "Zg==Zg==",      // dado depois do padding
            "Zm 9v Ym Fy =", // padding num grupo que ja esta completo
            "Zg=",           // padding de menos para o que sobra
        ] {
            assert!(decodificar(ruim).is_err(), "deveria recusar: {ruim:?}");
        }
    }

    #[test]
    fn base64_nao_esconde_nada_de_quem_captura() {
        // Este teste existe como documentacao executavel: qualquer um decodifica.
        let interceptado = codificar(b"adriano:senha123");
        assert_eq!(
            decodificar_texto(&interceptado).unwrap(),
            "adriano:senha123",
            "Base64 e codificacao, nao cifra -- quem tem o texto tem a senha"
        );
    }
}

//! Tecla do navegador (`KeyboardEvent.key` + modificadores) para os bytes que um terminal
//! xterm mandaria.
//!
//! A traducao mora aqui, e nao no JavaScript da tela, por dois motivos: depende do MODO
//! do terminal (seta em modo de cursor de aplicacao vira `ESC O A`, nao `ESC [ A`), e esse
//! modo so o emulador conhece; e qualquer outra frente que precise mandar tecla (teste,
//! agente, outra tela) chama a mesma funcao em vez de repetir a tabela.

use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Tecla {
    /// O `KeyboardEvent.key`: "a", "Enter", "ArrowUp", "F5"...
    pub tecla: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub meta: bool,
}

impl Tecla {
    pub fn simples(tecla: &str) -> Self {
        Self {
            tecla: tecla.into(),
            ..Self::default()
        }
    }
}

/// Parametro de modificador do xterm: 1 + shift + 2*alt + 4*ctrl.
fn modificador(t: &Tecla) -> u8 {
    1 + u8::from(t.shift) + 2 * u8::from(t.alt) + 4 * u8::from(t.ctrl)
}

/// `None` para o que nao vira byte (Shift sozinho, tecla morta, atalho do sistema com
/// Meta): a tela deixa o navegador tratar.
pub fn codificar(t: &Tecla, cursor_de_aplicacao: bool) -> Option<Vec<u8>> {
    if t.meta {
        return None;
    }
    let m = modificador(t);
    // Teclas com final de letra (setas, Home, End, F1-F4): sem modificador, o modo de
    // cursor decide entre SS3 e CSI; com modificador e sempre CSI 1;m.
    let letra = match t.tecla.as_str() {
        "ArrowUp" => Some(b'A'),
        "ArrowDown" => Some(b'B'),
        "ArrowRight" => Some(b'C'),
        "ArrowLeft" => Some(b'D'),
        "Home" => Some(b'H'),
        "End" => Some(b'F'),
        "F1" => Some(b'P'),
        "F2" => Some(b'Q'),
        "F3" => Some(b'R'),
        "F4" => Some(b'S'),
        _ => None,
    };
    if let Some(l) = letra {
        let funcao = matches!(l, b'P'..=b'S');
        return Some(if m > 1 {
            format!("\x1b[1;{m}{}", l as char).into_bytes()
        } else if funcao || cursor_de_aplicacao {
            vec![0x1b, b'O', l]
        } else {
            vec![0x1b, b'[', l]
        });
    }
    let til = match t.tecla.as_str() {
        "Insert" => Some(2),
        "Delete" => Some(3),
        "PageUp" => Some(5),
        "PageDown" => Some(6),
        "F5" => Some(15),
        "F6" => Some(17),
        "F7" => Some(18),
        "F8" => Some(19),
        "F9" => Some(20),
        "F10" => Some(21),
        "F11" => Some(23),
        "F12" => Some(24),
        _ => None,
    };
    if let Some(n) = til {
        return Some(if m > 1 {
            format!("\x1b[{n};{m}~").into_bytes()
        } else {
            format!("\x1b[{n}~").into_bytes()
        });
    }
    let mut saida = match t.tecla.as_str() {
        "Enter" => vec![b'\r'],
        "Tab" if t.shift => return Some(b"\x1b[Z".to_vec()),
        "Tab" => vec![b'\t'],
        "Escape" => vec![0x1b],
        "Backspace" if t.ctrl => vec![0x08],
        "Backspace" => vec![0x7f],
        k => {
            let mut chars = k.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                // "Shift", "Control", "Dead", "Unidentified", "CapsLock"...
                return None;
            }
            if t.ctrl {
                vec![controle(c)?]
            } else {
                let mut b = [0u8; 4];
                c.encode_utf8(&mut b).as_bytes().to_vec()
            }
        }
    };
    if t.alt {
        saida.insert(0, 0x1b);
    }
    Some(saida)
}

/// Ctrl+tecla no xterm: as letras caem em 0x01..0x1a, e os simbolos de borda da tabela
/// ASCII nos codigos que sobram.
fn controle(c: char) -> Option<u8> {
    Some(match c {
        'a'..='z' => c as u8 - b'a' + 1,
        'A'..='Z' => c as u8 - b'A' + 1,
        '@' | ' ' | '2' => 0,
        '[' | '3' => 0x1b,
        '\\' | '4' => 0x1c,
        ']' | '5' => 0x1d,
        '^' | '6' => 0x1e,
        '_' | '-' | '7' => 0x1f,
        '?' | '8' => 0x7f,
        _ => return None,
    })
}

/// Texto colado: em modo de colagem delimitada, entre `ESC[200~` e `ESC[201~`, e sem ESC
/// nenhum dentro. Sem tirar o ESC, um texto colado com `ESC[201~` no meio fecharia a
/// delimitacao e o resto seria executado como se tivesse sido digitado.
pub fn colar(texto: &str, delimitada: bool) -> Vec<u8> {
    let limpo: String = texto
        .chars()
        .filter(|&c| c != '\x1b')
        .collect::<String>()
        .replace("\r\n", "\r")
        .replace('\n', "\r");
    if delimitada {
        let mut v = b"\x1b[200~".to_vec();
        v.extend_from_slice(limpo.as_bytes());
        v.extend_from_slice(b"\x1b[201~");
        v
    } else {
        limpo.into_bytes()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn t(tecla: &str, ctrl: bool, alt: bool, shift: bool) -> Tecla {
        Tecla {
            tecla: tecla.into(),
            ctrl,
            alt,
            shift,
            meta: false,
        }
    }

    #[test]
    fn seta_depende_do_modo_de_cursor() {
        let up = Tecla::simples("ArrowUp");
        assert_eq!(codificar(&up, false).unwrap(), b"\x1b[A");
        assert_eq!(codificar(&up, true).unwrap(), b"\x1bOA");
        assert_eq!(
            codificar(&t("ArrowLeft", true, false, false), true).unwrap(),
            b"\x1b[1;5D"
        );
    }

    #[test]
    fn controle_alt_e_especiais() {
        assert_eq!(codificar(&t("c", true, false, false), false).unwrap(), [3]);
        assert_eq!(codificar(&t("C", true, false, true), false).unwrap(), [3]);
        assert_eq!(
            codificar(&t("x", false, true, false), false).unwrap(),
            b"\x1bx"
        );
        assert_eq!(codificar(&Tecla::simples("Enter"), false).unwrap(), b"\r");
        assert_eq!(
            codificar(&Tecla::simples("Backspace"), false).unwrap(),
            [0x7f]
        );
        assert_eq!(
            codificar(&t("Tab", false, false, true), false).unwrap(),
            b"\x1b[Z"
        );
        assert_eq!(
            codificar(&Tecla::simples("F5"), false).unwrap(),
            b"\x1b[15~"
        );
        assert_eq!(codificar(&Tecla::simples("F1"), false).unwrap(), b"\x1bOP");
        assert_eq!(
            codificar(&t("Delete", false, false, true), false).unwrap(),
            b"\x1b[3;2~"
        );
        assert_eq!(
            codificar(&Tecla::simples("ç"), false).unwrap(),
            "ç".as_bytes()
        );
        assert_eq!(codificar(&Tecla::simples("Shift"), false), None);
        let mut cmd = Tecla::simples("c");
        cmd.meta = true;
        assert_eq!(codificar(&cmd, false), None);
    }

    #[test]
    fn colagem_nao_fecha_a_delimitacao_por_dentro() {
        let v = colar("ls\x1b[201~; rm -rf x\n", true);
        assert_eq!(v, b"\x1b[200~ls[201~; rm -rf x\r\x1b[201~");
        assert_eq!(colar("a\r\nb", false), b"a\rb");
    }
}

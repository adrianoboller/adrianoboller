//! Resolucao de cor de celula para RGB.
//!
//! A grade sai daqui com a cor ja resolvida (0xRRGGBB) em vez do enum do VT: assim a tela
//! nao precisa conhecer paleta nenhuma, e o OSC 4/10/11 que o programa mandar (sobrescrever
//! a cor 1, por exemplo) vale do mesmo jeito para quem desenha e para quem responde a
//! consulta de cor. Uma resolucao so, nos dois caminhos.

use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};

/// Fundo da marca (`#010418`): o terminal mora dentro da janela do PhxClaw e nao pode
/// aparecer como um recorte preto de outra aplicacao.
pub const FUNDO_PADRAO: Rgb = Rgb {
    r: 0x01,
    g: 0x04,
    b: 0x18,
};
pub const TEXTO_PADRAO: Rgb = Rgb {
    r: 0xe6,
    g: 0xed,
    b: 0xf3,
};
pub const CURSOR_PADRAO: Rgb = Rgb {
    r: 0xf7,
    g: 0xc2,
    b: 0x4a,
};

/// As 16 cores ANSI. Clareadas o bastante para passar de 4,5:1 sobre o fundo da marca,
/// que e quase preto-azulado: o vermelho e o azul classicos (0x80/0xcd) somem nele.
const ANSI16: [u32; 16] = [
    0x1b2433, 0xff6b6b, 0x57e6a8, 0xf7c24a, 0x5ea8ff, 0xc792ea, 0x36d7ff, 0xc5d1de, 0x5c6b80,
    0xff8f8f, 0x86f2c0, 0xffd77a, 0x8cc4ff, 0xdcb3ff, 0x7fe6ff, 0xffffff,
];

fn rgb(v: u32) -> Rgb {
    Rgb {
        r: (v >> 16) as u8,
        g: (v >> 8) as u8,
        b: v as u8,
    }
}

pub fn para_u32(c: Rgb) -> u32 {
    (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b)
}

/// Cor de indice 0..=255 da paleta xterm: 16 ANSI, cubo 6x6x6 e rampa de cinza.
pub fn indice_padrao(i: u8) -> Rgb {
    match i {
        0..=15 => rgb(ANSI16[usize::from(i)]),
        16..=231 => {
            let n = i - 16;
            let nivel = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Rgb {
                r: nivel(n / 36),
                g: nivel((n / 6) % 6),
                b: nivel(n % 6),
            }
        }
        _ => {
            let v = 8 + (i - 232) * 10;
            Rgb { r: v, g: v, b: v }
        }
    }
}

fn escurecer(c: Rgb) -> Rgb {
    // Mesmo fator do alacritty (2/3): "dim" e a mesma cor com menos luz, nao outra cor.
    Rgb {
        r: (u16::from(c.r) * 2 / 3) as u8,
        g: (u16::from(c.g) * 2 / 3) as u8,
        b: (u16::from(c.b) * 2 / 3) as u8,
    }
}

/// Cor de um indice da tabela do terminal (0..269), com a sobrescrita do programa valendo.
pub fn por_indice(cores: &Colors, i: usize) -> Rgb {
    if let Some(c) = cores[i] {
        return c;
    }
    match i {
        0..=255 => indice_padrao(i as u8),
        x if x == NamedColor::Foreground as usize => TEXTO_PADRAO,
        x if x == NamedColor::Background as usize => FUNDO_PADRAO,
        x if x == NamedColor::Cursor as usize => CURSOR_PADRAO,
        x if x == NamedColor::BrightForeground as usize => rgb(0xffffff),
        x if x == NamedColor::DimForeground as usize => escurecer(TEXTO_PADRAO),
        // DimBlack..DimWhite vem logo depois de Cursor, na ordem das 8 cores base.
        x if (NamedColor::DimBlack as usize..=NamedColor::DimWhite as usize).contains(&x) => {
            escurecer(por_indice(cores, x - NamedColor::DimBlack as usize))
        }
        _ => TEXTO_PADRAO,
    }
}

pub fn resolver(cores: &Colors, c: Color) -> Rgb {
    match c {
        Color::Spec(rgb) => rgb,
        Color::Indexed(i) => por_indice(cores, usize::from(i)),
        Color::Named(n) => por_indice(cores, n as usize),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cubo_e_rampa_da_paleta_xterm() {
        assert_eq!(para_u32(indice_padrao(16)), 0x000000);
        assert_eq!(para_u32(indice_padrao(231)), 0xffffff);
        assert_eq!(para_u32(indice_padrao(196)), 0xff0000);
        assert_eq!(para_u32(indice_padrao(232)), 0x080808);
        assert_eq!(para_u32(indice_padrao(255)), 0xeeeeee);
    }

    #[test]
    fn sobrescrita_do_programa_vale_e_dim_escurece_a_base() {
        let mut cores = Colors::default();
        assert_eq!(
            resolver(&cores, Color::Named(NamedColor::Background)),
            FUNDO_PADRAO
        );
        cores[1] = Some(Rgb { r: 1, g: 2, b: 3 });
        assert_eq!(
            resolver(&cores, Color::Indexed(1)),
            Rgb { r: 1, g: 2, b: 3 }
        );
        let dim = resolver(&cores, Color::Named(NamedColor::DimRed));
        assert_eq!(dim, Rgb { r: 0, g: 1, b: 2 });
    }
}

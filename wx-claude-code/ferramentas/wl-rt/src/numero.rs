//! Round, IntegerPart e NumToString sobre reais (Help 3050063, 3050008, 3024024).
//! Para dinheiro use `Moeda`; estas sao para o `real` do legado, com o cuidado de
//! recuperar os 15 digitos significativos antes de arredondar.

/// Round(<real>, <casas>): metade para longe do zero, sobre o valor com 15 digitos
/// significativos -- e o que faz Round(1500.005, 2) dar 1500.01 aqui como la.
pub fn round(x: f64, casas: u32) -> f64 {
    let pre: f64 = format!("{:.15e}", x).parse().unwrap_or(x);
    let f = 10_f64.powi(casas as i32);
    let v = (pre * f).abs() + 0.5;
    let r = (v.floor()) / f;
    if pre < 0.0 {
        -r
    } else {
        r
    }
}

/// IntegerPart (3050008): o maior inteiro menor ou igual; -3.5 da -4.
pub fn parte_inteira(x: f64) -> f64 {
    x.floor()
}

/// NumToString(<numero>[, <mascara>]) (3024024). Mascara: [-][+][0]<largura>[.|,]<casas>(d|f)[S].
/// Sem mascara: inteiro sem casas, real como o WLanguage mostra ("1.23").
pub fn num_para_texto(x: f64, mascara: &str) -> String {
    if mascara.is_empty() {
        return if x.fract() == 0.0 && x.abs() < 1e15 {
            format!("{}", x as i64)
        } else {
            format!("{}", x)
        };
    }
    let mut m = mascara;
    let (mut esq, mut sinal, mut zeros, mut milhar, mut virgula) =
        (false, false, false, false, false);
    loop {
        if let Some(r) = m.strip_prefix('-') {
            esq = true;
            m = r;
        } else if let Some(r) = m.strip_prefix('+') {
            sinal = true;
            m = r;
        } else if let Some(r) = m.strip_prefix('0') {
            zeros = true;
            m = r;
        } else {
            break;
        }
    }
    if let Some(r) = m.strip_suffix('S') {
        milhar = true;
        m = r;
    }
    let real = m.ends_with('f');
    let m = m.trim_end_matches(['d', 'f']);
    let (largura, casas): (usize, u32) = match m.split_once(['.', ',']) {
        Some((l, c)) => {
            virgula = mascara.contains(',');
            (l.parse().unwrap_or(0), c.parse().unwrap_or(0))
        }
        None => (m.parse().unwrap_or(0), 0),
    };
    let casas = if real { casas } else { 0 };
    let v = round(x, casas);
    let neg = v < 0.0;
    let mut corpo = format!("{:.*}", casas as usize, v.abs());
    if milhar {
        let (i, d) = corpo
            .split_once('.')
            .map(|(a, b)| (a.to_string(), Some(b.to_string())))
            .unwrap_or((corpo.clone(), None));
        let mut agrupado = String::new();
        for (k, ch) in i.chars().enumerate() {
            if k > 0 && (i.len() - k) % 3 == 0 {
                agrupado.push(' ');
            }
            agrupado.push(ch);
        }
        corpo = match d {
            Some(d) => format!("{agrupado}.{d}"),
            None => agrupado,
        };
    }
    if virgula {
        corpo = corpo.replace('.', ",");
    }
    let prefixo = if neg {
        "-"
    } else if sinal {
        "+"
    } else {
        ""
    };
    let falta = largura.saturating_sub(prefixo.len() + corpo.len());
    if esq {
        format!("{prefixo}{corpo}{}", " ".repeat(falta))
    } else if zeros {
        format!("{prefixo}{}{corpo}", "0".repeat(falta))
    } else {
        format!("{}{prefixo}{corpo}", " ".repeat(falta))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn round_do_help_3050063() {
        assert_eq!(round(-16.238167, 2), -16.24);
        assert_eq!(round(1500.005, 2), 1500.01);
        assert_eq!(round(16.999, 0), 17.0);
        assert_eq!(round(2493.80 * 10.0 / 100.0, 2), 249.38);
    }
    #[test]
    fn integerpart_do_help_3050008() {
        assert_eq!(parte_inteira(16.999), 16.0);
        assert_eq!(parte_inteira(-3.5), -4.0);
    }
    #[test]
    fn numtostring_os_exemplos_do_help_3024024() {
        assert_eq!(num_para_texto(123.0, ""), "123");
        assert_eq!(num_para_texto(1.23, ""), "1.23");
        assert_eq!(num_para_texto(123.0, "05d"), "00123");
        assert_eq!(num_para_texto(12345.5, "+10.2f"), " +12345.50");
        assert_eq!(num_para_texto(12345.5, "-+10.2f"), "+12345.50 ");
        assert_eq!(num_para_texto(12345.5, "+010.2f"), "+012345.50");
        assert_eq!(num_para_texto(12345.5, "010.2f"), "0012345.50");
        assert_eq!(num_para_texto(12345.5, "10.2fS"), " 12 345.50");
        assert_eq!(num_para_texto(12345.5, "10,2fS"), " 12 345,50");
        // o do ESTOQUE: NumToString(SALDO.QUANTIDADE, "12.3f")
        assert_eq!(num_para_texto(40.0, "12.3f"), "      40.000");
        assert_eq!(num_para_texto(-7.5, "8.2f"), "   -7.50");
    }
}

//! `currency` (Help 1514043): decimal de 10 bytes, 24 digitos significativos, 17 a
//! esquerda e 6 a direita do ponto. Aqui: i128 escalado por 10^6. Sem f64 no meio.
use std::fmt;

const ESCALA: i128 = 1_000_000;
/// 2^79 - 1: o maior valor em milionesimos que os 10 bytes do currency guardam (Help 1514043).
const MAXIMO: i128 = 604_462_909_807_314_587_353_087;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Hash)]
pub struct Moeda(i128);

impl Moeda {
    pub const ZERO: Moeda = Moeda(0);

    /// Valor em milionesimos (a unidade interna), para quem precisa gravar no banco.
    pub fn milionesimos(self) -> i128 {
        self.0
    }
    pub fn de_milionesimos(v: i128) -> Moeda {
        Moeda(v)
    }
    pub fn de_centavos(c: i64) -> Moeda {
        Moeda(c as i128 * 10_000)
    }
    /// Centavos, arredondados como Round(x, 2): metade para longe do zero.
    pub fn centavos(self) -> i64 {
        self.round(2).0.div_euclid(10_000) as i64
            + if self.round(2).0 < 0 && self.round(2).0 % 10_000 != 0 {
                1
            } else {
                0
            }
    }
    /// Literal decimal ("2493.80", "-16.238167", "0m1234"): sem passar por f64.
    pub fn parse(texto: &str) -> Result<Moeda, String> {
        let t = texto.trim().trim_start_matches("0m");
        let (neg, t) = match t.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, t.strip_prefix('+').unwrap_or(t)),
        };
        let (int, frac) = t.split_once('.').unwrap_or((t, ""));
        if int.is_empty() && frac.is_empty()
            || !int.chars().all(|c| c.is_ascii_digit())
            || !frac.chars().all(|c| c.is_ascii_digit())
        {
            return Err(format!("moeda invalida: {texto}"));
        }
        let mut v: i128 = if int.is_empty() {
            0
        } else {
            int.parse::<i128>().map_err(|e| e.to_string())?
        } * ESCALA;
        let mut f = frac.to_string();
        // a setima casa e alem: o Help da 6; arredonda metade para longe do zero
        let extra = if f.len() > 6 {
            f.split_off(6)
        } else {
            String::new()
        };
        while f.len() < 6 {
            f.push('0');
        }
        v += f.parse::<i128>().unwrap_or(0);
        if extra.chars().next().map(|c| c >= '5').unwrap_or(false) {
            v += 1;
        }
        // O Help fala em "17 digitos a esquerda", mas a faixa que ele mesmo publica vai a
        // 604.462.909.807.314.587,353087 -- 18 digitos: e 2^79 - 1 milionesimos, os 10 bytes.
        // O limite e o VALOR, e e ele que se confere.
        if v > MAXIMO {
            return Err(format!(
                "moeda fora da faixa do currency (±604462909807314587.353087): {texto}"
            ));
        }
        Ok(Moeda(if neg { -v } else { v }))
    }
    /// De um f64 que veio de JSON ou de conta em real: 15 digitos significativos,
    /// que e o que separa 1500.005 (o que o usuario digitou) de 1500.00499999.
    pub fn de_f64(x: f64) -> Moeda {
        Moeda::parse(&format!(
            "{:.6}",
            format!("{:.15e}", x).parse::<f64>().unwrap_or(x)
        ))
        .unwrap_or(Moeda::ZERO)
    }
    pub fn para_f64(self) -> f64 {
        self.0 as f64 / ESCALA as f64
    }
    /// Round(<valor>, <casas>) do Help 3050063: metade para longe do zero.
    /// `Round(-16.238167, 2)` = -16.24.
    pub fn round(self, casas: u32) -> Moeda {
        let casas = casas.min(6);
        let passo = 10_i128.pow(6 - casas);
        let (q, r) = (self.0 / passo, self.0 % passo);
        let mut q = q;
        if r.abs() * 2 >= passo {
            q += if self.0 < 0 { -1 } else { 1 };
        }
        Moeda(q * passo)
    }
    /// IntegerPart (3050008): o maior inteiro menor ou igual -- -3.5 da -4.
    pub fn parte_inteira(self) -> i128 {
        self.0.div_euclid(ESCALA)
    }
    pub fn mul_inteiro(self, n: i64) -> Moeda {
        Moeda(self.0 * n as i128)
    }
    /// Divisao com o resultado ja na precisao da moeda (6 casas), truncado como o
    /// WLanguage faz na atribuicao; quem quer arredondar chama `round` depois.
    pub fn div_inteiro(self, n: i64) -> Moeda {
        Moeda(self.0 / n as i128)
    }
    /// `x * p / 100` (desconto percentual) sem perder as 6 casas.
    pub fn percentual(self, p: Moeda) -> Moeda {
        Moeda(self.0 * p.0 / ESCALA / 100)
    }
}

impl std::ops::Add for Moeda {
    type Output = Moeda;
    fn add(self, o: Moeda) -> Moeda {
        Moeda(self.0 + o.0)
    }
}
impl std::ops::Sub for Moeda {
    type Output = Moeda;
    fn sub(self, o: Moeda) -> Moeda {
        Moeda(self.0 - o.0)
    }
}
impl std::ops::Neg for Moeda {
    type Output = Moeda;
    fn neg(self) -> Moeda {
        Moeda(-self.0)
    }
}
impl std::ops::AddAssign for Moeda {
    fn add_assign(&mut self, o: Moeda) {
        self.0 += o.0;
    }
}
impl std::ops::SubAssign for Moeda {
    fn sub_assign(&mut self, o: Moeda) {
        self.0 -= o.0;
    }
}

impl fmt::Display for Moeda {
    /// Como o Trace do WLanguage mostra: sem zeros a direita, sem ponto quando inteiro.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let neg = self.0 < 0;
        let a = self.0.abs();
        let (i, d) = (a / ESCALA, a % ESCALA);
        let mut s = format!("{}{}", if neg { "-" } else { "" }, i);
        if d != 0 {
            let mut dec = format!("{d:06}");
            while dec.ends_with('0') {
                dec.pop();
            }
            s.push('.');
            s.push_str(&dec);
        }
        f.write_str(&s)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    fn m(s: &str) -> Moeda {
        Moeda::parse(s).unwrap()
    }
    #[test]
    fn round_do_help_3050063_e_metade_para_longe_do_zero() {
        assert_eq!(m("-16.238167").round(2), m("-16.24"));
        assert_eq!(m("2.5").round(0), m("3"));
        assert_eq!(m("-2.5").round(0), m("-3"));
        assert_eq!(m("1500.005").round(2), m("1500.01"), "o que o f64 erraria");
    }
    #[test]
    fn de_f64_recupera_o_que_o_usuario_digitou() {
        assert_eq!(Moeda::de_f64(1500.005), m("1500.005"));
        assert_eq!(Moeda::de_f64(2493.80), m("2493.8"));
        assert_eq!(Moeda::de_f64(0.1 + 0.2), m("0.3"));
    }
    #[test]
    fn seis_casas_e_dezessete_inteiros_como_o_help_1514043() {
        assert_eq!(m("0m1.1234567"), m("1.123457"), "a setima casa arredonda");
        // a faixa publicada pelo Help e a de 2^79 - 1 milionesimos; um a mais e erro
        assert_eq!(
            m("604462909807314587.353087").to_string(),
            "604462909807314587.353087"
        );
        assert_eq!(m("-604462909807314587.353087").milionesimos(), -MAXIMO);
        assert!(Moeda::parse("604462909807314587.353088").is_err());
        assert!(Moeda::parse("abc").is_err());
    }
    #[test]
    fn desconto_e_parcelas_do_estoque_batem_com_o_golden() {
        // BR-001: Round(2493.80 * 10 / 100, 2) = 249.38
        assert_eq!(m("2493.80").percentual(m("10")).round(2), m("249.38"));
        // BR-004: Round(100 / 3, 2) = 33.33 e a ultima leva a diferenca
        let parcela = m("100").div_inteiro(3).round(2);
        assert_eq!(parcela, m("33.33"));
        assert_eq!(m("100") - parcela.mul_inteiro(2), m("33.34"));
        // BR-003: Round(748.14 * 0.02 / 30 * 15, 2) = 7.48
        assert_eq!(
            m("748.14")
                .percentual(m("2"))
                .div_inteiro(30)
                .mul_inteiro(15)
                .round(2),
            m("7.48")
        );
    }
    #[test]
    fn mostra_como_o_trace() {
        assert_eq!(m("1234.500000").to_string(), "1234.5");
        assert_eq!(m("-7").to_string(), "-7");
        assert_eq!(m("0.000001").to_string(), "0.000001");
        assert_eq!(
            m("-0.5").parte_inteira(),
            -1,
            "IntegerPart(-0.5) = -1, Help 3050008"
        );
    }
}

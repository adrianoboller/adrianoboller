//! Os ANEIS da bolha que passou do tamanho maximo (pedido 780).
//!
//! Ordem do dono, 09/10/2026: «o tamanho aumenta com o tempo de execucao ate
//! uma medida maxima que ainda permita ver as outras bolhas; dai comeca a ter
//! mais cores internas ou camadas ate ficar preta».
//!
//! O primeiro trecho e da tela (`ui/aquario.js`, `raioAlvo`: o raio e
//! logaritmico e para de crescer aos [`TETO_DO_RAIO_MS`]). O segundo mora
//! AQUI, e so aqui: o `aquario.log` tem de registrar a troca de anel sem
//! ninguem olhando (item 5 do dono, «o log fica»), e por isso quem decide o
//! anel e o servidor. A tela desenha o `anel` que o retrato manda; na volta
//! de cinco minutos, onde nao ha retrato, ela le os [`limites`] que o mesmo
//! retrato publica em `limiares.aneis_ms` -- uma tabela, nao uma segunda
//! formula.
//!
//! # A escala: cada DOBRA de tempo alem do teto e um anel
//!
//! Anel `k` (1..=[`ANEIS`]) quando a tarefa roda ha `30 s × 2^k` ou mais:
//!
//! | anel | rodando ha |
//! |---|---|
//! | 1 | 1 min |
//! | 2 | 2 min |
//! | 3 | 4 min |
//! | 4 | 8 min |
//! | 5 (preta) | 16 min |
//!
//! Por que dobra, e nao um anel por minuto: o raio ja e logaritmico ate o
//! teto, e o anel continua a MESMA regua depois dele -- cada anel diz
//! «demorou o dobro», como cada passo do raio dizia «demorou dez vezes». Um
//! anel por minuto pintaria de preto aos seis minutos uma carga de
//! madrugada que e normal levar quinze, e passaria a dizer a mesma coisa
//! para seis minutos e para seis horas.
//!
//! Por que cinco: com o raio no teto (46 px) cinco aneis ficam a ~7,7 px um
//! do outro, e com a escala do tanque encolhendo tudo pela metade ainda a
//! ~3,8 px -- seis ou mais viravam um borrao, que deixa de ser FORMA. E 16
//! minutos de uma unica operacao e o ponto em que, nesta casa, ela ja passou
//! de qualquer prazo de transacao: preta e «olhe esta agora».

/// O tempo em que o raio da bolha para de crescer. O MESMO numero do
/// `msGrande` do `ui/aquario.js`: um teste le o literal de la e compara.
pub const TETO_DO_RAIO_MS: u64 = 30_000;

/// Quantos aneis ate a bolha inteira preta.
pub const ANEIS: u8 = 5;

/// O tempo de cada anel: `limites()[k-1]` e quando o anel `k` aparece.
pub fn limites() -> [u64; ANEIS as usize] {
    let mut v = [0u64; ANEIS as usize];
    for (i, l) in v.iter_mut().enumerate() {
        *l = TETO_DO_RAIO_MS << (i + 1);
    }
    v
}

/// O anel de uma tarefa que roda ha `ms` (relogio de parede, o mesmo `ms`
/// do retrato que o raio usa). Zero ate o primeiro limite.
pub fn anel(ms: u64) -> u8 {
    limites().iter().filter(|l| ms >= **l).count() as u8
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_tabela_dos_aneis() {
        assert_eq!(limites(), [60_000, 120_000, 240_000, 480_000, 960_000]);
        for (ms, k) in [
            (0, 0),
            (29_999, 0),
            (30_000, 0),
            (59_999, 0),
            (60_000, 1),
            (119_999, 1),
            (120_000, 2),
            (240_000, 3),
            (480_000, 4),
            (959_999, 4),
            (960_000, 5),
            (86_400_000, 5),
            (u64::MAX, 5),
        ] {
            assert_eq!(anel(ms), k, "{ms} ms");
        }
    }

    /// O teto do raio e um numero so: o da tela tem de ser este.
    #[test]
    fn o_teto_do_raio_e_o_mesmo_da_tela() {
        let js = include_str!("../../ui/aquario.js");
        let i = js.find("msGrande:").expect("msGrande sumiu do aquario.js");
        let resto = &js[i + "msGrande:".len()..];
        let num: String = resto
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '_')
            .filter(|c| *c != '_')
            .collect();
        assert_eq!(num.parse::<u64>().ok(), Some(TETO_DO_RAIO_MS));
    }
}

//! Datas no formato do WLanguage: texto AAAAMMDD (Help 1514066, 3027001, 3027003, 3027026).
use std::time::{SystemTime, UNIX_EPOCH};

fn partes(d: &str) -> Option<(i64, i64, i64)> {
    let d = d.trim();
    if d.len() != 8 || !d.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((
        d[0..4].parse().ok()?,
        d[4..6].parse().ok()?,
        d[6..8].parse().ok()?,
    ))
}

fn bissexto(a: i64) -> bool {
    a % 4 == 0 && (a % 100 != 0 || a % 400 == 0)
}

fn dias_no_mes(a: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if bissexto(a) => 29,
        2 => 28,
        _ => 0,
    }
}

/// DateValid (3027003): entre 01/01/0001 e 31/12/9999; "20012512" e falso, "2001" e falso.
pub fn data_valida(d: &str) -> bool {
    match partes(d) {
        Some((a, m, dia)) => {
            (1..=9999).contains(&a) && (1..=12).contains(&m) && dia >= 1 && dia <= dias_no_mes(a, m)
        }
        None => false,
    }
}

/// Dias desde 1970-01-01 (calendario civil proleptico), a base de DateDifference e da soma de dias.
fn dias_civis(d: &str) -> Option<i64> {
    if !data_valida(d) {
        return None;
    }
    let (y, m, dia) = partes(d)?;
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + dia - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe - 719468)
}

fn de_dias_civis(z: i64) -> String {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}{m:02}{d:02}")
}

/// DateDifference(<inicio>, <fim>) (3027001): dias de <fim> menos <inicio>; negativo se o fim vem antes.
/// Data invalida e erro aqui -- no WLanguage vira erro de execucao, nao zero silencioso.
pub fn diferenca_de_datas(inicio: &str, fim: &str) -> Result<i64, String> {
    let a = dias_civis(inicio).ok_or_else(|| format!("data invalida: {inicio}"))?;
    let b = dias_civis(fim).ok_or_else(|| format!("data invalida: {fim}"))?;
    Ok(b - a)
}

/// `DateSys() + 30 * i`: no WLanguage somar inteiro a uma Date soma DIAS (1514066).
pub fn data_mais_dias(d: &str, dias: i64) -> Result<String, String> {
    let z = dias_civis(d).ok_or_else(|| format!("data invalida: {d}"))?;
    Ok(de_dias_civis(z + dias))
}

/// DateSys() / Today() (3027026, 3027016): AAAAMMDD do relogio da maquina, em UTC --
/// o WLanguage usa o fuso do sistema; quem precisa do fuso local ajusta antes.
pub fn data_hoje() -> String {
    let s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    de_dias_civis(s.div_euclid(86_400))
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn datevalid_do_help_3027003() {
        assert!(data_valida("20011225"));
        assert!(!data_valida("20012512"));
        assert!(!data_valida("2001"));
        assert!(data_valida("14821225"));
        assert!(data_valida("20240229") && !data_valida("20230229"));
    }
    #[test]
    fn datedifference_conta_dias_e_sinal() {
        assert_eq!(diferenca_de_datas("20260801", "20260816").unwrap(), 15);
        assert_eq!(diferenca_de_datas("20260801", "20260801").unwrap(), 0);
        assert_eq!(diferenca_de_datas("20260816", "20260801").unwrap(), -15);
        assert_eq!(diferenca_de_datas("19980101", "20260908").unwrap(), 10477);
        assert!(diferenca_de_datas("20260231", "20260301").is_err());
    }
    #[test]
    fn somar_inteiro_a_data_soma_dias() {
        assert_eq!(data_mais_dias("20260908", 30).unwrap(), "20261008");
        assert_eq!(data_mais_dias("20260908", 90).unwrap(), "20261207");
        assert_eq!(data_mais_dias("20240228", 1).unwrap(), "20240229");
        assert_eq!(data_mais_dias("20260101", -1).unwrap(), "20251231");
    }
    #[test]
    fn hoje_tem_oito_digitos_e_e_valida() {
        let h = data_hoje();
        assert_eq!(h.len(), 8);
        assert!(data_valida(&h));
        assert!(h.as_str() >= "20260101");
    }
}

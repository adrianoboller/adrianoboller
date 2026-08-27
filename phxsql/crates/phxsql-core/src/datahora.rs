//! Conversao entre os inteiros gravados no `.reg` e datas/horas do calendario.
//!
//! O PhxSql guarda:
//!
//! * `Date`     -- dias desde 1970-01-01 (i32), negativo antes disso;
//! * `Time`     -- centesimos de segundo desde a meia-noite (i32);
//! * `DateTime` -- milissegundos desde 1970-01-01T00:00:00Z (i64).
//!
//! O algoritmo de calendario e o `civil_from_days` de Howard Hinnant, valido
//! para todo o calendario gregoriano proleptico.

/// Converte dias desde a epoca em (ano, mes, dia).
pub fn civil_de_dias(dias: i32) -> (i32, u32, u32) {
    let z = dias as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // 0..=146096
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // 0..=399
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // 0..=365
    let mp = (5 * doy + 2) / 153; // 0..=11
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // 1..=31
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // 1..=12
    ((y + i64::from(m <= 2)) as i32, m, d)
}

/// Converte (ano, mes, dia) em dias desde a epoca.
pub fn dias_de_civil(ano: i32, mes: u32, dia: u32) -> i32 {
    let y = (ano - i32::from(mes <= 2)) as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = mes as i64;
    let d = dia as i64;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) as i32
}

/// Formata uma data no padrao ISO (`AAAA-MM-DD`).
pub fn data_iso(dias: i32) -> String {
    let (a, m, d) = civil_de_dias(dias);
    format!("{a:04}-{m:02}-{d:02}")
}

/// Decompoe centesimos de segundo em (hora, minuto, segundo, centesimo).
pub fn hora_partes(centesimos: i32) -> (u32, u32, u32, u32) {
    let c = centesimos.rem_euclid(8_640_000) as u32;
    (c / 360_000, (c / 6_000) % 60, (c / 100) % 60, c % 100)
}

/// Formata uma hora no padrao `HH:MM:SS,cc`.
pub fn hora_iso(centesimos: i32) -> String {
    let (h, mi, s, c) = hora_partes(centesimos);
    format!("{h:02}:{mi:02}:{s:02},{c:02}")
}

/// Formata um instante em milissegundos desde a epoca como
/// `AAAA-MM-DD HH:MM:SS,mmm`.
pub fn instante_iso(milissegundos: i64) -> String {
    let dias = milissegundos.div_euclid(86_400_000) as i32;
    let resto = milissegundos.rem_euclid(86_400_000);
    let (h, m, s, ms) = (
        resto / 3_600_000,
        (resto / 60_000) % 60,
        (resto / 1_000) % 60,
        resto % 1_000,
    );
    format!("{} {h:02}:{m:02}:{s:02},{ms:03}", data_iso(dias))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoca_unix() {
        assert_eq!(civil_de_dias(0), (1970, 1, 1));
        assert_eq!(dias_de_civil(1970, 1, 1), 0);
        assert_eq!(data_iso(0), "1970-01-01");
    }

    #[test]
    fn datas_conhecidas() {
        assert_eq!(data_iso(20_000), "2024-10-04");
        assert_eq!(dias_de_civil(2024, 10, 4), 20_000);
        assert_eq!(data_iso(-1), "1969-12-31");
        assert_eq!(data_iso(11_016), "2000-02-29"); // ano bissexto secular
        assert_eq!(dias_de_civil(2000, 2, 29), 11_016);
    }

    #[test]
    fn ida_e_volta_em_toda_a_faixa() {
        // De 1900 a 2100, dia a dia.
        let inicio = dias_de_civil(1900, 1, 1);
        let fim = dias_de_civil(2100, 1, 1);
        for dias in inicio..fim {
            let (a, m, d) = civil_de_dias(dias);
            assert_eq!(dias_de_civil(a, m, d), dias, "falhou em {a}-{m}-{d}");
        }
    }

    #[test]
    fn instante_completo() {
        // 2024-10-04 13:45:30,250
        let ms = 20_000i64 * 86_400_000 + 13 * 3_600_000 + 45 * 60_000 + 30 * 1_000 + 250;
        assert_eq!(instante_iso(ms), "2024-10-04 13:45:30,250");
        assert_eq!(instante_iso(0), "1970-01-01 00:00:00,000");
    }

    #[test]
    fn horas() {
        assert_eq!(hora_partes(0), (0, 0, 0, 0));
        assert_eq!(hora_iso(0), "00:00:00,00");
        // 13:45:30,25
        let c = 13 * 360_000 + 45 * 6_000 + 30 * 100 + 25;
        assert_eq!(hora_partes(c), (13, 45, 30, 25));
        assert_eq!(hora_iso(c), "13:45:30,25");
    }
}

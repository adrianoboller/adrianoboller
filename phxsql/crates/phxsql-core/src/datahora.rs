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

/// Le um instante em texto e devolve os milissegundos desde a epoca.
///
/// # Por que ela existe, e por que aceita mais de uma forma
///
/// E a volta do [`instante_iso`], e nasceu para o PITR: quem pede «restaure
/// ate as 15h» escreve `2026-09-08T15:00:00Z`, e quem le o `quando` de um
/// manifesto de backup gravado por esta casa recebe `2026-09-08 15:00:00,749`.
/// Sao a mesma data em duas roupas -- a de fora, do ISO 8601, e a nossa, que o
/// `instante_iso` escreve. **Um leitor que so entendesse o proprio dialeto
/// recusaria justamente o texto que o operador acabou de copiar da tela**, e
/// essa licao ja foi paga aqui pelo leitor de ZIP que so falava Huffman fixo.
///
/// O que ela aceita:
///
/// ```text
/// 2026-09-08                    -- meia-noite daquele dia
/// 2026-09-08T15:00:00Z          -- o que a tela e o contrato escrevem
/// 2026-09-08 15:00:00           -- o mesmo com espaco, sem o Z
/// 2026-09-08 15:00:00,749       -- o que o `instante_iso` grava
/// 2026-09-08T15:00:00.749Z      -- o mesmo com ponto
/// ```
///
/// **Tudo e UTC**, e um deslocamento explicito (`+03:00`) e RECUSADO em vez de
/// ignorado: em lugar nenhum deste motor existe fuso, e engolir o `+03:00`
/// devolveria um instante tres horas errado sem ninguem perceber -- que e pior
/// que recusar. `None` diz «nao entendi», e quem chama nomeia o campo.
pub fn ms_de_instante_iso(texto: &str) -> Option<i64> {
    let t = texto.trim();
    // O `Z` e o unico sufixo de fuso aceito, porque e o unico que nao muda
    // nada. Qualquer outro cai fora pelos digitos que sobram.
    let t = t
        .strip_suffix('Z')
        .or_else(|| t.strip_suffix('z'))
        .unwrap_or(t);
    let (data, resto) = match t.split_once(['T', 't', ' ']) {
        Some((d, r)) => (d, r.trim()),
        None => (t, ""),
    };
    let mut partes = data.split('-');
    let ano: i32 = partes.next()?.parse().ok()?;
    let mes: u32 = partes.next()?.parse().ok()?;
    let dia: u32 = partes.next()?.parse().ok()?;
    if partes.next().is_some() || !(1..=12).contains(&mes) || !(1..=31).contains(&dia) {
        return None;
    }
    // O calendario tem de FECHAR: `2026-02-31` vira 3 de marco na conta de
    // Hinnant, e aceitar isso deixaria um erro de digitacao virar outro dia.
    let dias = dias_de_civil(ano, mes, dia);
    if civil_de_dias(dias) != (ano, mes, dia) {
        return None;
    }
    let mut ms = dias as i64 * 86_400_000;
    if resto.is_empty() {
        return Some(ms);
    }
    // A fracao aceita virgula (a nossa) ou ponto (a de fora), e vale sempre em
    // milissegundos: `,7` e 700 ms, e nao 7.
    let (hms, fracao) = match resto.split_once([',', '.']) {
        Some((h, f)) => (h, f),
        None => (resto, ""),
    };
    let mut hp = hms.split(':');
    let h: i64 = hp.next()?.parse().ok()?;
    let mi: i64 = hp.next().unwrap_or("0").parse().ok()?;
    let sg: i64 = hp.next().unwrap_or("0").parse().ok()?;
    if hp.next().is_some()
        || !(0..24).contains(&h)
        || !(0..60).contains(&mi)
        || !(0..60).contains(&sg)
    {
        return None;
    }
    ms += h * 3_600_000 + mi * 60_000 + sg * 1_000;
    if !fracao.is_empty() {
        if !fracao.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let mut digitos = fracao.to_string();
        digitos.truncate(3);
        while digitos.len() < 3 {
            digitos.push('0');
        }
        ms += digitos.parse::<i64>().ok()?;
    }
    Some(ms)
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

    /// A volta do `instante_iso` fecha em toda a faixa util, dia a dia.
    ///
    /// Ida e volta com o proprio formatador nao prova que a gente le o que os
    /// OUTROS escrevem -- por isso os casos de fora estao no teste seguinte --,
    /// mas prova a unica coisa que este par tem de garantir sozinho: que o
    /// texto que esta casa grava num manifesto volta como o mesmo numero.
    #[test]
    fn a_volta_do_instante_fecha_em_toda_a_faixa() {
        let inicio = dias_de_civil(1970, 1, 1) as i64;
        let fim = dias_de_civil(2100, 1, 1) as i64;
        for dias in inicio..fim {
            let ms = dias * 86_400_000 + 13 * 3_600_000 + 45 * 60_000 + 30 * 1_000 + 250;
            assert_eq!(ms_de_instante_iso(&instante_iso(ms)), Some(ms));
        }
    }

    /// O que vem de FORA: o ISO 8601 com `T` e `Z`, que e o que o contrato do
    /// PITR escreve e o que uma pessoa copia da tela.
    #[test]
    fn as_formas_de_fora_tambem_entram() {
        let meia_noite = dias_de_civil(2026, 9, 8) as i64 * 86_400_000;
        assert_eq!(ms_de_instante_iso("2026-09-08"), Some(meia_noite));
        let quinze = meia_noite + 15 * 3_600_000;
        for t in [
            "2026-09-08T15:00:00Z",
            "2026-09-08t15:00:00z",
            "2026-09-08 15:00:00",
            "2026-09-08T15:00:00",
            "  2026-09-08T15:00:00Z  ",
            "2026-09-08 15:00",
        ] {
            assert_eq!(ms_de_instante_iso(t), Some(quinze), "falhou em {t:?}");
        }
        // A fracao vale em MILISSEGUNDOS, com ponto ou virgula, e o quarto
        // digito e cortado em vez de virar dez vezes o valor.
        assert_eq!(
            ms_de_instante_iso("2026-09-08T15:00:00.7Z"),
            Some(quinze + 700)
        );
        assert_eq!(
            ms_de_instante_iso("2026-09-08 15:00:00,749"),
            Some(quinze + 749)
        );
        assert_eq!(
            ms_de_instante_iso("2026-09-08T15:00:00.7499Z"),
            Some(quinze + 749)
        );
    }

    /// O que ela RECUSA, e o caso que decide e o fuso: engolir o `+03:00`
    /// devolveria um instante tres horas errado sem ninguem perceber.
    #[test]
    fn o_que_nao_e_instante_nao_vira_numero() {
        for t in [
            "",
            "agora",
            "2026-09-08T15:00:00+03:00",
            "2026-09-08T15:00:00-03:00",
            "2026-02-31", // o calendario nao fecha
            "2026-13-01",
            "2026-09-08T25:00:00Z",
            "2026-09-08T15:61:00Z",
            "2026-09-08T15:00:00,7a9",
            "2026-09",
            "2026-09-08-09T15:00:00Z",
        ] {
            assert_eq!(ms_de_instante_iso(t), None, "devia recusar {t:?}");
        }
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

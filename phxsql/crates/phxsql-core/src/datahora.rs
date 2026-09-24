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

/// Le uma hora em texto e devolve os centesimos de segundo desde a
/// meia-noite.
///
/// # Por que ela existe
///
/// E a volta do [`hora_iso`], pelo mesmo motivo do [`ms_de_instante_iso`]
/// para o instante: `valor_para_json` (e o exportador de CSV) SEMPRE
/// escrevem `Time` como texto de relogio -- nunca inteiro --, entao ler uma
/// linha, mexer noutro campo e mandar ela de volta pede um leitor que
/// entenda esse mesmo texto, e nao so o numero em centesimos.
///
/// Os tres motores maduros (PostgreSQL, MySQL, MariaDB, SQLite) convergem em
/// `HH:MM:SS`. Isso sozinho TRUNCARIA os centesimos que esta casa grava --
/// entao a forma aceita carrega a fracao por cima, com virgula (a nossa, a
/// que o `hora_iso` grava) ou ponto (a de fora):
///
/// ```text
/// 14:30:00,25   -- o que o `hora_iso` grava
/// 14:30:00.25   -- o mesmo com ponto
/// 14:30:00      -- sem fracao -- os centesimos ficam em zero
/// 14:30         -- sem segundos -- minutos bastam, o resto fica em zero
/// 14:30:00Z     -- o "Z" de UTC, que nao muda nada e por isso e aceito
/// ```
///
/// Um deslocamento explicito (`+03:00`) e RECUSADO em vez de ignorado, pelo
/// mesmo motivo do `ms_de_instante_iso`: aqui nao existe fuso nenhum, e
/// engolir um deslocamento moveria o relogio sem ninguem perceber. `None` diz
/// «nao entendi», e quem chama nomeia o campo.
pub fn centesimos_de_hora_iso(texto: &str) -> Option<i32> {
    let t = texto.trim();
    // O "Z" e o unico sufixo de fuso aceito, porque e o unico que nao muda
    // nada -- qualquer outro (`+03:00`, `-03:00`) sobra dentro do campo dos
    // segundos e quebra o parse dele, mais abaixo.
    let t = t
        .strip_suffix('Z')
        .or_else(|| t.strip_suffix('z'))
        .unwrap_or(t);
    let (hms, fracao) = match t.split_once([',', '.']) {
        Some((h, f)) => (h, f),
        None => (t, ""),
    };
    let mut partes = hms.split(':');
    let h: i32 = partes.next()?.parse().ok()?;
    let mi: i32 = partes.next().unwrap_or("0").parse().ok()?;
    let sg: i32 = partes.next().unwrap_or("0").parse().ok()?;
    if partes.next().is_some()
        || !(0..24).contains(&h)
        || !(0..60).contains(&mi)
        || !(0..60).contains(&sg)
    {
        return None;
    }
    let mut centesimos = h * 360_000 + mi * 6_000 + sg * 100;
    if !fracao.is_empty() {
        if !fracao.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        // Dois digitos e o que um centesimo comporta -- o terceiro em diante
        // e cortado, igual o `ms_de_instante_iso` corta o quarto do
        // milissegundo.
        let mut digitos = fracao.to_string();
        digitos.truncate(2);
        while digitos.len() < 2 {
            digitos.push('0');
        }
        centesimos += digitos.parse::<i32>().ok()?;
    }
    Some(centesimos)
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

/// O mesmo instante do calendario `anos` anos antes -- a conta de um prazo de
/// retencao dito em ANOS.
///
/// # Por que calendario, e nao `anos x 365,25 dias`
///
/// Porque prazo em anos e prazo de calendario: «cinco anos depois de 24 de
/// setembro de 2021» e 24 de setembro de 2026, e nao 1.826,25 dias depois. A
/// conta pela media erra para os DOIS lados conforme quantos 29 de fevereiro
/// cairam no meio -- e errar para o lado curto e apagar antes do prazo, que e o
/// unico erro que um expurgo nao pode cometer.
///
/// # O 29 de fevereiro
///
/// Recuar de 29/02 para um ano comum cai em **28/02**, e nao em 01/03: o dia
/// mais cedo dos dois. Um limite mais cedo derruba MENOS, nunca mais -- e
/// entre guardar um dia a mais e apagar um dia antes, esta funcao escolhe
/// guardar. A hora do dia fica a mesma.
pub fn recuar_anos(milissegundos: i64, anos: u32) -> i64 {
    let dias = milissegundos.div_euclid(86_400_000);
    let resto = milissegundos.rem_euclid(86_400_000);
    let (ano, mes, dia) = civil_de_dias(dias as i32);
    let alvo = ano - anos as i32;
    // O dia que o ano de chegada nao tem (29/02 num ano comum) volta um dia:
    // `dias_de_civil` aceitaria o 29 e devolveria 01/03 calado.
    let dia = if mes == 2 && dia == 29 && civil_de_dias(dias_de_civil(alvo, 2, 29)) != (alvo, 2, 29)
    {
        28
    } else {
        dia
    };
    dias_de_civil(alvo, mes, dia) as i64 * 86_400_000 + resto
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O prazo e de CALENDARIO: cinco anos antes de 24/09/2026 as 10h e
    /// 24/09/2021 as 10h, com dois 29 de fevereiro no meio (2024) ou um -- a
    /// media de 365,25 dias erraria por horas para um lado ou para o outro.
    #[test]
    fn recuar_anos_e_conta_de_calendario() {
        let agora = ms_de_instante_iso("2026-09-24T10:00:00Z").unwrap();
        assert_eq!(
            instante_iso(recuar_anos(agora, 5)),
            "2021-09-24 10:00:00,000"
        );
        assert_eq!(recuar_anos(agora, 0), agora);
        // Antes da epoca continua valendo: o calendario de Hinnant e
        // proleptico.
        assert_eq!(
            instante_iso(recuar_anos(ms_de_instante_iso("1971-03-01").unwrap(), 5)),
            "1966-03-01 00:00:00,000"
        );
    }

    /// 29/02 recua para 28/02 -- o dia mais CEDO --, nunca para 01/03: um
    /// limite mais cedo apaga menos, e e so esse o lado em que um expurgo pode
    /// errar.
    #[test]
    fn recuar_de_29_de_fevereiro_cai_no_dia_mais_cedo() {
        let bissexto = ms_de_instante_iso("2028-02-29T23:59:59Z").unwrap();
        assert_eq!(
            instante_iso(recuar_anos(bissexto, 5)),
            "2023-02-28 23:59:59,000"
        );
        // Para outro bissexto o 29 existe e fica.
        assert_eq!(
            instante_iso(recuar_anos(bissexto, 4)),
            "2024-02-29 23:59:59,000"
        );
    }

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

    /// A volta da hora fecha em toda a faixa do dia, segundo a segundo.
    ///
    /// Ida e volta com o proprio formatador nao prova que a gente le o que os
    /// OUTROS escrevem -- por isso os casos de fora estao no teste seguinte
    /// --, mas prova a unica coisa que este par tem de garantir sozinho: que
    /// o texto que esta casa grava (`hora_iso`) volta como o mesmo numero,
    /// para toda hora, minuto e segundo do dia -- e os quatro cantos do
    /// centesimo (0, 1, 50, 99), para a fracao nao se perder calada.
    #[test]
    fn a_volta_da_hora_fecha_em_toda_a_faixa() {
        for segundos in 0..86_400i32 {
            for cc in [0, 1, 50, 99] {
                let c = segundos * 100 + cc;
                assert_eq!(
                    centesimos_de_hora_iso(&hora_iso(c)),
                    Some(c),
                    "falhou em {c} ({})",
                    hora_iso(c)
                );
            }
        }
    }

    /// O que vem de FORA: sem fracao, com ponto em vez de virgula, sem
    /// segundos, e o "Z" de quem copiou um horario ISO da tela.
    #[test]
    fn as_formas_de_fora_tambem_entram_na_hora() {
        let quatorze_e_meia = 14 * 360_000 + 30 * 6_000; // 14:30:00
        for t in [
            "14:30:00",
            "14:30:00Z",
            "14:30:00z",
            "  14:30:00  ",
            "14:30",
        ] {
            assert_eq!(
                centesimos_de_hora_iso(t),
                Some(quatorze_e_meia),
                "falhou em {t:?}"
            );
        }
        assert_eq!(
            centesimos_de_hora_iso("14:30:00,25"),
            Some(quatorze_e_meia + 25)
        );
        assert_eq!(
            centesimos_de_hora_iso("14:30:00.25"),
            Some(quatorze_e_meia + 25)
        );
        // A fracao vale em CENTESIMOS: o terceiro digito em diante e
        // CORTADO, nao arredondado -- o mesmo criterio do `ms_de_instante_iso`.
        assert_eq!(
            centesimos_de_hora_iso("14:30:00.7"),
            Some(quatorze_e_meia + 70)
        );
        assert_eq!(
            centesimos_de_hora_iso("14:30:00.2599"),
            Some(quatorze_e_meia + 25)
        );
    }

    /// O que ela RECUSA, e o caso que decide e o fuso: engolir um
    /// deslocamento (`-03:00`) moveria o relogio sem ninguem perceber -- o
    /// mesmo motivo do `ms_de_instante_iso`.
    #[test]
    fn o_que_nao_e_hora_nao_vira_numero() {
        for t in [
            "",
            "agora",
            "14:30:00+03:00",
            "14:30:00-03:00",
            "25:00:00",
            "14:61:00",
            "14:30:61",
            "14:30:00,7a9",
            "14:30:00:00",
        ] {
            assert_eq!(centesimos_de_hora_iso(t), None, "devia recusar {t:?}");
        }
    }
}

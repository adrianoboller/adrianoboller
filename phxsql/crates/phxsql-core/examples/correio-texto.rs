//! Regra de texto do correio PhxSql, decidida pelo dono (11/09/2026): o CORPO
//! da mensagem sai SEM ACENTO (transliterado para ASCII); apenas o ANEXO
//! preserva o acento, byte a byte.
//!
//! Por que separar as duas pontas: o corpo e texto que o correio normaliza (a
//! mesma disciplina do endereco/assunto, que evita truque de homoglifo); o
//! anexo e um blob opaco — um PDF, uma planilha — e transliterar os bytes dele
//! o corromperia. Entao a transliteracao morde SO o corpo.
//!
//! Prova real nos dois sentidos: corpo com acento MUDA (e o resultado e ASCII),
//! corpo que ja e ASCII nao muda, e o anexo com bytes acentuados volta
//! IDENTICO. Zero-deps, so `std`.
//!
//! Rodar: cargo run -q --example correio-texto -p phxsql-core

/// Tira o acento do texto, deixando-o ASCII. Os diacriticos latinos viram a
/// letra-base; qualquer outro nao-ASCII (um travessao, um simbolo de moeda,
/// um emoji) vira '?', porque a garantia e dura: o corpo TEM de sair ASCII, e
/// "sem acento" quer dizer isso. Rotulo se traduz, dado nunca — e aqui o corpo
/// e tratado como texto do correio, decisao explicita do dono, nao como dado
/// gravado cru: o anexo e quem guarda o original.
fn transliterar_ascii(texto: &str) -> String {
    let mut out = String::with_capacity(texto.len());
    for c in texto.chars() {
        if c.is_ascii() {
            out.push(c);
            continue;
        }
        let sub = match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => "a",
            'é' | 'è' | 'ê' | 'ë' => "e",
            'í' | 'ì' | 'î' | 'ï' => "i",
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => "o",
            'ú' | 'ù' | 'û' | 'ü' => "u",
            'ç' => "c",
            'ñ' => "n",
            'ý' | 'ÿ' => "y",
            'Á' | 'À' | 'Â' | 'Ã' | 'Ä' | 'Å' => "A",
            'É' | 'È' | 'Ê' | 'Ë' => "E",
            'Í' | 'Ì' | 'Î' | 'Ï' => "I",
            'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => "O",
            'Ú' | 'Ù' | 'Û' | 'Ü' => "U",
            'Ç' => "C",
            'Ñ' => "N",
            _ => "?",
        };
        out.push_str(sub);
    }
    out
}

/// O que o correio prepara para enviar: o corpo ja SEM acento, e os anexos
/// intocados (byte a byte).
struct Preparada {
    corpo: String,
    anexos: Vec<Vec<u8>>,
}
fn preparar(corpo: &str, anexos: &[&[u8]]) -> Preparada {
    Preparada {
        corpo: transliterar_ascii(corpo),
        anexos: anexos.iter().map(|a| a.to_vec()).collect(),
    }
}

fn main() {
    let mut res: Vec<(bool, String)> = vec![];
    let mut ok = |c: bool, n: &str| {
        println!("   {}  {}", if c { " ok  " } else { "FALHA" }, n);
        res.push((c, n.to_string()));
    };

    println!("===== correio PhxSql: corpo SEM acento, anexo PRESERVA =====\n");

    println!("1) corpo transliterado (o acento cai, o resultado e ASCII):");
    let corpo = "São José: açaí, coração à vontade, ninho de João";
    let t = transliterar_ascii(corpo);
    println!("   antes: {corpo}");
    println!("   depois: {t}");
    ok(
        t == "Sao Jose: acai, coracao a vontade, ninho de Joao",
        "diacriticos viram a letra-base (minusculas)",
    );
    ok(t.is_ascii(), "o corpo resultante e 100% ASCII");
    ok(
        transliterar_ascii("ÁGUA & AÇÃO EM AÇÚCAR") == "AGUA & ACAO EM ACUCAR",
        "maiusculas acentuadas tambem caem",
    );

    println!("\n2) o que NAO e acento e ainda assim nao-ASCII vira '?':");
    let esq = transliterar_ascii("preço — 5€ 🙂");
    println!("   'preço — 5€ 🙂' -> '{esq}'");
    ok(
        esq == "preco ? 5? ?" && esq.is_ascii(),
        "travessao, moeda e emoji viram '?' (o corpo continua ASCII)",
    );

    println!("\n3) prova nos dois sentidos:");
    ok(t != corpo, "corpo COM acento realmente muda (nao e no-op)");
    ok(
        transliterar_ascii("ja estava em ascii") == "ja estava em ascii",
        "corpo que ja era ASCII passa intocado",
    );
    ok(
        transliterar_ascii(&t) == t,
        "idempotente: transliterar de novo nao muda mais nada",
    );

    println!("\n4) o ANEXO preserva o acento, byte a byte:");
    // Um "anexo" com acento nos bytes (ex.: um nome/rotulo dentro do arquivo).
    let anexo_original = "relatório-anual-coração.txt: conteúdo em Português".as_bytes();
    let m = preparar(
        "corpo com açaí e coração",
        &[anexo_original, &[0xC3, 0xA9, 0x00, 0xFF]],
    );
    ok(m.corpo.is_ascii(), "na preparacao, o corpo sai ASCII");
    ok(
        m.anexos[0] == anexo_original,
        "anexo de texto volta IDENTICO (acento preservado nos bytes)",
    );
    ok(
        !m.anexos[0].is_ascii(),
        "e esses bytes NAO sao ASCII — prova que o anexo nao foi transliterado",
    );
    ok(
        m.anexos[1] == vec![0xC3u8, 0xA9, 0x00, 0xFF],
        "anexo binario (bytes crus, ate 0x00/0xFF) passa intocado",
    );

    println!("\n===== RESULTADO =====");
    let mut falhas = 0;
    for (c, n) in &res {
        if !c {
            falhas += 1;
        }
        println!("  {}  {}", if *c { " ok  " } else { "FALHA" }, n);
    }
    println!(
        "\n{} checagens, {} ok, {} falha(s).",
        res.len(),
        res.len() - falhas,
        falhas
    );
    println!(
        "{}",
        if falhas == 0 {
            "PROVA VERDE"
        } else {
            "PROVA VERMELHA"
        }
    );
    if falhas != 0 {
        std::process::exit(1);
    }
}

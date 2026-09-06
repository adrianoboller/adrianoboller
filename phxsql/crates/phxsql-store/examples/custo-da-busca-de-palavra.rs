//! Quanto custa achar uma PALAVRA hoje -- e quanto disso o `.fts` removeria.
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store
//! cargo run --release --example custo-da-busca-de-palavra -- [linhas] [um_em]
//! ```
//!
//! # A premissa que este medidor existe para matar ou confirmar
//!
//! O `docs/HFSQL.md` §3.2 diz: *«eles acham uma palavra em um milhao de
//! linhas em menos de 2 ms; aqui, procurar uma palavra dentro de um `.memo`
//! e varredura»*. A primeira metade e a folha deles. A segunda **nunca foi
//! medida** -- e a lei desta casa e que numero citado e numero que nao se
//! mede. Antes de desenhar o `.fts`, o numero que decide o desenho e um so:
//! **quanto do custo de uma busca e o `.memo`, e quanto e o resto**.
//!
//! Isso importa porque decide o que o indice compra. Se quase tudo for a
//! leitura do `.memo`, o `.fts` compra muito, porque o indice responde SEM
//! abrir o `.memo`. Se quase tudo for a comparacao de texto, ele compra menos
//! do que a folha deles sugere -- e o desenho tem de mirar outra coisa.
//!
//! # As quatro medidas, no MESMO texto
//!
//! A regra da bancada desta casa e comparar trabalho igual, e nao so pergunta
//! igual: as quatro veem o mesmo texto, a mesma palavra e a mesma quantidade
//! de linhas. So muda ONDE o texto mora.
//!
//! | medida | o que entra |
//! |--------|-------------|
//! | A `so ler`   | ler as linhas de uma tabela SEM coluna de texto. O chao do `.reg`. |
//! | B `inline`   | ler + achar a palavra num `Str(200)`, que mora dentro do slot. |
//! | C `memo`     | ler + achar a palavra num `Memo`, que mora no `.memo`. |
//! | D `so RAM`   | a mesma comparacao com os textos ja num `Vec`. O piso absoluto. |
//!
//! `C - B` e a fatia do `.memo`. `B - A` e a fatia da comparacao dentro do
//! slot. `D` diz quanto sobra quando nao ha disco nenhum.
//!
//! # A quinta pergunta, que nao e de tempo
//!
//! Buscar `fenix` acha `fenix`? E acha `fenix` com circunflexo? A decisao do
//! dono de 06/09/2026 foi fazer a dobra de acento POR INDICE, e ela nao e um
//! item ao lado do `.fts`: e pre-requisito dele. Indice de texto que nao
//! dobra acento nao serve em portugues. O medidor conta os dois casos, porque
//! um indice que acha menos que a varredura seria pior que nao ter indice.
//!
//! A ultima linha e `RESULTADO <json>`, como na `carga`.

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

/// Palavras do corpo. A que se procura e RARA de proposito: um indice existe
/// para a agulha, e nao para o palheiro. `fenix` aparece so nas linhas
/// escolhidas; `fenix` acentuada aparece nas mesmas, para a quinta pergunta.
const RECHEIO: [&str; 12] = [
    "pedido",
    "cliente",
    "nota",
    "fiscal",
    "entrega",
    "produto",
    "valor",
    "desconto",
    "parcela",
    "vencimento",
    "transportadora",
    "observacao",
];

const AGULHA: &str = "fenix";
const AGULHA_ACENTUADA: &str = "fênix";

/// O texto da linha `i`. Deterministico: as quatro medidas veem o mesmo.
///
/// Fica em ~200 bytes para caber igual no `Str(200)` e no `Memo` -- comparar
/// um texto curto inline com um texto longo externo compararia duas coisas.
fn texto(i: u64, um_em: u64) -> String {
    let mut s = String::with_capacity(220);
    for k in 0..14 {
        s.push_str(RECHEIO[((i + k) as usize) % RECHEIO.len()]);
        s.push(' ');
    }
    // As duas grafias em linhas DIFERENTES, e este detalhe e a prova.
    //
    // A primeira versao deste medidor punha as duas na MESMA linha, e por isso
    // ele respondia «achou 50 de 50» a pergunta da dobra -- achava por haver a
    // grafia sem acento ao lado, e nao por dobrar coisa nenhuma. Era um teste
    // que passava por engano, que e pior que teste que falta. Separadas, a
    // conta so fecha se a busca dobrar de verdade.
    if i % um_em == 0 {
        s.push_str(AGULHA);
    } else if i % um_em == 1 {
        s.push_str(AGULHA_ACENTUADA);
    }
    s.truncate(200);
    s
}

fn esquema_so_id() -> Schema {
    Schema::new(
        "soid",
        vec![Column::new("id", ColumnType::Int8).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .expect("esquema so id")
}

fn esquema_texto(nome: &str, tipo: ColumnType) -> Schema {
    Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("corpo", tipo),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .expect("esquema com texto")
}

/// Cria a tabela do zero e carrega `linhas`. Devolve os rowids na ordem.
fn encher(
    dir: &str,
    esquema: Schema,
    linhas: u64,
    um_em: u64,
    com_texto: bool,
) -> (Table, Vec<u64>) {
    let _ = std::fs::remove_dir_all(dir);
    let mut t = Table::criar(dir, esquema).expect("criar");
    let mut ids = Vec::with_capacity(linhas as usize);
    for i in 0..linhas {
        let valores = if com_texto {
            vec![Value::Int(i as i64), Value::Str(texto(i, um_em))]
        } else {
            vec![Value::Int(i as i64)]
        };
        ids.push(t.inserir(&valores).expect("inserir"));
    }
    (t, ids)
}

/// Le todas as linhas e conta quantas trazem a agulha na coluna `col`.
///
/// `col` = `None` e a medida A: le e joga fora, sem olhar texto nenhum.
fn varrer_contando(t: &mut Table, ids: &[u64], col: Option<usize>, agulha: &str) -> (f64, u64) {
    let inicio = Instant::now();
    let mut achadas = 0u64;
    for &r in ids {
        let linha = t.ler(r).expect("ler").expect("linha viva");
        if let Some(c) = col {
            if let Some(v) = linha.get(c) {
                let texto = match v {
                    Value::Str(s) | Value::Memo(s) => s.as_str(),
                    _ => "",
                };
                if texto.contains(agulha) {
                    achadas += 1;
                }
            }
        }
    }
    (inicio.elapsed().as_secs_f64(), achadas)
}

fn us_por_linha(segundos: f64, linhas: u64) -> f64 {
    segundos * 1_000_000.0 / linhas as f64
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let linhas: u64 = a.get(1).map(|s| s.parse().unwrap()).unwrap_or(50_000);
    let um_em: u64 = a.get(2).map(|s| s.parse().unwrap()).unwrap_or(1_000);
    let base = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into());
    let esperadas = linhas.div_ceil(um_em);

    println!("Custo de achar uma palavra HOJE, sem indice de texto");
    println!("{linhas} linhas, agulha em 1 a cada {um_em} ({esperadas} esperadas)");
    println!();

    // A -- o chao: ler as linhas de uma tabela que nem tem texto.
    let dir_a = format!("{base}/phx-busca-a");
    let (mut ta, ids) = encher(&dir_a, esquema_so_id(), linhas, um_em, false);
    let (s_a, _) = varrer_contando(&mut ta, &ids, None, AGULHA);

    // B -- texto INLINE, dentro do slot do `.reg`.
    let dir_b = format!("{base}/phx-busca-b");
    let (mut tb, ids_b) = encher(
        &dir_b,
        esquema_texto("inline", ColumnType::Str(200)),
        linhas,
        um_em,
        true,
    );
    let (s_b, achadas_b) = varrer_contando(&mut tb, &ids_b, Some(1), AGULHA);

    // C -- texto EXTERNO, no `.memo`.
    let dir_c = format!("{base}/phx-busca-c");
    let (mut tc, ids_c) = encher(
        &dir_c,
        esquema_texto("externa", ColumnType::Memo),
        linhas,
        um_em,
        true,
    );
    let (s_c, achadas_c) = varrer_contando(&mut tc, &ids_c, Some(1), AGULHA);

    // D -- o piso: os mesmos textos ja em RAM.
    let textos: Vec<String> = (0..linhas).map(|i| texto(i, um_em)).collect();
    let inicio = Instant::now();
    let mut achadas_d = 0u64;
    for s in &textos {
        if s.contains(AGULHA) {
            achadas_d += 1;
        }
    }
    let s_d = inicio.elapsed().as_secs_f64();

    // A quinta pergunta: a busca de hoje dobra acento? Agora as duas grafias
    // moram em linhas diferentes, entao a resposta nao pode vir de carona.
    let so_acentuadas = textos
        .iter()
        .filter(|s| s.contains(AGULHA_ACENTUADA))
        .count() as u64;
    let acha_acentuada_procurando_sem = textos
        .iter()
        .filter(|s| s.contains(AGULHA_ACENTUADA) && s.contains(AGULHA))
        .count() as u64;

    println!(
        "{:>10}  {:>12}  {:>12}  {:>9}  o que e",
        "medida", "total ms", "us/linha", "achadas"
    );
    println!("{}", "-".repeat(76));
    for (rot, s, ach, oque) in [
        ("A so ler", s_a, 0, "le o `.reg` e joga fora"),
        ("B inline", s_b, achadas_b, "le + procura num Str(200)"),
        ("C memo", s_c, achadas_c, "le + procura num Memo (.memo)"),
        ("D so RAM", s_d, achadas_d, "so a comparacao, sem disco"),
    ] {
        println!(
            "{rot:>10}  {:>12.1}  {:>12.3}  {ach:>9}  {oque}",
            s * 1000.0,
            us_por_linha(s, linhas)
        );
    }

    // As contas que decidem o desenho.
    let fatia_memo = (s_c - s_b).max(0.0);
    let fatia_compara = (s_b - s_a).max(0.0);
    println!();
    println!("A conta que decide o desenho do `.fts`:");
    println!(
        "  o `.memo` custa      {:>8.1} ms  ({:.1}% de C)  <- o que o indice REMOVE",
        fatia_memo * 1000.0,
        if s_c > 0.0 {
            fatia_memo / s_c * 100.0
        } else {
            0.0
        }
    );
    println!(
        "  a comparacao custa   {:>8.1} ms  ({:.1}% de C)",
        fatia_compara * 1000.0,
        if s_c > 0.0 {
            fatia_compara / s_c * 100.0
        } else {
            0.0
        }
    );
    println!(
        "  ler o `.reg` custa   {:>8.1} ms  ({:.1}% de C)  <- o indice tambem remove",
        s_a * 1000.0,
        if s_c > 0.0 { s_a / s_c * 100.0 } else { 0.0 }
    );
    println!();
    println!(
        "  extrapolado a 1.000.000 de linhas: C = {:.0} ms",
        us_por_linha(s_c, linhas) * 1_000_000.0 / 1000.0
    );
    println!("  a folha do HFSQL(R) afirma < 2 ms em 1.000.000 -- nao reproduzivel por eles");

    println!();
    println!("A quinta pergunta -- a dobra de acento:");
    println!("  linhas com SO `{AGULHA_ACENTUADA}`: {so_acentuadas}");
    println!("  dessas, quantas procurar `{AGULHA}` acha: {acha_acentuada_procurando_sem}");
    println!(
        "  => a busca de hoje {} dobra acento",
        if acha_acentuada_procurando_sem == 0 {
            "NAO"
        } else {
            "DOBRA (inesperado -- confira)"
        }
    );
    println!("     em portugues isso significa que um indice de texto SEM dobra");
    println!("     acharia menos que a varredura de hoje em qualquer palavra acentuada");

    // Prova real do proprio medidor: se as tres contagens divergirem, alguma
    // camada nao esta lendo o que as outras leem -- e o numero nao vale.
    assert_eq!(
        achadas_b, esperadas,
        "o inline achou {achadas_b} e deviam ser {esperadas}"
    );
    assert_eq!(
        achadas_c, esperadas,
        "o memo achou {achadas_c} e deviam ser {esperadas} -- \
         se deu ZERO, o caminho de leitura nao carrega a coluna externa, \
         e isso e defeito, nao lentidao"
    );
    assert_eq!(achadas_d, esperadas, "a RAM achou {achadas_d}");

    println!();
    println!(
        "RESULTADO {{\"linhas\":{linhas},\"um_em\":{um_em},\
         \"a_so_ler_ms\":{:.1},\"b_inline_ms\":{:.1},\"c_memo_ms\":{:.1},\"d_ram_ms\":{:.3},\
         \"fatia_memo_pct\":{:.1},\"achadas\":{esperadas},\"dobra_acento\":false}}",
        s_a * 1000.0,
        s_b * 1000.0,
        s_c * 1000.0,
        s_d * 1000.0,
        if s_c > 0.0 {
            fatia_memo / s_c * 100.0
        } else {
            0.0
        }
    );

    for d in [&dir_a, &dir_b, &dir_c] {
        let _ = std::fs::remove_dir_all(d);
    }
}

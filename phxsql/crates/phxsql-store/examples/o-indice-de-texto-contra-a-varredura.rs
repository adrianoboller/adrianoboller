//! O `.fts` contra a varredura, na MESMA pergunta e com o MESMO resultado.
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store
//! cargo run --release --example o-indice-de-texto-contra-a-varredura -- [linhas] [buscas]
//! ```
//!
//! # A regra que este medidor tem de honrar
//!
//! *Bancada compara trabalho igual, e nao so pergunta igual.* Esta casa ja
//! errou nos dois sentidos -- um `WHERE id IN (...)` contra vinte mil buscas
//! separadas, e um `COUNT(*)+SUM` sobre 1.250.000 linhas contra a leitura de
//! 20.000. Nenhum dos dois aparecia no numero.
//!
//! Entao aqui a igualdade nao e afirmada, e sim **conferida**: as duas faixas
//! respondem «quais linhas contem esta palavra?», devolvem o CONJUNTO de
//! rowids, e o medidor **aborta** se os conjuntos diferirem em qualquer
//! busca. Um numero que sai de trabalho diferente e pior que numero nenhum.
//!
//! | faixa | o que ela faz |
//! |-------|---------------|
//! | varredura | le TODA linha, quebra o texto em termos, compara |
//! | indice    | desce o `.fts` e le so as linhas que ele apontou |
//!
//! As duas dobram o acento do mesmo jeito, pelo mesmo `phxsql_core::termo` --
//! se a varredura nao dobrasse, o indice acharia mais que ela e a diferenca
//! seria de significado, nao de tempo.
//!
//! # As palavras procuradas
//!
//! Metade existe e metade nao. Palavra que nao existe e o caso em que o indice
//! ganha mais (desce a arvore e volta com zero, enquanto a varredura le tudo),
//! e omiti-la inflaria o ganho medio a nosso favor -- que e exatamente o erro
//! que esta casa ja cometeu.
//!
//! A ultima linha e `RESULTADO <json>`, como na `carga`.

use std::collections::BTreeSet;
use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, IndiceDeTexto, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::{Table, Visao};

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

/// O vocabulario e grande de proposito: um punhado de palavras caberia todo
/// numa pagina do `.fts`, e a arvore nunca desceria.
const SUFIXOS: u64 = 5_000;

fn esquema() -> Schema {
    Schema::new(
        "docs",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("corpo", ColumnType::Str(200)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .expect("esquema")
    .com_indices_de_texto(vec![IndiceDeTexto::new("porCorpo", 1)])
    .expect("indice de texto")
}

fn texto(i: u64) -> String {
    let mut s = String::with_capacity(220);
    for k in 0..14 {
        s.push_str(RECHEIO[((i + k) as usize) % RECHEIO.len()]);
        s.push('_');
        s.push_str(&((i + k) % SUFIXOS).to_string());
        s.push(' ');
    }
    s.truncate(200);
    s
}

/// A varredura honesta: le a linha, quebra em termos, compara termo inteiro.
///
/// **Termo inteiro, e nao `contains`**, porque e isso que o indice responde.
/// Um `contains` acharia `pedido_12` procurando `pedido_1`, e ai as duas
/// faixas responderiam perguntas diferentes.
fn varrer_procurando(t: &mut Table, palavra: &str) -> BTreeSet<u64> {
    let alvo = phxsql_core::termo::dobrar(palavra);
    let mut achados = BTreeSet::new();
    let mut depois = 0u64;
    loop {
        let pagina = t.pagina_depois_de(depois, 5_000, Visao::Ativas).unwrap();
        if pagina.is_empty() {
            break;
        }
        for rowid in pagina {
            if let Some(linha) = t.ler(rowid).unwrap() {
                if let Some(Value::Str(s)) = linha.get(1) {
                    if phxsql_core::termo::termos(s).contains(&alvo) {
                        achados.insert(rowid);
                    }
                }
            }
            depois = rowid;
        }
    }
    achados
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let linhas: u64 = a.get(1).map(|s| s.parse().unwrap()).unwrap_or(100_000);
    let buscas: u64 = a.get(2).map(|s| s.parse().unwrap()).unwrap_or(20);
    let base = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into());
    let dir = format!("{base}/phx-fts-contra-varredura");

    println!("O indice de texto contra a varredura -- MESMA pergunta, MESMO conjunto");
    println!("{linhas} linhas, {buscas} buscas (metade existe, metade nao)");
    println!();

    let _ = std::fs::remove_dir_all(&dir);
    let mut t = Table::criar(&dir, esquema()).expect("criar");
    let inicio = Instant::now();
    for i in 0..linhas {
        t.inserir(&[Value::Int(i as i64), Value::Str(texto(i))])
            .unwrap();
    }
    let carga = inicio.elapsed().as_secs_f64();
    println!(
        "  carga com o indice de texto ligado: {:.2} s ({:.2} us/linha)",
        carga,
        carga * 1e6 / linhas as f64
    );

    // Metade das palavras existe (sai do proprio recheio) e metade nao.
    let mut palavras: Vec<String> = Vec::new();
    for k in 0..buscas {
        if k % 2 == 0 {
            let i = (k * 977) % linhas.max(1);
            palavras.push(format!(
                "{}_{}",
                RECHEIO[(i as usize) % RECHEIO.len()],
                i % SUFIXOS
            ));
        } else {
            palavras.push(format!("inexistente_{k}"));
        }
    }

    let mut us_varredura = 0.0f64;
    let mut us_indice = 0.0f64;
    let mut achados_total = 0usize;
    for palavra in &palavras {
        let ini = Instant::now();
        let pela_varredura = varrer_procurando(&mut t, palavra);
        us_varredura += ini.elapsed().as_secs_f64() * 1e6;

        let ini = Instant::now();
        let pelo_indice: BTreeSet<u64> = t
            .procurar_texto("porCorpo", palavra)
            .unwrap()
            .rowids
            .into_iter()
            .collect();
        us_indice += ini.elapsed().as_secs_f64() * 1e6;

        // **O portao do medidor.** Sem isto o numero de baixo nao vale nada.
        assert_eq!(
            pela_varredura, pelo_indice,
            "as duas faixas responderam DIFERENTE para «{palavra}»: \
             a varredura achou {} e o indice {} -- o numero seria de \
             trabalhos diferentes",
            pela_varredura.len(),
            pelo_indice.len()
        );
        achados_total += pelo_indice.len();
    }

    let media_varredura = us_varredura / buscas as f64;
    let media_indice = us_indice / buscas as f64;
    let ganho = media_varredura / media_indice.max(f64::MIN_POSITIVE);

    println!();
    println!("  {:<14} {:>14} {:>14}", "faixa", "us/busca", "vezes");
    println!("  {:-<14} {:->14} {:->14}", "", "", "");
    println!("  {:<14} {:>14.1} {:>14}", "varredura", media_varredura, "1,00x");
    println!(
        "  {:<14} {:>14.1} {:>13.2}x",
        "indice", media_indice, ganho
    );
    println!();
    println!(
        "  linhas achadas nas {buscas} buscas: {achados_total} \
         (os conjuntos das duas faixas bateram em TODAS)"
    );

    println!(
        "RESULTADO {{\"linhas\":{linhas},\"buscas\":{buscas},\
         \"carga_s\":{carga:.4},\"us_varredura\":{media_varredura:.2},\
         \"us_indice\":{media_indice:.2},\"ganho\":{ganho:.2},\
         \"achados\":{achados_total}}}"
    );
}

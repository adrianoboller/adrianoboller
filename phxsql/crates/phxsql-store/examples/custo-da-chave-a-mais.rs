//! Quanto custa MAIS UMA chave de indice numa insercao?
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store
//! cargo run --release --example custo-da-chave-a-mais -- [linhas]
//! ```
//!
//! # A premissa que este medidor existe para matar ou confirmar
//!
//! O `docs/FTS.md` §4.2(a) nomeia o risco que pode matar o desenho do indice
//! de texto antes de ele existir: **83,5% do tempo de uma insercao ja esta no
//! `.ndx`** (`DESEMPENHO.md`), e um texto de 200 bytes tem ~14 palavras. Se o
//! `.fts` escrever uma chave por palavra, cada `inserir` passa a escrever ~14
//! chaves a mais -- e «inserir e o laco quente» e lei desta casa.
//!
//! O numero que decide nao e o custo do `.fts` (que nao existe): e o custo
//! MARGINAL de uma chave a mais na arvore que ja existe. Ele se mede hoje, com
//! tabelas de 1, 2, 4 e 8 indices, e a inclinacao da reta responde quanto
//! custariam 14.
//!
//! Se a resposta for «cada chave custa como a primeira», o desenho do `.fts`
//! muda para despejo em lote antes de alguem escrever uma linha dele. Se for
//! «a segunda em diante custa pouco», ele segue como esta.
//!
//! # O cuidado que faz a comparacao valer
//!
//! As tabelas tem as MESMAS colunas e os MESMOS dados; so muda quantos indices
//! elas declaram. Comparar uma tabela de 8 colunas indexadas com uma de 1
//! coluna compararia payloads diferentes, e o numero seria de outra coisa.
//!
//! A ultima linha e `RESULTADO <json>`, como na `carga`.

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

/// Quantos indices cada caso declara. O primeiro e o `porId`, unico, que toda
/// tabela desta casa tem.
///
/// **O 15 esta aqui de proposito, e ele e o ponto que importa:** 14 chaves de
/// texto mais o `porId`. A primeira versao deste medidor parava em 8 e
/// EXTRAPOLAVA os 14 por uma reta -- e os proprios dados negam a reta, porque
/// o custo por chave SOBE (0,59 -> 0,79 -> 0,93 us de 2 para 8 indices).
/// Extrapolar dali subestimava. Numero citado e numero que nao se mede,
/// inclusive quando quem cita e o medidor.
const CASOS: [usize; 6] = [1, 2, 4, 8, 15, 17];

/// Colunas indexaveis, iguais em todos os casos.
const COLUNAS: usize = 16;

fn esquema(indices: usize) -> Schema {
    let mut cols = vec![Column::new("id", ColumnType::Int8).obrigatoria()];
    for c in 0..COLUNAS {
        cols.push(Column::new(format!("c{c}"), ColumnType::Int8));
    }
    let mut idx = vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()];
    for c in 0..(indices - 1) {
        idx.push(IndexDef::new(
            format!("por_c{c}"),
            vec![IndexColumn::asc(c + 1)],
        ));
    }
    Schema::new("marginal", cols, idx).expect("esquema")
}

/// Os mesmos valores em todos os casos: so muda quantos indices os leem.
fn linha(i: u64) -> Vec<Value> {
    let mut v = Vec::with_capacity(COLUNAS + 1);
    v.push(Value::Int(i as i64));
    for c in 0..COLUNAS {
        // Valores ESPALHADOS, e nao sequenciais: uma chave que so cresce cai
        // sempre na mesma folha da direita, e mediria o melhor caso da arvore.
        v.push(Value::Int(
            ((i * 2_654_435_761) % 1_000_003) as i64 + c as i64,
        ));
    }
    v
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let linhas: u64 = a.get(1).map(|s| s.parse().unwrap()).unwrap_or(50_000);
    let base = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into());

    println!("Custo marginal de uma chave de indice a mais, na insercao");
    println!("{linhas} linhas, mesmas colunas e mesmos dados em todos os casos");
    println!();
    println!(
        "{:>8}  {:>12}  {:>12}  {:>14}",
        "indices", "total ms", "us/linha", "us/chave extra"
    );
    println!("{}", "-".repeat(54));

    let mut medidas: Vec<(usize, f64)> = Vec::new();
    for &n in &CASOS {
        let dir = format!("{base}/phx-marginal-{n}");
        let _ = std::fs::remove_dir_all(&dir);
        let mut t = Table::criar(&dir, esquema(n)).expect("criar");
        let inicio = Instant::now();
        for i in 0..linhas {
            t.inserir(&linha(i)).expect("inserir");
        }
        let s = inicio.elapsed().as_secs_f64();
        let us = s * 1_000_000.0 / linhas as f64;
        medidas.push((n, us));
        let base_us = medidas[0].1;
        let extra = if n > 1 {
            (us - base_us) / (n - 1) as f64
        } else {
            0.0
        };
        println!("{n:>8}  {:>12.1}  {us:>12.3}  {extra:>14.3}", s * 1000.0);
        drop(t);
        let _ = std::fs::remove_dir_all(&dir);
    }

    let base_us = medidas[0].1;
    // O ponto de 15 indices e MEDIDO, nao extrapolado: 14 chaves de texto mais
    // o `porId`. E o unico numero que decide o desenho.
    let com_14 = medidas
        .iter()
        .find(|(n, _)| *n == 15)
        .map(|(_, us)| *us)
        .expect("o caso de 15 indices tem de existir");
    let fator = com_14 / base_us;

    println!();
    println!("A conta que decide o desenho do `.fts`, MEDIDA e nao extrapolada:");
    println!("  insercao com 1 indice          {base_us:.3} us");
    println!("  insercao com 15 (= 14 + porId) {com_14:.3} us");
    println!("  o texto custaria               {fator:.2}x a insercao de hoje");
    println!();
    println!("  e a inclinacao, que a versao anterior deste medidor ignorou:");
    for j in 1..medidas.len() {
        let (n0, u0) = medidas[j - 1];
        let (n1, u1) = medidas[j];
        println!(
            "    de {n0:>2} para {n1:>2} indices: {:.3} us por chave",
            (u1 - u0) / (n1 - n0) as f64
        );
    }
    println!();
    if fator > 3.0 {
        println!("  VEREDITO: a escrita SINCRONA do .fts custa caro no laco quente.");
        println!("  O FTS.md §4.2(a) ja nomeia a saida: despejo em LOTE.");
    } else {
        println!("  VEREDITO: cabe. A escrita sincrona do .fts segue como esta no FTS.md.");
    }

    println!();
    print!("RESULTADO {{\"linhas\":{linhas},\"casos\":[");
    for (i, (n, us)) in medidas.iter().enumerate() {
        if i > 0 {
            print!(",");
        }
        print!("{{\"indices\":{n},\"us_por_linha\":{us:.3}}}");
    }
    println!("],\"us_com_1\":{base_us:.3},\"us_com_15\":{com_14:.3},\"fator_medido\":{fator:.2}}}");
}

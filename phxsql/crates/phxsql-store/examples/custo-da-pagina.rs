//! Quanto custa devolver UMA pagina de uma tabela grande.
//!
//! ```bash
//! cargo run --release --example custo-da-pagina -- <dir> <n_linhas> <pagina>
//! ```
//!
//! Existe por uma suspeita que precisava virar numero: o caminho que o
//! servidor usa hoje para responder `varrer` le a TABELA INTEIRA, decodifica
//! cada linha com os anexos, monta tudo em memoria, e so entao joga fora tudo
//! menos as primeiras `max`. Se for isso, pedir 200 linhas de um milhao custa
//! um milhao de leituras -- e o custo cresce com a tabela, nao com a pagina.
//!
//! Mede tres jeitos de responder a mesma pergunta:
//!
//! 1. `varrer_com`  -- o caminho de hoje: tudo em memoria, com anexos;
//! 2. `pagina`      -- so os rowids, sem decodificar, parando no teto;
//! 3. `depois_de`   -- busca binaria pelo cursor, e le so a pagina.
//!
//! A ultima linha e `RESULTADO <json>`, para nao ter de adivinhar nada.

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::{Salto, Table, Visao};

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Cliente {i:07}")),
        Value::Str(if i % 3 == 0 { "Blumenau" } else { "Itajai" }.into()),
        // Um memo em toda linha: e o que faz a diferenca aparecer, e e o caso
        // real -- ficha de cliente, observacao, descricao.
        Value::Memo(format!("ficha do cliente {i}, com texto que mora no .memo")),
    ]
}

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(30)),
            Column::new("ficha", ColumnType::Memo),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

fn ms(t: Instant) -> u128 {
    t.elapsed().as_millis()
}

/// A bisseccao nao aparece em milissegundo: vinte leituras num arquivo quente
/// somam microssegundos. Medir em ms devolveria «0» e nao provaria nada.
fn us(t: Instant) -> u128 {
    t.elapsed().as_micros()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = a
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/tmp/phx-pagina".into());
    let n: i64 = a.get(2).map(|s| s.parse().unwrap()).unwrap_or(200_000);
    let pagina: u64 = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(200);

    let _ = std::fs::remove_dir_all(&dir);
    let mut t = Table::criar(&dir, esquema()).unwrap();

    let carga = Instant::now();
    for i in 1..=n {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();
    let t_carga = ms(carga);

    // 1. O caminho de hoje.
    let c = Instant::now();
    let tudo = t.varrer_com(Visao::Ativas).unwrap();
    let primeiras: Vec<u64> = tudo.iter().take(pagina as usize).map(|(r, _)| *r).collect();
    let t_hoje = ms(c);
    let linhas_lidas_hoje = tudo.len();
    drop(tudo);

    // 2. So os rowids, parando no teto.
    let c = Instant::now();
    let p2 = t.pagina(0, pagina, Visao::Ativas).unwrap();
    let t_pagina = ms(c);

    // 3. Pelo cursor, no MEIO da tabela -- que e onde o OFFSET afunda.
    let meio = (n as u64) / 2;
    let c = Instant::now();
    let p3 = t.pagina_depois_de(meio, pagina, Visao::Ativas).unwrap();
    let t_cursor = ms(c);

    // 4. E o mesmo ponto do meio pelo caminho de hoje, para a comparacao ser
    //    entre trabalhos IGUAIS: mesma pagina, mesmo resultado.
    let c = Instant::now();
    let tudo = t.varrer_com(Visao::Ativas).unwrap();
    let p4: Vec<u64> = tudo
        .iter()
        .skip(meio as usize)
        .take(pagina as usize)
        .map(|(r, _)| *r)
        .collect();
    let t_hoje_meio = ms(c);
    drop(tudo);

    // 5. A MESMA pagina do meio, agora pela posicao -- que e o `OFFSET` do
    //    SQL. E a comparacao que interessa: mesmo pedido, mesmo resultado,
    //    caminhos com custo diferente.
    let c = Instant::now();
    let (p5, como) = t.pagina_por_posicao(meio, pagina, Visao::Ativas).unwrap();
    let t_salto = us(c);
    assert_eq!(como, Salto::Bissecao, "a tabela intacta tinha de bissetar");

    // 6. E o mesmo pedido depois de UM buraco: a igualdade entre posicao e
    //    rownum cai, e o motor tem de voltar a andar -- com a MESMA resposta.
    t.excluir_de_vez(n as u64, "medicao").unwrap();
    let c = Instant::now();
    let (p6, como6) = t.pagina_por_posicao(meio, pagina, Visao::Ativas).unwrap();
    let t_passo = us(c);
    assert_eq!(como6, Salto::Passo, "com buraco nao pode bissetar");
    assert_eq!(p5, p6, "os dois caminhos deram paginas diferentes");

    assert_eq!(
        primeiras.len(),
        p2.len(),
        "a pagina 1 deu tamanhos diferentes"
    );
    assert_eq!(primeiras, p2, "a pagina 1 deu rowids diferentes");
    assert_eq!(p3, p4, "a pagina do meio deu rowids diferentes");
    assert_eq!(
        p3, p5,
        "o salto por posicao deu rowids diferentes do cursor"
    );

    println!("linhas na tabela .... {n}");
    println!("pagina .............. {pagina}");
    println!("carga ............... {t_carga} ms");
    println!();
    println!("PAGINA 1");
    println!("  varrer_com (hoje) . {t_hoje} ms  ({linhas_lidas_hoje} linhas decodificadas)");
    println!("  pagina ............ {t_pagina} ms");
    println!();
    println!("PAGINA DO MEIO (offset {meio})");
    println!("  varrer_com (hoje) . {t_hoje_meio} ms");
    println!("  depois_de (cursor)  {t_cursor} ms");
    println!("  posicao, bissecao . {t_salto} us");
    println!("  posicao, andando .. {t_passo} us  (a mesma pagina, com um buraco)");

    println!(
        "RESULTADO {{\"linhas\":{n},\"pagina\":{pagina},\
         \"hoje_ms\":{t_hoje},\"pagina_ms\":{t_pagina},\
         \"hoje_meio_ms\":{t_hoje_meio},\"cursor_ms\":{t_cursor},\
         \"salto_us\":{t_salto},\"passo_us\":{t_passo}}}"
    );
}

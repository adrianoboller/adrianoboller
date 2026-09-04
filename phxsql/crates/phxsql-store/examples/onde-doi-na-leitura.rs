//! Onde vao os ~4,8 ms de um `varrer` que devolve UMA linha?
//!
//! ```bash
//! cargo run --release --example onde-doi-na-leitura -p phxsql-store -- [linhas]
//! ```
//!
//! # A pergunta, e o que ja esta descartado
//!
//! Medido pela rede em 04/09 (`docs/CONCORRENCIA.md` §13.2), na mesma bateria:
//!
//! | operacao | custo | o que ela paga |
//! |---|---:|---|
//! | `ping` | 97 us | soquete + JSON + despacho, SEM a trava de dados |
//! | `inserir` 1 linha | 231 us | tudo isso + trava global + ABRIR a tabela + gravar |
//! | `varrer` limite 1 | 5.000 us | tudo isso + ler UMA linha |
//! | `varrer` limite 200 | 4.975 us | tudo isso + ler DUZENTAS |
//!
//! Entao os ~4,8 ms nao sao do soquete, do JSON, do despacho, da trava nem do
//! `open`: o `inserir` paga todos eles por 231 us. E nao sao das linhas: 200
//! custam o mesmo que 1.
//!
//! E nao e o defeito que a §1.4 conhece. Aquele esta no caminho por INDICE,
//! que le a tabela inteira e recorta a pagina depois. Este `varrer` nao manda
//! indice nem cursor, entao o `op_varrer` cai em `pagina_por_posicao`, que com
//! `pular = 0` chama `pagina(0, limite, ativas)` -- e essa retorna na primeira
//! linha viva. Lido no fonte antes de medir.
//!
//! # O que este medidor separa
//!
//! As pecas do caminho de leitura, cada uma com a tabela ja aberta, para que
//! o `open` (que o `inserir` tambem paga) nao se misture ao que se procura:
//!
//! | peca | o que entra |
//! |---|---|
//! | `abrir` | `Table::abrir` e largar -- o que o `inserir` tambem paga |
//! | `pagina` | `pagina(0, limite, ativas)`: so escolher os rowids |
//! | `ler` | `ler(rowid)` de cada um da pagina |
//! | `pagina+ler` | as duas, que e o que o `op_varrer` faz antes do JSON |
//!
//! E ele roda com limite 1 e com limite 200, porque **a diferenca entre os
//! dois e o achado**: se as pecas somarem microssegundos nos dois casos, os
//! 4,8 ms NAO estao no `phxsql-store`, e a proxima cacada e uma camada acima
//! -- no `phxsql-server`. Um medidor que nao encontra tambem responde, desde
//! que diga onde nao esta.
//!
//! As rodadas sao INTERCALADAS e sai a MEDIANA: medir uma peca de cada vez em
//! sequencia faria a primeira pagar a arvore fria das outras.

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::{Table, Visao};

fn esquema() -> Schema {
    Schema::new(
        "c",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("nome", ColumnType::Str(20)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn mediana(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let linhas: u64 = std::env::args()
        .nth(1)
        .and_then(|x| x.parse().ok())
        .unwrap_or(2000);
    let rodadas = 9;

    let dir = std::env::temp_dir().join(format!("phx-onde-le-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();

    let mut t = Table::criar(&dir, esquema()).unwrap();
    for i in 0..linhas {
        t.inserir(&[Value::Int(i as i64 + 1), Value::Str("semente".into())])
            .unwrap();
    }
    t.sincronizar().unwrap();
    drop(t);

    println!("=== onde doi na LEITURA, com {linhas} linhas na tabela ===\n");
    println!("  o alvo: os ~4,8 ms que separam `varrer` de 1 linha (5.000 us)");
    println!("  do `inserir` de 1 linha (231 us), medidos pela rede na §13.2.\n");

    for limite in [1u64, 200] {
        let (mut ab, mut pag, mut le, mut ambas) = (vec![], vec![], vec![], vec![]);
        for _ in 0..rodadas {
            let i = Instant::now();
            let t = Table::abrir(&dir, "c").unwrap();
            ab.push(i.elapsed().as_secs_f64() * 1e6);
            drop(t);

            let mut t = Table::abrir(&dir, "c").unwrap();
            let i = Instant::now();
            let ids = t.pagina(0, limite, Visao::Ativas).unwrap();
            pag.push(i.elapsed().as_secs_f64() * 1e6);

            let i = Instant::now();
            for &id in &ids {
                std::hint::black_box(t.ler(id).unwrap());
            }
            le.push(i.elapsed().as_secs_f64() * 1e6);

            let mut t2 = Table::abrir(&dir, "c").unwrap();
            let i = Instant::now();
            let ids2 = t2.pagina(0, limite, Visao::Ativas).unwrap();
            for &id in &ids2 {
                std::hint::black_box(t2.ler(id).unwrap());
            }
            ambas.push(i.elapsed().as_secs_f64() * 1e6);
        }
        let (m_ab, m_pag, m_le, m_ambas) = (
            mediana(ab),
            mediana(pag),
            mediana(le),
            mediana(ambas),
        );
        println!("-- limite {limite}");
        println!("   abrir a tabela ............ {m_ab:>9.1} us");
        println!("   pagina (so os rowids) ..... {m_pag:>9.1} us");
        println!("   ler cada linha ............ {m_le:>9.1} us");
        println!("   pagina + ler .............. {m_ambas:>9.1} us");
        println!("   abrir + pagina + ler ...... {:>9.1} us\n", m_ab + m_ambas);
    }

    println!("=== o veredito ===\n");
    println!("  Se a ultima linha de cada bloco for MUITO menor que 5.000 us,");
    println!("  os 4,8 ms nao estao no `phxsql-store`: estao na camada do");
    println!("  servidor, e a proxima cacada e la. Medidor que nao encontra");
    println!("  tambem responde, desde que diga ONDE nao esta.");

    std::fs::remove_dir_all(&dir).ok();
}

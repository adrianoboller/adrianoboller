//! Prova a corrida: dois `Table` abertos ao mesmo tempo na mesma tabela.
//!
//! E exatamente o que o servidor faz -- `abrir` solta a trava antes de
//! devolver, entao duas operacoes simultaneas abrem a tabela, cada uma le
//! `slot_count` do cabecalho, e as duas gravam no MESMO rowid.

use phxsql_core::schema::{Column, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn main() {
    let dir = std::env::temp_dir().join(format!("phx-corrida-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let esquema = Schema::new(
        "t",
        vec![
            Column::new("quem", ColumnType::Str(10)),
            Column::new("n", ColumnType::Int8),
        ],
        vec![],
    )
    .unwrap();
    {
        let mut t = Table::criar(&dir, esquema).unwrap();
        t.inserir(&[Value::Str("inicial".into()), Value::Int(0)]).unwrap();
        t.sincronizar().unwrap();
    }

    // Duas aberturas, como duas operacoes concorrentes do servidor.
    let mut a = Table::abrir(&dir, "t").unwrap();
    let mut b = Table::abrir(&dir, "t").unwrap();

    let ra = a.inserir(&[Value::Str("A".into()), Value::Int(1)]).unwrap();
    a.sincronizar().unwrap();
    let rb = b.inserir(&[Value::Str("B".into()), Value::Int(2)]).unwrap();
    b.sincronizar().unwrap();

    println!("A gravou no rowid {ra}");
    println!("B gravou no rowid {rb}");

    let mut c = Table::abrir(&dir, "t").unwrap();
    let n = c.registros();
    println!("a tabela diz ter {n} registros");
    for r in 1..=c.slots() {
        match c.ler(r).unwrap() {
            Some(l) => println!("  rowid {r}: {:?}", l[0]),
            None => println!("  rowid {r}: (vazio)"),
        }
    }
    if ra == rb {
        println!("\nPERDEU: as duas gravaram no mesmo slot, e uma sobrescreveu a outra.");
    } else {
        println!("\nOK: rowids diferentes.");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

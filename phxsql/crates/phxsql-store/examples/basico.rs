//! Uso basico da tabela PhxSql -- o mesmo exemplo do README, compilado de
//! verdade para que nunca fique desatualizado.
//!
//! ```bash
//! cargo run --example basico
//! ```

use phxsql_core::{Column, ColumnType, IndexColumn, IndexDef, Result, Schema, Value};
use phxsql_store::Table;

fn main() -> Result<()> {
    let dir = std::env::temp_dir().join(format!("phxsql-exemplo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let esquema = Schema::new(
        "cadastroClientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(40)),
            Column::new("ficha", ColumnType::Memo),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porNome", vec![IndexColumn::asc(1).sem_caixa()]),
        ],
    )?;

    let mut t = Table::criar(&dir, esquema)?;

    let rowid = t.inserir(&[
        Value::Int(1),
        Value::Str("Adriano Boller".into()),
        Value::Str("Blumenau".into()),
        Value::Memo("Cliente desde 1998.".into()),
    ])?;

    t.inserir(&[
        Value::Int(2),
        Value::Str("Marcia Alves".into()),
        Value::Str("Joinville".into()),
        Value::Null,
    ])?;

    // Busca pelo indice, sem distinguir maiusculas.
    let achados = t.buscar("porNome", &[Value::Str("adriano boller".into())])?;
    assert_eq!(achados, vec![rowid]);
    println!("busca NOCASE por 'adriano boller' achou o rowid {rowid}");

    // Varredura na ordem de digitacao, direto do .reg.
    println!("\nordem de digitacao (.reg):");
    for (rowid, linha) in t.varrer()? {
        println!("  {rowid}: {:?} / {:?}", linha[1], linha[3]);
    }

    // Varredura na ordem do indice.
    println!("\nordem alfabetica (.ndx, indice porNome):");
    for rowid in t.varrer_indice("porNome")? {
        let linha = t.ler(rowid)?.unwrap();
        println!("  {rowid}: {:?}", linha[1]);
    }

    let relatorio = t.verificar()?;
    println!("\nintegridade conferida: {relatorio:?}");

    t.sincronizar()?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

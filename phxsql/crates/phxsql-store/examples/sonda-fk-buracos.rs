//! SONDA descartavel: os caminhos de escrita que ninguem confere.
use phxsql_core::schema::{Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::catalogo::Instancia;
use phxsql_store::table::Table;

fn esq_mae() -> Schema {
    Schema::new(
        "clientes",
        vec![Column::new("id", ColumnType::Int4).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn esq_filha() -> Schema {
    Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("cliente_id", ColumnType::Int4),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCliente", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
    .com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_cliente",
        vec![1],
        "clientes",
        vec!["id".into()],
    )
    .conferindo(true)])
    .unwrap()
}

fn dir(n: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("phx-buraco-{n}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn main() {
    // ============ 1. a mae EXCLUIDA SUAVE ainda satisfaz o conferir_fks?
    let d = dir("mae-suave");
    let mut m = Table::criar(&d, esq_mae()).unwrap();
    m.inserir(&[Value::Int(1)]).unwrap();
    println!("1. MAE EXCLUIDA SUAVE, e uma filha NOVA apontando para ela");
    println!(
        "   excluir_suave da mae sem filha: {:?}",
        m.excluir_suave(1, "teste")
    );
    m.sincronizar().unwrap();
    drop(m);
    let mut f = Table::criar(&d, esq_filha()).unwrap();
    match f.inserir(&[Value::Int(10), Value::Int(1)]) {
        Ok(r) => println!("   >>> inserir filha apontando para mae MORTA: ACEITOU (rowid {r})"),
        Err(e) => println!("   inserir filha apontando para mae morta: recusou ({e})"),
    }
    f.sincronizar().unwrap();
    drop(f);

    // ============ 2. restaurar a filha depois de a mae ter sumido
    let d = dir("restaurar");
    let mut m = Table::criar(&d, esq_mae()).unwrap();
    m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);
    let mut f = Table::criar(&d, esq_filha()).unwrap();
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.excluir_suave(1, "teste").unwrap();
    f.sincronizar().unwrap();
    drop(f);
    println!("\n2. FILHA EXCLUIDA SUAVE, e a mae tentando sair");
    let mut m = Table::abrir(&d, "clientes").unwrap();
    println!(
        "   excluir_de_vez da mae com filha SO suave: {:?}",
        m.excluir_de_vez(1, "t").err().map(|e| e.to_string())
    );
    println!(
        "   excluir_suave  da mae com filha SO suave: {:?}",
        m.excluir_suave(1, "t").err().map(|e| e.to_string())
    );

    // ============ 3. DROP TABLE da mae com filhas vivas
    let base = dir("drop");
    let inst = Instancia::nova(&base).unwrap();
    let db = inst.criar_database("loja").unwrap();
    let mut m = db.criar_tabela(None, esq_mae()).unwrap();
    m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);
    let mut f = db.criar_tabela(None, esq_filha()).unwrap();
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();
    drop(f);
    println!("\n3. DROP TABLE da mae com filha viva apontando para ela");
    match db.renomear_tabela("clientes", "clientes_velho") {
        Ok(_) => println!("   renomear_tabela: ACEITOU"),
        Err(e) => println!("   renomear_tabela: recusou -- {e}"),
    }
    match db.excluir_tabela("clientes") {
        Ok(v) => println!(
            "   >>> excluir_tabela: ACEITOU e apagou {} arquivo(s)",
            v.len()
        ),
        Err(e) => println!("   excluir_tabela: recusou -- {e}"),
    }
    let mut f = db.abrir_tabela(None, "pedidos").unwrap();
    println!(
        "   a filha continua com a linha: {:?}",
        f.ler(1).unwrap().map(|l| l[1].clone())
    );
    match f.inserir(&[Value::Int(11), Value::Int(1)]) {
        Ok(_) => println!("   inserir outra filha: ACEITOU"),
        Err(e) => println!("   inserir outra filha agora: {e}"),
    }

    // ============ 4. declarar chave nova numa tabela que JA tem orfas
    let d = dir("redeclarar");
    let mut m = Table::criar(&d, esq_mae()).unwrap();
    m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);
    let esq_sem_fk = Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("cliente_id", ColumnType::Int4),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCliente", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap();
    let mut f = Table::criar(&d, esq_sem_fk).unwrap();
    f.inserir(&[Value::Int(10), Value::Int(999)]).unwrap(); // orfa: nao ha cliente 999
    f.sincronizar().unwrap();
    println!("\n4. DECLARAR chave conferida numa tabela que ja tem ORFA (cliente_id=999)");
    let fk = ForeignKey::new("fk_cliente", vec![1], "clientes", vec!["id".into()]).conferindo(true);
    match f.redeclarar_chaves_estrangeiras(vec![fk]) {
        Ok(_) => println!("   >>> redeclarar_chaves_estrangeiras: ACEITOU, e a orfa continua la"),
        Err(e) => println!("   redeclarar: recusou -- {e}"),
    }
    println!("   a orfa: {:?}", f.ler(1).unwrap().map(|l| l[1].clone()));
}

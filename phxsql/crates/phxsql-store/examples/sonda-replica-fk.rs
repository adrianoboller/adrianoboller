//! SONDA descartavel: a replica confere FK, e o que isso faz com a ordem.
//! Cada aplicacao abre a tabela do zero, como o `aplicar_lote_da_replica` faz.
use phxsql_core::schema::{Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::log::Operacao;
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
    let d = std::env::temp_dir().join(format!("phx-sonda-{n}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Abre, aplica UM evento, sincroniza e fecha -- o ciclo do lote da replica.
fn aplicar(d: &std::path::Path, tab: &str, op: Operacao, rowid: u64, img: &[u8]) -> String {
    let mut t = Table::abrir(d, tab).unwrap().com_imagem_no_diario(true);
    let r = match t.aplicar_evento(op, rowid, img) {
        Ok(_) => "OK".to_string(),
        Err(e) => format!("RECUSOU: {e}"),
    };
    t.sincronizar().unwrap();
    r
}

fn eventos(d: &std::path::Path, tab: &str) -> u64 {
    Table::abrir(d, tab).unwrap().eventos().unwrap()
}

fn main() {
    let ds = dir("source");

    // ---- source: mae + filha, e uma alteracao de chave que CASCATEIA
    let mut m = Table::criar(&ds, esq_mae())
        .unwrap()
        .com_imagem_no_diario(true);
    m.inserir(&[Value::Int(1)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);
    let mut f = Table::criar(&ds, esq_filha())
        .unwrap()
        .com_imagem_no_diario(true);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();
    drop(f);
    let mut m = Table::abrir(&ds, "clientes")
        .unwrap()
        .com_imagem_no_diario(true);
    m.atualizar(1, &[Value::Int(2)]).unwrap();
    m.sincronizar().unwrap();
    drop(m);

    let mut m = Table::abrir(&ds, "clientes").unwrap();
    let mut f = Table::abrir(&ds, "pedidos").unwrap();
    println!(
        "SOURCE  clientes: {} eventos | pedidos: {} eventos",
        m.eventos().unwrap(),
        f.eventos().unwrap()
    );
    println!(
        "SOURCE  a filha ficou com cliente_id = {:?}",
        f.ler(1).unwrap().unwrap()[1]
    );
    println!(
        "        >>> a cascata do source gerou {} evento(s) em pedidos alem da insercao",
        f.eventos().unwrap() - 1
    );
    let em: Vec<_> = m.diario_com_imagem(0, 0).unwrap();
    let ef: Vec<_> = f.diario_com_imagem(0, 0).unwrap();
    println!(
        "        eventos de clientes: {:?}",
        em.iter().map(|(e, _)| e.operacao).collect::<Vec<_>>()
    );
    println!(
        "        eventos de pedidos:  {:?}",
        ef.iter().map(|(e, _)| e.operacao).collect::<Vec<_>>()
    );

    for (rotulo, mae_primeiro) in [
        ("A: tabela MAE primeiro", true),
        ("B: tabela FILHA primeiro", false),
    ] {
        let dr = dir(if mae_primeiro { "rep-a" } else { "rep-b" });
        drop(Table::criar(&dr, esq_mae()).unwrap());
        drop(Table::criar(&dr, esq_filha()).unwrap());
        println!("\n--- ORDEM {rotulo} ---");
        let passo_mae = |d: &std::path::Path| {
            for (e, img) in &em {
                println!(
                    "  clientes {:?} rowid {} -> {}",
                    e.operacao,
                    e.rowid,
                    aplicar(d, "clientes", e.operacao, e.rowid, img)
                );
            }
        };
        let passo_filha = |d: &std::path::Path| {
            for (e, img) in &ef {
                println!(
                    "  pedidos  {:?} rowid {} -> {}",
                    e.operacao,
                    e.rowid,
                    aplicar(d, "pedidos", e.operacao, e.rowid, img)
                );
            }
        };
        if mae_primeiro {
            passo_mae(&dr);
            passo_filha(&dr);
        } else {
            passo_filha(&dr);
            passo_mae(&dr);
        }
        println!(
            "  RESULTADO: clientes {} eventos (source {}), pedidos {} eventos (source {})",
            eventos(&dr, "clientes"),
            em.len(),
            eventos(&dr, "pedidos"),
            ef.len()
        );
        let mut rf = Table::abrir(&dr, "pedidos").unwrap();
        println!(
            "  filha na replica: {:?}",
            rf.ler(1).ok().flatten().map(|l| l[1].clone())
        );
    }

    // ---- ordem C: entrelacada, na ordem real em que os eventos nasceram
    let dr = dir("rep-c");
    drop(Table::criar(&dr, esq_mae()).unwrap());
    drop(Table::criar(&dr, esq_filha()).unwrap());
    println!("\n--- ORDEM C: entrelacada (clientes.ins, pedidos.ins, clientes.alt) ---");
    println!(
        "  clientes ins -> {}",
        aplicar(&dr, "clientes", em[0].0.operacao, em[0].0.rowid, &em[0].1)
    );
    println!(
        "  pedidos  ins -> {}",
        aplicar(&dr, "pedidos", ef[0].0.operacao, ef[0].0.rowid, &ef[0].1)
    );
    let antes = eventos(&dr, "pedidos");
    println!(
        "  clientes alt -> {}",
        aplicar(&dr, "clientes", em[1].0.operacao, em[1].0.rowid, &em[1].1)
    );
    let depois = eventos(&dr, "pedidos");
    println!(
        "  >>> pedidos na replica: {antes} evento(s) antes da alteracao da mae, {depois} DEPOIS"
    );
    println!(
        "  >>> a cascata rodou DE NOVO na replica e gerou {} evento(s) que o source nao mandou",
        depois - antes
    );
    println!(
        "  >>> a posicao de pedidos na replica e {depois}; o source so tem {}",
        ef.len()
    );
    let mut rf = Table::abrir(&dr, "pedidos").unwrap();
    println!(
        "  filha na replica: {:?}",
        rf.ler(1).ok().flatten().map(|l| l[1].clone())
    );
}

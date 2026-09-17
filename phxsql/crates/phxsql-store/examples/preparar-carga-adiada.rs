//! Instrumento de bancada -- NAO e o motor, e NAO simula o BULKINSERT sozinho.
//!
//! Prepara, no diretorio de um banco REAL do `phxsqld`, uma tabela `carga`
//! no estado em que uma hipotetica "carga adiada, indice reconstruido em
//! thread" deixaria: `.reg`/`.log` com as N linhas, e `.ndx` **vazio**,
//! aguardando `reindexar()`.
//!
//! ```bash
//! cargo run --release --example preparar-carga-adiada -p phxsql-store -- <dir-do-banco> <n>
//! ```
//!
//! Por que nao existe um jeito de fazer isto pelo protocolo: o motor nao tem
//! (ainda) um modo "grava sem manter indice" -- essa e a divida que
//! `crates/phxsql-server/src/carga.rs` cita. Este instrumento SIMULA o
//! resultado que essa divida, paga, produziria: usa a MESMA tecnica que
//! `crates/phxsql-store/tests/paginacao_log_reindex.rs` ja prova correta
//! (apagar o `.ndx` e recriar vazio com `NdxFile::criar`) -- so que aqui e
//! sobre uma tabela de verdade, no diretorio onde o `phxsqld` vai abri-la.
//!
//! O custo de inserir as N linhas conta como PREPARO (a bancada que chama
//! isto nao cronometra este processo) -- o que se mede depois e o
//! `reindexar()` sozinho, pela RPC `"op":"reindexar"`, com o servidor de pe.

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::ndx::NdxFile;
use phxsql_store::table::Table;

struct Rng(u64);
impl Rng {
    fn proximo(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

const CIDADES: [&str; 8] = [
    "Blumenau",
    "Joinville",
    "Itajai",
    "Curitiba",
    "Chapeco",
    "Lages",
    "Florianopolis",
    "Criciuma",
];

fn colunas() -> Vec<Column> {
    vec![
        Column::new("id", ColumnType::Int8).obrigatoria(),
        Column::new("produto", ColumnType::Str(40)).obrigatoria(),
        Column::new("cidade", ColumnType::Str(20)),
    ]
}

fn indices() -> Vec<IndexDef> {
    vec![
        IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria(),
        IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
    ]
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir_banco = args.next().expect("uso: <dir-do-banco> <n>");
    let n: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(200_000);

    let esquema = Schema::new("carga", colunas(), indices()).unwrap();
    let mut t = Table::criar(&dir_banco, esquema.clone()).unwrap();

    let mut ids: Vec<i64> = (1..=n).collect();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for i in (1..ids.len()).rev() {
        let j = (rng.proximo() % (i as u64 + 1)) as usize;
        ids.swap(i, j);
    }
    for i in ids {
        t.inserir(&[
            Value::Int(i),
            Value::Str(format!("Produto {i:08}")),
            Value::Str(CIDADES[(i as usize) % CIDADES.len()].into()),
        ])
        .unwrap();
    }
    t.sincronizar().unwrap();
    let caminho_ndx = std::path::Path::new(&dir_banco).join("carga.ndx");
    drop(t);

    // A tecnica provada em `tests/paginacao_log_reindex.rs`: apagar o `.ndx`
    // inteiro e recriar vazio, casando o esquema. O `.reg`/`.log` ficam
    // intactos -- e por isso `registros()` continua certo e `buscar` comeca
    // a responder vazio ate alguem chamar `reindexar()`.
    std::fs::remove_file(&caminho_ndx).unwrap();
    NdxFile::criar(&caminho_ndx, &esquema).unwrap();

    println!(
        "preparado: {dir_banco}/carga.{{reg,log}} com {n} linhas, .ndx vazio \
         (precisa de reindexar)"
    );
}

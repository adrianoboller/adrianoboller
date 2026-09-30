//! Quanto a marca `.tx` da cascata do embutido custa -- pedido 563.
//!
//! Tres alteracoes da mae pelo `Table::atualizar`, a porta do FFI e do CLI:
//!
//! * **com filha** -- troca a chave de uma mae com uma filha cascateando. E o
//!   caso que passou a gravar a marca (`create_new`, `write`, `sync_all`) e a
//!   apaga-la no fim;
//! * **chave sem filha** -- troca a chave de uma mae que ninguem referencia. O
//!   plano varre as irmas e sai vazio: tem de custar o que custava;
//! * **fora da chave** -- muda a coluna que nenhum indice ve. O portao da
//!   coluna indexada sai na primeira linha: tem de custar o que custava.
//!
//! Roda com:
//! `cargo run --release --example custo-da-marca-do-embutido -p phxsql-store`
//!
//! Cada caso roda em rodadas intercaladas, e a saida da a mediana e a faixa
//! min-max por operacao: so ha diferenca quando as faixas nao se cruzam. O
//! «antes» se mede rodando ESTE arquivo contra a arvore anterior ao 563 -- a
//! API usada e a publica de sempre.

use phxsql_core::schema::{AcaoRi, Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;
use std::time::Instant;

/// Irmas sem chave nenhuma, para a varredura dos esquemas custar o que custa
/// num diretorio real, e nao num de brinquedo.
const IRMAS: usize = 10;
/// Alteracoes por rodada, em cada caso.
const POR_RODADA: usize = 50;
/// Rodadas intercaladas.
const RODADAS: usize = 7;

fn dir(rotulo: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("phx-563-{rotulo}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn mae(d: &std::path::Path) -> Table {
    let e = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    Table::criar(d, e).unwrap()
}

fn filha(d: &std::path::Path) -> Table {
    let e = Schema::new(
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
    .ao_alterar(AcaoRi::Cascata)
    .conferindo(true)])
    .unwrap();
    Table::criar(d, e).unwrap()
}

fn irmas(d: &std::path::Path) {
    for i in 0..IRMAS {
        let e = Schema::new(
            format!("irma{i}"),
            vec![Column::new("x", ColumnType::Int4)],
            vec![],
        )
        .unwrap();
        Table::criar(d, e).unwrap();
    }
}

fn mediana_e_faixa(mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[0], v[v.len() - 1])
}

fn main() {
    let d = dir("custo");
    irmas(&d);
    let mut m = mae(&d);
    // rowid 1: a mae COM filha; rowid 2: a mae sem filha nenhuma.
    let com = m
        .inserir(&[Value::Int(1), Value::Str("Ana".into())])
        .unwrap();
    let sem = m
        .inserir(&[Value::Int(2), Value::Str("Bia".into())])
        .unwrap();
    m.sincronizar().unwrap();
    let mut f = filha(&d);
    f.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
    f.sincronizar().unwrap();
    drop(f);

    // A chave anda por valores que ninguem mais usa: `com` em 1_000_000+,
    // `sem` em 2_000_000+.
    let mut chave_com = 1i64;
    let mut chave_sem = 2i64;
    let mut n = 0i64;
    let mut t_com = Vec::new();
    let mut t_sem = Vec::new();
    let mut t_fora = Vec::new();
    for rodada in 0..RODADAS {
        let casos: [u8; 3] = match rodada % 3 {
            0 => [0, 1, 2],
            1 => [1, 2, 0],
            _ => [2, 0, 1],
        };
        for caso in casos {
            let comeco = Instant::now();
            for _ in 0..POR_RODADA {
                n += 1;
                match caso {
                    0 => {
                        chave_com = 1_000_000 + n;
                        m.atualizar(com, &[Value::Int(chave_com), Value::Str("Ana".into())])
                            .unwrap();
                    }
                    1 => {
                        chave_sem = 2_000_000 + n;
                        m.atualizar(sem, &[Value::Int(chave_sem), Value::Str("Bia".into())])
                            .unwrap();
                    }
                    _ => {
                        m.atualizar(
                            sem,
                            &[Value::Int(chave_sem), Value::Str(format!("Bia {n}"))],
                        )
                        .unwrap();
                    }
                }
            }
            let us = comeco.elapsed().as_secs_f64() * 1e6 / POR_RODADA as f64;
            match caso {
                0 => t_com.push(us),
                1 => t_sem.push(us),
                _ => t_fora.push(us),
            }
        }
    }
    // A filha acompanhou a ultima chave: o medidor mediu o trabalho inteiro.
    let mut f = Table::abrir(&d, "pedidos").unwrap();
    assert_eq!(f.ler(1).unwrap().unwrap()[1], Value::Int(chave_com));
    let marcas = std::fs::read_dir(&d)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tx"))
        .count();
    assert_eq!(marcas, 0, "sobrou marca no diretorio");
    for (rotulo, v) in [
        ("com filha (marca + fsync)", t_com),
        ("chave sem filha", t_sem),
        ("fora da chave", t_fora),
    ] {
        let (med, min, max) = mediana_e_faixa(v);
        println!("{rotulo:28} {med:>9.1} us/op   faixa {min:.1}-{max:.1}");
    }
    std::fs::remove_dir_all(&d).ok();
}

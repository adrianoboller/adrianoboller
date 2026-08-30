//! Quanto custa acrescentar uma coluna a uma tabela que ja tem dado.
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store
//! ./target/release/examples/custo-do-alter [linhas] [linhas] ...
//! ```
//!
//! O sprint 25 chegou com um numero **inferido**: «a casa dos minutos para dez
//! milhoes». Este medidor existe para trocar a inferencia por medida, e para
//! medir as tres saidas que o desenho tinha:
//!
//! * **(a) reescrever a tabela**, que e a saida escolhida: passada unica, slot
//!   a slot, na mesma ordem, e um `rename` no fim. Aqui ela sai em `us/linha`
//!   e em MiB/s, com o custo separado do preparo;
//! * **(b) duas larguras de slot convivendo**, que o formato nao permite --
//!   e o preco que ela cobraria esta medido do outro lado: o endereco deixaria
//!   de sair de uma multiplicacao e passaria a sair de uma busca. As duas
//!   leituras estao aqui, lado a lado;
//! * **(c) so em tabela vazia**, que e o que o Aria exige para desligar
//!   indice. Tambem medida -- e o numero dela e o que mostra que ela nao
//!   resolve o problema, e sim o evita.
//!
//! O medidor nao chuta 10 milhoes: ele mede varios tamanhos, mostra que o
//! custo por linha e constante, e so entao PROJETA -- dizendo que projeta.

use std::time::{Duration, Instant};

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(20)),
            Column::new(
                "saldo",
                ColumnType::Decimal {
                    precisao: 15,
                    escala: 2,
                },
            ),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)])
                .unico()
                .primaria(),
            IndexDef::new("porNome", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Cliente {i:08}")),
        Value::Str(format!("Cidade {}", i % 40)),
        Value::Decimal(i as i128 * 137),
    ]
}

fn dir(rotulo: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("phxsql-alter-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn bytes_do_reg(d: &std::path::Path) -> u64 {
    std::fs::read_dir(d)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("reg"))
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

fn mib(b: u64) -> f64 {
    b as f64 / (1024.0 * 1024.0)
}

struct Medida {
    linhas: u64,
    preparo: Duration,
    alter: Duration,
    antes: u64,
    depois: u64,
}

fn medir(linhas: u64) -> Medida {
    let d = dir(&format!("n{linhas}"));
    let t0 = Instant::now();
    {
        let mut t = Table::criar(&d, esquema()).unwrap();
        let lote: Vec<Vec<Value>> = (1..=linhas as i64).map(linha).collect();
        t.inserir_lote(&lote, true).unwrap();
        t.sincronizar().unwrap();
    }
    let preparo = t0.elapsed();
    let antes = bytes_do_reg(&d);

    let mut t = Table::abrir(&d, "clientes").unwrap();
    let t1 = Instant::now();
    let n = t
        .acrescentar_coluna(
            Column::new("situacao", ColumnType::Str(12)),
            Some(Value::Str("ativo".into())),
        )
        .unwrap();
    let alter = t1.elapsed();
    assert_eq!(n, linhas);
    drop(t);

    let depois = bytes_do_reg(&d);
    let _ = std::fs::remove_dir_all(&d);
    Medida {
        linhas,
        preparo,
        alter,
        antes,
        depois,
    }
}

/// (c) a saida pequena: alterar so enquanto a tabela esta vazia.
fn medir_vazia() -> Duration {
    let d = dir("vazia");
    let mut t = Table::criar(&d, esquema()).unwrap();
    let t0 = Instant::now();
    t.acrescentar_coluna(Column::new("situacao", ColumnType::Str(12)), None)
        .unwrap();
    let dt = t0.elapsed();
    drop(t);
    let _ = std::fs::remove_dir_all(&d);
    dt
}

/// (b) o preco que a largura variavel cobraria de TODA leitura.
///
/// Hoje o endereco sai de `data_offset + (rowid-1) * slot_size` -- uma
/// multiplicacao. Com slots de duas larguras ele teria de sair de uma busca,
/// que e o que o `.ndx` ja faz e ja da para medir: a mesma linha, achada pela
/// conta e achada pela arvore.
fn medir_endereco(linhas: u64) -> (Duration, Duration) {
    let d = dir("endereco");
    let mut t = Table::criar(&d, esquema()).unwrap();
    let lote: Vec<Vec<Value>> = (1..=linhas as i64).map(linha).collect();
    t.inserir_lote(&lote, true).unwrap();
    t.sincronizar().unwrap();
    drop(t);

    let mut t = Table::abrir(&d, "clientes").unwrap();
    let repeticoes = 20_000u64;
    let mut passo = 1u64;

    let t0 = Instant::now();
    for k in 0..repeticoes {
        passo = (passo * 48271) % linhas.max(2);
        let r = passo.max(1);
        std::hint::black_box(t.ler(r).unwrap());
        std::hint::black_box(k);
    }
    let por_conta = t0.elapsed();

    let t1 = Instant::now();
    for k in 0..repeticoes {
        passo = (passo * 48271) % linhas.max(2);
        let r = passo.max(1) as i64;
        let achados = t.buscar("porId", &[Value::Int(r)]).unwrap();
        std::hint::black_box(&achados);
        if let Some(&rowid) = achados.first() {
            std::hint::black_box(t.ler(rowid).unwrap());
        }
        std::hint::black_box(k);
    }
    let por_busca = t1.elapsed();

    drop(t);
    let _ = std::fs::remove_dir_all(&d);
    (por_conta / repeticoes as u32, por_busca / repeticoes as u32)
}

fn main() {
    let tamanhos: Vec<u64> = {
        let dados: Vec<u64> = std::env::args()
            .skip(1)
            .filter_map(|a| a.parse().ok())
            .collect();
        if dados.is_empty() {
            vec![50_000, 200_000, 1_000_000]
        } else {
            dados
        }
    };

    println!("== (a) reescrever a tabela, slot a slot, preservando o rowid ==\n");
    println!(
        "{:>10}  {:>10}  {:>10}  {:>10}  {:>9}  {:>9}",
        "linhas", "preparo", "alter", "us/linha", "MiB antes", "MiB/s"
    );
    let mut medidas = Vec::new();
    for n in &tamanhos {
        let m = medir(*n);
        let us = m.alter.as_secs_f64() * 1e6 / m.linhas as f64;
        // Uma passada le o arquivo velho e escreve o novo: os dois contam.
        let taxa = mib(m.antes + m.depois) / m.alter.as_secs_f64();
        println!(
            "{:>10}  {:>9.2}s  {:>9.3}s  {:>10.3}  {:>9.1}  {:>9.1}",
            m.linhas,
            m.preparo.as_secs_f64(),
            m.alter.as_secs_f64(),
            us,
            mib(m.antes),
            taxa
        );
        medidas.push(m);
    }

    let ultimo = medidas.last().unwrap();
    let us_linha = ultimo.alter.as_secs_f64() * 1e6 / ultimo.linhas as f64;
    println!();
    println!(
        "  o slot cresceu {} bytes por linha ({:.1} -> {:.1} MiB no maior)",
        (ultimo.depois - ultimo.antes) / ultimo.linhas.max(1),
        mib(ultimo.antes),
        mib(ultimo.depois)
    );
    println!(
        "  PROJECAO (e projecao, nao medida): 10.000.000 de linhas a {:.3} us/linha = {:.1} s",
        us_linha,
        us_linha * 10_000_000.0 / 1e6
    );

    println!("\n== (c) so em tabela vazia ==\n");
    let vazia = medir_vazia();
    println!(
        "  alterar uma tabela vazia: {:.3} ms -- e nao resolve o problema, evita-o",
        vazia.as_secs_f64() * 1e3
    );

    println!("\n== (b) o preco da largura variavel, no caminho de LEITURA ==\n");
    let (conta, busca) = medir_endereco(200_000);
    println!(
        "  endereco por conta  (hoje): {:>8.2} us por linha lida",
        conta.as_secs_f64() * 1e6
    );
    println!(
        "  endereco por busca  (b)   : {:>8.2} us por linha lida  = {:.2}x",
        busca.as_secs_f64() * 1e6,
        busca.as_secs_f64() / conta.as_secs_f64().max(1e-12)
    );
    println!(
        "\n  a (b) trocaria uma multiplicacao por uma descida de arvore em TODA\n  \
         leitura, para poupar uma passada UMA vez. E o formato nem a permite:\n  \
         o `slot_size` e um campo so, no cabecalho do volume."
    );
}

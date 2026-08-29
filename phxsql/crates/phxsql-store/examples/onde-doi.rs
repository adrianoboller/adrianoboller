//! Onde a insercao gasta o tempo. Um fator por vez.
//!
//! ```bash
//! cargo run --release --example onde-doi -- [linhas]
//! ```
//!
//! A bancada diz QUE a insercao e lenta (248 us por linha, 99% de CPU, disco
//! parado). Nao diz ONDE. Este medidor separa as parcelas montando a mesma
//! tabela com esquemas diferentes e inserindo as mesmas linhas em cada um:
//!
//! - `so .reg` -- sem indice nenhum: o custo do heap mais o diario
//! - `+1 indice` -- um indice comum
//! - `+1 unico` -- o mesmo indice, agora unico. A diferenca e a busca que toda
//!   insercao faz antes de gravar, para conferir a chave
//! - `+2 indices` -- a forma da bancada, com o segundo indice de baixa
//!   cardinalidade
//!
//! A conta de cada parcela sai da subtracao, e o resto do relatorio mede
//! diretamente as duas suspeitas que aparecem no caminho de cada pagina do
//! `.ndx`: o CRC-32 da pagina inteira e a chamada de sistema.

use std::time::Instant;

use phxsql_core::crc::crc32;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

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
        Column::new(
            "valor",
            ColumnType::Decimal {
                precisao: 15,
                escala: 2,
            },
        ),
        Column::new("cadastro", ColumnType::Date),
    ]
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Produto {i:08}")),
        Value::Str(CIDADES[(i as usize) % CIDADES.len()].into()),
        Value::Decimal(((i % 900_000) + 100) as i128),
        Value::Date(20_000 + (i % 400) as i32),
    ]
}

/// Quanto custou por linha, e quantas paginas do `.ndx` cada linha tocou.
struct Medida {
    us_por_linha: f64,
    acertos: f64,
    lidas: f64,
    gravadas: f64,
}

fn medir(rotulo: &str, indices: Vec<IndexDef>, n: i64) -> Medida {
    let dir = std::env::temp_dir().join(format!("phx-onde-doi-{}-{}", std::process::id(), rotulo));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let esquema = Schema::new("precos", colunas(), indices).unwrap();
    let mut t = Table::criar(&dir, esquema).unwrap();

    // As linhas sao montadas ANTES do cronometro: o custo de formatar texto e
    // o mesmo em todas as variantes e nao e o que se quer medir.
    let linhas: Vec<Vec<Value>> = (1..=n).map(linha).collect();

    let inicio = Instant::now();
    for l in &linhas {
        t.inserir(l).unwrap();
    }
    t.sincronizar().unwrap();
    let s = inicio.elapsed().as_secs_f64();

    let (acertos, lidas, gravadas) = t.estatisticas_paginas();
    let _ = std::fs::remove_dir_all(&dir);
    println!(
        "  {rotulo:<14} {:>8.2}s  {:>9.0} linhas/s  {:>7.1} us por linha",
        s,
        n as f64 / s,
        s * 1e6 / n as f64
    );
    Medida {
        us_por_linha: s * 1e6 / n as f64,
        acertos: acertos as f64 / n as f64,
        lidas: lidas as f64 / n as f64,
        gravadas: gravadas as f64 / n as f64,
    }
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100_000);

    println!("=== insercao de {n} linhas, um fator por vez ===\n");

    let so_reg = medir("so .reg", vec![], n).us_por_linha;
    let um = medir(
        "+1 indice",
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])],
        n,
    )
    .us_por_linha;
    let um_unico = medir(
        "+1 unico",
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        n,
    )
    .us_por_linha;
    let m = medir(
        "+2 indices",
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
        n,
    );
    let dois = m.us_por_linha;

    println!("\n=== o que cada parcela custa, por linha ===\n");
    println!(
        "  .reg + .log ................ {so_reg:>7.1} us   {:>5.1}%",
        so_reg / dois * 100.0
    );
    println!(
        "  primeiro indice ............ {:>7.1} us   {:>5.1}%",
        um - so_reg,
        (um - so_reg) / dois * 100.0
    );
    println!(
        "  conferir a chave unica ..... {:>7.1} us   {:>5.1}%",
        um_unico - um,
        (um_unico - um) / dois * 100.0
    );
    println!(
        "  segundo indice ............. {:>7.1} us   {:>5.1}%",
        dois - um_unico,
        (dois - um_unico) / dois * 100.0
    );
    println!("  {:-<31} {dois:>7.1} us   100.0%", " TOTAL ");

    // --------------------------------------------------------------- CRC
    // Toda leitura e toda gravacao de pagina do `.ndx` passa a pagina inteira
    // pelo CRC-32. Quanto isso custa, isolado?
    let pagina = vec![0x5Au8; 4096];
    let voltas = 200_000;
    let inicio = Instant::now();
    let mut acc = 0u32;
    for _ in 0..voltas {
        acc = acc.wrapping_add(crc32(&pagina));
    }
    let por_pagina = inicio.elapsed().as_secs_f64() * 1e6 / voltas as f64;
    // A terceira suspeita, que o proprio texto abaixo insinuava sem medir:
    // `ler_pagina` devolve `Vec<u8>`, entao um ACERTO de cache copia os 4 KiB
    // inteiros. Nao paga CRC, mas paga a copia -- e o numero de acertos cresce
    // com a altura da arvore, que cresce com a tabela.
    //
    // `black_box` nao e decoracao: sem ele o LLVM ve que a copia nao e usada e
    // apaga o laco inteiro -- a primeira versao deste medidor mediu 0,00 us
    // para copiar 4 KiB, que e impossivel, e o numero passaria como bom.
    let inicio = Instant::now();
    let mut soma = 0usize;
    for _ in 0..voltas {
        let copia = std::hint::black_box(&pagina).clone();
        soma += std::hint::black_box(&copia)[0] as usize;
    }
    let copia_pagina = inicio.elapsed().as_secs_f64() * 1e6 / voltas as f64;

    println!("\n=== as tres suspeitas do caminho de cada pagina ===\n");
    println!("  CRC-32 de uma pagina de 4 KiB .... {por_pagina:.2} us   (acumulador {acc:x})");
    println!("  COPIAR uma pagina de 4 KiB ....... {copia_pagina:.2} us   (soma {soma})");

    // --------------------------------------------------- chamada de sistema
    // Um `lseek` num arquivo ja aberto e a chamada mais barata que o caminho
    // faz. Serve de piso para o custo de ir ao nucleo.
    use std::io::{Seek, SeekFrom};
    let alvo = std::env::temp_dir().join(format!("phx-seek-{}", std::process::id()));
    let mut f = std::fs::File::create(&alvo).unwrap();
    let inicio = Instant::now();
    for i in 0..voltas {
        f.seek(SeekFrom::Start((i % 4096) as u64)).unwrap();
    }
    let por_seek = inicio.elapsed().as_secs_f64() * 1e6 / voltas as f64;
    let _ = std::fs::remove_file(&alvo);
    println!("  um lseek .......................... {por_seek:.2} us");

    // Os toques de pagina sao CONTADOS, e nao citados de um `strace` de outro
    // dia: o cache de paginas mudou esses numeros, e um numero escrito a mao
    // teria continuado dizendo o de antes.
    println!("\n=== o que cada linha toca no `.ndx`, na forma de 2 indices ===\n");
    println!(
        "  paginas servidas pelo cache ....... {:.2} por linha",
        m.acertos
    );
    println!(
        "  paginas lidas do arquivo .......... {:.2} por linha",
        m.lidas
    );
    println!(
        "  paginas gravadas .................. {:.2} por linha",
        m.gravadas
    );
    let com_crc = m.lidas + m.gravadas;
    println!(
        "\n  So a leitura do arquivo e a gravacao passam pelo CRC -- {com_crc:.2} paginas\n  \
         por linha, ou {:.1} us de CRC, de {dois:.1} us medidos ({:.0}%).",
        com_crc * por_pagina,
        com_crc * por_pagina / dois * 100.0
    );
    // O acerto de cache nao paga CRC -- mas paga a COPIA, porque `ler_pagina`
    // devolve `Vec<u8>`. E o numero de acertos cresce com a altura da arvore.
    let copiadas = m.acertos + m.lidas + m.gravadas;
    println!(
        "  E toda pagina que entra ou sai do cache e COPIADA: {copiadas:.2} por\n  \
         linha, ou {:.1} us, {:.0}% -- e este cresce com a tabela, porque a\n  \
         arvore fica mais alta e a descida toca mais paginas.",
        copiadas * copia_pagina,
        copiadas * copia_pagina / dois * 100.0
    );
    println!(
        "\n  Um lseek custa {por_seek:.2} us: mesmo 41 chamadas por linha dariam {:.1} us.",
        41.0 * por_seek
    );
}

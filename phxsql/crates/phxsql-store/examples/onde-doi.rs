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
use phxsql_store::log::{LogFile, Operacao};
use phxsql_store::reg::{RegFile, SLOT_CAB};
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

/// So o `.log` (o diario da replicacao), isolado do `.reg`.
///
/// O diario e montado EXATAMENTE como a `Table::criar` monta o dela -- a mesma
/// `esquema.paginacao().para_externos()` --, e `registrar` e o que o `inserir`
/// chama por linha (`registrar_detalhado(op, rowid, versao, &[], None, 0)`, com
/// `imagem_no_diario: false`). Assim o custo isolado e o MESMO que o inserto
/// paga, e nao um parente. Sem `fsync` por linha, como o laco do `inserir`.
fn medir_so_log(n: i64) -> f64 {
    let dir = std::env::temp_dir().join(format!("phx-onde-doi-log-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let esquema = Schema::new("precos", colunas(), vec![]).unwrap();
    let externos = esquema.paginacao().para_externos();
    let mut log = LogFile::criar(&dir, "precos", externos).unwrap();
    let inicio = Instant::now();
    for i in 1..=n {
        log.registrar(Operacao::Inclusao, i as u64, 1).unwrap();
    }
    log.sincronizar().unwrap();
    let s = inicio.elapsed().as_secs_f64();
    let _ = std::fs::remove_dir_all(&dir);
    s * 1e6 / n as f64
}

/// O split do `.reg` heap: onde vao os ~3,8 us da gravacao do DADO em si -- a
/// parcela que sobrou como gargalo do inserto depois que o `.ndx` ganhou cache.
///
/// O `RegFile` sozinho NAO toca o `.log` (isso e do `Table`), entao medi-lo
/// direto da o custo PURO do heap, sem a subtracao que o resto do medidor usa.
/// Por linha, sem cifra e num volume so, ele faz duas coisas:
///
/// - montar o slot: alocar `slot_size` bytes, copiar o payload, e o CRC-32 do
///   corpo (o que vai ao disco);
/// - DOIS `escrever`: o slot no offset dele, e o cabecalho de contadores no
///   offset 0 -- este ultimo a CADA linha, dentro de `gravar_contadores`.
///
/// As parcelas saem medidas ISOLADAS e reconciliadas contra o custo direto; o
/// que sobra e nomeado, nao varrido para baixo do tapete.
fn medir_reg_split(n: i64) {
    use std::hint::black_box;
    use std::io::{Seek, SeekFrom, Write};

    // --- verdade de campo: o custo direto e puro do `.reg` ---
    let dir = std::env::temp_dir().join(format!("phx-reg-split-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let esquema = Schema::new("precos", colunas(), vec![]).unwrap();
    let plen = esquema.payload_len();
    let mut reg = RegFile::criar(&dir, "precos", esquema).unwrap();
    // O conteudo do payload nao entra na conta: o inserir copia `plen` bytes e
    // passa o corpo pelo CRC, seja qual for o valor. Bytes fixos bastam.
    let payload = vec![0x5Au8; plen];
    let inicio = Instant::now();
    for _ in 1..=n {
        reg.inserir(&payload).unwrap();
    }
    reg.sincronizar().unwrap();
    let direto = inicio.elapsed().as_secs_f64() * 1e6 / n as f64;
    let slot_size = reg.slot_size();
    drop(reg);
    let _ = std::fs::remove_dir_all(&dir);

    let corpo = slot_size - SLOT_CAB; // exatamente o que o CRC cobre

    // --- A. CRC-32 do corpo do slot, isolado ---
    let corpo_bytes = vec![0x5Au8; corpo];
    let inicio = Instant::now();
    let mut acc = 0u32;
    for _ in 0..n {
        acc = acc.wrapping_add(crc32(black_box(&corpo_bytes)));
    }
    let crc = inicio.elapsed().as_secs_f64() * 1e6 / n as f64;

    // --- B. montar o slot como o inserir monta (sem cifra), com o CRC dentro.
    // `black_box` no payload e no slot para o LLVM nao apagar o trabalho: a
    // primeira versao deste medidor sem ele mediu 0,00 us, que e impossivel. ---
    let inicio = Instant::now();
    let mut soma = 0u64;
    for _ in 0..n {
        let mut slot = vec![0u8; slot_size];
        slot[0] = 1; // STATUS_ATIVO
        slot[8..16].copy_from_slice(&1u64.to_le_bytes()); // versao
        slot[SLOT_CAB..SLOT_CAB + plen].copy_from_slice(black_box(&payload));
        let c = crc32(&slot[SLOT_CAB..]);
        slot[4..8].copy_from_slice(&c.to_le_bytes());
        soma += black_box(&slot)[0] as u64;
    }
    let montagem = inicio.elapsed().as_secs_f64() * 1e6 / n as f64;

    // --- C e D. O padrao de syscall que `volume::escrever` faz: seek + write_all
    // num arquivo CRU (o volume nao usa BufWriter), sem fsync por linha, como o
    // laco do inserir. C e' so o slot; D acrescenta o segundo write -- o
    // cabecalho de contadores no offset 0, que `gravar_contadores` faz a CADA
    // linha. `D - C` e' o preco desse segundo write.
    //
    // O cabecalho tem 128 B (o CAB_LEN uncifrado do reg.rs, privado). O custo do
    // segundo write e' do SYSCALL (seek + write), nao dos bytes: 64, 128 ou 192
    // dariam o mesmo numero, entao mudar o CAB_LEN nao move este resultado. ---
    const CAB: usize = 128;
    let slot_buf = vec![0x5Au8; slot_size];
    let cab_buf = vec![0u8; CAB];
    let data_offset: u64 = 4096;

    let alvo = std::env::temp_dir().join(format!("phx-reg-1w-{}", std::process::id()));
    let mut f = std::fs::File::create(&alvo).unwrap();
    let inicio = Instant::now();
    for i in 0..n as u64 {
        f.seek(SeekFrom::Start(data_offset + i * slot_size as u64))
            .unwrap();
        f.write_all(black_box(&slot_buf)).unwrap();
    }
    let um_write = inicio.elapsed().as_secs_f64() * 1e6 / n as f64;
    drop(f);
    let _ = std::fs::remove_file(&alvo);

    let alvo = std::env::temp_dir().join(format!("phx-reg-2w-{}", std::process::id()));
    let mut f = std::fs::File::create(&alvo).unwrap();
    let inicio = Instant::now();
    for i in 0..n as u64 {
        f.seek(SeekFrom::Start(data_offset + i * slot_size as u64))
            .unwrap();
        f.write_all(black_box(&slot_buf)).unwrap();
        f.seek(SeekFrom::Start(0)).unwrap();
        f.write_all(black_box(&cab_buf)).unwrap();
    }
    let dois_writes = inicio.elapsed().as_secs_f64() * 1e6 / n as f64;
    drop(f);
    let _ = std::fs::remove_file(&alvo);

    // O `stat` por linha do `garantir` -- medido em ~29% do custo do `.reg` na
    // 1a corrida (DESEMPENHO.md 2.2.1) -- SAIU do caminho na Fix 2: `garantir`
    // passou a confiar no cache `abertos`. Por isso ele nao aparece mais aqui: o
    // medidor mede o caminho de HOJE, e o `direto` acima ja caiu de 4,45 para
    // ~2,64 us por linha. A conta da recusa fica no papel, nao no laco.

    // --- F. o que `montar_cabecalho` faz a CADA linha, dentro de
    // `gravar_contadores`, ALEM do write que ja contamos: alocar o buf, o
    // `agora()` (um clock_gettime) e o CRC-32 do cabecalho. Os ~15 campos e o
    // `material.gravar` sao copias pequenas e ficam no resto. ---
    // `agora()` do store e' exatamente este SystemTime::now() -> duration_since,
    // um clock_gettime. Replicado aqui porque o modulo `util` e' privado.
    let inicio = Instant::now();
    let mut soma2 = 0u64;
    for _ in 0..n {
        let mut buf = vec![0u8; CAB];
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        buf[68..76].copy_from_slice(&black_box(t).to_le_bytes());
        let c = crc32(&buf[..CAB - 4]);
        buf[CAB - 4..].copy_from_slice(&c.to_le_bytes());
        soma2 += black_box(&buf)[0] as u64;
    }
    let cabecalho = inicio.elapsed().as_secs_f64() * 1e6 / n as f64;

    let contadores = dois_writes - um_write;
    let montagem_sem_crc = montagem - crc;
    let resto = direto - montagem - dois_writes - cabecalho;
    // Os acumuladores existem so para o `black_box` ter onde ancorar; imprimi-los
    // impede o "valor nao usado" e prova que o laco nao foi apagado.
    let ancora = acc as u64 ^ soma ^ soma2;

    println!("\n=== o split do .reg heap, por linha ({n} linhas) ===\n");
    println!(
        "  slot {slot_size} B  (corpo sob CRC {corpo} B; payload {plen} B)  ancora={ancora:x}\n"
    );
    println!("  .reg inserir DIRETO (verdade de campo) ...... {direto:>7.2} us   100.0%");
    println!("  {:-<49}", "");
    println!(
        "  montar o slot (aloca+copia+campos+CRC) ...... {montagem:>7.2} us   {:>5.1}%",
        montagem / direto * 100.0
    );
    println!(
        "     -- CRC-32 do corpo ....................... {crc:>7.2} us   {:>5.1}%",
        crc / direto * 100.0
    );
    println!(
        "     -- aloca + copia + campos ................ {montagem_sem_crc:>7.2} us   {:>5.1}%",
        montagem_sem_crc / direto * 100.0
    );
    println!(
        "  os dois writes (slot + contadores) .......... {dois_writes:>7.2} us   {:>5.1}%",
        dois_writes / direto * 100.0
    );
    println!(
        "     -- o slot (1 seek+write) ................. {um_write:>7.2} us   {:>5.1}%",
        um_write / direto * 100.0
    );
    println!(
        "     -- os contadores (2o seek+write/linha) ... {contadores:>7.2} us   {:>5.1}%",
        contadores / direto * 100.0
    );
    println!(
        "  montar_cabecalho (alloc + agora() + CRC) .... {cabecalho:>7.2} us   {:>5.1}%",
        cabecalho / direto * 100.0
    );
    println!("  {:-<49}", "");
    println!(
        "  resto (garantir no cache, paginacao/localizar,\n         arquivo(), campos do cabecalho, marcar) {resto:>7.2} us   {:>5.1}%",
        resto / direto * 100.0
    );
    println!(
        "\n  (o stat por linha do garantir, ~29% na 1a corrida, saiu na Fix 2:\n   agora garantir confia no cache abertos -- DESEMPENHO.md 2.2.1)"
    );
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100_000);

    println!("=== insercao de {n} linhas, um fator por vez ===\n");

    let so_reg = medir("so .reg", vec![], n).us_por_linha;
    let so_log = medir_so_log(n);
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
        "  .reg (heap) ................ {:>7.1} us   {:>5.1}%",
        so_reg - so_log,
        (so_reg - so_log) / dois * 100.0
    );
    println!(
        "  .log (diario/replicacao) ... {so_log:>7.1} us   {:>5.1}%",
        so_log / dois * 100.0
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

    // O `.reg` heap e a parcela que sobrou como gargalo do inserto depois que o
    // `.ndx` ganhou cache. Aqui ela se abre em CRC x montagem x syscall.
    medir_reg_split(n);
}

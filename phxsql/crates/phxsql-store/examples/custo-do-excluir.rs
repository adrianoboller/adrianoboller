//! Onde doi a exclusao fisica: o tempo POR ARQUIVO e POR CHAMADA, e as
//! chamadas de sistema por exclusao contadas pelo nucleo -- na mesma corrida.
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store          # binario velho mede o passado
//! cargo run --release --example custo-do-excluir -- <n> <m> [lote]
//! cargo run --release --example custo-do-excluir -- 200000 20000
//! PHX_EXCLUSAO_NA_JANELA=1 cargo run --release --example custo-do-excluir -- 200000 20000 200
//! ```
//!
//! # As duas perguntas que ele responde
//!
//! **A primeira e' a de sempre** (`docs/DESEMPENHO.md` §4.12, Sprint 1 do
//! `SPRINTS-CASSANDRA.md`): quanto do excluir e' o `fsync` que
//! `LixeiraFile::guardar` faz por exclusao. A variavel de ambiente liga o
//! mesmo interruptor que `recursos.exclusao_na_janela` liga no servidor, e o
//! terceiro argumento e' o tamanho da janela (`200` = `recursos.lote_operacoes`
//! de fabrica; sem ele a janela nunca fecha e o numero e' o teto, honesto so'
//! para a carga reservada). Essa secao continua saindo primeiro, igual.
//!
//! **A segunda e' a do pedido 259**: sem `fsync` nenhum, o excluir custa
//! 24-28 us onde o inserir custa 3,7-4,4 (bancada CRUD, regime `sistema`), e
//! o `strace` da bancada viu 8 `write` e ~5 `openat` por exclusao. Este
//! medidor divide esse tempo em vez de supor -- e' o irmao do `onde-doi`:
//!
//! - **por ablacao**: a mesma tabela com uma peca a menos (sem indice; com
//!   trinta irmas no diretorio; com o `fsync` da lixeira ligado), e a
//!   diferenca e' o preco da peca;
//! - **por chamada isolada**: cada arquivo sozinho fazendo EXATAMENTE o que o
//!   excluir lhe pede (`LixeiraFile::guardar`, `MotivoFile::registrar`,
//!   `LogFile::registrar_detalhado`, `RegFile::excluir`, `Table::ler`, a
//!   varredura `Database::tabelas`), reconciliado contra o direto;
//! - **por chamada de sistema**: o proprio binario se reexecuta com `--sonda`
//!   sob `strace -f -y`, e conta `openat`/`write`/`read`... POR ARQUIVO, por
//!   diferenca entre 1.000 e 200 exclusoes -- contado nesta corrida, e nao
//!   citado de um `strace` de outro dia, que foi a licao do `onde-doi`.
//!
//! O cache do `.ndx` e' o quente da bancada CRUD (`definir_cache_paginas(1e6)`)
//! para o numero daqui ser comparavel ao de la'.

#[path = "apoio/strace.rs"]
mod strace;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::catalogo::Instancia;
use phxsql_store::lixeira::LixeiraFile;
use phxsql_store::log::{LogFile, Operacao};
use phxsql_store::motivo::{MotivoFile, Tipo};
use phxsql_store::reg::RegFile;
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

const IRMAS: usize = 30;

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

fn dois_indices() -> Vec<IndexDef> {
    vec![
        IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
        IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
    ]
}

fn esquema_com(indices: Vec<IndexDef>) -> Schema {
    Schema::new("precos", colunas(), indices).expect("esquema da carga")
}

fn esquema() -> Schema {
    esquema_com(dois_indices())
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

/// O mesmo espalhamento da bancada: alvos pela tabela inteira, nao so' o fim.
fn alvo(k: i64, n: i64) -> u64 {
    ((k * 7_919) % n.max(1) + 1) as u64
}

fn montar(dir: &Path, indices: Vec<IndexDef>, n: i64) -> Table {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    let mut t = Table::criar(dir, esquema_com(indices)).unwrap();
    for i in 1..=n {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();
    t
}

/// Trinta tabelas ao lado, SEM chave estrangeira: e' o que a busca reversa da
/// integridade abre a cada exclusao para perguntar «alguem aponta para mim?».
/// Os nomes nao levam `_` seguido de digito: isso e' sufixo de volume para o
/// catalogo, e trinta irmas virariam uma.
fn criar_irmas(dir: &Path) {
    for i in 0..IRMAS {
        let nome = format!(
            "irma{}{}",
            (b'a' + (i / 26) as u8) as char,
            (b'a' + (i % 26) as u8) as char
        );
        let esquema = Schema::new(
            &nome,
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("nome", ColumnType::Str(20)),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap();
        let mut t = Table::criar(dir, esquema).unwrap();
        t.inserir(&[Value::Int(1), Value::Str("x".into())]).unwrap();
        t.sincronizar().unwrap();
    }
}

/// O laco de `m` exclusoes fisicas, cronometrado sozinho. `lote > 0` fecha a
/// janela a cada `lote` exclusoes, como `recursos.lote_operacoes` faz.
fn laco_de_excluir(t: &mut Table, n: i64, m: i64, lote: i64) -> (f64, u64) {
    let inicio = Instant::now();
    let mut feitas = 0u64;
    for k in 0..m {
        if t.excluir(alvo(k, n)).unwrap() {
            feitas += 1;
        }
        if lote > 0 && (k + 1) % lote == 0 {
            t.sincronizar().unwrap();
        }
    }
    let s = inicio.elapsed().as_secs_f64();
    t.sincronizar().unwrap();
    (s, feitas)
}

/// Microssegundos por exclusao numa tabela montada do zero com estas pecas.
fn medir(
    rotulo: &str,
    base: &Path,
    indices: Vec<IndexDef>,
    irmas: bool,
    na_janela: bool,
    n: i64,
    m: i64,
) -> f64 {
    let dir = base.join(rotulo.replace(' ', "-"));
    if irmas {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        criar_irmas(&dir);
    }
    let mut t = if irmas {
        let mut t = Table::criar(&dir, esquema_com(indices)).unwrap();
        for i in 1..=n {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
        t
    } else {
        montar(&dir, indices, n)
    };
    phxsql_store::lixeira::definir_na_janela(na_janela);
    let (s, feitas) = laco_de_excluir(&mut t, n, m, 0);
    drop(t);
    let _ = std::fs::remove_dir_all(&dir);
    let us = s * 1e6 / feitas.max(1) as f64;
    println!("  {rotulo:<34} {us:>8.2} us por exclusao");
    us
}

/// Cronometra `f` `m` vezes e devolve microssegundos por chamada.
fn isolado<F: FnMut(i64)>(rotulo: &str, m: i64, mut f: F) -> f64 {
    let inicio = Instant::now();
    for k in 0..m {
        f(k);
    }
    let us = inicio.elapsed().as_secs_f64() * 1e6 / m as f64;
    println!("  {rotulo:<34} {us:>8.2} us por chamada");
    us
}

/// O corpo tracado: abre a tabela semeada e exclui. Nada antes, nada depois.
fn sonda(dir: &str, n: i64, m: i64) {
    phxsql_store::ndx::definir_cache_paginas(1_000_000);
    phxsql_store::lixeira::definir_na_janela(true);
    let mut t = Table::abrir(dir, "precos").expect("abrir a tabela semeada");
    for k in 0..m {
        t.excluir(alvo(k, n)).unwrap();
    }
}

/// A que arquivo (ou diretorio) uma chamada se refere, no nome que a casa usa.
fn balde(alvo: &str, dir: &Path) -> String {
    let p = Path::new(alvo);
    if p == dir || alvo.is_empty() {
        return "diretorio".to_string();
    }
    let nome = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
    if nome.starts_with("irma") {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("?");
        return format!("irma*.{ext}");
    }
    nome.to_string()
}

/// Chamadas de sistema por exclusao, `(syscall, arquivo) -> quantas`, por
/// diferenca entre 1.000 e 200 exclusoes -- a abertura da tabela cancela.
fn contar_syscalls(base: &Path, n: i64, irmas: bool) -> Option<BTreeMap<(String, String), f64>> {
    let eu = std::env::current_exe().ok()?;
    let mut por_m = Vec::new();
    for m in [200i64, 1_000] {
        let dir = base.join(format!("sonda-{}-{m}", if irmas { "irmas" } else { "so" }));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // `montar` apaga o diretorio antes de criar: as irmas entram DEPOIS
        // da tabela, senao a sonda "com irmas" roda sem irma nenhuma -- foi o
        // que a primeira corrida deste medidor fez, e as duas secoes sairam
        // iguais.
        drop(montar(&dir, dois_indices(), n));
        if irmas {
            criar_irmas(&dir);
        }
        let log = base.join(format!("strace-{m}.log"));
        let args = [
            "--sonda".to_string(),
            dir.display().to_string(),
            n.to_string(),
            m.to_string(),
        ];
        strace::tracar(
            &eu,
            &args,
            "openat,statx,newfstatat,getdents64,read,pread64,write,pwrite64,lseek,fsync,close",
            &log,
        )?;
        let mut contagem: BTreeMap<(String, String), f64> = BTreeMap::new();
        for (chamada, alvo) in strace::chamadas(&log) {
            *contagem.entry((chamada, balde(&alvo, &dir))).or_insert(0.0) += 1.0;
        }
        let _ = std::fs::remove_dir_all(&dir);
        por_m.push(contagem);
    }
    let (pequena, grande) = (&por_m[0], &por_m[1]);
    let mut saida = BTreeMap::new();
    for (k, v) in grande {
        let por_op = (v - pequena.get(k).copied().unwrap_or(0.0)) / 800.0;
        if por_op >= 0.005 {
            saida.insert(k.clone(), por_op);
        }
    }
    Some(saida)
}

fn imprimir_syscalls(rotulo: &str, contagem: &BTreeMap<(String, String), f64>) {
    println!("\n  --- {rotulo} ---");
    let mut por_syscall: BTreeMap<&str, f64> = BTreeMap::new();
    for ((s, _), v) in contagem {
        *por_syscall.entry(s).or_insert(0.0) += v;
    }
    let mut linhas: Vec<_> = contagem.iter().collect();
    linhas.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    for ((s, arq), v) in linhas {
        println!("    {s:<12} {arq:<22} {v:>7.2} por exclusao");
    }
    let resumo: Vec<String> = por_syscall
        .iter()
        .map(|(s, v)| format!("{s} {v:.2}"))
        .collect();
    println!("    total: {}", resumo.join(", "));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--sonda") {
        sonda(&args[i + 1], args[i + 2].parse()?, args[i + 3].parse()?);
        return Ok(());
    }
    let n: i64 = args.first().and_then(|s| s.parse().ok()).unwrap_or(200_000);
    let m: i64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let lote: i64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    phxsql_store::ndx::definir_cache_paginas(1_000_000);

    let base: PathBuf =
        std::env::temp_dir().join(format!("phx-custo-do-excluir-{}", std::process::id()));
    std::fs::create_dir_all(&base)?;

    // ------------------------------------------------ 0. a pergunta de sempre
    // O mesmo global que `recursos.exclusao_na_janela` liga no servidor.
    let na_janela =
        std::env::var("PHX_EXCLUSAO_NA_JANELA").is_ok_and(|v| v != "0" && !v.is_empty());
    phxsql_store::lixeira::definir_na_janela(na_janela);
    let mut t = montar(&base.join("sempre"), dois_indices(), n);
    let (s, feitas) = laco_de_excluir(&mut t, n, m, lote);
    drop(t);
    let _ = std::fs::remove_dir_all(base.join("sempre"));
    println!(
        "excluir {feitas} de {n} ({}): {s:.3} s  ({:.2} us/linha, {:.0}/s)",
        match (na_janela, lote) {
            (false, _) => "fsync por exclusao".to_string(),
            (true, 0) => "na janela, sem fechar (o teto)".to_string(),
            (true, n) => format!("na janela, fechando a cada {n}"),
        },
        s * 1e6 / feitas.max(1) as f64,
        feitas as f64 / s.max(1e-9)
    );

    // ------------------------------------------------ 1. por ablacao
    println!(
        "\n=== pedido 259: sem fsync nenhum (regime `sistema`), {m} exclusoes em {n} linhas ===\n"
    );
    println!("--- por ablacao: a mesma tabela, uma peca a menos ---");
    let direto = medir(
        "direto (2 indices, sozinha)",
        &base,
        dois_indices(),
        false,
        true,
        n,
        m,
    );
    let sem_indice = medir("sem indice nenhum", &base, vec![], false, true, n, m);
    let com_irmas = medir(
        &format!("com {IRMAS} irmas no diretorio"),
        &base,
        dois_indices(),
        true,
        true,
        n,
        m,
    );
    let com_fsync = medir(
        "com o fsync da lixeira (fabrica)",
        &base,
        dois_indices(),
        false,
        false,
        n,
        m,
    );
    phxsql_store::lixeira::definir_na_janela(true);

    // ------------------------------------------------ 2. por chamada isolada
    println!("\n--- por chamada isolada: cada arquivo fazendo so' o que o excluir lhe pede ---");
    let esq = esquema();
    let plen = esq.payload_len();
    let pag = esq.paginacao().para_externos();
    let payload = vec![0x5Au8; plen];

    // A varredura das irmas, pela funcao de verdade (`catalogo::tabelas_em`,
    // que `Database::tabelas` chama), no diretorio com a tabela sozinha.
    let inst = Instancia::nova(&base)?;
    let db = inst.criar_database("bancada")?;
    let dir_db = db.diretorio(None)?;
    drop(montar(&dir_db, dois_indices(), 1));
    let varredura = isolado("catalogo: tabelas do diretorio (8 arq)", m, |_| {
        std::hint::black_box(db.tabelas(None).unwrap());
    });

    let dir_l = base.join("isolado-lixeira");
    std::fs::create_dir_all(&dir_l)?;
    let mut lix = LixeiraFile::criar(&dir_l, "precos", pag)?;
    let lixeira = isolado(".trash: LixeiraFile::guardar", m, |k| {
        lix.guardar(alvo(k, n), &payload, Vec::new()).unwrap();
    });
    drop(lix);

    let dir_r = base.join("isolado-reason");
    std::fs::create_dir_all(&dir_r)?;
    let mut mot = MotivoFile::criar(&dir_r, "precos", pag)?;
    let motivo = isolado(".reason: MotivoFile::registrar", m, |k| {
        mot.registrar(Tipo::Fisica, alvo(k, n), "", "id=12345678")
            .unwrap();
    });
    drop(mot);

    let dir_g = base.join("isolado-log");
    std::fs::create_dir_all(&dir_g)?;
    let mut log = LogFile::criar(&dir_g, "precos", pag)?;
    let diario = isolado(".log: LogFile::registrar_detalhado", m, |k| {
        log.registrar_detalhado(Operacao::Exclusao, alvo(k, n), 0, &[], None, 0)
            .unwrap();
    });
    drop(log);

    let dir_e = base.join("isolado-reg");
    std::fs::create_dir_all(&dir_e)?;
    let mut reg = RegFile::criar(&dir_e, "precos", esquema_com(vec![]))?;
    for _ in 1..=n {
        reg.inserir(&payload)?;
    }
    reg.sincronizar()?;
    let reg_ler = isolado(".reg: RegFile::ler (o slot cru)", m, |k| {
        std::hint::black_box(reg.ler(alvo(k, n)).unwrap());
    });
    let reg_excluir = isolado(".reg: RegFile::excluir (status+contadores)", m, |k| {
        reg.excluir(alvo(k, n)).unwrap();
    });
    drop(reg);

    // O registro do `.trash` e o do `.reason` levam um UUID v7 cada (o do
    // `.log` nao leva), e o `sortear` do `phxsql-core/src/uuid.rs` abre
    // `/dev/urandom` a cada chamada -- e mais uma vez a cada milissegundo
    // novo. Sao os quatro `openat` que o strace da bancada viu por exclusao
    // (sob traco toda chamada cai em milissegundo novo), e ninguem sabia de
    // onde vinham.
    let uuid = isolado("Uuid::v7 (abre /dev/urandom)", m, |_| {
        std::hint::black_box(phxsql_core::uuid::Uuid::v7());
    });

    let mut cheia = montar(&base.join("isolado-ler"), dois_indices(), n);
    let ler = isolado("Table::ler (slot + decodificar)", m, |k| {
        std::hint::black_box(cheia.ler(alvo(k, n)).unwrap());
    });
    drop(cheia);

    // ------------------------------------------------ 3. a reconciliacao
    let ndx = direto - sem_indice;
    let irmas = com_irmas - direto;
    let fsync = com_fsync - direto;
    // O excluir le a linha DUAS vezes: `conferir_filhas` chama `ler` (decodificada)
    // e `excluir_de_vez` chama `reg.ler` (o slot) e decodifica de novo para a
    // identidade e as chaves. As duas entram na soma.
    let leituras = ler + reg_ler + ler;
    let soma = varredura + lixeira + motivo + diario + reg_excluir + leituras + ndx;
    let resto = direto - soma;
    let pct = |x: f64| x / direto * 100.0;
    println!("\n=== o excluir, dividido (sem fsync; {direto:.2} us = 100%) ===\n");
    println!(
        "  busca reversa: varrer o diretorio ........ {varredura:>7.2} us  {:>5.1}%",
        pct(varredura)
    );
    println!("     + abrir {IRMAS} irmas (RegFile::abrir cada) {irmas:>7.2} us  (so' com irmas; {:.1}x o excluir)", com_irmas / direto);
    println!(
        "  ler a linha (2x decodificada + 1x slot) .. {leituras:>7.2} us  {:>5.1}%",
        pct(leituras)
    );
    println!(
        "  .trash  guardar ........................... {lixeira:>7.2} us  {:>5.1}%",
        pct(lixeira)
    );
    println!(
        "  .ndx    remover (2 indices, por ablacao) .. {ndx:>7.2} us  {:>5.1}%",
        pct(ndx)
    );
    println!(
        "  .reg    excluir ........................... {reg_excluir:>7.2} us  {:>5.1}%",
        pct(reg_excluir)
    );
    println!(
        "  .reason registrar ......................... {motivo:>7.2} us  {:>5.1}%",
        pct(motivo)
    );
    println!(
        "  .log    registrar ......................... {diario:>7.2} us  {:>5.1}%",
        pct(diario)
    );
    println!(
        "     (dentro do .trash e do .reason: um Uuid::v7 cada, {uuid:.2} us por chamada = {:.2} us, {:.1}%)",
        2.0 * uuid,
        pct(2.0 * uuid)
    );
    println!("  {:-<52}", "");
    println!(
        "  soma das parcelas ......................... {soma:>7.2} us  {:>5.1}%",
        pct(soma)
    );
    println!(
        "  resto (chaves, identidade, motivo, marcas)  {resto:>7.2} us  {:>5.1}%",
        pct(resto)
    );
    println!(
        "\n  o fsync da lixeira, quando ligado (fabrica): +{fsync:.2} us por exclusao ({:.1}x)",
        com_fsync / direto
    );

    // ------------------------------------------------ 4. as chamadas de sistema
    println!("\n=== chamadas de sistema por exclusao (strace -f -y no filho, 1.000 - 200 exclusoes, N = {n}) ===");
    match contar_syscalls(&base, n, false) {
        Some(c) => imprimir_syscalls("tabela sozinha no diretorio", &c),
        None => println!("  sem `strace` nesta maquina -- a contagem nao se substitui."),
    }
    if let Some(c) = contar_syscalls(&base, n, true) {
        imprimir_syscalls(&format!("com {IRMAS} irmas no diretorio"), &c);
    }

    let _ = std::fs::remove_dir_all(&base);
    Ok(())
}

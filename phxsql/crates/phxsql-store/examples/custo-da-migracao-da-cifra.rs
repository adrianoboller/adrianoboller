//! Quanto custa `Criptografar`/`Descriptografar` uma tabela que ja tem dado
//! (pedido 268).
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store
//! ./target/release/examples/custo-da-migracao-da-cifra <linhas> [--por-volume <n>] \
//!     [--modo preparo|cifrar|decifrar]
//! ```
//!
//! # O que mede, e o que NAO mede
//!
//! * **FASE A** (`preparar_migracao_da_cifra`): a reescrita de cada volume ao
//!   lado, sincronizada. E a parte que o servidor roda FORA da trava global.
//!   Sai com o PBKDF2 separado, porque o sal novo custa uma derivacao (~300 ms
//!   a 210.000 iteracoes, uma vez por migracao) e misturar os dois numa media
//!   por linha esconderia os dois.
//! * **FASE B** (`aplicar_migracao_da_cifra`): so `rename` e `fsync` de pasta.
//!   E a parte que o servidor roda DENTRO da trava.
//! * Nao mede o espelho (`.bkp`): ele dobra a escrita da FASE A e nao muda a
//!   conta por slot.
//!
//! # O `fsync` nao se conta aqui dentro
//!
//! O motor nao tem contador de `fsync`; quem conta e a bancada
//! (`bancada/cifra-migracao/medir.py`), sob `strace`, por DIFERENCA entre
//! `--modo preparo` (so o cenario), `cifrar` (cenario + Criptografar) e
//! `decifrar` (os dois). A diferenca isola a migracao do `fsync` do preparo.
//!
//! O modo `decifrar` roda Criptografar primeiro (e a unica forma de ter uma
//! tabela cifrada para decifrar) e mede o Descriptografar.

use std::time::Instant;

use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::value::Value;
use phxsql_store::cofre;
use phxsql_store::table::Table;

fn esquema(por_volume: Option<u64>) -> Schema {
    let e = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
            Column::new("cpf", ColumnType::Str(14)).com_dado_pessoal(DadoPessoal::Sensivel),
            Column::new("cidade", ColumnType::Str(20)),
            Column::new(
                "saldo",
                ColumnType::Decimal {
                    precisao: 15,
                    escala: 2,
                },
            ),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap();
    match por_volume {
        Some(n) => e.com_paginacao(Paginacao::nova(n, 999).unwrap()).unwrap(),
        None => e,
    }
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Cliente {i:08}")),
        Value::Str(format!("{:03}.456.789-00", i % 1000)),
        Value::Str(format!("Cidade {}", i % 40)),
        Value::Decimal(i as i128 * 137),
    ]
}

fn pasta(rotulo: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("phxsql-migracao-{}-{rotulo}", std::process::id()));
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

fn valor_de(args: &[String], nome: &str) -> Option<String> {
    args.iter()
        .position(|a| a == nome)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let linhas: i64 = args
        .first()
        .and_then(|a| a.parse().ok())
        .expect("uso: custo-da-migracao-da-cifra <linhas> [--por-volume n] [--modo m]");
    let por_volume: Option<u64> = valor_de(&args, "--por-volume").and_then(|v| v.parse().ok());
    let modo = valor_de(&args, "--modo").unwrap_or_else(|| "decifrar".into());

    // O cenario nasce EM CLARO (cofre desligado): e a tabela que o dono
    // marcou antes de ter cofre.
    cofre::desligar();
    let d = pasta("tabela");
    let t0 = Instant::now();
    {
        let mut t = Table::criar(&d, esquema(por_volume)).unwrap();
        for i in 1..=linhas {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    let preparo_s = t0.elapsed().as_secs_f64();
    let volumes = std::fs::read_dir(&d)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("reg"))
        .count();
    println!(
        "preparo: {linhas} linhas, {volumes} volume(s), {:.1} MiB, {preparo_s:.2} s",
        bytes_do_reg(&d) as f64 / 1048576.0
    );
    if modo == "preparo" {
        let _ = std::fs::remove_dir_all(&d);
        return;
    }

    // Iteracoes no PADRAO de proposito: baixa-las barataria o sal novo e
    // publicaria um numero que producao nenhuma tem.
    cofre::definir(
        "senha-de-bancada-nao-usar-em-producao",
        cofre::ITERACOES_PADRAO,
    )
    .unwrap();
    // O PBKDF2 do sal novo, medido a parte.
    let tk = Instant::now();
    let _ = cofre::Material::novo().unwrap();
    let pbkdf2_ms = tk.elapsed().as_secs_f64() * 1e3;
    println!("pbkdf2_ms: {pbkdf2_ms:.1}");

    let mut passos: Vec<(&str, bool)> = vec![("cifrar", true)];
    if modo == "decifrar" {
        passos.push(("decifrar", false));
    }
    for (nome, cifrar) in passos {
        let mut t = Table::abrir(&d, "clientes").unwrap();
        let ta = Instant::now();
        let troca = t.preparar_migracao_da_cifra(cifrar).unwrap();
        let fase_a_s = ta.elapsed().as_secs_f64();
        let slots = troca.slots();
        let tb = Instant::now();
        t.aplicar_migracao_da_cifra(troca).unwrap();
        let fase_b_ms = tb.elapsed().as_secs_f64() * 1e3;
        drop(t);
        // O custo por slot sem o PBKDF2, que e por migracao e nao por linha.
        let sem_pbkdf2 = (fase_a_s * 1e3 - if cifrar { pbkdf2_ms } else { 0.0 }).max(0.0);
        println!(
            "{nome}: slots={slots} fase_a_ms={:.1} fase_b_ms={fase_b_ms:.2} us_por_slot={:.3} \
             volumes={volumes} bytes_depois={}",
            fase_a_s * 1e3,
            sem_pbkdf2 * 1e3 / slots.max(1) as f64,
            bytes_do_reg(&d)
        );
    }
    // A prova de que mediu uma migracao e nao uma copia: a tabela abre e
    // conta as linhas.
    let t = Table::abrir(&d, "clientes").unwrap();
    assert_eq!(t.registros(), linhas as u64);
    let _ = std::fs::remove_dir_all(&d);
}

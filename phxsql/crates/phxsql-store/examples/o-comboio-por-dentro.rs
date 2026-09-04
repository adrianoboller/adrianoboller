//! O comboio do fecho de janela, por dentro: quanto e ABRIR e quanto e `fsync`.
//!
//! ```bash
//! cargo run --release --example o-comboio-por-dentro -p phxsql-store -- [K_max] [linhas]
//! ```
//!
//! ## Por que este medidor existe
//!
//! A §12 do `docs/CONCORRENCIA.md` mediu o comboio POR FORA, pela rede: com os
//! escritores fixos em 4 e variando so quantas tabelas distintas eles escrevem,
//! o p99 do leitor -- que le uma tabela que NINGUEM escreve -- vai a 1,96x-2,01x
//! de K=1 para K=4. O mecanismo estava lido no fonte: `descarregar_sujas_com`
//! roda com a trava global na mao e faz, por tabela suja,
//! `abrir_database -> abrir_qualificada -> sincronizar`, em laco, sem soltar.
//!
//! O que NUNCA se separou foram as duas parcelas desse `K x (abrir + fsync)`.
//! E a separacao decide o conserto:
//!
//! - se o peso for o **abrir**, guardar o descritor resolve boa parte SEM
//!   encostar na ordem do group commit -- risco zero;
//! - se o peso for o **fsync**, soltar a trava entre uma tabela e a seguinte e
//!   a unica saida, e isso inverte a ordem que o comentario do laco diz que
//!   «nao se inverte» -- decisao de durabilidade, papel C.
//!
//! *Medir a premissa do item vem antes de implementar o item* -- e o pedido 113
//! ja cobrou isso uma vez, com o alvo certo e a causa errada.
//!
//! ## O que ele NAO mede
//!
//! Nao mede espera de cliente nenhum: e um processo so, sem rede e sem trava.
//! O numero daqui e o TEMPO DE TRABALHO que o fecho segura a trava presa; a
//! espera que isso causa nos outros esta medida na §12, por fora. Os dois se
//! conferem: se as duas metades somadas nao baterem com o que a §12 viu crescer
//! por K, uma das duas medicoes esta errada -- e ja aconteceu de a de fora estar
//! (o campo `max`, pedido 183).

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::catalogo::Instancia;
use phxsql_store::table::Table;

fn esquema(nome: &str) -> Schema {
    Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("produto", ColumnType::Str(40)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(20)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
    )
    .unwrap()
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Produto {i:08}")),
        Value::Str("Blumenau".into()),
    ]
}

fn main() {
    let mut arg = std::env::args().skip(1);
    let k_max: usize = arg.next().and_then(|s| s.parse().ok()).unwrap_or(16);
    let semear: i64 = arg.next().and_then(|s| s.parse().ok()).unwrap_or(2_000);
    let janelas = 30;

    let base = std::env::temp_dir().join(format!("phx-comboio-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let inst = Instancia::nova(&base).unwrap();
    let db = inst.criar_database("bancada").unwrap();

    // As tabelas nascem semeadas: tabela vazia abre mais barato que tabela com
    // dado, e o comboio de uma base real acontece sobre tabelas com dado.
    let nomes: Vec<String> = (0..k_max).map(|i| format!("tab{i:02}")).collect();
    for n in &nomes {
        let mut t = Table::criar(db.caminho(), esquema(n)).unwrap();
        for i in 1..=semear {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }

    println!("=== o comboio do fecho de janela, por dentro ===");
    println!("    {k_max} tabelas, {semear} linhas semeadas em cada, {janelas} janelas por K\n");
    println!(
        "  {:>3}  {:>11}  {:>11}  {:>11}  {:>7}",
        "K", "abrir (us)", "fsync (us)", "fecho (us)", "fsync%"
    );
    println!("  {}", "-".repeat(52));

    let mut linhas_do_relatorio = Vec::new();
    let mut proxima: i64 = semear;

    let mut k = 1usize;
    while k <= k_max {
        // aquece: a primeira janela de cada K paga cache frio de diretorio, e
        // isso e do medidor, nao do comboio.
        for _ in 0..2 {
            proxima += 1;
            sujar(&db, &nomes[..k], proxima);
            let _ = fechar(&inst, &nomes[..k]);
        }

        let mut abrir_us = 0.0;
        let mut sync_us = 0.0;
        for _ in 0..janelas {
            proxima += 1;
            sujar(&db, &nomes[..k], proxima);
            let (a, s) = fechar(&inst, &nomes[..k]);
            abrir_us += a;
            sync_us += s;
        }
        let a = abrir_us / janelas as f64;
        let s = sync_us / janelas as f64;
        let total = a + s;
        let pct = if total > 0.0 { s / total * 100.0 } else { 0.0 };
        println!("  {k:>3}  {a:>11.1}  {s:>11.1}  {total:>11.1}  {pct:>6.1}%");
        linhas_do_relatorio.push((k, a, s, total));
        k *= 2;
    }

    let _ = std::fs::remove_dir_all(&base);

    println!("\n=== o veredito ===\n");
    let (_, a1, s1, t1) = linhas_do_relatorio[0];
    let (kn, an, sn, tn) = *linhas_do_relatorio.last().unwrap();
    println!(
        "  De K=1 para K={kn} o fecho vai de {t1:.0} us para {tn:.0} us ({:.2}x).",
        tn / t1
    );
    println!("  Dessas duas parcelas, o `abrir` sai de {a1:.0} para {an:.0} us e o");
    println!("  `fsync` de {s1:.0} para {sn:.0} us.\n");
    let pct_final = if tn > 0.0 { sn / tn * 100.0 } else { 0.0 };
    if pct_final >= 80.0 {
        println!("  O fecho e `fsync`: {pct_final:.0}% em K={kn}. Guardar o descritor aberto");
        println!(
            "  compraria no maximo {:.0}% -- e o conserto de verdade e soltar a",
            100.0 - pct_final
        );
        println!("  trava entre uma tabela e a seguinte, que mexe na ordem do group");
        println!("  commit. DECISAO DE DURABILIDADE, papel C.");
    } else if pct_final <= 40.0 {
        println!("  O fecho e ABRIR: so {pct_final:.0}% e `fsync` em K={kn}. Guardar o descritor");
        println!(
            "  aberto compraria ate {:.0}% do comboio SEM encostar na ordem do",
            100.0 - pct_final
        );
        println!("  group commit -- risco zero, e o conserto certo para comecar.");
    } else {
        println!("  As duas parcelas pesam: `fsync` e {pct_final:.0}% em K={kn}. O cache de");
        println!("  descritor compra a outra parte sem risco, e sozinho nao fecha a conta.");
    }
}

/// Uma janela de escrita: uma linha em cada tabela, SEM sincronizar -- que e
/// exatamente o que `gravar_de_verdade` faz em `por_lote`. Sincronizar aqui
/// mediria outra coisa: o comboio existe porque o `fsync` foi ADIADO ate o
/// fecho, entao adiantar o `fsync` esvazia o que se quer medir.
fn sujar(db: &phxsql_store::catalogo::Database, nomes: &[String], id: i64) {
    for n in nomes {
        let mut t = db.abrir_qualificada(n).unwrap();
        t.inserir(&linha(id)).unwrap();
    }
}

/// O laco de `descarregar_sujas_com`, com o cronometro entre as duas metades.
/// A ordem e a mesma do servidor, de proposito: medir `sincronizar` numa tabela
/// que ja estava aberta mediria o conserto, nao o codigo de hoje.
fn fechar(inst: &Instancia, nomes: &[String]) -> (f64, f64) {
    let mut abrir = 0.0;
    let mut sync = 0.0;
    for n in nomes {
        let t0 = Instant::now();
        let mut t = inst
            .abrir_database("bancada")
            .and_then(|d| d.abrir_qualificada(n))
            .unwrap();
        abrir += t0.elapsed().as_secs_f64() * 1e6;

        let t1 = Instant::now();
        t.sincronizar().unwrap();
        sync += t1.elapsed().as_secs_f64() * 1e6;
    }
    (abrir, sync)
}

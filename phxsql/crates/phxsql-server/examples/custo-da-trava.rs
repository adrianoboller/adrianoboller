//! Quanto custa a guarda de reentrancia da trava de dados?
//!
//! ```bash
//! cargo run --release -p phxsql-server --example custo-da-trava
//! ```
//!
//! # Por que este medidor existe
//!
//! A guarda entrou no `travar_dados`, que e o caminho de **toda** leitura e
//! **toda** escrita deste servidor. Uma pergunta a mais ali nao e um detalhe:
//! e uma pergunta por operacao, no servidor inteiro. A regra do projeto diz
//! que instrumentacao desligada tem de custar zero, e «custar zero» so vale
//! como frase depois de alguem medir.
//!
//! # O que ele mede, e por que assim
//!
//! Dois cenarios sobre o MESMO `Mutex`, no mesmo processo, com as rodadas
//! INTERCALADAS -- 1,2, 1,2, ... -- e nao um cenario inteiro de cada vez.
//! Medindo em bloco, qualquer deriva da maquina (outro processo entrando, a
//! frequencia do processador mudando) fica toda dentro de um cenario e vira
//! «custo» dele:
//!
//! 1. **tomar e soltar a trava** -- o que o servidor fazia antes;
//! 2. **o mesmo, mais a guarda** -- uma leitura e duas escritas numa `Cell`
//!    de thread, que e exatamente o que a guarda acrescenta.
//!
//! E entao a comparacao que decide: quanto isso e da operacao mais barata do
//! servidor de verdade, medida no mesmo processo. O erro que este projeto ja
//! cometeu foi o contrario -- dizer que «o mutex era o pior pedaco, porque
//! serializa» sem numero, quando o `lock` sem disputa custava 13,2 ns e o
//! parse do lote custava 3.456 us. Diagnostico plausivel nao e diagnostico
//! medido.
//!
//! Argumentos: `<tomadas por rodada> <rodadas>` (padrao 2000000 e 7).

use std::cell::Cell;
use std::sync::Mutex;
use std::time::Instant;

use phxsql_core::json::Json;
use phxsql_server::mcp::Executor as _;
use phxsql_server::servidor::ExecutorLocal;
use phxsql_server::{Config, Servidor};

thread_local! {
    /// A copia fiel da `COM_A_TRAVA` do `servidor.rs`. Copia, e nao a de la:
    /// aquela e privada, e o que se mede aqui e a FORMA do custo -- uma
    /// `Cell<bool>` de thread lida e escrita por tomada.
    static COM_A_TRAVA: Cell<bool> = const { Cell::new(false) };
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Nanossegundos por tomada, so com o `lock`.
fn sem_guarda(trava: &Mutex<u64>, n: u64) -> f64 {
    let inicio = Instant::now();
    for _ in 0..n {
        let mut g = trava.lock().unwrap();
        // Um toque no dado para o otimizador nao apagar o bloco inteiro.
        *g = g.wrapping_add(1);
    }
    inicio.elapsed().as_nanos() as f64 / n as f64
}

/// Nanossegundos por tomada, com a pergunta e as duas marcas.
fn com_guarda(trava: &Mutex<u64>, n: u64) -> f64 {
    let inicio = Instant::now();
    for _ in 0..n {
        if COM_A_TRAVA.with(Cell::get) {
            unreachable!("o medidor nunca aninha");
        }
        let mut g = trava.lock().unwrap();
        COM_A_TRAVA.with(|c| c.set(true));
        *g = g.wrapping_add(1);
        COM_A_TRAVA.with(|c| c.set(false));
    }
    inicio.elapsed().as_nanos() as f64 / n as f64
}

/// A operacao mais barata do servidor de verdade, em microssegundos.
///
/// E o denominador da conta. Sem ele o medidor devolveria nanossegundos que
/// nao querem dizer nada: o que decide se a guarda «aparece» nao e o tamanho
/// dela, e sim o tamanho dela DIVIDIDO pelo que uma operacao custa.
fn custo_de_uma_operacao(n: u64) -> (f64, f64) {
    let dir = std::env::temp_dir().join(format!(
        "phx-trava-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let c = Config {
        base: dir.clone(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    let s = ExecutorLocal::novo(Servidor::novo(c).unwrap(), "medidor");
    s.executar(&pedido(
        r#"{"token":"t","op":"criar_database","database":"m"}"#,
    ))
    .unwrap();
    s.executar(&pedido(
        r#"{"token":"t","op":"criar_tabela","database":"m","tabela":"alvo",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                       {"nome":"nome","tipo":"Str(20)"}]}"#,
    ))
    .unwrap();
    let mut id = 0i64;
    for _ in 0..500 {
        id += 1;
        s.executar(&pedido(&format!(
            r#"{{"token":"t","op":"inserir","database":"m","tabela":"alvo","linha":{{"id":{id},"nome":"aq"}}}}"#
        )))
        .unwrap();
    }
    // A escrita: o caminho caro, com `.reg`, `.ndx` e `.log`.
    let inicio = Instant::now();
    for _ in 0..n {
        id += 1;
        s.executar(&pedido(&format!(
            r#"{{"token":"t","op":"inserir","database":"m","tabela":"alvo","linha":{{"id":{id},"nome":"linha"}}}}"#
        )))
        .unwrap();
    }
    let escrita = inicio.elapsed().as_micros() as f64 / n as f64;
    // A leitura de UMA linha: a operacao mais barata que passa pela trava, e
    // portanto o pior caso para a guarda -- e o denominador que interessa.
    let inicio = Instant::now();
    for i in 0..n {
        s.executar(&pedido(&format!(
            r#"{{"token":"t","op":"ler","database":"m","tabela":"alvo","rowid":{}}}"#,
            i % 400 + 1
        )))
        .unwrap();
    }
    let leitura = inicio.elapsed().as_micros() as f64 / n as f64;
    let _ = std::fs::remove_dir_all(&dir);
    (escrita, leitura)
}

fn mediana_e_faixa(mut m: Vec<f64>) -> (f64, f64, f64) {
    m.sort_by(|a, b| a.total_cmp(b));
    (m[m.len() / 2], m[0], m[m.len() - 1])
}

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(2_000_000);
    let rodadas: usize = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(7);

    let trava = Mutex::new(0u64);
    let mut sem = Vec::new();
    let mut com = Vec::new();
    // Aquece os dois, para a primeira rodada nao pagar a pagina fria da TLS.
    sem_guarda(&trava, 100_000);
    com_guarda(&trava, 100_000);
    for _ in 0..rodadas {
        sem.push(sem_guarda(&trava, n));
        com.push(com_guarda(&trava, n));
    }

    let (m_sem, lo_sem, hi_sem) = mediana_e_faixa(sem);
    let (m_com, lo_com, hi_com) = mediana_e_faixa(com);
    println!("{n} tomadas por rodada, {rodadas} rodadas intercaladas\n");
    println!(
        "1. lock + unlock, sem guarda            mediana {m_sem:6.2} ns   faixa {lo_sem:.2}..{hi_sem:.2}"
    );
    println!(
        "2. lock + unlock + guarda de thread     mediana {m_com:6.2} ns   faixa {lo_com:.2}..{hi_com:.2}"
    );
    let guarda = m_com - m_sem;
    let ruido = (hi_sem - lo_sem).max(hi_com - lo_com);
    println!(
        "\na guarda = {guarda:+.2} ns por tomada; maior espalhamento dentro de um \
         cenario = {ruido:.2} ns"
    );

    println!("\nagora o denominador, num servidor limpo:");
    let (escrita, leitura) = custo_de_uma_operacao(20_000);
    println!("   inserir  {escrita:8.2} us/operacao");
    println!("   ler      {leitura:8.2} us/operacao   <- a mais barata, o pior caso para a guarda");
    let parte = guarda / 1000.0 / leitura * 100.0;
    println!("\n=> a guarda e {parte:.4}% da operacao mais barata do servidor.");
    println!(
        "{}",
        if guarda.abs() < ruido {
            "=> e ela nao aparece nem acima do ruido do proprio medidor."
        } else {
            "=> ela aparece acima do ruido do medidor; a conta acima e que decide se importa."
        }
    );
}

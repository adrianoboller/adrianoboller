//! Quanto custa a linha de base do aquario (pedido 707, fatia A4) por pedido?
//!
//! ```bash
//! cargo run --release -p phxsql-server --example custo-da-base-do-aquario
//! ```
//!
//! Mede o caminho exato do `anotar` do servidor -- `aquario_se_ligada()` e,
//! quando ha aquario, `Aquario::anotar` -- com a telemetria DESLIGADA e
//! LIGADA, em rodadas intercaladas (desligada, ligada, desligada, ...), e
//! mostra mediana e faixa de cada lado. A promessa conferida e a do
//! CLAUDE.md: desligada custa zero, porque o portao vem antes do hash, da
//! trava e do `ln`. A ligada se compara ao «≤ 1% de um pedido» da §11.2.
//!
//! Argumentos: `<chamadas por rodada> <rodadas>` (padrao 2000000 e 7).

use std::hint::black_box;
use std::time::Instant;

use phxsql_server::acesso::Acesso;
use phxsql_server::telemetria::Telemetria;

fn rodada(t: &Telemetria, acesso: &Acesso, n: u64) -> f64 {
    let t0 = Instant::now();
    let mut alarmes = 0u64;
    for _ in 0..n {
        if let Some(aq) = black_box(t).aquario_se_ligada() {
            alarmes += aq.anotar(black_box(acesso)).desvio().is_some() as u64;
        }
    }
    black_box(alarmes);
    t0.elapsed().as_nanos() as f64 / n as f64
}

fn resumo(nome: &str, v: &mut [f64]) {
    v.sort_by(|a, b| a.total_cmp(b));
    println!(
        "{nome:>10}: mediana {:7.2} ns/pedido  (faixa {:.2}–{:.2}, {} rodadas)",
        v[v.len() / 2],
        v[0],
        v[v.len() - 1],
        v.len()
    );
}

fn main() {
    let args: Vec<u64> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let n = args.first().copied().unwrap_or(2_000_000);
    let rodadas = args.get(1).copied().unwrap_or(7);

    let desligada = Telemetria::nova(false);
    let ligada = Telemetria::nova(true);
    let acesso = Acesso {
        quando_ms: 1_760_000_000_000,
        op: "inserir".into(),
        ok: true,
        database: "loja".into(),
        tabela: "pedidos".into(),
        duracao_ms: 0,
        us: 320,
        ..Acesso::default()
    };
    // Aquece os dois lados (a chave nasce na primeira chamada ligada).
    rodada(&desligada, &acesso, n / 10);
    rodada(&ligada, &acesso, n / 10);

    let (mut d, mut l) = (Vec::new(), Vec::new());
    for _ in 0..rodadas {
        d.push(rodada(&desligada, &acesso, n));
        l.push(rodada(&ligada, &acesso, n));
    }
    resumo("desligada", &mut d);
    resumo("ligada", &mut l);
    println!(
        "amostras na base: desligada {}, ligada {}",
        desligada.aquario().base().atualizacoes(),
        ligada.aquario().base().atualizacoes()
    );
}

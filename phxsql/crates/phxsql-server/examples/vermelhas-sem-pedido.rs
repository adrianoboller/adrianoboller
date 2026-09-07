//! O relatorio das PROVAS VERMELHAS: guardas desligadas e onde elas aparecem.
//!
//! ```bash
//! cargo run --example vermelhas-sem-pedido -p phxsql-server
//! ```
//!
//! Achado na revisao completa de 07/09/2026: duas guardas vermelhas na arvore,
//! nenhuma no `docs/PENDENCIAS.md`. Prova vermelha desligada e a forma mais
//! educada de esquecer um defeito -- a bateria fica verde e o `cargo test` diz
//! "0 falharam".

use phxsql_server::conferidor_vermelhas as cv;

fn main() {
    let todas = cv::varrer();
    let soltas: Vec<_> = todas.iter().filter(|v| !v.tem_pedido).collect();

    println!("PROVAS VERMELHAS — guardas entregues falhando de proposito\n");
    println!("  vermelhas na arvore .... {}", todas.len());
    println!("  com pedido no PENDENCIAS {}", todas.len() - soltas.len());
    println!("  SEM pedido (contam) .... {}", soltas.len());
    println!(
        "  catraca (so desce) ..... {}",
        cv::TETO_VERMELHA_SEM_PEDIDO
    );

    // SAIDA DE MAQUINA -- gerador que le prosa publica o numero de ontem
    // quando a prosa muda; e a mesma razao dos irmaos deste exemplo.
    if std::env::args().any(|a| a == "--numeros") {
        println!(
            "catraca:nome=TETO_VERMELHA_SEM_PEDIDO;\
             onde=crates/phxsql-server/src/conferidor_vermelhas.rs;\
             valor={};medido={};mede=provas vermelhas sem pedido no PENDENCIAS.md",
            cv::TETO_VERMELHA_SEM_PEDIDO,
            soltas.len()
        );
    }

    if todas.is_empty() {
        println!("\nNenhuma prova vermelha na arvore.");
        return;
    }

    println!("\nAS VERMELHAS, uma a uma\n");
    for v in &todas {
        let marca = if v.tem_pedido {
            "no PENDENCIAS"
        } else {
            "SEM PEDIDO"
        };
        println!("  [{marca}]  {}:{}", v.arquivo, v.linha);
        println!("      {}", v.funcao);
        println!("      {}", v.motivo);
    }
}

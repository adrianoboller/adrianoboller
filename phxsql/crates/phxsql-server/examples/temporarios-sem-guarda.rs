//! O relatorio do conferidor de temporarios: quem cria diretorio em `/tmp`
//! sem um guarda que o apague.
//!
//! ```bash
//! cargo run --example temporarios-sem-guarda -p phxsql-server
//! cargo run --example temporarios-sem-guarda -p phxsql-server -- --isentos
//! ```
//!
//! Pedido 150. Uma corrida da bateria deixava 265 diretorios para tras; o
//! numero que este relatorio imprime e o que diz se isso voltou.

use phxsql_server::conferidor_temporarios as ct;

fn main() {
    let r = ct::varrer();

    println!("TEMPORARIOS — quem chama std::env::temp_dir() sem guarda\n");
    println!("  soltas (contam) ........ {}", r.soltas.len());
    println!(
        "  isentas (catalogadas) .. {}",
        ct::ISENTOS.iter().map(|(_, q, _)| q).sum::<usize>()
    );
    println!("  catraca (so desce) ..... {}", ct::TETO_TEMP_DIR_SOLTO);

    // SAIDA DE MAQUINA -- gerador que le prosa publica o numero de ontem
    // quando a prosa muda; e a mesma razao dos irmaos deste exemplo.
    if std::env::args().any(|a| a == "--numeros") {
        println!(
            "catraca:nome=TETO_TEMP_DIR_SOLTO;\
             onde=crates/phxsql-server/src/conferidor_temporarios.rs;\
             valor={};medido={};mede=chamadas a std::env::temp_dir() fora do catalogo",
            ct::TETO_TEMP_DIR_SOLTO,
            r.soltas.len()
        );
    }

    if !r.isentos_mudaram.is_empty() {
        println!("\nCATALOGO DESATUALIZADO — o disco nao bate com ISENTOS\n");
        for (arq, esperado, achado) in &r.isentos_mudaram {
            println!("  {arq}: esperava {esperado}, achou {achado}");
        }
    }

    if std::env::args().any(|a| a == "--isentos") {
        println!("\nISENTOS — quem pode chamar, e por que\n");
        for (arq, quantas, porque) in ct::ISENTOS {
            println!("  {arq}  ({quantas})\n      {porque}");
        }
        return;
    }

    if r.soltas.is_empty() {
        println!("\nNenhuma solta. Todo diretorio de teste nasce com guarda.");
        return;
    }

    println!("\nSOLTAS — arquivo, linha e a linha inteira\n");
    let mut arquivo_atual = String::new();
    for s in &r.soltas {
        if s.arquivo != arquivo_atual {
            arquivo_atual = s.arquivo.clone();
            let quantas = r
                .soltas
                .iter()
                .filter(|o| o.arquivo == arquivo_atual)
                .count();
            println!("\n{arquivo_atual} — {quantas}");
        }
        println!("  {:>6}  {}", s.linha, s.texto);
    }
}

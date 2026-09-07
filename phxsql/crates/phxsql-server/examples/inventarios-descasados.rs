//! O relatorio do conferidor de inventario: as extensoes do codigo contra a
//! Figura 1, a Figura 8 e a tabela-mestra do `docs/FORMATO.md`.
//!
//! ```bash
//! cargo run --example inventarios-descasados -p phxsql-server
//! cargo run --example inventarios-descasados -p phxsql-server -- --numeros
//! ```
//!
//! Pedido 213. O `.fts` faltava nas tres copias porque faltava no proprio
//! codigo (`EXTENSOES_TODAS`, em `phxsql-store`); consertado o codigo, este
//! relatorio diz se as copias continuam de acordo com ele.

use phxsql_server::conferidor_inventario as ci;

fn main() {
    let r = ci::varrer();

    println!("INVENTARIO — as extensoes de uma tabela, em quatro lugares\n");
    println!(
        "  no codigo (Database::extensoes_de_uma_tabela) {}",
        r.canonico.len()
    );
    println!(
        "  na Figura 1 (organograma) ..................... {}",
        r.figura1.len()
    );
    println!(
        "  na Figura 8 (caminho de uma insercao) ......... {}",
        r.figura8.len()
    );
    println!(
        "  na tabela-mestra do FORMATO.md ................ {}",
        r.formato_md.len()
    );
    println!(
        "  descasamentos (contam para a catraca) ......... {}",
        r.descasamentos.len()
    );
    println!(
        "  catraca (so desce) ............................ {}",
        ci::TETO_INVENTARIO_DESCASADO
    );

    // SAIDA DE MAQUINA -- gerador que le prosa publica o numero de ontem
    // quando a prosa muda; e a mesma razao dos irmaos deste exemplo.
    if std::env::args().any(|a| a == "--numeros") {
        println!(
            "catraca:nome=TETO_INVENTARIO_DESCASADO;\
             onde=crates/phxsql-server/src/conferidor_inventario.rs;\
             valor={};medido={};mede=extensoes que faltam ou sobram entre o codigo e as tres copias",
            ci::TETO_INVENTARIO_DESCASADO,
            r.descasamentos.len()
        );
    }

    if std::env::args().any(|a| a == "--isentos") {
        println!("\nFORA_DA_INSERCAO — extensoes que a Figura 8 nao precisa desenhar\n");
        for (ext, motivo) in ci::FORA_DA_INSERCAO {
            println!("  .{ext}\n      {motivo}");
        }
        return;
    }

    println!("\nCANONICO (o codigo): {}", r.canonico.join(", "));

    if r.descasamentos.is_empty() {
        println!("\nNenhum descasamento. As tres copias batem com o codigo.");
        return;
    }

    println!("\nDESCASAMENTOS — extensao, onde e o tipo\n");
    for d in &r.descasamentos {
        let tipo = if d.falta {
            "FALTA"
        } else {
            "SOBRA (desconhecida)"
        };
        println!("  [{tipo}]  .{}  em {}", d.extensao, d.onde);
    }
}

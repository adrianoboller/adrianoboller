//! O relatorio do conferidor de segredos: material de chave dentro da arvore
//! que o `git add` alcanca.
//!
//! ```bash
//! cargo run --example segredos-soltos -p phxsql-server
//! cargo run --example segredos-soltos -p phxsql-server -- --isentos
//! ```
//!
//! Pedido 402. Uma `chave-do-fio.hex` de 65 bytes nasceu numa corrida de teste
//! e apareceu na lista de um commit. Este relatorio diz se voltou -- e diz
//! **caminho e tamanho**, nunca um byte do arquivo.

use phxsql_server::conferidor_segredos as cs;

fn main() {
    let r = cs::varrer();

    println!("SEGREDOS SOLTOS — material de chave dentro da arvore\n");
    println!("  raiz varrida ........... {}", cs::raiz().display());
    println!("  arquivos varridos ...... {}", r.arquivos);
    println!("  soltos (contam) ........ {}", r.soltos.len());
    println!("  isentos (catalogados) .. {}", cs::ISENTOS.len());
    println!("  catraca (so desce) ..... {}", cs::TETO_SEGREDO_SOLTO);

    // SAIDA DE MAQUINA -- gerador que le prosa publica o numero de ontem
    // quando a prosa muda; e a mesma razao do irmao `temporarios-sem-guarda`.
    if std::env::args().any(|a| a == "--numeros") {
        println!(
            "catraca:nome=TETO_SEGREDO_SOLTO;\
             onde=crates/phxsql-server/src/conferidor_segredos.rs;\
             valor={};medido={};varridos={};\
             mede=arquivos com cara de chave na arvore do repositorio",
            cs::TETO_SEGREDO_SOLTO,
            r.soltos.len(),
            r.arquivos
        );
    }

    if !r.isentos_sumiram.is_empty() {
        println!("\nCATALOGO DESATUALIZADO — isento que nao existe mais\n");
        for a in &r.isentos_sumiram {
            println!("  {a}");
        }
    }

    if std::env::args().any(|a| a == "--isentos") {
        println!("\nISENTOS — quem pode ficar, e por que\n");
        if cs::ISENTOS.is_empty() {
            println!("  (nenhum: a porta existe e ninguem precisou dela ainda)");
        }
        for (arq, porque) in cs::ISENTOS {
            println!("  {arq}\n      {porque}");
        }
        return;
    }

    if r.soltos.is_empty() {
        println!("\nNenhum solto. Nenhuma chave a um `git add` de ser publicada.");
        return;
    }

    println!("\nSOLTOS — caminho, tamanho e o motivo do crivo\n");
    for s in &r.soltos {
        println!("  {}  ({} bytes)", s.caminho, s.bytes);
        println!("      {}", s.motivo.dizer());
    }
    // Saida diferente de zero: quem chamar isto de um roteiro de pre-commit
    // precisa que o shell saiba que reprovou.
    std::process::exit(1);
}

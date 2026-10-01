//! O censo do par `(e_ledger, colunas marcadas)` de uma raiz de dados --
//! pedido 424, receita F2 do parecer C de 23/09/2026.
//!
//!   cargo run --example censo-do-ledger -p phxsql-store -- <raiz-de-dados>
//!
//! Roda em TODO no, e nao so no primario: a replica remonta o esquema do
//! source pelo caminho de leitura, que nao julga, e a cadeia com coluna
//! marcada nasce igual nela. Nada aqui escreve -- cada tabela abre pela ficha
//! compartilhada, e a que exigiria escrever para abrir sai como NAO LIDA, com
//! o motivo, em vez de sumir da conta.
//!
//! Sai 1 quando acha a combinacao proibida, para o censo servir de portao num
//! roteiro; sai 2 quando a raiz nao se le.

use phxsql_store::ledger::censo;

fn main() {
    let Some(base) = std::env::args().nth(1) else {
        eprintln!("uso: censo-do-ledger <raiz-de-dados>");
        std::process::exit(2);
    };
    let linhas = match censo(std::path::Path::new(&base)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("censo-do-ledger: {e}");
            std::process::exit(2);
        }
    };
    let mut ledgers = 0usize;
    let mut proibidas = 0usize;
    let mut nao_lidas = 0usize;
    for l in &linhas {
        let nome = if l.tabela.is_empty() {
            format!("{}/*", l.database)
        } else {
            format!("{}/{}", l.database, l.tabela)
        };
        if let Some(m) = &l.nao_lida {
            nao_lidas += 1;
            println!("NAO LIDA   {nome}: {m}");
            continue;
        }
        if !l.e_ledger {
            continue;
        }
        ledgers += 1;
        if l.proibida() {
            proibidas += 1;
            println!(
                "PROIBIDA   {nome}: {} bloco(s), marcada(s): {}",
                l.blocos,
                l.colunas_marcadas.join(", ")
            );
        } else {
            println!("ledger     {nome}: {} bloco(s), sem marca", l.blocos);
        }
    }
    println!(
        "tabelas {} · ledger {ledgers} · proibidas {proibidas} · nao lidas {nao_lidas}",
        linhas.iter().filter(|l| !l.tabela.is_empty()).count()
    );
    if proibidas > 0 {
        std::process::exit(1);
    }
}

//! Mostra o que a camada SQL faz com um comando, sem servidor nenhum.
//!
//! ```bash
//! cargo run -p phxsql-sql --example traduzir -- "SELECT * FROM Clientes WHERE id = 7"
//! cargo run -p phxsql-sql --example traduzir           # roda a bateria de exemplos
//! ```
//!
//! Serve para conferir a olho o que os testes conferem por dentro: o pedido
//! que sai, e o motivo escrito de cada recusa.

use phxsql_sql::{compilar, ColunaDoIndice, IndiceInfo};

/// Os indices da tabela do video de demonstracao -- os mesmos nomes.
fn indices() -> Vec<IndiceInfo> {
    let uma = |nome: &str, coluna: &str, desc: bool, unico: bool| IndiceInfo {
        nome: nome.into(),
        colunas: vec![ColunaDoIndice {
            nome: coluna.into(),
            desc,
        }],
        unico,
        primario: unico,
    };
    vec![
        uma("porId", "id", false, true),
        uma("porNome", "nome", false, false),
        uma("porCidade", "cidade", false, false),
    ]
}

fn mostrar(sql: &str) {
    println!("\x1b[1m{sql}\x1b[0m");
    match compilar(sql, &indices(), "Comercial") {
        Ok(p) => {
            println!("  op     {}", p.op);
            println!("  pedido {}", p.pedido.escrever());
            println!("  saida  {:?}", p.saida);
            for n in &p.notas {
                println!("  nota   {n}");
            }
        }
        Err(e) => println!("  RECUSA {e}"),
    }
    println!();
}

fn main() {
    let arg: Vec<String> = std::env::args().skip(1).collect();
    if !arg.is_empty() {
        mostrar(&arg.join(" "));
        return;
    }
    println!("Tabela de exemplo: Comercial.Clientes, indices porId, porNome, porCidade\n");
    for sql in [
        "SELECT * FROM Clientes",
        "SELECT id, nome AS cliente FROM Clientes ORDER BY nome LIMIT 20 OFFSET 40",
        "SELECT COUNT(*) FROM Clientes",
        "SELECT * FROM Clientes WHERE id = 7",
        "SELECT * FROM matriz.estoque",
        "SELECT * FROM Outro.filial.estoque WHERE id = 1",
        // E o que ele recusa, que e a metade que importa.
        "SELECT * FROM Clientes WHERE limite > 1000",
        "SELECT * FROM Clientes ORDER BY limite",
        "SELECT SUM(limite) FROM Clientes",
        "SELECT * FROM Clientes WHERE uf = 'SC' AND cidade = 'Blumenau'",
        "SELECT * FROM BULKINSERT",
        "BEGIN",
        "UPDATE Clientes SET nome = 'x'",
    ] {
        mostrar(sql);
    }
}

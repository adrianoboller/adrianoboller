//! Prova de campo do cliente MySQL(R): conecta num servidor de verdade.
//!
//! Nao e teste automatico de proposito -- exige um MySQL(R) rodando, e um
//! teste que depende de servico externo quebra a suite de quem nao o tem.
//!
//! ```text
//! cargo run --example prova-dblink -- 127.0.0.1 3306 phxlink senha loja
//! ```

use phxsql_core::json::Json;
use phxsql_server::dblink::{mysql, Definicao};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (host, porta, usuario, senha, base) = (
        a.first().cloned().unwrap_or("127.0.0.1".into()),
        a.get(1).and_then(|x| x.parse().ok()).unwrap_or(3306u16),
        a.get(2).cloned().unwrap_or("phxlink".into()),
        a.get(3).cloned().unwrap_or_default(),
        a.get(4).cloned().unwrap_or_default(),
    );
    let d = Definicao::de_json(
        &Json::analisar(&format!(
            r#"{{"nome":"prova","host":"{host}","porta":{porta},
                 "usuario":"{usuario}","senha":"{senha}","database":"{base}"}}"#
        ))
        .unwrap(),
    )
    .unwrap();

    let mut c = match d.conectar() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("FALHOU ao conectar: {e}");
            std::process::exit(1);
        }
    };
    println!("conectado | versao {} | conexao {}", c.versao, c.conexao_id);
    c.ping().expect("ping");
    println!("ping ok");

    for sql in [
        "SHOW TABLES",
        "SELECT id, nome, cidade, limite, nascimento, ativo, obs FROM clientes ORDER BY id",
        "SELECT count(*) AS quantos, sum(limite) AS soma FROM clientes",
    ] {
        println!("\n--- {sql}");
        match c.consultar(sql, 100) {
            Ok(r) => mostrar(&r),
            Err(e) => println!("ERRO: {e}"),
        }
    }
    c.encerrar();
}

fn mostrar(r: &mysql::Resultado) {
    println!(
        "colunas: {}",
        r.colunas
            .iter()
            .map(|c| format!(
                "{}:{}{}{}",
                c.nome,
                c.tipo,
                if c.primaria { " PK" } else { "" },
                if c.nulavel { "" } else { " NN" }
            ))
            .collect::<Vec<_>>()
            .join("  ")
    );
    for l in &r.linhas {
        println!(
            "  {}",
            l.iter()
                .map(|v| v.clone().unwrap_or_else(|| "<NULO>".into()))
                .collect::<Vec<_>>()
                .join(" | ")
        );
    }
    println!(
        "{} linha(s){}",
        r.linhas.len(),
        if r.truncado { " (cortado)" } else { "" }
    );
}

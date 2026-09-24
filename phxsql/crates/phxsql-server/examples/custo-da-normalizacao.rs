//! Quanto custa, por `sql` observado com arquivo, normalizar o texto?
//!
//! ```bash
//! cargo run --release -p phxsql-server --example custo-da-normalizacao
//! ```
//!
//! # Por que este medidor existe
//!
//! O pedido 365 pos o lexico de SQL no ponto de captura: o `sql` vai ao
//! `perfil.txt` com todo literal trocado por `?`. Normalizar custa uma passada
//! do lexico sobre o texto, uma segunda passada de redacao sobre a arvore (o
//! anel continua com o texto inteiro) e a escrita dela -- e a frase «isso e
//! barato» sem numero e opiniao.
//!
//! # O que ele compara, e o que ele NAO mede
//!
//! O MESMO Profiler, gravando arquivo, com o MESMO texto:
//!
//! | corrida | o que muda |
//! |---|---|
//! | `sqx` | o `op` que nao e `sql`: analisa, redige e grava, e nao normaliza |
//! | `sql` | o de agora: tudo isso e mais a normalizacao |
//!
//! Tres letras nos dois `op`, para o JSON ter o mesmo tamanho. Dois tamanhos de
//! frase: uma linha (o `INSERT` de sempre) e um LOTE de `VALUES` com muitas
//! linhas, que e onde a passada do lexico pesa.
//!
//! Nao mede o Profiler DESLIGADO nem o ligado SO NO ANEL: nos dois a
//! normalizacao nao roda por desenho (o espelho atomico decide antes de
//! qualquer trabalho, e sem arquivo o `analisar_pedido` nem pede a segunda
//! passada). Ali o certo e ZERO, e zero se prova por teste
//! (`sem_arquivo_o_sql_nao_e_normalizado`), nao por um numero pequeno.
//!
//! Rodadas INTERCALADAS e MEDIANA, como o `custo-da-pergunta-ao-disco`.
//!
//! Argumentos: `<linhas do lote> <pedidos> <rodadas>` (padrao 1000, 200 e 5).

use std::path::Path;
use std::time::Instant;

use phxsql_server::profiler::{Filtro, Profiler};

/// O pedido `op` com um `INSERT` de `linhas` linhas.
fn pedido(op: &str, linhas: usize) -> String {
    let valores: Vec<String> = (0..linhas)
        .map(|i| {
            format!(
                "({i}, '{:03}.888.777-66', 'Fulano de Tal {i}', 1234.5)",
                i % 1000
            )
        })
        .collect();
    format!(
        r#"{{"op":"{op}","database":"loja","texto":"INSERT INTO clientes (id, cpf, nome, limite) VALUES {}"}}"#,
        valores.join(", ")
    )
}

/// Microssegundos por pedido observado, numa corrida.
fn uma_corrida(arquivo: &Path, op: &str, texto: &str, pedidos: u32) -> f64 {
    let mut p = Profiler::default();
    p.ligar(Filtro::default(), arquivo.to_str().unwrap(), 500, 0)
        .unwrap();
    let t = Instant::now();
    for _ in 0..pedidos {
        let Some(s) = p.chegou(texto, op, "adm", "loja", "", "ip", 0) else {
            panic!("o filtro recusou o pedido: o medidor mediria o nada");
        };
        p.terminou(s, 1, true, "");
    }
    let us = t.elapsed().as_secs_f64() * 1e6 / pedidos as f64;
    let _ = std::fs::remove_file(arquivo);
    us
}

fn mediana(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn main() {
    let mut arg = std::env::args().skip(1);
    let lote: usize = arg.next().and_then(|a| a.parse().ok()).unwrap_or(1_000);
    let pedidos: u32 = arg.next().and_then(|a| a.parse().ok()).unwrap_or(200);
    let rodadas: usize = arg.next().and_then(|a| a.parse().ok()).unwrap_or(5);

    let base = std::env::temp_dir().join(format!("phx-custo-normalizacao-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();

    println!("pedidos por corrida: {pedidos} | rodadas intercaladas: {rodadas}");
    for linhas in [1, lote] {
        let (sem_txt, com_txt) = (pedido("sqx", linhas), pedido("sql", linhas));
        // A premissa, conferida e nao suposta: o `sqx` nao normaliza e o
        // `sql` normaliza -- sem isto o medidor compararia duas coisas iguais.
        let mut prova = Profiler::default();
        let alvo = base.join("prova.txt");
        prova
            .ligar(Filtro::default(), alvo.to_str().unwrap(), 10, 0)
            .unwrap();
        prova.chegou(&sem_txt, "sqx", "adm", "loja", "", "ip", 0);
        assert!(prova.eventos(1)[0].pedido_do_arquivo.is_none());
        prova.chegou(&com_txt, "sql", "adm", "loja", "", "ip", 0);
        assert!(prova.eventos(1)[0].pedido_do_arquivo.is_some());

        let mut sem = Vec::new();
        let mut com = Vec::new();
        for r in 0..rodadas {
            sem.push(uma_corrida(
                &base.join(format!("sem-{linhas}-{r}.txt")),
                "sqx",
                &sem_txt,
                pedidos,
            ));
            com.push(uma_corrida(
                &base.join(format!("com-{linhas}-{r}.txt")),
                "sql",
                &com_txt,
                pedidos,
            ));
        }
        let (a, b) = (mediana(sem), mediana(com));
        println!(
            "{linhas:>5} linha(s), {:>7} B: sem normalizar {a:>9.2} us | normalizando {b:>9.2} us | {:+.2} us ({:+.0}%)",
            com_txt.len(),
            b - a,
            (b / a - 1.0) * 100.0
        );
    }
    println!(
        "lembrete: isto e o custo de quem pediu ARQUIVO e observa `sql`. Desligado e \
         so no anel nao normalizam -- e isso e teste, nao medida."
    );
    let _ = std::fs::remove_dir_all(&base);
}

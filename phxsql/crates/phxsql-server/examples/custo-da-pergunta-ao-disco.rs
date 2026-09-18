//! Quanto custa, por pedido observado, a pergunta que o Profiler faz ao DISCO?
//!
//! ```bash
//! cargo run --release -p phxsql-server --example custo-da-pergunta-ao-disco
//! ```
//!
//! # Por que este medidor existe
//!
//! O conserto do pedido 356 pos uma leitura de arquivo no ponto de captura: o
//! `perfil.txt` passou a esconder o texto do pedido de toda tabela cujo `.reg`
//! esta CIFRADO, e quem responde isso e o cabecalho do proprio `.reg` -- dez
//! bytes, a cada pedido observado.
//!
//! O comentario que acompanhava o conserto dizia que isso era «barato». **Numero
//! citado e numero que nao se mede**, e o medidor entra junto com a frase: sem
//! ele, a proxima pessoa a mexer aqui herda uma opiniao no lugar de um numero.
//!
//! # O que ele compara, e o que ele NAO mede
//!
//! Duas configuracoes do MESMO Profiler, gravando arquivo, com o mesmo pedido:
//!
//! | corrida | o que muda |
//! |---|---|
//! | `sem raiz` | o comportamento de antes do 356: so a lista em memoria |
//! | `com raiz` | o de agora: a lista, e depois o disco |
//!
//! O que ele **nao** mede: o Profiler DESLIGADO (custa zero por desenho -- o
//! espelho atomico decide antes de qualquer trabalho) e o Profiler ligado **sem
//! arquivo**, que e o caso da tela: sem arquivo pedido nao ha linha de arquivo,
//! e o portao do `sigilo_dos_alvos` nao pergunta ao disco. Os dois casos estao
//! travados por teste, e nao por medicao, porque ali o certo e ZERO e nao um
//! numero pequeno.
//!
//! As rodadas sao INTERCALADAS e sai a MEDIANA: medir uma configuracao inteira
//! e depois a outra faria a primeira pagar o cache frio da segunda.
//!
//! Argumentos: `<pedidos> <rodadas>` (padrao 5000 e 5).

use std::path::{Path, PathBuf};
use std::time::Instant;

use phxsql_server::profiler::{Filtro, Profiler};

/// O `.reg` mais curto que responde a pergunta: magic e versao.
///
/// Fabricado, e nao criado por `Table::criar`, porque o que se mede aqui e o
/// CUSTO DA PERGUNTA -- achar o primeiro volume e ler o cabecalho. Uma tabela
/// de verdade daria o mesmo numero e faria o medidor depender do cofre, que e
/// global ao processo.
fn reg_falso(base: &Path, database: &str, tabela: &str, versao: u16) {
    let dir = base.join(database);
    std::fs::create_dir_all(&dir).unwrap();
    let mut bytes = phxsql_store::reg::MAGIC_REG.to_vec();
    bytes.extend_from_slice(&versao.to_le_bytes());
    bytes.extend_from_slice(&[0u8; 118]);
    std::fs::write(dir.join(format!("{tabela}.reg")), bytes).unwrap();
}

/// Microssegundos por pedido observado, numa corrida.
fn uma_corrida(base: &Path, arquivo: &Path, com_raiz: bool, pedidos: u32) -> f64 {
    let pedido = r#"{"op":"inserir","database":"loja","tabela":"clientes","linha":{"cpf":"111.222.333-44"}}"#;
    let mut p = Profiler::default();
    if com_raiz {
        p.definir_raiz_dos_dados(base);
    }
    p.ligar(Filtro::default(), arquivo.to_str().unwrap(), 500, 0)
        .unwrap();
    let t = Instant::now();
    for _ in 0..pedidos {
        let Some(s) = p.chegou(pedido, "inserir", "adm", "loja", "clientes", "ip", 0) else {
            panic!("o filtro recusou o pedido: o medidor mediria o nada");
        };
        p.terminou(s, 1, true, "");
    }
    t.elapsed().as_secs_f64() * 1e6 / pedidos as f64
}

fn mediana(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn main() {
    let mut arg = std::env::args().skip(1);
    let pedidos: u32 = arg.next().and_then(|a| a.parse().ok()).unwrap_or(5_000);
    let rodadas: usize = arg.next().and_then(|a| a.parse().ok()).unwrap_or(5);

    let base_tmp: PathBuf = std::env::temp_dir().join(format!(
        "phx-custo-pergunta-{}-{}",
        std::process::id(),
        pedidos
    ));
    let base = base_tmp.join("dados");
    std::fs::create_dir_all(&base).unwrap();
    // Em CLARO de proposito: e o caso comum e o caro. A tabela cifrada sai da
    // pergunta com a mesma leitura, e a em claro e a que todo pedido paga.
    reg_falso(&base, "loja", "clientes", 4);

    let mut sem = Vec::new();
    let mut com = Vec::new();
    for r in 0..rodadas {
        sem.push(uma_corrida(
            &base,
            &base_tmp.join(format!("perfil-sem-{r}.txt")),
            false,
            pedidos,
        ));
        com.push(uma_corrida(
            &base,
            &base_tmp.join(format!("perfil-com-{r}.txt")),
            true,
            pedidos,
        ));
    }
    let (a, b) = (mediana(sem), mediana(com));

    println!("pedidos por corrida: {pedidos} | rodadas intercaladas: {rodadas}");
    println!("sem raiz (so a lista, antes do 356): {a:.2} us por pedido observado");
    println!("com raiz (a lista e o disco, agora): {b:.2} us por pedido observado");
    println!(
        "a pergunta ao disco custa: {:+.2} us ({:+.0}%)",
        b - a,
        (b / a - 1.0) * 100.0
    );
    println!(
        "lembrete: isto e o custo de quem pediu ARQUIVO. Profiler desligado e \
         profiler so no anel nao perguntam ao disco -- e isso e teste, nao medida."
    );

    let _ = std::fs::remove_dir_all(&base_tmp);
}

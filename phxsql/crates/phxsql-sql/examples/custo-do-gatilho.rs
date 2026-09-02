//! Quanto tempo o corpo de um gatilho pode segurar a trava de dados?
//!
//!     cargo run --release --example custo-do-gatilho -p phxsql-sql
//!
//! # A pergunta, e por que ela nao e teorica
//!
//! O gatilho `BEFORE` roda **com a trava global de dados na mao** -- e uma das
//! cinco secoes criticas que executam codigo do dono do banco. Enquanto ele
//! roda, nenhuma outra conexao escreve nem le.
//!
//! O mapa da concorrencia registrou que a duracao dessas cinco **nao tem
//! teto**. Isso e FALSO, e o proprio codigo diz: `PASSOS_MAX` existe, e o
//! comentario dele nomeia esta razao exata. O que ninguem tinha medido e o que
//! esse teto vale em MILISSEGUNDOS -- porque teto em passos so limita a trava
//! se um milhao de passos for rapido. Teto que ninguem converteu para tempo e
//! numero citado, e numero citado e numero que nao se mede.
//!
//! # O que ele mede
//!
//! O pior caso honesto: um `WHILE` sem fim, que gasta o orcamento inteiro e
//! morre no teto. E o teto de tempo que um gatilho consegue impor a todas as
//! outras conexoes do servidor.
//!
//! Mede tres vezes e mostra a mediana: uma medicao so de coisa curta pega o
//! ruido da maquina como se fosse o numero.

use std::time::Instant;

use phxsql_sql::rotina::{
    analisar_corpo, executar, regras_de_procedimento, Contexto, MotorNulo, Numero, Tipo, Valor,
    PASSOS_MAX,
};

fn uma_volta(texto: &str) -> (u128, String) {
    let corpo = analisar_corpo(texto, &regras_de_procedimento()).expect("o corpo nao compila");
    let comeco = Instant::now();
    let r = executar(
        &corpo,
        &mut Contexto::de_procedimento(vec![(
            "x".into(),
            Tipo::Inteiro,
            Valor::Numero(Numero::inteiro(0)),
        )]),
        &mut MotorNulo,
    );
    let ms = comeco.elapsed().as_micros();
    let fim = match r {
        Ok(()) => "terminou sozinho".to_string(),
        Err(e) => {
            let t = e.to_string();
            if t.contains("passos") {
                "estourou o teto de passos".to_string()
            } else {
                t
            }
        }
    };
    (ms, fim)
}

fn mediana(mut v: Vec<u128>) -> u128 {
    v.sort_unstable();
    v[v.len() / 2]
}

fn main() {
    println!("Custo do corpo de gatilho, com a trava de dados na mao\n");
    println!("  teto de passos (PASSOS_MAX) ... {PASSOS_MAX}");

    // O pior caso: laco sem fim, gastando o orcamento inteiro.
    let mut tempos = Vec::new();
    let mut como = String::new();
    for _ in 0..3 {
        let (us, fim) = uma_volta("WHILE TRUE DO SET x = x + 1; END WHILE");
        tempos.push(us);
        como = fim;
    }
    let pior = mediana(tempos.clone());
    println!("\n  PIOR CASO -- `WHILE TRUE DO SET x = x + 1`");
    println!("    {como}");
    println!(
        "    mediana de 3: {} us  ({:.1} ms)",
        pior,
        pior as f64 / 1000.0
    );
    println!("    (as tres: {tempos:?} us)");

    // E o caso honesto, para ter com o que comparar: uma condicao e uma soma.
    let mut curtos = Vec::new();
    for _ in 0..3 {
        curtos.push(uma_volta("IF x < 10 THEN SET x = x + 1; END IF").0);
    }
    let curto = mediana(curtos.clone()).max(1);
    println!("\n  CASO HONESTO -- `IF x < 10 THEN SET x = x + 1`");
    println!("    mediana de 3: {curto} us");

    println!("\n  o pior caso custa {}x o honesto", pior / curto);
    println!(
        "\n  LEITURA: enquanto isso corre, NENHUMA outra conexao escreve nem le.\n  \
         O teto existe -- o que este medidor responde e quanto ele vale em tempo."
    );
}

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
//!
//! # E o que ele PASSOU a medir, quando a resposta acima se mostrou errada
//!
//! Os 18,3 ms deste medidor viraram, no `CONCORRENCIA.md`, a frase «existe
//! teto, e ele vale dezoito milissegundos». **A frase estava errada, e o erro
//! era do medidor: ele so media corpos cujo PASSO e barato.**
//!
//! Teto de passos nao e teto de trabalho. `SET s = CONCAT(s, s)` dobra o texto
//! a cada volta: trinta passos de um orcamento de um milhao chegam a um
//! gigabyte, e o corpo nao morre no teto — morre no alocador, que em Rust
//! ABORTA o processo. Medido com o processo limitado a 2 GiB:
//!
//! ```text
//! $ (ulimit -v 2000000; ./custo-do-gatilho)
//! memory allocation of 536870912 bytes failed
//! ```
//!
//! 10,2 s com a trava GLOBAL de dados na mao, e entao o servidor inteiro cai.
//! Por isso ha agora DOIS tetos, e este medidor mede os dois: `TEXTO_MAX`
//! (o que um passo pode alocar) e o prazo de parede do `Contexto::com_prazo`
//! (o que o corpo inteiro pode segurar a trava).

use std::time::{Duration, Instant};

use phxsql_sql::rotina::{
    analisar_corpo, executar, regras_de_procedimento, Contexto, MotorNulo, Numero, Tipo, Valor,
    PASSOS_MAX, TEXTO_MAX,
};

/// O prazo que o servidor poe no corpo de um `BEFORE`, copiado aqui de
/// proposito para o medidor nao depender do `phxsql-server`. O numero de
/// verdade e o `PRAZO_DO_GATILHO_ANTES` do `servidor.rs`.
const PRAZO_DO_SERVIDOR_MS: u64 = 500;

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

/// Uma volta com um corpo que mexe em TEXTO, que e a forma que o teto de
/// passos nao ve. Devolve (microssegundos, passos dados, bytes do texto, fim).
fn volta_de_texto(texto: &str, semente: String, prazo: Option<u64>) -> (u128, u64, usize, String) {
    let corpo = analisar_corpo(texto, &regras_de_procedimento()).expect("o corpo nao compila");
    let mut ctx = Contexto::de_procedimento(vec![("s".into(), Tipo::Texto, Valor::Texto(semente))]);
    if let Some(ms) = prazo {
        ctx = ctx.com_prazo(Duration::from_millis(ms));
    }
    let comeco = Instant::now();
    let r = executar(&corpo, &mut ctx, &mut MotorNulo);
    let us = comeco.elapsed().as_micros();
    let tam = match ctx.valor_de("s") {
        Some(Valor::Texto(t)) => t.len(),
        _ => 0,
    };
    let fim = match r {
        Ok(()) => "terminou sozinho".to_string(),
        Err(e) => {
            let t = e.to_string();
            if t.contains("prazo") {
                "PAROU NO PRAZO DE PAREDE".to_string()
            } else if t.contains("CONCAT") {
                "PAROU NO TETO DE TEXTO".to_string()
            } else if t.contains("passos") {
                "estourou o teto de passos".to_string()
            } else {
                t
            }
        }
    };
    (us, ctx.passos_dados(), tam, fim)
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
    // ---------------------------------------------------------- o teto de texto
    println!("\n  teto de texto (TEXTO_MAX) ..... {TEXTO_MAX} bytes");
    let (us, passos, tam, fim) = volta_de_texto(
        "WHILE TRUE DO SET s = CONCAT(s, s); END WHILE",
        "a".into(),
        None,
    );
    println!("\n  O PASSO SEM FUNDO -- `WHILE TRUE DO SET s = CONCAT(s, s)`");
    println!("    {fim}");
    println!(
        "    {} us ({:.1} ms), em {passos} passos, com o texto em {} MiB",
        us,
        us as f64 / 1000.0,
        tam / (1024 * 1024)
    );
    println!(
        "    o orcamento era de {PASSOS_MAX} passos: o corpo usou {:.4}% dele.\n    \
         SEM o teto de texto isto NAO para -- dobra ate o alocador falhar, e\n    \
         alocacao que falha em Rust ABORTA o processo (medido: 10,2 s com\n    \
         `ulimit -v 2000000`, e entao o servidor inteiro cai)",
        passos as f64 * 100.0 / PASSOS_MAX as f64
    );

    // ---------------------------------------------------- o prazo de parede
    let semente = "y".repeat(512 * 1024);
    let (us_sem, passos_sem, _, fim_sem) = volta_de_texto(
        "WHILE TRUE DO SET s = CONCAT('x', s); END WHILE",
        semente.clone(),
        None,
    );
    let (us_com, passos_com, _, fim_com) = volta_de_texto(
        "WHILE TRUE DO SET s = CONCAT('x', s); END WHILE",
        semente,
        Some(PRAZO_DO_SERVIDOR_MS),
    );
    println!("\n  O PASSO CARO -- `WHILE TRUE DO SET s = CONCAT('x', s)`, texto de 512 KiB");
    println!("    o teto de texto nao morde: o texto cresce um byte por volta.");
    println!(
        "    SEM prazo: {fim_sem} -- {:.1} ms em {passos_sem} passos",
        us_sem as f64 / 1000.0
    );
    println!(
        "    COM o prazo do servidor ({PRAZO_DO_SERVIDOR_MS} ms): {fim_com} -- {:.1} ms em {passos_com} passos",
        us_com as f64 / 1000.0
    );
    if us_com > 0 {
        println!("    a trava fica presa {}x menos tempo", us_sem / us_com);
    }

    println!(
        "\n  LEITURA: enquanto isso corre, NENHUMA outra conexao escreve nem le.\n  \
         Os DOIS tetos sao precisos, e cada um pega o que o outro nao ve:\n  \
         PASSOS_MAX limita o corpo barato, TEXTO_MAX limita UM passo, e o\n  \
         prazo de parede e o unico que limita a TRAVA."
    );
}

//! Quanto custa `Cadastro::por_login`, medido -- pedido 529.
//!
//! ```bash
//! cargo build --release --examples -p phxsql-server
//! cargo run --release -p phxsql-server --example custo-do-por-login [N] [repeticoes]
//! ```
//!
//! Mede TRES logins contra o MESMO cadastro de N usuarios, intercalados
//! (primeiro, ultimo, inexistente, primeiro, ultimo, inexistente, ...) para o
//! relogio do sistema nao virar variavel de confusao -- a mesma disciplina do
//! `docs/SEGURANCA.md` §26.2. `N` default 20.000 (o numero do achado do SEC);
//! passe um numero menor para medir um cadastro REALISTA antes de aceitar o
//! numero do pior caso como premissa do conserto.
//!
//! A pergunta que decide se vale a pena consertar: o `find` que para no
//! primeiro que casa devolve custo ~O(1) para o primeiro da lista e ~O(N)
//! para quem nao existe (ou para o ultimo). Com N pequeno essa diferenca pode
//! morrer dentro do ruido de medir; com N grande ela vira um relogio que
//! separa "existe" de "nao existe" sem nenhuma senha.

use std::time::Instant;

use phxsql_core::json::Json;
use phxsql_server::usuarios::Cadastro;

/// Um cadastro com `n` usuarios, todos com o MESMO hash valido -- o conteudo
/// do hash nao importa para o `por_login`, e gerar um de verdade por usuario
/// pagaria o PBKDF2 de `n` contas so para montar o cenario.
///
/// Login com LARGURA FIXA (`u00000`, `u00001`, ...), de proposito: `str ==`
/// rejeita por TAMANHO antes de comparar byte a byte, e login de tamanho
/// variavel (`u0`, `u1`, ..., `u19999`) mistura esse efeito -- menor e mais
/// legitimo -- com o que este medidor quer isolar, que e o custo de
/// PERCORRER o cadastro. Medido e descartado: com largura variavel o
/// "ultimo" (`u19999`, 6 bytes) custava 2x o "primeiro" (`u0`, 2 bytes) so
/// porque so ele compara byte a byte contra os outros 6 bytes; com largura
/// fixa esse 2x sai da conta e sobra so o que o pedido 529 mede.
fn cadastro_com(n: usize) -> Cadastro {
    let largura = n.max(1).saturating_sub(1).to_string().len();
    let mut usuarios = String::with_capacity(n * 56);
    for i in 0..n {
        if i > 0 {
            usuarios.push(',');
        }
        usuarios.push_str(&format!(
            r#"{{"login":"u{i:0largura$}","id":{},"senha_hash":"pbkdf2-sha256$1000$00$00"}}"#,
            i as u32 + 1
        ));
    }
    let j = Json::analisar(&format!(r#"{{"usuarios":[{usuarios}]}}"#)).unwrap();
    Cadastro::de_json(&j).unwrap()
}

fn mediana(mut v: Vec<i128>) -> i128 {
    v.sort_unstable();
    v[v.len() / 2]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let repeticoes: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2_000);

    let c = cadastro_com(n);
    let largura = n.max(1).saturating_sub(1).to_string().len();
    let primeiro = format!("u{:0largura$}", 0);
    let ultimo = format!("u{:0largura$}", n - 1);
    // Mesmo TAMANHO dos logins reais, de proposito (ver o comentario de
    // `cadastro_com`): comeca com "z", que nenhum login gerado usa.
    let inexistente = format!("z{:0largura$}", 0);

    let mut t_primeiro = Vec::with_capacity(repeticoes);
    let mut t_ultimo = Vec::with_capacity(repeticoes);
    let mut t_falta = Vec::with_capacity(repeticoes);
    for _ in 0..repeticoes {
        // `black_box` no CADASTRO e no LOGIN, nao so no resultado: uma
        // chamada pura sobre entradas que nao mudam de uma repeticao para a
        // outra e candidata a LICM (loop-invariant code motion) -- o
        // compilador prova que `por_login` nao tem efeito colateral e
        // levanta a chamada para FORA do laco, e ai o "primeiro" media quase
        // zero porque so mediu 2000 `Instant::now()` sem nenhum `por_login`
        // de verdade no meio. Isto foi medido, nao suposto: sem o
        // `black_box` na entrada, o release+LTO reduzia o "primeiro" a 50 ns
        // contra dezenas de milhares de ns dos outros dois -- proporcao
        // impossivel para uma varredura que la todo o vetor.
        let c = std::hint::black_box(&c);
        let ini = Instant::now();
        let r = c.por_login(std::hint::black_box(primeiro.as_str()));
        t_primeiro.push(ini.elapsed().as_nanos() as i128);
        assert!(std::hint::black_box(r).is_some());

        let ini = Instant::now();
        let r = c.por_login(std::hint::black_box(ultimo.as_str()));
        t_ultimo.push(ini.elapsed().as_nanos() as i128);
        assert!(std::hint::black_box(r).is_some());

        let ini = Instant::now();
        let r = c.por_login(std::hint::black_box(inexistente.as_str()));
        t_falta.push(ini.elapsed().as_nanos() as i128);
        assert!(std::hint::black_box(r).is_none());
    }

    let m_primeiro = mediana(t_primeiro);
    let m_ultimo = mediana(t_ultimo);
    let m_falta = mediana(t_falta);
    println!("cadastro com {n} usuarios, {repeticoes} repeticoes intercaladas (mediana, ns)");
    println!("  primeiro da lista : {m_primeiro} ns");
    println!("  ultimo da lista   : {m_ultimo} ns");
    println!("  nao existe        : {m_falta} ns");
    println!("  diferenca falta-primeiro : {} ns", m_falta - m_primeiro);
    println!("  diferenca ultimo-primeiro: {} ns", m_ultimo - m_primeiro);
}

//! O relatorio do conferidor da TELA: simbolo Unicode no lugar de icone, cor
//! fora de token, icone sem desenho e o vermelho da marca pintando texto.
//!
//! ```bash
//! cargo run --example simbolos-e-cores -p phxsql-server
//! cargo run --example simbolos-e-cores -p phxsql-server -- --numeros
//! ```

use std::collections::BTreeMap;

use phxsql_server::conferidor_tela as ct;

fn main() {
    let s = ct::simbolos();
    let c = ct::cores_soltas();
    let i = ct::icones_sem_desenho();
    let v = ct::vermelho_da_marca_em_texto();

    println!("A TELA — o que a fonte de quem abre desenharia, e a cor fora da paleta\n");
    let linha = |nome: &str, n: usize, teto: usize| println!("  {nome:<34} {n:>4}   (teto {teto})");
    linha(
        "simbolos pictograficos",
        s.len(),
        ct::TETO_SIMBOLOS_PICTOGRAFICOS,
    );
    linha("cores fora de token", c.len(), ct::TETO_CORES_SOLTAS);
    linha(
        "icones pedidos sem desenho",
        i.len(),
        ct::TETO_ICONE_SEM_DESENHO,
    );
    linha(
        "vermelho da marca em texto",
        v.len(),
        ct::TETO_VERMELHO_DA_MARCA_EM_TEXTO,
    );
    println!(
        "  desenhos no sprite ................ {:>4}",
        ct::desenhos().len()
    );

    if std::env::args().any(|a| a == "--numeros") {
        for (nome, valor, medido, mede) in [
            (
                "TETO_SIMBOLOS_PICTOGRAFICOS",
                ct::TETO_SIMBOLOS_PICTOGRAFICOS,
                s.len(),
                "simbolos pictograficos na ui",
            ),
            (
                "TETO_CORES_SOLTAS",
                ct::TETO_CORES_SOLTAS,
                c.len(),
                "cores hex fora de token na ui",
            ),
            (
                "TETO_ICONE_SEM_DESENHO",
                ct::TETO_ICONE_SEM_DESENHO,
                i.len(),
                "icones pedidos sem symbol no sprite",
            ),
            (
                "TETO_VERMELHO_DA_MARCA_EM_TEXTO",
                ct::TETO_VERMELHO_DA_MARCA_EM_TEXTO,
                v.len(),
                "D71A1A ou 8B0D0D em color:",
            ),
        ] {
            println!(
                "catraca:nome={nome};onde=crates/phxsql-server/src/conferidor_tela.rs;\
                 valor={valor};medido={medido};mede={mede}"
            );
        }
    }

    let mut por: BTreeMap<&str, usize> = BTreeMap::new();
    for a in &s {
        *por.entry(a.trecho.as_str()).or_default() += 1;
    }
    println!("\nSimbolos que sobraram, por caractere:");
    for (k, n) in &por {
        println!("  {k}  {n}");
    }
    for (titulo, lista) in [
        ("Simbolos", &s),
        ("Cores soltas", &c),
        ("Icones sem desenho", &i),
        ("Vermelho em texto", &v),
    ] {
        if !lista.is_empty() {
            println!("\n{titulo}:");
            for a in lista {
                println!("  {}:{}  {}", a.arquivo, a.linha, a.trecho);
            }
        }
    }
}

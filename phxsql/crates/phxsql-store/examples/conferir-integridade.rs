//! O verificador de consistencia referencial. **Ele RELATA, nao conserta.**
//!
//! ```bash
//! cargo run --release --example conferir-integridade -p phxsql-store -- <diretorio>
//! ```
//!
//! Sai `0` quando a base esta limpa e `1` quando ha violacao -- para caber num
//! script de manutencao sem alguem ter de ler o texto.
//!
//! Consertar dado do dono sem ele pedir e pior que o defeito: uma orfa pode
//! ser lixo de importacao, e pode ser a unica copia de um pedido cujo cliente
//! alguem apagou por engano. As duas sao indistinguiveis daqui, e apagar a
//! orfa destroi a segunda.

use phxsql_store::integridade;

fn main() {
    let dir = match std::env::args().nth(1) {
        Some(d) => std::path::PathBuf::from(d),
        None => {
            eprintln!("uso: conferir-integridade <diretorio das tabelas>");
            std::process::exit(2);
        }
    };
    let r = match integridade::conferir_diretorio(&dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("nao deu para varrer {}: {e}", dir.display());
            std::process::exit(2);
        }
    };

    println!(
        "{}: {} tabela(s), {} chave(s) declarada(s), {} linha(s) conferida(s)",
        dir.display(),
        r.tabelas,
        r.chaves,
        r.linhas
    );
    // NADA CONFERIDO NAO E LIMPO. `tabelas_em` devolve lista vazia para
    // caminho que nao e diretorio -- correto para quem abre uma instancia que
    // ainda nao tem pasta, e mentira aqui: um script de manutencao apontado
    // um nivel acima (as tabelas moram em `<servidor>/base/<banco>/`) ou com
    // um erro de digitacao no caminho recebia `limpo` e saida 0 para sempre.
    // Sai 2, o mesmo codigo de «nao deu para varrer», porque e o que e.
    if r.tabelas == 0 {
        eprintln!(
            "nenhuma tabela em {} -- nada foi conferido. As tabelas de um \
             servidor ficam em <servidor>/base/<banco>/.",
            dir.display()
        );
        std::process::exit(2);
    }
    for (t, e) in &r.nao_abriram {
        println!("  ! {t} nao abriu: {e}");
    }
    // Estrutura primeiro: um indice que falta trava a chave inteira, e mostra-
    // -lo depois de mil orfas esconderia a causa embaixo do efeito.
    let (estrutura, linhas): (Vec<_>, Vec<_>) =
        r.violacoes.iter().partition(|v| v.falha.e_de_estrutura());
    if !estrutura.is_empty() {
        println!("\n-- estrutura ({}) --", estrutura.len());
        for v in &estrutura {
            println!("  {v}");
        }
    }
    if !linhas.is_empty() {
        println!("\n-- linhas ({}) --", linhas.len());
        for v in &linhas {
            println!("  {v}");
        }
    }
    if r.limpo() {
        println!("\nlimpo: nenhuma violacao");
    } else {
        println!(
            "\n{} violacao(oes). O verificador RELATA -- nada foi mexido.",
            r.violacoes.len()
        );
        std::process::exit(1);
    }
}

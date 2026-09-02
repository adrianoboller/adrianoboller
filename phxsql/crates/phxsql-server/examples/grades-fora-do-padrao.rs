//! O relatorio do conferidor de grades: quais tabelas da tela ainda sao
//! montadas na mao, e quais ja sao `PhxGrid`.
//!
//! ```bash
//! cargo run --example grades-fora-do-padrao -p phxsql-server
//! cargo run --example grades-fora-do-padrao -p phxsql-server -- --isentas
//! ```
//!
//! O pedido do dono e «todas as table sao phxgrid com agrupamento dinamico».
//! Este relatorio e o que diz quanto disso ja e verdade -- medido, e nao
//! estimado.

use phxsql_server::conferidor_grades as cg;

fn main() {
    let ver_isentas = std::env::args().any(|a| a == "--isentas");

    let todas = cg::conferir();
    let na_mao = cg::sem_motivo();
    let isentas = todas.len() - na_mao.len();
    let grades = cg::no_padrao();

    println!("TABELAS DA TELA — quanto ja e PhxGrid\n");
    let crua = na_mao
        .iter()
        .filter(|t| t.forma == cg::Forma::Marcacao)
        .count();
    let ajud = na_mao.len() - crua;
    println!("  PhxGrid.criar( ......... {grades}");
    println!("  na mao, marcacao crua .. {crua}");
    println!("  na mao, pelo ajudante .. {ajud}  (chamadas a `tabela(`)");
    println!("  na mao, soma ........... {}", na_mao.len());
    println!("  na mao, isenta ......... {isentas} (com motivo registrado)");
    println!("  catraca (TETO) ......... {}", cg::TETO);

    if ver_isentas {
        println!("\nISENTAS — quem monta tabela na mao com motivo\n");
        for (arq, funcao, porque) in cg::ISENTAS {
            println!("  {arq}  {funcao}\n      {porque}");
        }
        return;
    }

    // Por arquivo, e dentro dele por funcao: e assim que se escolhe a proxima
    // leva de conversao, que e para isso que o relatorio serve.
    let mut arquivo_atual = "";
    let mut funcao_atual = String::new();
    for t in &na_mao {
        if t.arquivo != arquivo_atual {
            arquivo_atual = t.arquivo;
            funcao_atual.clear();
            let quantas = na_mao.iter().filter(|o| o.arquivo == arquivo_atual).count();
            println!("\n{arquivo_atual} — {quantas} na mao");
        }
        if t.funcao != funcao_atual {
            funcao_atual = t.funcao.clone();
            println!("  {funcao_atual}");
        }
        println!(
            "      linha {:<6} {}",
            t.linha,
            match t.forma {
                cg::Forma::Marcacao => "<table> cru",
                cg::Forma::Ajudante => "ajudante tabela(",
            }
        );
    }

    if na_mao.is_empty() {
        println!("\nNenhuma tabela na mao sem motivo. Todas as tabelas sao PhxGrid.");
    }
}

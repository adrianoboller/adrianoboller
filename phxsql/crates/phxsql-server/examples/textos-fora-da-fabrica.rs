//! O relatorio do conferidor de idiomas: quanto da tela ja passa pela fabrica
//! de textos, e ONDE esta o que falta.
//!
//! ```bash
//! cargo run --example textos-fora-da-fabrica -p phxsql-server
//! cargo run --example textos-fora-da-fabrica -p phxsql-server -- --tudo
//! cargo run --example textos-fora-da-fabrica -p phxsql-server -- --isentos
//! ```
//!
//! Sem chave ele mostra o placar e os trinta primeiros de cada arquivo, que e
//! o que cabe num olhar; `--tudo` lista todos, que e o que serve para escolher
//! a proxima leva de traducao.

use phxsql_server::conferidor::{self, Canal, Situacao};

fn main() {
    let tudo = std::env::args().any(|a| a == "--tudo");
    let ver_isentos = std::env::args().any(|a| a == "--isentos");

    let achados = conferidor::conferir();
    let placar = conferidor::Placar::medir();

    println!("TEXTOS DA TELA — o que passa pela fabrica de idiomas\n");
    println!("  na fabrica ......... {}", placar.cobertos);
    println!("  fora da fabrica .... {}", placar.fora);
    println!("  visiveis (soma) .... {}", placar.visiveis());
    println!("  cobertura .......... {}%", placar.por_cento());
    println!(
        "  isentos ............ {} (nome proprio, sigla, identificador)",
        placar.isentos
    );
    println!("  catraca (TETO) ..... {}", conferidor::TETO);

    // SAIDA DE MAQUINA, para o gerador do QA-PDCA nao ler a prosa acima.
    //
    // Ler o relatorio por `grep` seria resolver numero por comparacao de
    // FRASE -- a mesma armadilha que esta casa ja proibiu para texto de tela:
    // no dia em que alguem melhorar a redacao, o gerador quebra calado e
    // publica o numero de ontem. Aqui a chave e estavel e o rotulo e livre.
    if std::env::args().any(|a| a == "--numeros") {
        // Uma linha, e ela SE DESCREVE. O gerador do QA-PDCA nao precisa de
        // lista de catracas nenhuma: ele varre os exemplos, acha quem imprime
        // `catraca:`, e pergunta a cada um. Lista digitada num script e
        // exatamente a receita que envelhece.
        println!(
            "catraca:nome=TETO;onde=crates/phxsql-server/src/conferidor.rs;\
             valor={};medido={};mede=textos cravados fora da fabrica de idiomas",
            conferidor::TETO,
            placar.fora
        );
        // As duas catracas da FABRICA. Elas existiam, os testes as impunham, e
        // nenhum conferidor as reportava -- entao o numero delas no QA-PDCA era
        // digitado, e o gerador as acusou como «promessa» na primeira corrida.
        // Catraca que ninguem mede nao segura nada, e ainda parece que segura.
        println!(
            "catraca:nome=TETO_COLADO;onde=crates/phxsql-server/src/conferidor.rs;\
             valor={};medido={};mede=chaves com os seis idiomas identicos",
            conferidor::TETO_COLADO,
            conferidor::colados().len()
        );
        println!(
            "catraca:nome=TETO_FRASE_REPETIDA;\
             onde=crates/phxsql-server/src/conferidor.rs;\
             valor={};medido={};mede=frase longa repetida em tres ou mais idiomas",
            conferidor::TETO_FRASE_REPETIDA,
            conferidor::frases_repetidas().len()
        );
    }

    if ver_isentos {
        println!("\nISENTOS — o que nao se traduz, e por que\n");
        for a in achados.iter().filter(|a| a.situacao == Situacao::Isento) {
            println!("  {}:{} {:?} — {}", a.arquivo, a.linha, a.texto, a.porque);
        }
        return;
    }

    for (arquivo, _) in conferidor::FONTES {
        let dele: Vec<_> = achados
            .iter()
            .filter(|a| a.arquivo == *arquivo && a.situacao == Situacao::Fora)
            .collect();
        if dele.is_empty() {
            continue;
        }
        println!("\n{} — {} fora da fabrica", arquivo, dele.len());
        let quantos = if tudo { dele.len() } else { 30.min(dele.len()) };
        for a in &dele[..quantos] {
            let via = match a.canal {
                Canal::Marcacao => "marcacao",
                Canal::Rotulo => "rotulo  ",
            };
            println!("  {:>6}  {}  {}", a.linha, via, a.texto);
        }
        if quantos < dele.len() {
            println!("  … e mais {} (use --tudo)", dele.len() - quantos);
        }
    }
}

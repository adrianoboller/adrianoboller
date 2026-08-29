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

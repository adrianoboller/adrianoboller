//! `phxsqld` -- o servidor do PhxSql.
//!
//! ```text
//! phxsqld [--config <caminho>]     sobe o servidor (padrao: config.json)
//! phxsqld --exemplo <1|2|3>        imprime um config.json de exemplo
//! phxsqld --acessos [--config c]   mostra o log de acessos por IP
//! ```

use std::process::ExitCode;

use phxsql_server::{Config, LogAcessos, Servidor};

const USO: &str = "\
phxsqld -- servidor do PhxSql (porta 5000 por padrao)

USO:
  phxsqld [--config <caminho>]      sobe o servidor
  phxsqld --acessos [--config <c>]  mostra quem acessou, por IP
  phxsqld --exemplo <1|2|3>         imprime um config.json de exemplo
                                    1 = isolado, 2 = source, 3 = replica
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USO}");
        return ExitCode::SUCCESS;
    }

    if let Some(i) = args.iter().position(|a| a == "--exemplo") {
        let qual = args.get(i + 1).map(String::as_str).unwrap_or("1");
        return match phxsql_server::config_exemplo(qual) {
            Some(texto) => {
                println!("{texto}");
                ExitCode::SUCCESS
            }
            None => {
                eprintln!("exemplo desconhecido: {qual} (use 1, 2 ou 3)");
                ExitCode::FAILURE
            }
        };
    }

    let caminho = match args.iter().position(|a| a == "--config") {
        Some(i) => args
            .get(i + 1)
            .cloned()
            .unwrap_or_else(|| "config.json".to_string()),
        None => "config.json".to_string(),
    };

    let config = match Config::ler(&caminho) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("erro no {caminho}: {e}");
            eprintln!("\ngere um modelo com:  phxsqld --exemplo 1 > config.json");
            return ExitCode::FAILURE;
        }
    };

    if args.iter().any(|a| a == "--acessos") {
        return match LogAcessos::resumo_por_ip(&config.log_acessos) {
            Ok(resumo) if resumo.is_empty() => {
                println!(
                    "nenhum acesso registrado em {}",
                    config.log_acessos.display()
                );
                ExitCode::SUCCESS
            }
            Ok(resumo) => {
                println!(
                    "{:<40} {:>8} {:>10}  {:<23}  {:<23}",
                    "ip", "acessos", "recusados", "primeiro", "ultimo"
                );
                for r in &resumo {
                    println!(
                        "{:<40} {:>8} {:>10}  {:<23}  {:<23}",
                        r.ip,
                        r.acessos,
                        r.recusados,
                        r.primeiro(),
                        r.ultimo()
                    );
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("erro ao ler o log de acessos: {e}");
                ExitCode::FAILURE
            }
        };
    }

    let servidor = match Servidor::novo(config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("nao consegui iniciar: {e}");
            return ExitCode::FAILURE;
        }
    };
    match servidor.escutar() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("servidor encerrado: {e}");
            ExitCode::FAILURE
        }
    }
}

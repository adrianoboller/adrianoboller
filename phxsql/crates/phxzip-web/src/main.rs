//! `phxzipweb` -- a porta web do PhxZip. Toda a logica mora em `lib.rs`; este
//! arquivo so le os argumentos, abre a porta e diz onde ela esta.

use std::process::ExitCode;

use phxzip_web::{ler_argumentos, Acao, Servidor, AJUDA};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cfg = match ler_argumentos(&args) {
        Ok(Acao::Servir(cfg)) => cfg,
        Ok(Acao::Versao) => {
            println!("{}", phxsql_core::versao_completa("phxzipweb"));
            return ExitCode::SUCCESS;
        }
        Ok(Acao::Ajuda) => {
            print!("{AJUDA}");
            return ExitCode::SUCCESS;
        }
        Err(motivo) => {
            eprintln!("phxzipweb: {motivo}");
            return ExitCode::from(2);
        }
    };
    let servidor = match Servidor::escutar(cfg) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "phxzipweb: nao consegui abrir 127.0.0.1:{} ({e}); \
                 outra porta com --porta N",
                cfg.porta
            );
            return ExitCode::from(1);
        }
    };
    // O endereco impresso e o que o sistema devolveu, e nao o que se pediu:
    // e o que a pessoa vai colar no navegador.
    match servidor.endereco() {
        Ok(e) => println!("PhxZipWeb em http://{e}/ (so nesta maquina)"),
        Err(_) => println!("PhxZipWeb em http://127.0.0.1:{}/", cfg.porta),
    }
    match servidor.servir() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("phxzipweb: a porta caiu ({e})");
            ExitCode::from(1)
        }
    }
}

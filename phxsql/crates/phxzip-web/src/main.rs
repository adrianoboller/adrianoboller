//! `phxzipweb` -- sobe o PhxZip web.
//!
//! ```text
//! phxzipweb [--porta 4000] [--endereco 127.0.0.1] [--usuario NOME [--senha-]] [--max-mib 256]
//! ```
//!
//! Sem `--usuario`, a porta nao pede login. Com `--usuario`, a senha vem da
//! variavel `PHXZIP_WEB_SENHA` ou, com `--senha-`, da primeira linha da
//! entrada padrao -- nunca da linha de comando, que aparece na lista de
//! processos. O servidor guarda so o hash.

use std::io::BufRead;
use std::process::ExitCode;

use phxzip_web::{servir, Config};

fn main() -> ExitCode {
    let mut c = Config::default();
    let mut senha_da_entrada = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let valor = |args: &mut dyn Iterator<Item = String>| args.next().unwrap_or_default();
        match a.as_str() {
            "--porta" => match valor(&mut args).parse() {
                Ok(p) => c.porta = p,
                Err(_) => return falha("--porta precisa de um numero"),
            },
            "--endereco" => c.endereco = valor(&mut args),
            "--usuario" => c.usuario = Some(valor(&mut args)).filter(|u| !u.is_empty()),
            "--senha-" => senha_da_entrada = true,
            "--fios" => match valor(&mut args).parse::<usize>() {
                Ok(f) if f > 0 => c.fios = f,
                _ => return falha("--fios precisa de um numero positivo"),
            },
            "--max-mib" => match valor(&mut args).parse::<usize>() {
                Ok(m) if m > 0 => c.max_corpo = m << 20,
                _ => return falha("--max-mib precisa de um numero positivo"),
            },
            _ => return falha(&format!("opcao desconhecida: {a}")),
        }
    }
    if c.usuario.is_some() {
        let senha = if senha_da_entrada {
            let mut l = String::new();
            let _ = std::io::stdin().lock().read_line(&mut l);
            l.trim_end_matches(['\r', '\n']).to_string()
        } else {
            std::env::var("PHXZIP_WEB_SENHA").unwrap_or_default()
        };
        if senha.is_empty() {
            return falha("--usuario exige senha (PHXZIP_WEB_SENHA ou --senha-)");
        }
        c.senha_hash = Some(phxsql_core::senha::cifrar(&senha));
    }
    let login = match &c.usuario {
        Some(u) => format!("login exigido (usuario {u})"),
        None => "sem login".into(),
    };
    println!("PhxZip web em http://{}:{} -- {login}", c.endereco, c.porta);
    match servir(c) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => falha(&format!("{e}")),
    }
}

fn falha(m: &str) -> ExitCode {
    eprintln!("phxzipweb: {m}");
    ExitCode::from(1)
}

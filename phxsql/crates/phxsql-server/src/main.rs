//! `phxsqld` -- o servidor do PhxSql.
//!
//! ```text
//! phxsqld [--config <caminho>]     sobe o servidor (padrao: config.json)
//! phxsqld --exemplo <1|2|3>        imprime um config.json de exemplo
//! phxsqld --acessos [--config c]   mostra o log de acessos por IP
//! phxsqld --senha [senha]          gera a linha senha_hash para o config.json
//! phxsqld --gerar-chave             gera um par de chaves Ed25519
//! phxsqld --usuarios [--config c]  lista o cadastro e o poder de cada um
//! phxsqld --bloqueios              lista os IPs bloqueados
//! phxsqld --desbloquear <ip>       tira um IP da lista
//! ```

use std::process::ExitCode;

use phxsql_server::{Config, LogAcessos, Servidor};

const USO: &str = "\
phxsqld -- servidor do PhxSql (porta 5000 por padrao)

USO:
  phxsqld [--config <caminho>]      sobe o servidor
  phxsqld --acessos [--config <c>]  mostra quem acessou, por IP
  phxsqld --usuarios [--config <c>] lista o cadastro e o poder de cada um
  phxsqld --bloqueios [--config <c>]      lista os IPs bloqueados
  phxsqld --desbloquear <ip> [--config c] tira um IP da lista de bloqueio
  phxsqld --senha [senha]           gera a linha senha_hash para o config.json
  phxsqld --gerar-chave             gera um par de chaves Ed25519 (2o fator)
  phxsqld --exemplo <1|2|3>         imprime um config.json de exemplo
                                    1 = isolado, 2 = source, 3 = replica

A senha NUNCA vai em texto puro no config.json. Gere o hash assim:

  phxsqld --senha                   pergunta a senha (ela aparece na tela)
  echo -n 'minha senha' | phxsqld --senha    nao aparece, nem no historico
";

/// Gera a linha `senha_hash` para colar no `config.json`.
///
/// A senha e lida da entrada padrao para nao ficar no historico do shell nem
/// aparecer no `ps`. Passar como argumento funciona, mas e menos seguro.
/// Um par de chaves Ed25519.
///
/// A privada sai UMA vez, aqui, e o servidor nunca a ve. Se ela se perder,
/// nao ha como recuperar -- gera-se outra e troca-se a publica no
/// `config.json`. E o preco de o servidor nao guardar nada que assine.
fn gerar_chave() -> ExitCode {
    let privada = phxsql_core::ed25519::gerar_privada();
    let publica = phxsql_core::ed25519::chave_publica(&privada);
    println!("# Guarde a chave PRIVADA fora do servidor. Ela nao aparece de novo.");
    println!("chave_privada = {}", phxsql_core::hash::para_hex(&privada));
    println!();
    println!("# Esta linha vai no config.json, dentro do usuario:");
    println!(
        "\"chave_publica\": \"{}\"",
        phxsql_core::hash::para_hex(&publica)
    );
    ExitCode::SUCCESS
}

fn gerar_senha(args: &[String]) -> ExitCode {
    use std::io::{IsTerminal, Read};
    let i = args.iter().position(|a| a == "--senha").unwrap();
    let clara = match args.get(i + 1).filter(|a| !a.starts_with("--")) {
        Some(a) => a.clone(),
        None => {
            if std::io::stdin().is_terminal() {
                eprintln!("Digite a senha e tecle Enter (ela vai aparecer na tela).");
                eprintln!("Para nao aparecer:  echo -n 'a senha' | phxsqld --senha");
            }
            let mut entrada = String::new();
            if std::io::stdin().read_to_string(&mut entrada).is_err() {
                eprintln!("nao consegui ler a senha da entrada padrao");
                return ExitCode::FAILURE;
            }
            entrada.trim_end_matches(['\n', '\r']).to_string()
        }
    };
    if clara.is_empty() {
        eprintln!("senha vazia; nada a gerar");
        return ExitCode::FAILURE;
    }
    let hash = phxsql_core::senha::cifrar(&clara);
    println!("\"senha_hash\": \"{hash}\"");
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USO}");
        return ExitCode::SUCCESS;
    }

    if args.iter().any(|a| a == "--senha") {
        return gerar_senha(&args);
    }

    if args.iter().any(|a| a == "--gerar-chave") {
        return gerar_chave();
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

    if let Some(i) = args.iter().position(|a| a == "--desbloquear") {
        let ip = match args.get(i + 1).filter(|a| !a.starts_with("--")) {
            Some(a) => a.clone(),
            None => {
                eprintln!("informe o IP: phxsqld --desbloquear 203.0.113.9");
                return ExitCode::FAILURE;
            }
        };
        return match phxsql_server::Blacklist::abrir(&config.blacklist)
            .and_then(|mut bl| bl.desbloquear(&ip, &config.politica))
        {
            Ok(true) => {
                println!("{ip} desbloqueado");
                ExitCode::SUCCESS
            }
            Ok(false) => {
                println!("{ip} nao estava na lista");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("erro: {e}");
                ExitCode::FAILURE
            }
        };
    }

    if args.iter().any(|a| a == "--bloqueios") {
        let bl = match phxsql_server::Blacklist::abrir(&config.blacklist) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("erro ao ler a lista: {e}");
                return ExitCode::FAILURE;
            }
        };
        let agora = phxsql_server::agora_ms();
        let ativos = bl.ativos(agora);
        if ativos.is_empty() {
            println!("nenhum IP bloqueado em {}", bl.caminho().display());
            return ExitCode::SUCCESS;
        }
        println!(
            "{:<40} {:<23} {:<23} {:>5}  {:<8}  motivo",
            "ip", "desde", "ate", "tent", "firewall"
        );
        for b in ativos {
            println!(
                "{:<40} {:<23} {:<23} {:>5}  {:<8}  {} ({})",
                b.ip,
                b.desde(),
                b.ate(),
                b.tentativas,
                if b.firewall { "sim" } else { "nao" },
                b.motivo,
                b.comando
            );
        }
        return ExitCode::SUCCESS;
    }

    if args.iter().any(|a| a == "--usuarios") {
        let c = &config.cadastro;
        if c.vazio() {
            println!("nenhum usuario cadastrado neste config.json");
            println!("o token de servico e a unica credencial; qualquer pedido com ele");
            println!("tem poder total. Cadastre usuarios para restringir.");
            return ExitCode::SUCCESS;
        }
        for aviso in &c.avisos {
            eprintln!("AVISO: {aviso}");
        }
        println!(
            "{:<14} {:<24} {:<10} {:<7}  poder por base",
            "login", "nome", "nivel", "ativo"
        );
        let todos = c.root.iter().chain(c.usuarios.iter());
        for u in todos {
            let bases: Vec<String> = if u.supervisor {
                vec!["(supervisor: tudo em toda base)".to_string()]
            } else if u.bases.is_empty() {
                // Sem regra de base, quem manda e o nivel -- e a listagem tem
                // de dizer isso, senao "(nenhuma)" mente sobre quem pode ler.
                let podem: Vec<&str> = phxsql_server::Atividade::TODAS
                    .iter()
                    .filter(|a| u.nivel.permissoes().pode(**a))
                    .map(|a| a.nome())
                    .collect();
                vec![if podem.is_empty() {
                    "(nada, em base nenhuma)".to_string()
                } else {
                    format!("(pelo nivel, em toda base: {})", podem.join("+"))
                }]
            } else {
                u.bases
                    .iter()
                    .map(|(b, p)| {
                        let podem: Vec<&str> = phxsql_server::Atividade::TODAS
                            .iter()
                            .filter(|a| p.pode(**a))
                            .map(|a| a.nome())
                            .collect();
                        format!(
                            "{b}={}",
                            if podem.is_empty() {
                                "nada".to_string()
                            } else {
                                podem.join("+")
                            }
                        )
                    })
                    .collect()
            };
            println!(
                "{:<14} {:<24} {:<10} {:<7}  {}",
                u.login,
                u.nome,
                if u.supervisor {
                    "supervisor"
                } else {
                    u.nivel.nome()
                },
                if u.ativo { "sim" } else { "nao" },
                bases.join("  ")
            );
        }
        return ExitCode::SUCCESS;
    }

    for aviso in &config.cadastro.avisos {
        eprintln!("AVISO: {aviso}");
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

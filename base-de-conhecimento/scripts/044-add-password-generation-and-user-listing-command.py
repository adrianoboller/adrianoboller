# Add password generation and user listing commands
# 27/08 19:05

p='crates/phxsql-server/src/main.rs'
s=open(p).read()
s=s.replace('''//! phxsqld --acessos [--config c]   mostra o log de acessos por IP
//! ```''','''//! phxsqld --acessos [--config c]   mostra o log de acessos por IP
//! phxsqld --senha [senha]          gera a linha senha_hash para o config.json
//! phxsqld --usuarios [--config c]  lista o cadastro e o poder de cada um
//! ```''')
s=s.replace('''  phxsqld --exemplo <1|2|3>         imprime um config.json de exemplo
                                    1 = isolado, 2 = source, 3 = replica
";''','''  phxsqld --usuarios [--config <c>] lista o cadastro e o poder de cada um
  phxsqld --senha [senha]           gera a linha senha_hash para o config.json
  phxsqld --exemplo <1|2|3>         imprime um config.json de exemplo
                                    1 = isolado, 2 = source, 3 = replica

A senha NUNCA vai em texto puro no config.json. Gere o hash assim:

  phxsqld --senha                   pergunta a senha (ela aparece na tela)
  echo -n 'minha senha' | phxsqld --senha    nao aparece, nem no historico
";''')
s=s.replace('''    if let Some(i) = args.iter().position(|a| a == "--exemplo") {''','''    if args.iter().any(|a| a == "--senha") {
        return gerar_senha(&args);
    }

    if let Some(i) = args.iter().position(|a| a == "--exemplo") {''')

s=s.replace('''    let servidor = match Servidor::novo(config) {''','''    if args.iter().any(|a| a == "--usuarios") {
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
            "{:<14} {:<26} {:<9} {:<7}  poder por base",
            "login", "nome", "supervisor", "ativo"
        );
        let todos = c.root.iter().chain(c.usuarios.iter());
        for u in todos {
            let bases: Vec<String> = if u.supervisor {
                vec!["(supervisor: tudo em toda base)".to_string()]
            } else if u.bases.is_empty() {
                vec!["(nenhuma)".to_string()]
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
                "{:<14} {:<26} {:<9} {:<7}  {}",
                u.login,
                u.nome,
                if u.supervisor { "sim" } else { "nao" },
                if u.ativo { "sim" } else { "nao" },
                bases.join("  ")
            );
        }
        return ExitCode::SUCCESS;
    }

    for aviso in &config.cadastro.avisos {
        eprintln!("AVISO: {aviso}");
    }

    let servidor = match Servidor::novo(config) {''')

s=s.replace('''fn main() -> ExitCode {''','''/// Gera a linha `senha_hash` para colar no `config.json`.
///
/// A senha e lida da entrada padrao para nao ficar no historico do shell nem
/// aparecer no `ps`. Passar como argumento funciona, mas e menos seguro.
fn gerar_senha(args: &[String]) -> ExitCode {
    use std::io::Read;
    let i = args.iter().position(|a| a == "--senha").unwrap();
    let clara = match args.get(i + 1).filter(|a| !a.starts_with("--")) {
        Some(a) => a.clone(),
        None => {
            if atty_provavel() {
                eprintln!("Digite a senha e tecle Enter (ela vai aparecer na tela).");
                eprintln!("Para nao aparecer:  echo -n 'a senha' | phxsqld --senha");
            }
            let mut entrada = String::new();
            if std::io::stdin().read_to_string(&mut entrada).is_err() {
                eprintln!("nao consegui ler a senha da entrada padrao");
                return ExitCode::FAILURE;
            }
            entrada.trim_end_matches(['\\n', '\\r']).to_string()
        }
    };
    if clara.is_empty() {
        eprintln!("senha vazia; nada a gerar");
        return ExitCode::FAILURE;
    }
    let hash = phxsql_core::senha::cifrar(&clara);
    println!("\\"senha_hash\\": \\"{hash}\\"");
    ExitCode::SUCCESS
}

/// Heuristica simples: sem terminal, a entrada costuma vir de um cano.
fn atty_provavel() -> bool {
    std::env::var("TERM").is_ok()
}

fn main() -> ExitCode {''')
open(p,'w').write(s)

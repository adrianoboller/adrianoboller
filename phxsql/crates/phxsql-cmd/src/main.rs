//! `phxsqlcmd` -- o console interativo do PhxSql.
//!
//! ```text
//! phxsqlcmd [--host h] [--porta 5000] [--token t] [--usuario u] [--database d]
//! phxsqlcmd --comando 'bancos'        roda uma linha e sai
//! ```

use std::io::{BufRead, IsTerminal, Write};
use std::process::ExitCode;
use std::time::Duration;

use phxsql_cmd::{Console, Saida};

const USO: &str = "\
phxsqlcmd -- console do PhxSql (fala o protocolo JSON com um servidor)

USO:
  phxsqlcmd [--host 127.0.0.1] [--porta 5000] [--token <t>]
            [--usuario <login>] [--database <banco>] [--comando '<linha>']

NA LINHA DO CONSOLE:
  bancos                              uma operacao sem argumento
  tabelas database=loja               argumentos sao chave=valor
  ler tabela=clientes rowid=42        o /use preenche o database sozinho
  nome=\"Ana Maria\"                    aspas seguram o valor com espaco
  SELECT * FROM clientes LIMIT 10     vira a operacao sql
  {\"op\":\"ping\"}                       JSON cru, para o que a linha nao alcanca

  /help                               a lista de operacoes, VINDA DO SERVIDOR
  /help buscar                        parametros e exemplo de uma
  /use loja                           escolhe o banco corrente
  /cru                                alterna entre tabela e JSON cru
  /sair                               termina

SEM HISTORICO E SEM SETAS NESTA RODADA. A seta para cima escreve ^[[A na tela
em vez de trazer o comando anterior, e ctrl+R nao procura nada. Um readline de
verdade e um terminal em modo cru, e isso e uma crate -- e a regra do projeto e
zero dependencias externas. A leitura de linha da std faz o resto.

A SENHA vem de PHXSQL_SENHA, ou e perguntada (e aparece na tela). Passar
--senha funciona e e menos seguro: o argumento aparece no `ps` e fica no
historico do shell.

  PHXSQL_SENHA='a senha' phxsqlcmd --usuario adriano
";

fn valor(args: &[String], chave: &str) -> Option<String> {
    let i = args.iter().position(|a| a == chave)?;
    args.get(i + 1).filter(|a| !a.starts_with("--")).cloned()
}

/// A senha, na ordem do mais seguro para o menos.
///
/// O argumento fica por ultimo e avisa: ele aparece no `ps` de qualquer um na
/// maquina e fica no historico do shell. E o mesmo cuidado do `phxsqld
/// --senha`, que ja lia da entrada padrao pela mesma razao.
fn senha(args: &[String]) -> String {
    if let Ok(s) = std::env::var("PHXSQL_SENHA") {
        if !s.is_empty() {
            return s;
        }
    }
    if let Some(s) = valor(args, "--senha") {
        eprintln!("AVISO: --senha aparece no `ps` e fica no historico do shell.");
        eprintln!("       Prefira:  PHXSQL_SENHA='a senha' phxsqlcmd ...");
        return s;
    }
    eprint!("senha (vai aparecer na tela): ");
    let _ = std::io::stderr().flush();
    let mut linha = String::new();
    let _ = std::io::stdin().read_line(&mut linha);
    linha.trim_end_matches(['\n', '\r']).to_string()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USO}");
        return ExitCode::SUCCESS;
    }

    let host = valor(&args, "--host").unwrap_or_else(|| "127.0.0.1".to_string());
    let porta: u16 = valor(&args, "--porta")
        .and_then(|p| p.parse().ok())
        .unwrap_or(phxsql_server::PORTA_PADRAO);
    let token = valor(&args, "--token").unwrap_or_default();
    let usuario = valor(&args, "--usuario").unwrap_or_default();

    let mut console = match Console::ligar(&host, porta, &token, Duration::from_secs(30)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("nao conectei em {host}:{porta}: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Some(d) = valor(&args, "--database") {
        console.database = d;
    }

    if !usuario.is_empty() {
        let s = senha(&args);
        if let Err(e) = console.entrar(&usuario, &s) {
            eprintln!("login de {usuario} recusado: {e}");
            return ExitCode::FAILURE;
        }
    }

    // `--comando` roda uma linha e sai. E o que um script usa, e e o que faz o
    // console ser testavel por processo sem simular gente digitando.
    if let Some(linha) = valor(&args, "--comando") {
        return match console.executar_linha(&linha) {
            Saida::Texto(t) => {
                println!("{t}");
                // A linha que devolveu erro tem de sair com codigo de erro:
                // sem isso um script encadeado seguiria como se desse certo.
                if t.starts_with("erro: ") {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                }
            }
            _ => ExitCode::SUCCESS,
        };
    }

    let interativo = std::io::stdin().is_terminal();
    if interativo {
        println!("PhxSql console em {}", console.destino);
        println!("/help lista as operacoes (vem do servidor).  /sair termina.");
        if !console.database.is_empty() {
            println!("banco corrente: {}", console.database);
        }
    }

    // O primeiro prompt sai ANTES da primeira leitura, e os outros depois de
    // cada resposta. Escrito de outro jeito, quem abre o console fica olhando
    // uma tela vazia sem saber se ele subiu.
    if interativo {
        print!("phxsql> ");
        let _ = std::io::stdout().flush();
    }
    let entrada = std::io::stdin().lock();
    for linha in entrada.lines() {
        let linha = match linha {
            Ok(l) => l,
            Err(e) => {
                eprintln!("erro ao ler: {e}");
                return ExitCode::FAILURE;
            }
        };
        match console.executar_linha(&linha) {
            Saida::Nada => {}
            Saida::Texto(t) => println!("{t}"),
            Saida::Sair => break,
        }
        if interativo {
            print!("phxsql> ");
            let _ = std::io::stdout().flush();
        }
    }
    ExitCode::SUCCESS
}

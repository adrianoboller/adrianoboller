//! Comandos de contexto e dados: `skills importar`, `licenca conferir`, `indexar` e
//! `projeto confiar|mostrar`.
//! Cada um so chama a funcao do agente que a ferramenta e a montagem usam; nada de
//! politica mora aqui.

use anyhow::{Result, bail};
use std::path::PathBuf;

/// Primeiro argumento solto depois de `pula` posicoes, que nao e opcao nem valor dela.
fn solto(args: &[String], pula: usize) -> Option<String> {
    args.iter()
        .enumerate()
        .skip(pula)
        .find(|(i, a)| !a.starts_with("--") && !(*i > 0 && args[i - 1] == "--pasta"))
        .map(|(_, a)| a.clone())
}

const USO_SKILLS: &str = "uso: phxclaw skills importar DIR [--com-scripts] [--aceitar-licenca-desconhecida] [--pasta DIR]";

/// `skills importar DIR [--com-scripts] [--aceitar-licenca-desconhecida] [--pasta DIR]`.
pub fn skills(args: &[String]) -> Result<()> {
    if args.first().map(String::as_str) != Some("importar") {
        bail!("{USO_SKILLS}");
    }
    let Some(origem) = solto(args, 1) else {
        bail!("{USO_SKILLS}");
    };
    let raiz = super::pasta(args).join("tasks");
    let destino = phxclaw_agent::skills::pasta_do_ambiente(&raiz);
    let op = phxclaw_agent::importar_skills::Opcoes {
        com_scripts: args.iter().any(|a| a == "--com-scripts"),
        limite_licenca: None,
        aceitar_licenca_desconhecida: args
            .iter()
            .any(|a| a == phxclaw_agent::licenca::OPCAO_ACEITAR),
    };
    let r = phxclaw_agent::importar_skills::importar(&PathBuf::from(&origem), &destino, &op);
    let n = |f: fn(&phxclaw_agent::importar_skills::Importada) -> bool| {
        r.importadas.iter().filter(|i| f(i)).count()
    };
    println!(
        "{} SKILL.md achados em {origem}: {} importados, {} ja importados (mesmo SHA-256), {} recusados",
        r.achados,
        r.importadas.len(),
        r.repetidas,
        r.recusadas.len()
    );
    println!(
        "  destino {}\n  scripts desligados em {} skills{}; nome ajustado em {}; descricao derivada em {}, \
cortada em {}; corpo cortado em {}",
        destino.root().display(),
        n(|i| !i.scripts_desligados.is_empty()),
        if op.com_scripts {
            " (copiados: --com-scripts)"
        } else {
            ""
        },
        n(|i| i.nome_ajustado),
        n(|i| i.descricao_derivada),
        n(|i| i.descricao_cortada),
        n(|i| i.corpo_cortado),
    );
    let traducoes: usize = r.importadas.iter().flat_map(|i| i.traducoes.values()).sum();
    println!("  {traducoes} nomes de ferramenta traduzidos");
    let aceitas = r
        .importadas
        .iter()
        .filter(|i| {
            i.licenca
                .as_ref()
                .is_some_and(|l| l.decisao_do_operador.is_some())
        })
        .count();
    println!(
        "  licenca: aviso em {} de {} importadas; {aceitas} de licenca desconhecida aceitas por {}",
        phxclaw_agent::licenca::ARQUIVO_AVISO,
        r.importadas.len(),
        phxclaw_agent::licenca::OPCAO_ACEITAR
    );
    for (p, e) in &r.recusadas {
        println!("  recusado {}: {e}", p.display());
    }
    if !r.recusadas.is_empty() {
        std::process::exit(2);
    }
    Ok(())
}

const USO_LICENCA: &str =
    "uso: phxclaw licenca conferir DIR [--json] [--aceitar-licenca-desconhecida] [--limite DIR]";

/// `licenca conferir DIR [--json] [--aceitar-licenca-desconhecida] [--limite DIR]`: a mesma
/// porta do `skills importar`, para quem copia conteudo de terceiro por outro caminho (o
/// importador de papeis em Python). Sai 0 quando entra, 2 quando recusa.
pub fn licenca(args: &[String]) -> Result<()> {
    if args.first().map(String::as_str) != Some("conferir") {
        bail!("{USO_LICENCA}");
    }
    let mut alvo = None;
    let mut limite = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--limite" => {
                let Some(v) = args.get(i + 1) else {
                    bail!("{USO_LICENCA}");
                };
                limite = Some(PathBuf::from(v));
                i += 1;
            }
            "--json" => {}
            o if o == phxclaw_agent::licenca::OPCAO_ACEITAR => {}
            o if o.starts_with("--") => bail!("opcao desconhecida {o}; {USO_LICENCA}"),
            o => alvo = Some(PathBuf::from(o)),
        }
        i += 1;
    }
    let Some(alvo) = alvo else {
        bail!("{USO_LICENCA}");
    };
    if !alvo.exists() {
        bail!("{} nao existe", alvo.display());
    }
    let aceitar = args
        .iter()
        .any(|a| a == phxclaw_agent::licenca::OPCAO_ACEITAR);
    let c = phxclaw_agent::licenca::conferir(&alvo, limite.as_deref(), vec![]);
    let r = phxclaw_agent::licenca::relatorio_json(&c, aceitar);
    let entra = r["entra"].as_bool() == Some(true);
    if args.iter().any(|a| a == "--json") {
        println!("{}", serde_json::to_string_pretty(&r)?);
    } else {
        println!(
            "{}: {} ({}) -- {}",
            c.alvo.display(),
            c.classe.nome().to_uppercase(),
            c.licencas().join(", "),
            if entra { "entra" } else { "recusada" }
        );
        println!("  {}", c.motivo());
        println!("  busca de {} ate {}", c.alvo.display(), c.limite.display());
        for d in &c.declaracoes {
            println!("  {:<14} {:<10} {}", d.licenca, d.como, d.onde);
        }
        for l in &c.copyright {
            println!("  {l}");
        }
    }
    if !entra {
        std::process::exit(2);
    }
    Ok(())
}

/// `indexar DIR [--pasta DIR]`.
pub fn indexar(args: &[String]) -> Result<()> {
    let Some(dir) = solto(args, 0) else {
        bail!("uso: phxclaw indexar DIR [--pasta DIR]");
    };
    let pasta = super::pasta(args);
    let t = std::time::Instant::now();
    let r = phxclaw_agent::documentos::indexar(&pasta, &PathBuf::from(&dir))
        .map_err(anyhow::Error::msg)?;
    println!(
        "{dir}: {} arquivos, {} trechos, {} pulados em {} ms -> {} (doc_search, capacidade doc.read)",
        r.arquivos,
        r.trechos,
        r.pulados,
        t.elapsed().as_millis(),
        phxclaw_agent::documentos::arquivo_do_indice(&pasta).display()
    );
    Ok(())
}

/// `projeto confiar [DIR]` e `projeto mostrar [DIR]`.
pub fn projeto(args: &[String]) -> Result<()> {
    let uso = "uso: phxclaw projeto confiar|mostrar [DIR] [--pasta DIR]";
    let pasta = super::pasta(args);
    let dir = solto(args, 1)
        .map(PathBuf::from)
        .or_else(phxclaw_agent::montagem::raiz_do_projeto)
        .ok_or_else(|| anyhow::anyhow!(uso))?;
    match args.first().map(String::as_str) {
        Some("confiar") => {
            let raiz =
                phxclaw_agent::instrucoes::confiar(&pasta, &dir).map_err(anyhow::Error::msg)?;
            println!(
                "projeto confiado: {} (os AGENTS.md dele entram no prompt)",
                raiz.display()
            );
        }
        Some("mostrar") => match phxclaw_agent::instrucoes::do_projeto(&pasta, &dir) {
            Some(i) => {
                for a in &i.arquivos {
                    println!("lido: {}", a.display());
                }
                println!(
                    "{} bytes{}\n\n{}",
                    i.bloco.len(),
                    if i.cortado { " (cortado no teto)" } else { "" },
                    i.bloco
                );
            }
            None => println!("nenhuma instrucao de projeto entra no prompt"),
        },
        _ => bail!(uso),
    }
    Ok(())
}

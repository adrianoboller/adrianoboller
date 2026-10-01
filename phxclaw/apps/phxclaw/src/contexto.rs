//! Comandos de contexto e dados: `skills importar`, `indexar` e `projeto confiar|mostrar`.
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

/// `skills importar DIR [--com-scripts] [--pasta DIR]`.
pub fn skills(args: &[String]) -> Result<()> {
    if args.first().map(String::as_str) != Some("importar") {
        bail!("uso: phxclaw skills importar DIR [--com-scripts] [--pasta DIR]");
    }
    let Some(origem) = solto(args, 1) else {
        bail!("uso: phxclaw skills importar DIR [--com-scripts] [--pasta DIR]");
    };
    let raiz = super::pasta(args).join("tasks");
    let destino = phxclaw_agent::skills::pasta_do_ambiente(&raiz);
    let op = phxclaw_agent::importar_skills::Opcoes {
        com_scripts: args.iter().any(|a| a == "--com-scripts"),
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
    for (p, e) in &r.recusadas {
        println!("  recusado {}: {e}", p.display());
    }
    if !r.recusadas.is_empty() {
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

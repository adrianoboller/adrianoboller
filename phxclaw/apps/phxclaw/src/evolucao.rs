//! `phxclaw evoluir`: so le as opcoes e imprime. O ciclo, a politica, a escolha do item e a
//! decisao humana sao os de `phxclaw_agent::evolucao` -- a CLI nao decide nada que o motor
//! nao decida igual para qualquer outra porta.

use anyhow::{Context, Result, bail};
use phxclaw_agent::TaskStore;
use phxclaw_agent::evolucao::{self, Ciclo, Estado};
use phxclaw_agent::montagem::Montagem;
use std::path::PathBuf;
use std::sync::Mutex;

const USO: &str = "uso: phxclaw evoluir [--item NOME] [--modelo M] [--projeto DIR] [--pasta DIR] \
| evoluir itens | evoluir listar | evoluir aprovar ID | evoluir rejeitar ID [--motivo TEXTO]";

fn projeto(args: &[String]) -> Result<PathBuf> {
    crate::opcao(args, "--projeto")
        .map(PathBuf::from)
        .or_else(phxclaw_agent::montagem::raiz_do_projeto)
        .context("sem pasta de projeto (--projeto, PHXCLAW_PROJETO ou a pasta corrente)")
}

pub async fn comando(args: &[String]) -> Result<()> {
    let projeto = projeto(args)?;
    let sub = args
        .first()
        .map(String::as_str)
        .filter(|a| !a.starts_with("--"));
    match sub {
        None => ciclo(args, projeto).await,
        Some("itens" | "items") => {
            let pol = evolucao::Politica::carregar(&projeto).map_err(anyhow::Error::msg)?;
            let lista = evolucao::candidatos(&projeto, &pol).map_err(anyhow::Error::msg)?;
            println!("{} candidato(s), na ordem da escolha:", lista.len());
            for i in &lista {
                println!(
                    "  {:<8} {:<32} {}",
                    i.estado,
                    i.nome,
                    i.evidencia.chars().take(90).collect::<String>()
                );
            }
            println!("fora do alcance ({}):", evolucao::POLITICA);
            for v in &pol.itens_vetados {
                println!("  {:<41} {}", v.item, v.motivo);
            }
            Ok(())
        }
        Some("listar" | "list") => {
            for r in evolucao::listar(&projeto).map_err(anyhow::Error::msg)? {
                println!(
                    "{:<44} {:<32} {}",
                    r.id,
                    r.estado.nome(),
                    r.motivo.unwrap_or_default()
                );
            }
            Ok(())
        }
        Some("aprovar" | "approve") => {
            let id = args.get(1).context(USO)?;
            let (r, cmd) = evolucao::aprovar(&projeto, id).map_err(anyhow::Error::msg)?;
            println!(
                "evolucao {} marcada aprovada. Nada foi mesclado; o merge e seu:\n  {cmd}",
                r.id
            );
            Ok(())
        }
        Some("rejeitar" | "reject") => {
            let id = args.get(1).context(USO)?;
            let motivo = crate::opcao(args, "--motivo");
            let r =
                evolucao::rejeitar(&projeto, id, motivo.as_deref()).map_err(anyhow::Error::msg)?;
            println!(
                "evolucao {} rejeitada; o ramo {} ficou (apagar: git -C {} branch -D {})",
                r.id, r.ramo, r.repositorio, r.ramo
            );
            Ok(())
        }
        Some(outro) => bail!("{USO} (nao: {outro})"),
    }
}

async fn ciclo(args: &[String], projeto: PathBuf) -> Result<()> {
    let bwrap = phxclaw_agent::arquivos::achar_bwrap()
        .context("sem bwrap: o ciclo roda git e cargo no sandbox")?;
    let raiz = crate::pasta(args);
    let store = TaskStore::new(raiz.join("tasks"))?;
    let spec = crate::opcao(args, "--modelo")
        .or_else(|| phxclaw_agent::config::texto_de("modelo.padrao"))
        .unwrap_or_else(|| crate::MODELO_PADRAO.into());
    // A SP000015 pede modelo forte por API; sem chave, roda com o que houver e o relatorio
    // diz qual rodou. A nota diz tambem quando ha chave guardada e ela nao foi usada.
    let chave_forte = [
        &phxclaw_agent::chaves::ANTHROPIC,
        &phxclaw_agent::chaves::OPENAI,
    ]
    .iter()
    .find(|s| s.credencial(&raiz).is_ok())
    .map(|s| s.comando);
    let nota = match (spec.split(':').next().unwrap_or(""), chave_forte) {
        ("anthropic" | "openai", _) => None,
        (_, Some(c)) => Some(format!(
            "ha chave de modelo forte guardada (`phxclaw {c}`), mas rodou com {spec}: passe --modelo"
        )),
        (_, None) => Some(format!(
            "sem chave de modelo forte (`phxclaw anthropic chave` ou `phxclaw openai chave`): \
rodou com {spec}"
        )),
    };
    let agente = Montagem::new(store)
        .agent(&spec)
        .map_err(anyhow::Error::msg)?;
    let c = Ciclo {
        item: crate::opcao(args, "--item"),
        nota_do_modelo: nota,
        ..Ciclo::agora(projeto, bwrap)
    };
    println!("evoluir: modelo {}", agente.llm.id());
    let r = evolucao::evoluir(&agente, &c, &crate::Terminal(Mutex::new(0)))
        .await
        .map_err(anyhow::Error::msg)?;
    println!(
        "evolucao {}: {}\nitem {} ({})\nrelatorio {}/{}/{}.md",
        r.id,
        r.estado.nome(),
        r.item,
        r.porque,
        c.projeto.display(),
        evolucao::PASTA,
        r.id
    );
    for p in &r.portoes {
        println!(
            "  {:<7} {:<30} {} ({:.1} s)",
            p.nome,
            p.alvo,
            if p.verde { "verde" } else { "VERMELHO" },
            p.segundos
        );
    }
    if let Some(m) = &r.motivo {
        println!("motivo: {m}");
    }
    if r.estado == Estado::EsperandoGo {
        println!(
            "ramo {} esperando Go: phxclaw evoluir aprovar {} (so marca; o merge e seu)",
            r.ramo, r.id
        );
    } else {
        std::process::exit(1);
    }
    Ok(())
}

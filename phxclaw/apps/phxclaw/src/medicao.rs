//! Comandos de medicao: `repetir`, `avaliar` e `skill otimizar`. A logica mora em
//! `phxclaw_agent::{gravacao, avaliacao, otimizacao}`; aqui so se le a linha de comando e
//! se imprime, para a API e a CLI nao terem dois motores de medicao.

use crate::{MODELO_PADRAO, Terminal, opcao, pasta};
use anyhow::{Context, Result, bail};
use phxclaw_agent::avaliacao::{self, LeitorEnergia, ler_casos};
use phxclaw_agent::gravacao::{Gravacao, ModoFerramentas, Repetidor};
use phxclaw_agent::montagem::Montagem;
use phxclaw_agent::otimizacao::{
    PedidoOtimizacao, ab, com_pasta_de_skills, gerar_variante, registrar_recusa,
};
use phxclaw_agent::{CancelFlag, ScriptedLlm, Task, TaskStore};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Primeiro argumento solto (nao opcao nem valor de opcao).
fn solto<'a>(args: &'a [String], com_valor: &[&str]) -> Option<&'a String> {
    args.iter().enumerate().find_map(|(i, a)| {
        let valor = i > 0 && com_valor.contains(&args[i - 1].as_str());
        (!a.starts_with("--") && !valor).then_some(a)
    })
}

/// Segundos Unix: nome de pasta que ordena e nao pede o chrono so por isto.
fn carimbo() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
        .to_string()
}

pub async fn repetir(args: &[String]) -> Result<()> {
    let arq = solto(args, &["--ferramentas", "--pasta"])
        .context("uso: phxclaw repetir ARQ.jsonl [--ferramentas reais|gravadas] [--pasta DIR]")?;
    let modo = match opcao(args, "--ferramentas").as_deref() {
        None | Some("reais") => ModoFerramentas::Reais,
        Some("gravadas") => ModoFerramentas::Gravadas,
        Some(o) => bail!("--ferramentas {o}: use reais ou gravadas"),
    };
    let g = Gravacao::ler(Path::new(arq)).map_err(anyhow::Error::msg)?;
    println!(
        "gravacao de {} | modelo {} | {} resposta(s) do modelo, {} chamada(s) de ferramenta",
        g.data,
        g.modelo,
        g.modelo_respostas.len(),
        g.ferramentas.len()
    );
    let objetivo = g.objetivo.clone();
    let modelo = format!("repeticao:{}", g.modelo);
    let r = Repetidor::novo(g, modo);
    // A montagem e a desta maquina (portao, regras, capacidades); o modelo e trocado pelas
    // respostas gravadas, entao o roteiro vazio aqui nunca e consultado.
    let m = Montagem::new(TaskStore::new(pasta(args).join("tasks"))?);
    let agente = r.agente(m.agent_with(Arc::new(ScriptedLlm::new(vec![]))));
    println!("repetindo sem modelo (ferramentas {modo:?})...");
    let fim = agente
        .run(
            Task::new(objetivo, modelo),
            &CancelFlag::default(),
            &Terminal(Mutex::new(0)),
        )
        .await;
    let rel = r.relatorio();
    println!("\nestado: {:?}", fim.status);
    println!("gravada : {}", rel.gravada.join(" -> "));
    println!("repetida: {}", rel.repetida.join(" -> "));
    println!(
        "respostas do modelo usadas: {}/{}",
        rel.respostas_usadas, rel.respostas_gravadas
    );
    if modo == ModoFerramentas::Reais {
        println!(
            "saidas de ferramenta diferentes da gravada: {} (nao e divergencia)",
            rel.saidas_diferentes
        );
    }
    if rel.igual() {
        println!(
            "MESMA SEQUENCIA: {} chamada(s) de ferramenta",
            rel.repetida.len()
        );
        Ok(())
    } else {
        println!("DIVERGIU ({}):", rel.divergencias.len());
        for d in &rel.divergencias {
            println!("  {d}");
        }
        std::process::exit(3);
    }
}

pub async fn avaliar(args: &[String]) -> Result<()> {
    const USO: &str = "uso: phxclaw avaliar --modelos A,B --tarefas DIR [--rodadas N] [--saida DIR] [--pasta DIR]";
    let modelos: Vec<String> = opcao(args, "--modelos")
        .context(USO)?
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let casos = ler_casos(Path::new(&opcao(args, "--tarefas").context(USO)?))
        .map_err(anyhow::Error::msg)?;
    let rodadas: usize = opcao(args, "--rodadas")
        .map(|r| r.parse())
        .transpose()
        .context("--rodadas")?
        .unwrap_or(3);
    let saida = opcao(args, "--saida")
        .map(PathBuf::from)
        .unwrap_or_else(|| pasta(args).join("avaliacoes").join(carimbo()));
    // As tarefas da avaliacao ficam na pasta da avaliacao, fora das sessoes do operador.
    let m = Montagem::new(TaskStore::new(saida.join("tasks"))?);
    let fabrica = |modelo: &str| m.agent(modelo);
    let a = avaliacao::avaliar(
        &fabrica,
        &modelos,
        &casos,
        rodadas,
        &saida,
        &LeitorEnergia::do_sistema(),
    )
    .await
    .map_err(anyhow::Error::msg)?;
    print!("{}", avaliacao::tabela(&a));
    let arq = saida.join("resultado.json");
    std::fs::write(&arq, serde_json::to_vec_pretty(&a)?)?;
    println!("\nresultado: {}", arq.display());
    Ok(())
}

pub async fn skill(args: &[String]) -> Result<()> {
    const USO: &str =
        "uso: phxclaw skill otimizar NOME --tarefas DIR [--modelo M] [--rodadas N] [--pasta DIR]";
    if args.first().map(String::as_str) != Some("otimizar") {
        bail!(USO);
    }
    let args = &args[1..];
    let nome = solto(args, &["--tarefas", "--modelo", "--rodadas", "--pasta"]).context(USO)?;
    let casos = ler_casos(Path::new(&opcao(args, "--tarefas").context(USO)?))
        .map_err(anyhow::Error::msg)?;
    let modelo = opcao(args, "--modelo").unwrap_or_else(|| MODELO_PADRAO.into());
    let rodadas: usize = opcao(args, "--rodadas")
        .map(|r| r.parse())
        .transpose()
        .context("--rodadas")?
        .unwrap_or(3);
    let base = pasta(args);
    let store = TaskStore::new(base.join("tasks"))?;
    let pasta_skills = phxclaw_agent::skills::pasta_do_ambiente(store.root())
        .root()
        .to_path_buf();
    let original = std::fs::read_to_string(pasta_skills.join(nome).join("SKILL.md"))
        .with_context(|| format!("skill {nome} em {}", pasta_skills.display()))?;
    let trabalho = base
        .join("otimizacoes")
        .join(format!("{nome}-{}", carimbo()));
    let m = Montagem::new(TaskStore::new(trabalho.join("tasks"))?);
    let llm = m.agent(&modelo).map_err(anyhow::Error::msg)?.llm;
    println!("gerando variante de {nome} com {modelo}...");
    let variante = match gerar_variante(llm.as_ref(), nome, &original).await {
        Ok(v) => v,
        Err(motivo) => {
            registrar_recusa(&pasta_skills, nome, &modelo, &motivo).map_err(anyhow::Error::msg)?;
            println!("NAO PROMOVIDA (sem A/B): {motivo}");
            println!(
                "registro: {}",
                pasta_skills.join(nome).join("otimizacao.jsonl").display()
            );
            return Ok(());
        }
    };
    let fabrica =
        |modelo: &str, dir: &Path| m.agent(modelo).and_then(|a| com_pasta_de_skills(a, dir));
    let p = PedidoOtimizacao {
        skill: nome,
        pasta_skills: &pasta_skills,
        modelo: &modelo,
        casos: &casos,
        rodadas,
        trabalho: &trabalho,
    };
    println!(
        "A/B: {} caso(s) x {rodadas} rodada(s) por braco...",
        casos.len()
    );
    let d = ab(&fabrica, &p, &variante)
        .await
        .map_err(anyhow::Error::msg)?;
    println!(
        "acerto por rodada: original [{:.2}–{:.2}] N={}, variante [{:.2}–{:.2}] N={} ({})",
        d.acerto_original.min,
        d.acerto_original.max,
        d.acerto_original.n,
        d.acerto_variante.min,
        d.acerto_variante.max,
        d.acerto_variante.n,
        d.data
    );
    println!(
        "{}: {}",
        if d.promovida {
            "PROMOVIDA"
        } else {
            "NAO PROMOVIDA"
        },
        d.motivo
    );
    println!(
        "registro: {}",
        pasta_skills.join(nome).join("otimizacao.jsonl").display()
    );
    Ok(())
}

/// `phxclaw ui fidelidade`: a prova de ida e volta da conversao de tela (SP000022).
pub async fn ui(args: &[String]) -> Result<()> {
    const USO: &str = "uso: phxclaw ui fidelidade [--telas N] [--modelo N] [--prazo S] [--saida DIR] [--capturas DIR]";
    if args.first().map(String::as_str) != Some("fidelidade") {
        bail!("{USO}");
    }
    let num = |k: &str, padrao: usize| -> Result<usize> {
        opcao(args, k)
            .map(|v| v.parse::<usize>())
            .transpose()
            .with_context(|| format!("{k}: numero"))
            .map(|v| v.unwrap_or(padrao))
    };
    let telas = num("--telas", phxclaw_agent::fidelidade_ui::TELAS)?;
    // o modelo leva ~90 s por tela: so quando pedido, e o comando diz quantas
    let com_modelo = num("--modelo", 0)?;
    let saida = opcao(args, "--saida")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/ui/fidelidade"));
    let capturas = opcao(args, "--capturas").map(PathBuf::from);
    // prazo por pergunta ao modelo: 300 s nao bastou com a maquina carregada (01/10: a
    // imagem 1280x1200 levou ~100 s so para decodificar)
    let prazo = num("--prazo", 300)?;
    let v = phxclaw_agent::fidelidade_ui::medir(
        telas,
        com_modelo,
        std::time::Duration::from_secs(prazo as u64),
        capturas.as_deref(),
    )
    .await
    .map_err(anyhow::Error::msg)?;
    print!("{}", phxclaw_agent::fidelidade_ui::tabela(&v));
    std::fs::create_dir_all(&saida)?;
    let dia = v["data"]
        .as_str()
        .unwrap_or("")
        .get(..10)
        .unwrap_or("")
        .to_string();
    let arq = saida.join(format!("fidelidade-{dia}.json"));
    std::fs::write(&arq, serde_json::to_vec_pretty(&v)?)?;
    println!("resultado: {}", arq.display());
    Ok(())
}

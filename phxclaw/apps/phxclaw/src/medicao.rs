//! Comandos de medicao: `repetir`, `medir`, `avaliar` e `skill otimizar`. A logica mora em
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

/// `phxclaw medir ARQ.jsonl`: as somas por tarefa de uma gravacao (SP000030). Tokens so
/// somam quando toda chamada ao modelo os informou; gravacao da versao 1 diz «nao medido».
pub fn medir(args: &[String]) -> Result<()> {
    let arq = solto(args, &[]).context("uso: phxclaw medir ARQ.jsonl [--json]")?;
    let g = Gravacao::ler(Path::new(arq)).map_err(anyhow::Error::msg)?;
    let somas = g.somas();
    if args.iter().any(|a| a == "--json") {
        println!("{}", serde_json::to_string_pretty(&somas)?);
        return Ok(());
    }
    println!("{}", texto_de_medir(&g, &somas));
    Ok(())
}

/// A tabela do `medir`, separada para a prova ler o mesmo texto que o terminal.
pub fn texto_de_medir(g: &Gravacao, somas: &[phxclaw_agent::gravacao::SomaDaTarefa]) -> String {
    let ms = |x: Option<f64>| match x {
        Some(v) => format!("{v:.0} ms"),
        None => "não medido (gravação v1)".to_string(),
    };
    let mut s = format!(
        "gravacao v{} de {} | modelo {} | prompt {} | {} skill(s)\n",
        g.versao,
        g.data,
        g.modelo,
        g.prompt_sha256
            .as_deref()
            .map(|h| h.chars().take(12).collect::<String>())
            .unwrap_or_else(|| "não gravado".into()),
        g.skills_sha256.len()
    );
    for t in somas {
        let tokens = match (t.tokens_entrada, t.tokens_saida) {
            (Some(e), Some(sa)) => format!("{e} entrada / {sa} saida"),
            _ => format!(
                "não informados pelo provedor ({} de {} chamada(s) sem contagem)",
                t.modelo_sem_tokens, t.chamadas_modelo
            ),
        };
        s.push_str(&format!(
            "\ntarefa {}{}\n  modelo      : {} chamada(s), {}\n  ferramentas : {} chamada(s), {}\n  tokens      : {}\n",
            t.tarefa.as_deref().unwrap_or("(sem id na gravacao)"),
            t.passo_pai
                .map(|p| format!("  (subagente dentro do passo {p})"))
                .unwrap_or_default(),
            t.chamadas_modelo,
            ms(t.duracao_modelo_ms),
            t.chamadas_ferramenta,
            ms(t.duracao_ferramentas_ms),
            tokens
        ));
    }
    s
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

/// Grava o resultado em `SAIDA/PREFIXO-DATA.json` sem apagar o do mesmo dia: o arquivo
/// anterior e a linha de base de quem compara antes e depois, e sobrescreve-lo pela segunda
/// corrida da tarde apagaria justamente o "antes".
fn gravar_resultado(saida: &Path, prefixo: &str, v: &serde_json::Value) -> Result<PathBuf> {
    std::fs::create_dir_all(saida)?;
    let data = v["data"].as_str().unwrap_or("");
    let dia = data.get(..10).unwrap_or("").to_string();
    let mut arq = saida.join(format!("{prefixo}-{dia}.json"));
    if arq.exists() {
        let hora: String = data.get(11..).unwrap_or("").replace(':', "");
        arq = saida.join(format!("{prefixo}-{dia}-{hora}.json"));
    }
    std::fs::write(&arq, serde_json::to_vec_pretty(v)?)?;
    Ok(arq)
}

/// `phxclaw ui fidelidade|responsivo`: as provas das telas geradas.
pub async fn ui(args: &[String]) -> Result<()> {
    const USO: &str = "uso: phxclaw ui fidelidade [--telas N] [--modelo N] [--prazo S] [--saida DIR] [--capturas DIR] \
                       | ui responsivo [--alvo html|bootstrap] [--bootstrap-css ARQ] [--telas N] [--saida DIR] [--capturas DIR] \
                       | ui responsivo --phx ARQ.phx.json [--bootstrap-css ARQ] [--exemplo INDEX.html] [--saida DIR] \
                       | ui importar ARQ.phx.json [--saida DIR] [--bootstrap-css CAMINHO]";
    match args.first().map(String::as_str) {
        Some("fidelidade") => {}
        Some("responsivo") => return ui_responsivo(args, USO).await,
        Some("importar") => return ui_importar(args, USO),
        _ => bail!("{USO}"),
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
    let arq = gravar_resultado(&saida, "fidelidade", &v)?;
    println!("resultado: {}", arq.display());
    Ok(())
}

/// `phxclaw ui importar ARQ.phx.json`: PHX JSON -> UI-IR -> telas phoenix e Bootstrap.
fn ui_importar(args: &[String], uso: &str) -> Result<()> {
    let Some(arq) = args.get(1).filter(|a| !a.starts_with("--")) else {
        bail!("{uso}");
    };
    let arq = PathBuf::from(arq);
    let saida = opcao(args, "--saida")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            arq.with_file_name(format!(
                "{}-gerado",
                arq.file_stem().and_then(|s| s.to_str()).unwrap_or("phx")
            ))
        });
    let r =
        phxclaw_agent::ui::importar_phx(&arq, &saida, opcao(args, "--bootstrap-css").as_deref())
            .map_err(anyhow::Error::msg)?;
    println!("{r}");
    Ok(())
}

/// `phxclaw ui responsivo`: as 20 telas do gabarito, ou o PHX JSON do dono (`--phx`).
async fn ui_responsivo(args: &[String], uso: &str) -> Result<()> {
    use phxclaw_agent::responsivo_ui::{self, Alvo};
    let saida = opcao(args, "--saida")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/ui/fidelidade"));
    if let Some(phx) = opcao(args, "--phx") {
        let texto = std::fs::read_to_string(&phx).with_context(|| phx.clone())?;
        let css = opcao(args, "--bootstrap-css")
            .map(|c| std::fs::read(&c).with_context(|| c.clone()))
            .transpose()?;
        let exemplo = opcao(args, "--exemplo")
            .map(|e| std::fs::read_to_string(&e).with_context(|| e.clone()))
            .transpose()?;
        let v = responsivo_ui::medir_phx(&texto, css, exemplo)
            .await
            .map_err(anyhow::Error::msg)?;
        print!("{}", responsivo_ui::tabela_phx(&v));
        let arq = gravar_resultado(&saida, "phx-exemplo", &v)?;
        println!("resultado: {}", arq.display());
        if v["passou"] != v["total"] {
            std::process::exit(1);
        }
        return Ok(());
    }
    let alvo = match opcao(args, "--alvo").as_deref().unwrap_or("html") {
        "html" => Alvo::Html,
        "bootstrap" => {
            // a prova nao baixa nada: a folha e um arquivo local, servido no loopback
            let Some(css) = opcao(args, "--bootstrap-css") else {
                bail!(
                    "--alvo bootstrap pede --bootstrap-css ARQ (o bootstrap.min.css local)\n{uso}"
                );
            };
            Alvo::Bootstrap {
                css: std::fs::read(&css).with_context(|| css.clone())?,
            }
        }
        outro => bail!("--alvo {outro}: use html ou bootstrap\n{uso}"),
    };
    let telas = opcao(args, "--telas")
        .map(|v| v.parse::<usize>())
        .transpose()
        .context("--telas: numero")?
        .unwrap_or(phxclaw_agent::fidelidade_ui::TELAS);
    let capturas = opcao(args, "--capturas").map(PathBuf::from);
    let v = responsivo_ui::medir(&alvo, telas, &[], capturas.as_deref())
        .await
        .map_err(anyhow::Error::msg)?;
    print!("{}", responsivo_ui::tabela(&v));
    let prefixo = format!("responsivo-{}", v["alvo"].as_str().unwrap_or("html"));
    let arq = gravar_resultado(&saida, &prefixo, &v)?;
    println!("resultado: {}", arq.display());
    Ok(())
}

//! A CLI da frente de interacao: responder pergunta e aprovar plano no terminal, buscar
//! nas sessoes, resumo do dia e estilos. Cada comando chama a MESMA funcao que a
//! ferramenta e a rota da API chamam; aqui so mora o que e de terminal.

use anyhow::{Result, bail};
use phxclaw_agent::{Task, TaskStatus, TaskStore};
use std::io::BufRead;
use std::sync::Mutex;

/// Ultima pergunta ja apresentada: o observador e chamado a cada gravacao, e a mesma
/// pergunta nao pode abrir duas leituras do stdin.
static PERGUNTA_VISTA: Mutex<Option<String>> = Mutex::new(None);

/// Chamado pelo observador do terminal. Tarefa esperando resposta: mostra a pergunta e le
/// UMA linha do stdin numa thread propria (o laco do agente continua dono do runtime), e
/// entrega por `perguntas::responder` -- o mesmo caminho da rota `/answer`.
pub fn ao_atualizar(t: &Task) {
    if t.status != TaskStatus::AwaitingInput {
        return;
    }
    let Some(q) = t.question.clone() else { return };
    let chave = format!("{}:{}:{q}", t.id, t.steps.len());
    {
        let mut v = PERGUNTA_VISTA.lock().unwrap_or_else(|p| p.into_inner());
        if v.as_deref() == Some(chave.as_str()) {
            return;
        }
        *v = Some(chave);
    }
    println!("\n  ? {q}");
    print!("  resposta> ");
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let id = t.id.clone();
    std::thread::spawn(move || {
        let mut linha = String::new();
        if std::io::stdin().lock().read_line(&mut linha).is_ok()
            && let Err(e) = phxclaw_agent::perguntas::responder(&id, &linha)
        {
            eprintln!("  resposta nao entregue: {e}");
        }
    });
}

/// O fluxo de aprovacao do Plan Mode no terminal: o plano so executa com um sim. `--sim`
/// aprova sem perguntar (roteiro), e o terminal sem resposta e um nao.
pub fn aprovar_plano(t: &Task, sem_perguntar: bool) -> bool {
    println!("plano (somente leitura ate aqui):");
    for (i, p) in t.plan.iter().enumerate() {
        println!("  {}. {p}", i + 1);
    }
    if sem_perguntar {
        return true;
    }
    print!("executar este plano? [s/N] ");
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let mut l = String::new();
    std::io::stdin().lock().read_line(&mut l).is_ok() && phxclaw_agent::perguntas::afirmativa(&l)
}

/// `phxclaw sessoes "termo" [--limite N]`
pub fn sessoes(store: &TaskStore, args: &[String]) -> Result<()> {
    let limite = super::opcao(args, "--limite")
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let consulta: Vec<&str> = args
        .iter()
        .enumerate()
        .filter(|(i, a)| !a.starts_with("--") && (*i == 0 || !args[i - 1].starts_with("--")))
        .map(|(_, a)| a.as_str())
        .collect();
    if consulta.is_empty() {
        bail!("uso: phxclaw sessoes \"termo\" [--limite N] [--pasta DIR]");
    }
    let achados = phxclaw_agent::sessoes::buscar(store, &consulta.join(" "), limite, None)
        .map_err(anyhow::Error::msg)?;
    if achados.is_empty() {
        println!("nenhuma tarefa anterior casa com a busca");
    }
    for a in achados {
        println!(
            "{} {} {:?} ({} pts)\n  {}\n  {}",
            a.id,
            a.quando.format("%Y-%m-%d %H:%M"),
            a.status,
            a.pontos,
            a.objetivo.replace('\n', " "),
            a.trecho
        );
    }
    Ok(())
}

/// `phxclaw resumo [--data AAAA-MM-DD|hoje|ontem]`
pub fn resumo(store: &TaskStore, args: &[String]) -> Result<()> {
    let dia = phxclaw_agent::sessoes::dia_do_argumento(super::opcao(args, "--data").as_deref())
        .map_err(anyhow::Error::msg)?;
    print!(
        "{}",
        phxclaw_agent::sessoes::resumo_do_dia(store, dia).map_err(anyhow::Error::msg)?
    );
    Ok(())
}

/// `phxclaw estilos`: os embutidos e os do projeto confiado, com a descricao -- a lista
/// mostra o que a montagem carregaria, nao o estilo que ela ignora.
pub fn estilos() {
    let agente = phxclaw_agent::config::pasta_padrao();
    let p = phxclaw_agent::montagem::pasta_confiada_para(&agente, "estilos");
    for e in phxclaw_agent::estilos::listar(p.as_deref()) {
        println!("{:<14} {}", e.nome, e.descricao);
    }
}

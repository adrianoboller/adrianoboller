//! O relatorio do conferidor de BOTOES: quantos a tela tem, quantos a bateria
//! clica, e quais ficaram sem prova.
//!
//! ```bash
//! cargo run --example botoes-sem-prova -p phxsql-server
//! cargo run --example botoes-sem-prova -p phxsql-server -- --dispensados
//! cargo run --example botoes-sem-prova -p phxsql-server -- --repetidos
//! cargo run --example botoes-sem-prova -p phxsql-server -- --numeros
//! ```
//!
//! A ordem do dono e «bateria de testes de todos os botoes». Este relatorio e
//! o que diz quanto disso ja e verdade -- medido, e nao estimado.

use phxsql_server::conferidor_botoes as cb;
use std::collections::BTreeMap;

fn main() {
    let todos = cb::conferir();
    let clicados = cb::exercitados();
    let provados = todos.iter().filter(|b| cb::provado(b, &clicados)).count();
    let dispensados = todos.iter().filter(|b| b.dispensa.is_some()).count();
    let faltam = cb::sem_prova();
    let sem_chave = todos.iter().filter(|b| b.chave.is_none()).count();
    let por_tipo = |t: cb::Tipo| todos.iter().filter(|b| b.tipo == Some(t)).count();
    let papel = todos.iter().filter(|b| b.forma == cb::Forma::Papel).count();

    println!("BOTOES DA TELA — quantos sao, e quantos a bateria exercita\n");
    println!("  botoes ................. {}", todos.len());
    println!("    marcacao `<button>` .. {}", todos.len() - papel);
    println!("    papel `role=button` .. {papel}");
    println!("  chave `#id` ............ {}", por_tipo(cb::Tipo::Id));
    println!("  chave `[data-*]` ....... {}", por_tipo(cb::Tipo::Dado));
    println!("  chave `.classe-gancho` . {}", por_tipo(cb::Tipo::Classe));
    println!("  SEM chave estavel ...... {sem_chave}");
    println!("  clicados pela bateria .. {provados}");
    println!("  dispensados com motivo . {dispensados}");
    println!("  SEM PROVA .............. {}", faltam.len());
    println!("  catraca (so desce) ..... {}", cb::TETO_BOTAO_SEM_PROVA);
    if clicados.is_empty() {
        println!(
            "\n  !! a evidencia `{}` nao existe ou esta vazia.\n     \
             Rode `node testes-web/bateria.mjs` INTEIRA -- ela a reescreve.",
            cb::EVIDENCIA
        );
    }

    // SAIDA DE MAQUINA -- mesmo motivo do irmao `grades-fora-do-padrao`:
    // gerador que le prosa publica o numero de ontem quando a prosa muda.
    if std::env::args().any(|a| a == "--numeros") {
        println!(
            "catraca:nome=TETO_BOTAO_SEM_PROVA;\
             onde=crates/phxsql-server/src/conferidor_botoes.rs;\
             valor={};medido={};mede=botoes da tela que a bateria nao clica",
            cb::TETO_BOTAO_SEM_PROVA,
            faltam.len()
        );
        println!(
            "botoes:total={};provados={};dispensados={};sem_chave={}",
            todos.len(),
            provados,
            dispensados,
            sem_chave
        );
        return;
    }

    if std::env::args().any(|a| a == "--dispensados") {
        println!("\nDISPENSADOS — quem nao se exercita, e por que\n");
        for (arq, alvo, porque) in cb::DISPENSADOS {
            println!("  {arq}  {alvo}\n      {porque}");
        }
        return;
    }

    // A chave repetida NAO e defeito -- e o mesmo botao em passos diferentes
    // de um assistente, e cada passo repinta o painel. Mas ela e um LIMITE
    // desta regua, e limite se declara: clicar `#azVolta` no passo 2 marca
    // como provadas as seis ocorrencias, inclusive as dos outros passos.
    if std::env::args().any(|a| a == "--repetidos") {
        let mut conta: BTreeMap<&str, Vec<&cb::Botao>> = BTreeMap::new();
        for b in &todos {
            if let Some(c) = &b.chave {
                conta.entry(c.as_str()).or_default().push(b);
            }
        }
        println!("\nCHAVES REPETIDAS — o limite declarado do cruzamento\n");
        for (chave, quais) in conta.iter().filter(|(_, v)| v.len() > 1) {
            println!("  {chave}  ({} ocorrencias)", quais.len());
            for b in quais {
                println!("      {}:{}  {}", b.arquivo, b.linha, b.funcao);
            }
        }
        return;
    }

    // Por arquivo e por funcao: e assim que se escolhe o proximo LOTE de
    // casos, que e para isso que o relatorio serve. Lote coerente por tela
    // vale mais que um caso por botao.
    let mut por_tela: BTreeMap<(&str, String), Vec<&cb::Botao>> = BTreeMap::new();
    for b in &faltam {
        por_tela
            .entry((b.arquivo, b.funcao.clone()))
            .or_default()
            .push(b);
    }
    let mut lotes: Vec<_> = por_tela.into_iter().collect();
    lotes.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));

    println!("\nSEM PROVA — por tela, do maior lote para o menor\n");
    for ((arq, funcao), quais) in &lotes {
        println!("{arq}  {funcao} — {}", quais.len());
        for b in quais {
            println!(
                "      linha {:<6} {:<28} {}",
                b.linha,
                b.chave.as_deref().unwrap_or("(SEM CHAVE)"),
                match b.forma {
                    cb::Forma::Marcacao => "<button>",
                    cb::Forma::Papel => "role=button",
                }
            );
        }
    }

    if faltam.is_empty() {
        println!("\nNenhum botao sem prova. A bateria clica todos.");
    }
}

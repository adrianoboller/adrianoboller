//! Paridade CLI x ferramenta: `phxclaw gonogo` e `go_no_go` chamam o MESMO `Conselho` do
//! `gonogo.rs` (abrir, registrar, ler, decidir). A prova: numa unica pasta, a CLI abre, a
//! ferramenta registra um parecer, a CLI registra o outro, e os dois lados leem a mesma
//! integracao com a mesma decisao e o mesmo codigo. Se um dos dois ganhar uma regra propria,
//! o estado diverge aqui.

use phxclaw_agent::gonogo::{Conselho, GoNoGoTool, cli, texto_da_vista, vista};
use phxclaw_agent_core::{Tool, ToolContext};
use serde_json::json;
use std::time::Duration;

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

/// A vista pela ferramenta. `async fn` e nao fechamento: um fechamento que devolve `async
/// move` capturando o `&GoNoGoTool` nao consegue nomear o lifetime do emprestimo.
async fn status(t: &GoNoGoTool, ctx: &ToolContext) -> serde_json::Value {
    let a = json!({"action": "status", "integration": "onda-7"});
    let o = t.run(a, ctx).await.unwrap();
    serde_json::from_str(&o.content).unwrap()
}

#[tokio::test]
async fn cli_e_ferramenta_leem_e_decidem_a_mesma_integracao() {
    let pasta = std::env::temp_dir().join(format!(
        "phxclaw-gonogo-paridade-{}",
        phxclaw_types::new_uuid_v7()
    ));
    std::fs::create_dir_all(&pasta).unwrap();
    let mut vazio = std::io::empty();

    // A CLI abre o conselho {a, b}; a ferramenta ve o mesmo estado: AGUARDAR (3).
    let (_, codigo) = cli(
        &args(&["abrir", "onda-7", "--integradores", "a,b"]),
        &pasta,
        &mut vazio,
    )
    .unwrap();
    assert_eq!(codigo, 3);
    let tool = GoNoGoTool {
        conselho: Conselho::da_pasta_do_agente(&pasta),
    };
    let ctx = ToolContext {
        task_id: "tarefa-a".into(),
        workdir: pasta.clone(),
        timeout: Duration::from_secs(5),
    };
    let aberta = Conselho::da_pasta_do_agente(&pasta)
        .ler("onda-7")
        .unwrap()
        .unwrap();
    assert_eq!(aberta.decidir().codigo(), 3);
    assert_eq!(status(&tool, &ctx).await, vista(&aberta));

    // A ferramenta registra a=OK; a CLI registra b=NOGO com erro. Mesmo arquivo.
    tool.run(
        json!({"action": "record", "integration": "onda-7", "integrator": "a", "verdict": "OK"}),
        &ctx,
    )
    .await
    .unwrap();
    let (texto_cli, codigo) = cli(
        &args(&["registrar", "onda-7", "b", "NOGO", "--erro", "teste falhou"]),
        &pasta,
        &mut vazio,
    )
    .unwrap();
    assert_eq!(codigo, 2, "{texto_cli}");

    // Os dois lados leem a mesma integracao: a vista da ferramenta e o texto da CLI saem da
    // mesma `Integracao`, e `decidir` da o mesmo codigo nos dois caminhos.
    let i = Conselho::da_pasta_do_agente(&pasta)
        .ler("onda-7")
        .unwrap()
        .unwrap();
    assert_eq!(status(&tool, &ctx).await, vista(&i));
    let (texto_ver, codigo_ver) = cli(&args(&["ver", "onda-7"]), &pasta, &mut vazio).unwrap();
    assert_eq!(texto_ver, texto_da_vista(&i));
    assert_eq!(codigo_ver, 2);
    let (decisao, codigo_decidir) = cli(&args(&["decidir", "onda-7"]), &pasta, &mut vazio).unwrap();
    assert_eq!(codigo_decidir, i.decidir().codigo());
    assert_eq!(decisao.trim(), i.decidir().nome());
    assert_eq!(
        vista(&i)["vigentes"].as_array().map(|v| v.len()),
        Some(2),
        "{}",
        vista(&i)
    );
    let _ = std::fs::remove_dir_all(&pasta);
}

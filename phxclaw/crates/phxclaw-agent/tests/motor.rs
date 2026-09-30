//! O motor com um LLM roteirizado: o que se prova e o laco, a politica, a evidencia e a
//! persistencia -- o modelo real entra em outro teste.

use phxclaw_agent::*;
use phxclaw_agent_core::Tool;
use phxclaw_evidence_ledger::EvidenceLedger;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

fn store() -> TaskStore {
    let d = std::env::temp_dir().join(format!("phx-motor-{}", phxclaw_types::new_uuid_v7()));
    TaskStore::new(d).unwrap()
}

fn shell() -> Option<Arc<dyn Tool>> {
    let bwrap = ["/usr/bin/bwrap", "/bin/bwrap"]
        .iter()
        .find(|p| std::path::Path::new(p).exists())?;
    Some(Arc::new(ShellTool {
        bwrap: bwrap.into(),
        network: false,
        timeout: Duration::from_secs(20),
    }))
}

fn tools_basicas() -> Vec<Arc<dyn Tool>> {
    let mut v: Vec<Arc<dyn Tool>> = vec![
        Arc::new(WriteFileTool),
        Arc::new(ReadFileTool),
        Arc::new(ListFilesTool),
    ];
    v.extend(shell());
    v
}

#[tokio::test]
async fn ciclo_completo_com_shell_real_arquivo_evidencia_e_resposta() {
    if shell().is_none() {
        eprintln!("bwrap ausente: pulado");
        return;
    }
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call(
            "c1",
            "shell",
            json!({"command": "python3 -c 'print(sum(range(101)))' > soma.txt && cat soma.txt"}),
        ),
        ScriptedLlm::call(
            "c2",
            "write_file",
            json!({"path": "relatorio.md", "content": "# Soma\n5050\n"}),
        ),
        ScriptedLlm::text("A soma de 0 a 100 e 5050; veja relatorio.md e soma.txt."),
    ]));
    let s = store();
    let a = Agent::new(
        llm.clone(),
        tools_basicas(),
        AgentConfig::default().grant(&["shell.exec", "fs.write", "fs.read"]),
        s.clone(),
    );
    let t = a
        .run(
            Task::new("some 0..100", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.error);
    assert!(t.answer.as_deref().unwrap().contains("5050"));
    // o shell rodou de verdade: o resultado dele voltou ao modelo
    let vistos = llm.seen.lock().unwrap();
    let resultado_shell = &vistos[1].0.last().unwrap().content;
    assert!(
        resultado_shell.contains("exit_code: 0") && resultado_shell.contains("5050"),
        "{resultado_shell}"
    );
    // artefatos com hash, e o arquivo existe na pasta da tarefa
    let caminhos: Vec<_> = t.artifacts.iter().map(|a| a.path.as_str()).collect();
    assert!(
        caminhos.contains(&"soma.txt") && caminhos.contains(&"relatorio.md"),
        "{caminhos:?}"
    );
    assert_eq!(
        std::fs::read_to_string(s.workdir(&t.id).join("soma.txt"))
            .unwrap()
            .trim(),
        "5050"
    );
    // evidencia: 2 chamadas, cadeia valida
    let rel = EvidenceLedger::open(s.evidence_path(&t.id))
        .unwrap()
        .verify()
        .unwrap();
    assert!(rel.valid && rel.records == 2, "{rel:?}");
    // estado persistido == devolvido
    assert_eq!(s.load(&t.id).unwrap(), t);
}

#[tokio::test]
async fn politica_esconde_e_nega_ferramenta_nao_concedida() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        // o modelo pede pelo nome uma ferramenta que nao lhe foi mostrada
        ScriptedLlm::call("c1", "shell", json!({"command": "rm -rf /work"})),
        ScriptedLlm::text("nao consegui"),
    ]));
    let s = store();
    let a = Agent::new(
        llm.clone(),
        tools_basicas(),
        AgentConfig::default().grant(&["fs.read"]),
        s.clone(),
    );
    let t = a
        .run(
            Task::new("x", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(t.status, TaskStatus::Completed);
    let vistos = llm.seen.lock().unwrap();
    assert!(
        !vistos[0].1.contains(&"shell".to_string()),
        "shell visivel: {:?}",
        vistos[0].1
    );
    assert!(vistos[1].0.last().unwrap().content.starts_with("NEGADO"));
    assert_eq!(
        t.steps
            .iter()
            .find(|p| p.tool.as_deref() == Some("shell"))
            .unwrap()
            .outcome,
        "negado"
    );
    let ev = EvidenceLedger::open(s.evidence_path(&t.id))
        .unwrap()
        .tail(5)
        .unwrap();
    assert_eq!(format!("{:?}", ev[0].outcome), "Denied");
}

#[tokio::test]
async fn teto_de_passos_termina_como_falha() {
    let passos: Vec<_> = (0..10)
        .map(|i| ScriptedLlm::call(&format!("c{i}"), "list_files", json!({})))
        .collect();
    let llm = Arc::new(ScriptedLlm::new(passos));
    let cfg = AgentConfig {
        max_steps: 3,
        ..AgentConfig::default()
    }
    .grant(&["fs.read"]);
    let a = Agent::new(llm, tools_basicas(), cfg, store());
    let t = a
        .run(
            Task::new("x", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(t.status, TaskStatus::Failed);
    assert!(t.error.unwrap().contains("3 passos"));
}

#[tokio::test]
async fn cancelamento_para_antes_do_proximo_passo() {
    let llm = Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text("nunca")]));
    let a = Agent::new(llm, vec![], AgentConfig::default(), store());
    let c = CancelFlag::default();
    c.cancel();
    let t = a.run(Task::new("x", "roteiro"), &c, &NoObserver).await;
    assert_eq!(t.status, TaskStatus::Cancelled);
}

#[tokio::test]
async fn caminho_para_fora_da_pasta_e_negado() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call(
            "c1",
            "write_file",
            json!({"path": "../../fuga.txt", "content": "x"}),
        ),
        ScriptedLlm::text("ok"),
    ]));
    let s = store();
    let a = Agent::new(
        llm.clone(),
        tools_basicas(),
        AgentConfig::default().grant(&["fs.write"]),
        s.clone(),
    );
    let t = a
        .run(
            Task::new("x", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(t.steps[0].outcome, "negado");
    assert!(!s.root().join("fuga.txt").exists());
}

#[tokio::test]
async fn subagentes_rodam_em_paralelo_e_ficam_ligados_a_mae() {
    // 1 chamada da mae + 3 respostas finais dos filhos + resposta final da mae
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call(
            "c1",
            "parallel_research",
            json!({"subtasks": ["a", "b", "c"]}),
        ),
        ScriptedLlm::text("resposta filho"),
        ScriptedLlm::text("resposta filho"),
        ScriptedLlm::text("resposta filho"),
        ScriptedLlm::text("consolidado"),
    ]));
    let s = store();
    let cfg = AgentConfig::default().grant(&["agent.spawn"]);
    let par: Arc<dyn Tool> = Arc::new(ParallelAgentsTool {
        llm: llm.clone(),
        tools: vec![],
        config: cfg.clone(),
        store: s.clone(),
        max_parallel: 5,
    });
    let a = Agent::new(llm, vec![par], cfg, s.clone());
    let t = a
        .run(
            Task::new("compare a b c", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.error);
    let filhos: Vec<_> = s
        .list()
        .unwrap()
        .into_iter()
        .filter(|x| x.parent.as_deref() == Some(t.id.as_str()))
        .collect();
    assert_eq!(filhos.len(), 3);
    assert!(filhos.iter().all(|f| f.status == TaskStatus::Completed));
}

#[tokio::test]
async fn ferramenta_lenta_vira_timeout_e_o_agente_segue() {
    if shell().is_none() {
        return;
    }
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call("c1", "shell", json!({"command": "sleep 5"})),
        ScriptedLlm::text("desisti do comando lento"),
    ]));
    let cfg = AgentConfig {
        tool_timeout: Duration::from_millis(300),
        ..AgentConfig::default()
    }
    .grant(&["shell.exec"]);
    let a = Agent::new(llm, tools_basicas(), cfg, store());
    let inicio = std::time::Instant::now();
    let t = a
        .run(
            Task::new("x", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert!(
        inicio.elapsed() < Duration::from_secs(4),
        "timeout nao cortou"
    );
    assert_eq!(t.steps[0].outcome, "erro");
    assert_eq!(t.status, TaskStatus::Completed);
}

#[tokio::test]
async fn terceira_chamada_identica_nao_roda() {
    let passos: Vec<_> = (0..4)
        .map(|i| ScriptedLlm::call(&format!("c{i}"), "list_files", json!({})))
        .chain([ScriptedLlm::text("fim")])
        .collect();
    let llm = Arc::new(ScriptedLlm::new(passos));
    let s = store();
    let a = Agent::new(
        llm.clone(),
        tools_basicas(),
        AgentConfig::default().grant(&["fs.read"]),
        s.clone(),
    );
    let t = a
        .run(
            Task::new("x", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    let saidas: Vec<_> = t
        .steps
        .iter()
        .filter(|p| p.tool.is_some())
        .map(|p| p.outcome.as_str())
        .collect();
    assert_eq!(saidas, vec!["ok", "ok", "repetida", "repetida"]);
    assert!(
        llm.seen.lock().unwrap()[3]
            .0
            .last()
            .unwrap()
            .content
            .starts_with("REPEATED CALL")
    );
    // a repetida nao virou evidencia de execucao: so as 2 que rodaram
    let rel = phxclaw_evidence_ledger::EvidenceLedger::open(s.evidence_path(&t.id))
        .unwrap()
        .verify()
        .unwrap();
    assert_eq!(rel.records, 2);
}

#[tokio::test]
async fn narrar_a_acao_nao_encerra_quando_o_fim_e_ferramenta() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::text("I will create relatorio.md now."),
        ScriptedLlm::call(
            "c1",
            "write_file",
            json!({"path": "relatorio.md", "content": "x"}),
        ),
        ScriptedLlm::call(
            "c2",
            "final_answer",
            json!({"answer": "relatorio.md criado"}),
        ),
    ]));
    let cfg = AgentConfig {
        require_final_tool: true,
        ..AgentConfig::default()
    }
    .grant(&["fs.write"]);
    let a = Agent::new(llm.clone(), tools_basicas(), cfg, store());
    let t = a
        .run(
            Task::new("x", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(t.status, TaskStatus::Completed);
    assert_eq!(t.answer.as_deref(), Some("relatorio.md criado"));
    assert_eq!(
        t.artifacts.len(),
        1,
        "a narracao encerrou antes de criar o arquivo"
    );
    assert!(
        llm.seen.lock().unwrap()[0]
            .1
            .contains(&"final_answer".to_string())
    );
}

#[tokio::test]
async fn final_answer_sem_o_arquivo_pedido_e_recusado() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call(
            "c0",
            "final_answer",
            json!({"answer": "rust.md has been created"}),
        ),
        ScriptedLlm::call(
            "c1",
            "write_file",
            json!({"path": "rust.md", "content": "1.98.1"}),
        ),
        ScriptedLlm::call("c2", "final_answer", json!({"answer": "rust.md criado"})),
    ]));
    let cfg = AgentConfig {
        require_final_tool: true,
        ..AgentConfig::default()
    }
    .grant(&["fs.write"]);
    let a = Agent::new(llm.clone(), tools_basicas(), cfg, store());
    let t = a
        .run(
            Task::new("write the version into rust.md", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(t.steps[0].outcome, "recusado");
    assert!(
        llm.seen.lock().unwrap()[1]
            .0
            .last()
            .unwrap()
            .content
            .contains("rust.md")
    );
    assert_eq!(t.answer.as_deref(), Some("rust.md criado"));
    assert_eq!(t.artifacts.len(), 1);
}

//! O motor com um LLM roteirizado: o que se prova e o laco, a politica, a evidencia e a
//! persistencia -- o modelo real entra em outro teste.

use phxclaw_agent::motor;
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

#[tokio::test]
async fn resposta_sem_o_arquivo_pedido_termina_falha_e_nao_concluida() {
    let llm = Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text(
        "Your website is live at [insert URL here]",
    )]));
    let a = Agent::new(
        llm,
        tools_basicas(),
        AgentConfig::default().grant(&["fs.write"]),
        store(),
    );
    let t = a
        .run(
            Task::new("write site/index.html and publish it", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(t.status, TaskStatus::Failed);
    assert!(t.error.unwrap().contains("site/index.html"));
}

// ------------------------------------------------------------------ fim conferido (SP000028)

fn office_e_arquivos() -> Vec<Arc<dyn Tool>> {
    let mut v = tools_basicas();
    v.extend(phxclaw_agent::adaptadores::office_tools());
    v
}

fn final_(resposta: serde_json::Value) -> phxclaw_agent_core::LlmReply {
    ScriptedLlm::call("f", "final_answer", json!({ "answer": resposta }))
}

async fn rodar(
    roteiro: Vec<phxclaw_agent_core::LlmReply>,
    cfg: AgentConfig,
    t: Task,
) -> (Task, Arc<ScriptedLlm>, TaskStore) {
    let llm = Arc::new(ScriptedLlm::new(roteiro));
    let s = store();
    let a = Agent::new(llm.clone(), office_e_arquivos(), cfg, s.clone());
    let fim = a.run(t, &CancelFlag::default(), &NoObserver).await;
    (fim, llm, s)
}

/// O caso real (qwen2.5:3b, 30/09, tarefa 01a0f2c9): `create_document` com `level` errado.
/// O erro volta com o CAMINHO do campo, o que veio e o esperado, e o modelo corrige; a
/// tabela com numero (`[[1, "Joao", ...]]`, a chamada medida que o serde recusava sem dizer
/// onde) agora passa.
#[tokio::test]
async fn create_document_devolve_o_caminho_do_campo_e_o_modelo_corrige() {
    let tabela = json!({"header":["ID","Nome","Email"],"rows":[[1,"João","joao@example.com"]],"type":"table"});
    let (t, llm, s) = rodar(
        vec![
            ScriptedLlm::call(
                "c1",
                "create_document",
                json!({"path":"erp/document.docx","blocks":[
                    {"level":"um","text":"ERP de Oficina","type":"heading"}, tabela.clone()]}),
            ),
            ScriptedLlm::call(
                "c2",
                "create_document",
                json!({"path":"erp/document.docx","blocks":[
                    {"level":1,"text":"ERP de Oficina","type":"heading"}, tabela]}),
            ),
            final_(json!("feito: erp/document.docx")),
        ],
        AgentConfig {
            require_final_tool: true,
            ..AgentConfig::default().grant(&["doc.write", "fs.write"])
        },
        Task::new("crie o documento", "roteiro"),
    )
    .await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.error);
    let p0 = &t.steps[0];
    assert_eq!(p0.outcome, motor::DESFECHO_INVALIDO, "{p0:?}");
    let ao_modelo = llm.seen.lock().unwrap()[1]
        .0
        .last()
        .unwrap()
        .content
        .clone();
    for trecho in [
        "\"field\":\"blocks[0].level\"",
        "\"got\":\"string \\\"um\\\"\"",
        "\"expected\":\"integer\"",
        "try again",
        "Retries left for create_document: 2",
    ] {
        assert!(ao_modelo.contains(trecho), "falta {trecho} em {ao_modelo}");
    }
    assert_eq!(t.steps[1].outcome, "ok", "{:?}", t.steps[1]);
    assert!(s.workdir(&t.id).join("erp/document.docx").exists());
}

/// Erro de argumento que o modelo larga para tras nao fecha `completed`: o fim e recusado
/// e devolvido ao modelo (o motivo nomeia a ferramenta), e esgotadas as recusas a tarefa
/// termina FALHA dizendo por que. Os dois caminhos de fim: `final_answer` e texto.
#[tokio::test]
async fn erro_de_argumento_sem_conserto_nao_fecha_completed() {
    let invalida = ScriptedLlm::call("c1", "create_document", json!({"answer":"x"}));
    let ok = ScriptedLlm::call("c0", "write_file", json!({"path":"a.txt","content":"a"}));
    let (t, llm, _) = rodar(
        vec![
            ok.clone(),
            invalida.clone(),
            final_(json!("pronto")),
            final_(json!("pronto")),
            final_(json!("pronto")),
            final_(json!("pronto")),
        ],
        AgentConfig {
            require_final_tool: true,
            ..AgentConfig::default().grant(&["doc.write", "fs.write"])
        },
        Task::new("x", "roteiro"),
    )
    .await;
    assert_eq!(t.status, TaskStatus::Failed);
    let e = t.error.clone().unwrap();
    assert!(
        e.contains("sem conserto") && e.contains("create_document"),
        "{e}"
    );
    let recusado = llm.seen.lock().unwrap()[3]
        .0
        .last()
        .unwrap()
        .content
        .clone();
    assert!(
        recusado.contains("never fixed: create_document"),
        "{recusado}"
    );

    // Fim em texto (sem `final_answer`): a mesma conferencia.
    let (t, _, _) = rodar(
        vec![
            ok,
            invalida,
            ScriptedLlm::text("pronto"),
            ScriptedLlm::text("pronto"),
            ScriptedLlm::text("pronto"),
            ScriptedLlm::text("pronto"),
        ],
        AgentConfig::default().grant(&["doc.write", "fs.write"]),
        Task::new("x", "roteiro"),
    )
    .await;
    assert_eq!(t.status, TaskStatus::Failed, "{:?}", t.steps);
    assert!(t.error.unwrap().contains("sem conserto"));
}

/// O orcamento: 2 novas tentativas SEGUIDAS por ferramenta; a que passa zera a conta.
#[tokio::test]
async fn orcamento_de_argumento_esgota_e_a_chamada_boa_zera() {
    // Caminhos diferentes: a mesma chamada pela 3a vez nem roda (`repetida`), e o teste
    // mediria o corte de repeticao em vez do orcamento.
    let ruim = |n: u32| {
        ScriptedLlm::call(
            "c",
            "create_document",
            json!({"path": format!("r{n}.docx")}),
        )
    };
    let boa = |n: u32| {
        ScriptedLlm::call(
            "b",
            "create_document",
            json!({"path": format!("d{n}.docx"), "blocks":[{"type":"paragraph","text":"oi"}]}),
        )
    };
    let cfg = || AgentConfig::default().grant(&["doc.write", "fs.write"]);
    // tres seguidas: a terceira passa do teto 2 e a tarefa para ali.
    let (t, _, _) = rodar(
        vec![ruim(1), ruim(2), ruim(3), ScriptedLlm::text("nunca chega")],
        cfg(),
        Task::new("x", "roteiro"),
    )
    .await;
    assert_eq!(t.status, TaskStatus::Failed);
    let e = t.error.unwrap();
    assert!(e.contains("orcamento de 2") && e.contains("blocks"), "{e}");
    // Duas invalidas, uma boa, duas invalidas, uma boa: nunca passa do teto.
    let (t, _, _) = rodar(
        vec![
            ruim(1),
            ruim(2),
            boa(1),
            ruim(3),
            ruim(4),
            boa(2),
            ScriptedLlm::text("ok"),
        ],
        cfg(),
        Task::new("x", "roteiro"),
    )
    .await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.error);
    // Orcamento 0: a primeira invalida ja encerra.
    let (t, _, _) = rodar(
        vec![ruim(1), ScriptedLlm::text("x")],
        AgentConfig {
            tentativas_de_argumento: 0,
            ..cfg()
        },
        Task::new("x", "roteiro"),
    )
    .await;
    assert_eq!(t.status, TaskStatus::Failed);
}

/// `--verificar`: codigo diferente de 0 recusa o fim e volta ao modelo; consertado, fecha.
/// Sempre falhando, a tarefa termina FALHA com o codigo. Sem `shell.exec`, nem comeca.
#[tokio::test]
async fn verificar_com_codigo_1_recusa_o_fim() {
    if shell().is_none() {
        eprintln!("bwrap ausente: pulado");
        return;
    }
    let cfg = || AgentConfig {
        require_final_tool: true,
        ..AgentConfig::default().grant(&["fs.write", "shell.exec"])
    };
    let tarefa = |cmd: &str| {
        let mut t = Task::new("x", "roteiro");
        t.verificar = Some(cmd.into());
        t
    };
    let escreve =
        |p: &str| ScriptedLlm::call("w", "write_file", json!({"path": p, "content": "1"}));
    let (t, llm, _) = rodar(
        vec![
            escreve("a.txt"),
            final_(json!("pronto")),
            escreve("pronto.txt"),
            final_(json!("pronto")),
        ],
        cfg(),
        tarefa("test -f pronto.txt"),
    )
    .await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.error);
    let recusa = llm.seen.lock().unwrap()[2]
        .0
        .last()
        .unwrap()
        .content
        .clone();
    assert!(recusa.contains("exited with 1"), "{recusa}");

    let (t, _, _) = rodar(
        vec![
            escreve("a.txt"),
            final_(json!("a")),
            final_(json!("a")),
            final_(json!("a")),
            final_(json!("a")),
        ],
        cfg(),
        tarefa("echo falta-o-teste >&2; exit 1"),
    )
    .await;
    assert_eq!(t.status, TaskStatus::Failed);
    let e = t.error.unwrap();
    assert!(
        e.contains("saiu com 1") && e.contains("falta-o-teste"),
        "{e}"
    );

    let (t, _, _) = rodar(
        vec![final_(json!("a"))],
        AgentConfig::default().grant(&["fs.write"]),
        tarefa("true"),
    )
    .await;
    assert_eq!(t.status, TaskStatus::Failed);
    assert!(t.error.unwrap().contains("shell.exec"));
}

/// `--saida-esquema`: o `final_answer` pede o objeto, o mesmo validador o confere, e o
/// texto que CONTEM o JSON (o que o modelo pequeno manda) e aceito.
#[tokio::test]
async fn saida_tipada_confere_a_resposta_final() {
    let mut tarefa = Task::new("some", "roteiro");
    tarefa.saida_esquema = Some(json!({"type":"object","properties":{
        "total":{"type":"integer"}},"required":["total"]}));
    let (t, llm, _) = rodar(
        vec![
            ScriptedLlm::call("w", "write_file", json!({"path":"a.txt","content":"1"})),
            final_(json!({"soma": 3})),
            final_(json!("Resultado:\n```json\n{\"total\": 3}\n```")),
        ],
        AgentConfig {
            require_final_tool: true,
            ..AgentConfig::default().grant(&["fs.write"])
        },
        tarefa,
    )
    .await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.error);
    assert_eq!(t.answer.as_deref(), Some("{\"total\":3}"));
    let recusa = llm.seen.lock().unwrap()[2]
        .0
        .last()
        .unwrap()
        .content
        .clone();
    assert!(
        recusa.contains("output schema") && recusa.contains("\"field\":\"total\""),
        "{recusa}"
    );
}

/// Linha-objeto na planilha (o formato medido do qwen2.5:1.5b): antes do portao virava
/// linha VAZIA e a planilha saia «criada» sem dado; agora as chaves viram o cabecalho e os
/// valores entram, e o portao aceita (o esquema diz `array` ou `object`).
#[tokio::test]
async fn planilha_com_linha_objeto_grava_os_valores() {
    let (t, llm, _) = rodar(
        vec![
            ScriptedLlm::call(
                "s",
                "create_spreadsheet",
                json!({"path":"estoque.xlsx","sheets":[{"name":"Itens","rows":[
                    {"Item":"Parafuso","Quantidade":"10"},{"Item":"Porca","Quantidade":20}]}]}),
            ),
            ScriptedLlm::call("r", "read_document", json!({"path":"estoque.xlsx"})),
            ScriptedLlm::text("estoque.xlsx"),
        ],
        AgentConfig::default().grant(&["doc.write", "fs.read"]),
        Task::new("x", "roteiro"),
    )
    .await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.steps);
    assert_eq!(t.steps[0].outcome, "ok", "{:?}", t.steps[0]);
    let lido = llm.seen.lock().unwrap()[2]
        .0
        .last()
        .unwrap()
        .content
        .clone();
    for trecho in ["Item", "Quantidade", "Parafuso", "Porca", "20"] {
        assert!(lido.contains(trecho), "falta {trecho} em {lido}");
    }
}

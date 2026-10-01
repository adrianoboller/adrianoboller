//! A equipe dos 110 papeis ativa no agente, com o LLM roteirizado do motor: o que se prova
//! e que delegar monta o prompt do papel, nunca da ao filho mais que o pai tem, recusa o
//! papel humano e respeita o roteamento de modelo da planilha.

use phxclaw_agent::equipe::{self, Delegacao, Equipe};
use phxclaw_agent::montagem::{Montagem, carregar_equipe};
use phxclaw_agent::*;
use phxclaw_agent_core::{Llm, Tool};
use serde_json::json;
use std::sync::Arc;

fn raiz() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn equipe() -> Arc<Equipe> {
    Arc::new(Equipe::carregar(raiz().join("config/agents")).unwrap())
}

fn store() -> TaskStore {
    let d = std::env::temp_dir().join(format!("phx-equipe-{}", phxclaw_types::new_uuid_v7()));
    TaskStore::new(d).unwrap()
}

fn basicas() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(WriteFileTool),
        Arc::new(ReadFileTool),
        Arc::new(ListFilesTool),
    ]
}

fn total_do_registro() -> u64 {
    // O total sai do registro dos papeis, nunca digitado: a equipe ja passou de 110 (o
    // Integrador entrou em 01/10/2026) e o numero cravado em teste envelhece calado.
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/agents/registry.index.json");
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(raiz).unwrap()).unwrap();
    v["count"].as_u64().unwrap()
}

#[test]
fn todos_carregam_e_cada_um_tem_ficha() {
    let e = equipe();
    let papeis = e.papeis();
    let total = total_do_registro() as usize;
    assert_eq!(e.len(), total);
    assert_eq!(papeis.len(), total);
    for m in &papeis {
        let f = e.ficha(m);
        assert_eq!(e.achar(&f.id.to_string()).unwrap().uuid, m.uuid);
        assert_eq!(e.achar(&f.uuid).unwrap().agent_id, f.id);
        assert!(!f.capability_principal.is_empty(), "{}", f.nome);
        assert!(!f.macroarea.is_empty() && !f.tipo.is_empty(), "{}", f.nome);
    }
    // A capability principal e a que distingue o papel, nao a de modulo que todos tem.
    let research = e.achar("Research Agent").unwrap();
    assert_eq!(e.capability_principal(research), "research.collect");
    assert_eq!(papeis.iter().filter(|m| equipe::e_humano(m)).count(), 1);
}

#[test]
fn achar_aceita_nome_sem_acento_e_recusa_ambiguo() {
    let e = equipe();
    assert_eq!(e.achar("Product Owner - Humano").unwrap().agent_id, 1);
    assert_eq!(e.achar("persefone").unwrap().name, "Perséfone");
    let ambiguo = e.achar("agent").unwrap_err();
    assert!(ambiguo.contains("diga o id"), "{ambiguo}");
    assert!(e.achar("999").is_err());
}

#[tokio::test]
async fn team_list_filtra_pela_mesma_funcao_da_cli() {
    let e = equipe();
    let t = equipe::TeamListTool { equipe: e.clone() };
    let ctx = phxclaw_agent_core::ToolContext {
        task_id: "t".into(),
        workdir: std::env::temp_dir(),
        timeout: std::time::Duration::from_secs(5),
    };
    let out = t
        .run(json!({"macroarea": "qualidade"}), &ctx)
        .await
        .unwrap();
    let esperado = e.listar(Some("Qualidade"), None);
    assert!(!esperado.is_empty() && (esperado.len() as u64) < total_do_registro());
    assert_eq!(
        out.content,
        equipe::texto_da_lista(&esperado, e.len()),
        "a ferramenta e a CLI tem de imprimir o mesmo"
    );
    let um = t.run(json!({"id": "8"}), &ctx).await.unwrap();
    assert!(um.content.contains("Johnson") && um.content.contains("missao:"));
}

/// O caso que esta frente existe para provar: o papel 8 (Johnson) autoriza escrever
/// arquivo (`code.implement`) e ler (`repo.read`); o pai tem leitura e delegacao, mas NAO
/// escrita. O filho tem de ver so a leitura -- nem a escrita que o pai nao tem, nem a
/// delegacao que o pai tem.
#[tokio::test]
async fn delegar_monta_o_prompt_do_papel_e_da_ao_filho_so_a_interseccao() {
    let e = equipe();
    let johnson = e.achar("8").unwrap();
    assert!(
        equipe::capacidades_do_papel(johnson).contains("fs.write"),
        "premissa: o papel autoriza escrita"
    );
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call(
            "c1",
            "team_delegate",
            json!({"role": "8", "task": "revise o modulo de juros"}),
        ),
        ScriptedLlm::text("revisado pelo papel"),
        ScriptedLlm::text("consolidado"),
    ]));
    let s = store();
    let cfg = AgentConfig::default().grant(&["fs.read", "memory.read", "team.delegate"]);
    let delegar: Arc<dyn Tool> = Arc::new(equipe::TeamDelegateTool {
        equipe: e.clone(),
        base: Agent::new(llm.clone(), basicas(), cfg.clone(), s.clone()),
        local: None,
    });
    let mut tools = basicas();
    tools.push(delegar);
    let pai = Agent::new(llm.clone(), tools, cfg, s.clone());
    let t = pai
        .run(
            Task::new("delegue a revisao", "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.error);

    let vistos = llm.seen.lock().unwrap().clone();
    assert_eq!(vistos.len(), 3, "pai, filho, pai");
    // O pai via a delegacao; o filho e a segunda chamada ao modelo.
    assert!(vistos[0].1.contains(&"team_delegate".to_string()));
    let (msgs_filho, tools_filho) = &vistos[1];
    let sistema = &msgs_filho[0].content;
    assert!(sistema.contains(&johnson.mission), "missao fora do prompt");
    assert!(sistema.contains("Limites (nao pode)"));
    assert_eq!(msgs_filho[1].content, "revise o modulo de juros");
    let mut tools_filho = tools_filho.clone();
    tools_filho.sort();
    assert_eq!(
        tools_filho,
        vec!["list_files".to_string(), "read_file".to_string()],
        "o filho ganhou ferramenta fora da interseccao papel x pai"
    );

    let filhos: Vec<Task> = s
        .list()
        .unwrap()
        .into_iter()
        .filter(|x| x.parent.as_deref() == Some(t.id.as_str()))
        .collect();
    assert_eq!(filhos.len(), 1);
    assert_eq!(filhos[0].objective, "revise o modulo de juros");
    assert_eq!(filhos[0].answer.as_deref(), Some("revisado pelo papel"));
    let resultado = t
        .steps
        .iter()
        .find(|p| p.tool.as_deref() == Some("team_delegate"))
        .unwrap();
    assert_eq!(resultado.outcome, "ok");
    assert!(resultado.summary.contains("modelo do agente pai"));
}

#[tokio::test]
async fn papel_humano_volta_pedindo_decisao_e_nada_roda() {
    let e = equipe();
    let llm = Arc::new(ScriptedLlm::new(vec![]));
    let s = store();
    let base = Agent::new(
        llm.clone(),
        basicas(),
        AgentConfig::default().grant(&["fs.read", "fs.write"]),
        s.clone(),
    );
    let d = equipe::delegar(&e, &base, "1", "aprovar a release 0.70", None, None)
        .await
        .unwrap();
    assert!(matches!(d, Delegacao::Humano { .. }));
    assert!(equipe::texto_da_delegacao(&d).starts_with("DECISAO HUMANA NECESSARIA"));
    assert!(llm.seen.lock().unwrap().is_empty(), "o modelo foi chamado");
    assert!(
        s.list().unwrap().is_empty(),
        "nasceu tarefa para papel humano"
    );
}

#[tokio::test]
async fn papel_roteado_para_ollama_usa_o_local_e_os_outros_o_do_pai() {
    let e = equipe();
    // 18 Documentador: so "ollama" na planilha. 8 Johnson: so "codex".
    assert!(equipe::prefere_local(e.achar("18").unwrap()));
    assert!(!equipe::prefere_local(e.achar("8").unwrap()));
    let pai = Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text("do pai")]));
    let local = Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text("do local")]));
    let local_dyn: Arc<dyn Llm> = local.clone();
    let base = Agent::new(
        pai.clone(),
        basicas(),
        AgentConfig::default().grant(&["fs.read"]),
        store(),
    );
    let d = equipe::delegar(&e, &base, "18", "documente", None, Some(&local_dyn))
        .await
        .unwrap();
    let Delegacao::Rodou {
        tarefa,
        motivo_modelo,
        ..
    } = &d
    else {
        panic!("papel 18 nao e humano")
    };
    assert_eq!(tarefa.answer.as_deref(), Some("do local"));
    assert!(motivo_modelo.contains(equipe::VAR_MODELO_LOCAL));
    assert_eq!(local.seen.lock().unwrap().len(), 1);
    assert!(pai.seen.lock().unwrap().is_empty());

    let d = equipe::delegar(&e, &base, "8", "implemente", None, Some(&local_dyn))
        .await
        .unwrap();
    let Delegacao::Rodou { tarefa, .. } = &d else {
        panic!()
    };
    assert_eq!(tarefa.answer.as_deref(), Some("do pai"));
    assert_eq!(
        local.seen.lock().unwrap().len(),
        1,
        "codex nao vai ao local"
    );
}

#[test]
fn catalogo_invalido_vira_agente_sem_equipe() {
    let d = std::env::temp_dir().join(format!("phx-equipe-ruim-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("001-x.agent.json"), "{ nao e json").unwrap();
    assert!(carregar_equipe(Equipe::carregar(&d)).is_none());
    assert!(carregar_equipe(Equipe::carregar(d.join("nao-existe"))).is_none());

    let nomes = |m: &Montagem| -> Vec<String> {
        let a = m.agent_with(Arc::new(ScriptedLlm::new(vec![])));
        a.visible_specs().into_iter().map(|s| s.name).collect()
    };
    let mut m = Montagem::new(store());
    m.equipe = None;
    let sem = nomes(&m);
    assert!(!sem.iter().any(|n| n.starts_with("team_")));
    m.equipe = Some(equipe());
    let com = nomes(&m);
    assert!(com.contains(&"team_list".to_string()));
    assert!(com.contains(&"team_delegate".to_string()));
}

/// O arquivo da interface e gerado; catalogo mudado sem regerar reprova aqui.
#[test]
fn equipe_json_da_interface_esta_em_dia() {
    let e = equipe();
    let arquivo = std::fs::read_to_string(raiz().join(equipe::ARQUIVO_DA_INTERFACE))
        .expect("falta o arquivo: cargo run -p phxclaw-agent --example equipe_json");
    assert!(
        arquivo == e.arquivo_da_interface(),
        "equipe.json desatualizado: cargo run -p phxclaw-agent --example equipe_json"
    );
    let v: serde_json::Value = serde_json::from_str(&arquivo).unwrap();
    assert_eq!(v["total"], total_do_registro());
    let soma: u64 = v["macroareas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["papeis"].as_array().unwrap().len() as u64)
        .sum();
    assert_eq!(soma, total_do_registro());
}

/// Papel que declarasse o poder de delegar nao o leva ao filho, mesmo com o pai tendo.
#[test]
fn subagente_nunca_recebe_o_poder_de_delegar() {
    let e = equipe();
    let mut m = e.achar("8").unwrap().clone();
    m.capabilities
        .extend(["agent.spawn", "team.delegate", "team.read"].map(String::from));
    let pai: std::collections::BTreeSet<String> =
        ["agent.spawn", "team.delegate", "team.read", "fs.read"]
            .map(String::from)
            .into();
    let caps = equipe::capacidades_do_subagente(&m, &pai);
    assert_eq!(caps.into_iter().collect::<Vec<_>>(), vec!["fs.read"]);
}

/// O Integrador decide Go/NoGo: le, roda portoes e revisa, mas nao grava no repositorio
/// (quem comita depois do Go e o Versionador).
#[test]
fn integrador_decide_sem_poder_escrever() {
    let e = equipe();
    let m = e
        .papeis()
        .into_iter()
        .find(|m| m.name == "Integrador")
        .expect("papel Integrador no catalogo");
    let caps = equipe::capacidades_do_papel(m);
    for proibida in ["fs.write", "doc.write", "git.write"] {
        assert!(
            !caps.contains(proibida),
            "Integrador ganhou {proibida}: {caps:?}"
        );
    }
    for exigida in ["fs.read", "shell.exec", "git.read", "code.review"] {
        assert!(caps.contains(exigida), "Integrador sem {exigida}: {caps:?}");
    }
    assert_eq!(e.capability_principal(m), "release.go_no_go.decide");
}

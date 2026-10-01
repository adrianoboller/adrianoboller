//! Memoria entre tarefas e skills, pela montagem de verdade: o que se prova e que uma
//! tarefa grava e a SEGUINTE recebe sem pedir, e que a skill da pasta chega ao prompt so
//! com nome e descricao e o corpo entra pela ferramenta.

use phxclaw_agent::montagem::{CAPACIDADES_PADRAO, Montagem};
use phxclaw_agent::*;
use serde_json::json;
use std::sync::Arc;

fn store() -> TaskStore {
    let d = std::env::temp_dir().join(format!("phx-memsk-{}", phxclaw_types::new_uuid_v7()));
    TaskStore::new(d).unwrap()
}

async fn rodar(
    m: &Montagem,
    objetivo: &str,
    roteiro: Vec<phxclaw_agent_core::LlmReply>,
) -> (Task, Arc<ScriptedLlm>) {
    let llm = Arc::new(ScriptedLlm::new(roteiro));
    let t = m
        .agent_with(llm.clone())
        .run(
            Task::new(objetivo, "roteiro"),
            &CancelFlag::default(),
            &NoObserver,
        )
        .await;
    (t, llm)
}

fn fim(id: &str) -> phxclaw_agent_core::LlmReply {
    ScriptedLlm::call(id, "final_answer", json!({"answer": "feito"}))
}

#[tokio::test]
async fn a_segunda_tarefa_recebe_a_memoria_da_primeira_sem_pedir_e_tarjada() {
    for c in ["memory.read", "memory.write", "skill.read"] {
        assert!(CAPACIDADES_PADRAO.contains(&c), "{c} fora do padrao");
    }
    let s = store();
    let m = Montagem::new(s.clone());
    let (t1, _) = rodar(
        &m,
        "anote a preferencia do cliente",
        vec![
            ScriptedLlm::call(
                "c1",
                "memory_save",
                json!({"text": "O cliente Acme quer relatorios sempre em PDF; a chave dele e sk-acme-0123456789abcdef"}),
            ),
            fim("c2"),
        ],
    )
    .await;
    assert_eq!(t1.status, TaskStatus::Completed, "{:?}", t1.error);
    assert_eq!(t1.steps[0].outcome, "ok", "{:?}", t1.steps[0]);
    // O arquivo mora no diretorio do store, fora da pasta descartavel da tarefa, e sem a chave.
    let arquivo = s.root().join("_memoria/padrao.json");
    let disco = std::fs::read_to_string(&arquivo).unwrap();
    assert!(
        !disco.contains("0123456789abcdef"),
        "chave gravada: {disco}"
    );
    assert!(!s.workdir(&t1.id).join("_memoria").exists());

    // Segunda tarefa: nenhuma ferramenta de memoria chamada, e o contexto ja traz a nota.
    let (_t2, llm2) = rodar(
        &m,
        "faca o relatorio mensal da Acme",
        vec![ScriptedLlm::call("d1", "list_files", json!({})), fim("d2")],
    )
    .await;
    let sistema = llm2.seen.lock().unwrap()[0].0[0].content.clone();
    assert!(
        sistema.contains("Memories saved by previous tasks")
            && sistema.contains("relatorios sempre em PDF")
            && sistema.contains("[saved 20"),
        "memoria nao injetada: {sistema}"
    );
    assert!(sistema.contains("[REDACTED]") && !sistema.contains("0123456789abcdef"));

    // Objetivo sem nada a ver nao recebe a nota.
    let (_t3, llm3) = rodar(
        &m,
        "converta o video para mp3",
        vec![fim("e1"), fim("e2"), fim("e3")],
    )
    .await;
    assert!(!llm3.seen.lock().unwrap()[0].0[0].content.contains("Acme"));

    // Sem `memory.read` concedida, nada se injeta: o portao vale para o motor tambem.
    let mut sem_leitura = Montagem::new(s.clone());
    sem_leitura.capabilities.retain(|c| c != "memory.read");
    let (_t4, llm4) = rodar(
        &sem_leitura,
        "relatorio da Acme",
        vec![fim("f1"), fim("f2"), fim("f3")],
    )
    .await;
    assert!(!llm4.seen.lock().unwrap()[0].0[0].content.contains("Acme"));
    let _ = std::fs::remove_dir_all(s.root());
}

#[tokio::test]
async fn skill_da_pasta_vai_ao_prompt_so_com_nome_e_o_corpo_vem_pela_ferramenta() {
    let s = store();
    let pasta = s.root().join("_skills/relatorio-pdf");
    std::fs::create_dir_all(&pasta).unwrap();
    std::fs::write(
        pasta.join("SKILL.md"),
        "---\nname: relatorio-pdf\ndescription: Como montar o relatorio mensal em PDF\n---\n\n\
1. Busque os numeros.\n2. CORPO-SECRETO-DA-SKILL: gere o PDF com sumario.\n",
    )
    .unwrap();
    // Uma skill fora da pasta, que o `../` tentaria alcancar.
    let fora = s.root().join("fora");
    std::fs::create_dir_all(&fora).unwrap();
    std::fs::write(
        fora.join("SKILL.md"),
        "---\nname: fora\ndescription: x\n---\nVAZOU",
    )
    .unwrap();

    let m = Montagem::new(s.clone());
    let (t, llm) = rodar(
        &m,
        "relatorio mensal",
        vec![
            ScriptedLlm::call("c1", "skill_load", json!({"name": "relatorio-pdf"})),
            ScriptedLlm::call("c2", "skill_load", json!({"name": "../fora"})),
            ScriptedLlm::call(
                "c3",
                "skill_load",
                json!({"name": "../_skills/relatorio-pdf"}),
            ),
            fim("c4"),
        ],
    )
    .await;
    assert_eq!(t.status, TaskStatus::Completed, "{:?}", t.error);
    let vistos = llm.seen.lock().unwrap();
    let sistema = &vistos[0].0[0].content;
    assert!(
        sistema.contains("- relatorio-pdf: Como montar o relatorio mensal em PDF"),
        "skill fora do prompt: {sistema}"
    );
    assert!(
        !sistema.contains("CORPO-SECRETO-DA-SKILL"),
        "o corpo nao vai no prompt"
    );
    assert!(vistos[0].1.contains(&"skill_load".to_string()));
    let corpo = &vistos[1].0.last().unwrap().content;
    assert!(corpo.contains("CORPO-SECRETO-DA-SKILL"), "{corpo}");
    for i in [2, 3] {
        let r = &vistos[i].0.last().unwrap().content;
        assert!(r.starts_with("NEGADO") && !r.contains("VAZOU"), "{r}");
    }
    let outcomes: Vec<_> = t.steps.iter().map(|p| p.outcome.as_str()).collect();
    assert_eq!(&outcomes[..3], ["ok", "negado", "negado"]);
    let _ = std::fs::remove_dir_all(s.root());
}

//! A porta irma da varredura de segredos: com `git.segredos.exigir` ligado, o portao do
//! motor recusa a linha de shell que grava historia no git, apontando o `git_write`.
//!
//! Arquivo proprio porque a configuracao e lida UMA vez por processo: o ambiente vai antes
//! da primeira leitura, e nenhum outro teste deste binario a faz antes.

use phxclaw_agent::ferramentas::ShellTool;
use phxclaw_agent::*;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn shell_nao_grava_no_git_quando_a_varredura_e_exigida() {
    let d = std::env::temp_dir().join(format!(
        "phx-segredo-shell-{}",
        phxclaw_types::new_uuid_v7()
    ));
    std::fs::create_dir_all(&d).unwrap();
    // SAFETY: unico teste deste binario, antes de qualquer leitura de configuracao.
    unsafe {
        std::env::set_var("PHXCLAW_HOME", d.join("agente"));
        std::env::set_var("PHXCLAW_PROJETO", &d);
        std::env::set_var("PHXCLAW_GITLEAKS_EXIGIR", "1");
    }
    let shell = Arc::new(ShellTool {
        // Sem bwrap de verdade: o que importa e o portao, que decide ANTES de rodar.
        bwrap: d.join("bwrap-inexistente"),
        network: false,
        timeout: Duration::from_secs(5),
    });
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call(
            "c1",
            "shell",
            json!({"command": "git add -A && git commit -m x"}),
        ),
        ScriptedLlm::call(
            "c2",
            "shell",
            json!({"command": "git -c a=b stash push -u"}),
        ),
        ScriptedLlm::call("c3", "shell", json!({"command": "git status"})),
        ScriptedLlm::text("fim"),
    ]));
    let ag = Agent::new(
        llm,
        vec![shell],
        AgentConfig::default().grant(&["shell.exec"]),
        TaskStore::new(d.join("tarefas")).unwrap(),
    );
    let t = ag
        .run(Task::new("x", "m"), &CancelFlag::default(), &NoObserver)
        .await;
    for i in 0..2 {
        assert_eq!(t.steps[i].outcome, "negado", "{:?}", t.steps[i]);
        assert!(t.steps[i].summary.contains("git_write"), "{:?}", t.steps[i]);
    }
    assert_ne!(t.steps[2].outcome, "negado", "{:?}", t.steps[2]);
    let _ = std::fs::remove_dir_all(&d);
}

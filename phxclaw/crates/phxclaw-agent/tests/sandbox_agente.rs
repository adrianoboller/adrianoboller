//! A pasta do agente dentro do projeto nao aparece em processo nenhum do sandbox.
//!
//! O layout padrao poe a pasta do agente (`var/agente`: chave-mestra do broker, cofre,
//! `api.token`) DENTRO do projeto, e o projeto e o `/work` do terminal do IDE, do explorador
//! de testes, dos servidores de linguagem e do tunel. O hotfix do minimapa fechou a porta de
//! disco (`confine`); estas provas fecham a de processo: toda montagem do bwrap que contem a
//! pasta do agente a ve vazia e so leitura, e a pasta de trabalho da propria tarefa
//! (`<agente>/tasks/<id>/work`) continua sendo dela.
//!
//! O broker e criado de verdade (a chave-mestra nasce no primeiro uso), como na prova do
//! minimapa (`ide_web.rs`, `o_minimapa_nao_serve_a_pasta_do_agente_nem_o_cofre`).

use phxclaw_agent::arquivos::achar_bwrap;
use phxclaw_sandbox::{SandboxExtras, WorkdirCommand, run_in_workdir_com};
use phxclaw_test_support::pulado;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

/// As provas mexem em `PHXCLAW_HOME` (ambiente do processo): uma de cada vez.
static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

/// O projeto com o layout padrao e o broker de verdade dentro dele. Solta a variavel e
/// apaga a pasta ao sair, inclusive no panico.
struct Projeto {
    raiz: PathBuf,
    mestra: String,
    envelope: String,
}
impl Drop for Projeto {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("PHXCLAW_HOME");
            std::env::remove_var("PHXCLAW_PROJETO");
        }
        let _ = std::fs::remove_dir_all(&self.raiz);
    }
}

fn projeto(nome: &str) -> Projeto {
    let raiz = std::env::temp_dir().join(format!("phx-sbx-ag-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(raiz.join("src")).unwrap();
    std::fs::write(raiz.join("src/main.rs"), "fn soma() {}\n").unwrap();
    let agente = raiz.join("var/agente");
    std::fs::create_dir_all(&agente).unwrap();
    unsafe {
        std::env::set_var("PHXCLAW_HOME", &agente);
        std::env::set_var("PHXCLAW_PROJETO", &raiz);
    }
    let broker = phxclaw_agent::canais::broker_em(&agente).unwrap();
    phxclaw_agent::canais::guardar_segredo(
        &broker,
        "prova",
        "ide",
        &["channel:ide:send"],
        phxclaw_secret_broker::SecretValue::new("valor-do-cofre-que-nao-sai".into()),
    )
    .unwrap();
    let mestra = std::fs::read_to_string(agente.join("segredos/master.key")).unwrap();
    assert!(mestra.trim().len() >= 40, "{mestra}");
    let envelope = std::fs::read_dir(agente.join("segredos/cofre"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "phxsecret"))
        .expect("o cofre tem um envelope");
    let envelope = std::fs::read_to_string(envelope).unwrap();
    std::fs::write(agente.join("api.token"), "bearer-da-pasta-do-agente\n").unwrap();
    Projeto {
        raiz,
        mestra: mestra.trim().to_string(),
        envelope: envelope.trim().to_string(),
    }
}

impl Projeto {
    fn agente(&self) -> PathBuf {
        std::fs::canonicalize(self.raiz.join("var/agente")).unwrap()
    }
    /// O que nao pode aparecer na saida de processo nenhum.
    fn vazou(&self, saida: &str) -> Vec<&'static str> {
        let mut v = vec![];
        if saida.contains(&self.mestra) {
            v.push("chave-mestra");
        }
        if saida.contains(&self.envelope) || saida.contains("phxsecret") {
            v.push("cofre");
        }
        if saida.contains("bearer-da-pasta-do-agente") {
            v.push("api.token");
        }
        // Cada listagem sai entre `LISTA[...]`: a pasta mascarada lista vazio.
        if saida.match_indices("LISTA[").count() != saida.match_indices("LISTA[]").count() {
            v.push("listagem da pasta do agente");
        }
        if !saida.contains("LISTA[") {
            v.push("nenhuma listagem: a prova nao rodou");
        }
        v
    }
}

/// O script que tenta tudo: relativo, absoluto (pela raiz montada no proprio caminho),
/// listagem e gravacao. O comum do projeto tem de continuar visivel.
fn script_que_tenta(agente: &Path) -> String {
    let a = agente.display();
    format!(
        "cat var/agente/segredos/master.key; cat var/agente/api.token; \
         echo \"LISTA[$(ls -A var/agente 2>&1)]\"; cat {a}/segredos/master.key; \
         echo \"LISTA[$(ls -A {a} 2>&1)]\"; cat var/agente/segredos/cofre/*; \
         (echo x > var/agente/gravado) 2>/dev/null || echo GRAVACAO-RECUSADA; \
         cat src/main.rs"
    )
}

fn sem_bwrap() -> Option<PathBuf> {
    let b = achar_bwrap();
    if b.is_none() {
        pulado::pular("bwrap", "bwrap ausente; a mascara so existe no sandbox");
    }
    b
}

/// (a) O caminho do `run_in_workdir_com` -- explorador de testes, git, hooks, tunel, Python
/// -- e o do `espec_no_bwrap` -- servidores de linguagem e MCP: com o projeto em `/work` e
/// a raiz do projeto montada no proprio caminho (o que o `lsp.rs` faz com as raizes do
/// workspace), a pasta do agente aparece vazia e so leitura.
#[test]
fn o_bwrap_nao_mostra_a_pasta_do_agente_que_mora_no_projeto() {
    let _s = UM_DE_CADA_VEZ.lock().unwrap_or_else(|p| p.into_inner());
    let Some(bwrap) = sem_bwrap() else { return };
    let p = projeto("a");
    let agente = p.agente();
    let raiz = std::fs::canonicalize(&p.raiz).unwrap();
    let extras = SandboxExtras {
        ro_binds: vec![(raiz.clone(), raiz.display().to_string())],
        env: vec![],
    };
    let cmd = WorkdirCommand {
        workdir: p.raiz.clone(),
        script: script_que_tenta(&agente),
        timeout: Duration::from_secs(30),
        network: false,
        max_output_bytes: 1 << 20,
    };

    let r = run_in_workdir_com(&bwrap, &cmd, &extras).unwrap();
    let saida = format!("{}\n{}", r.stdout, r.stderr);
    assert!(
        p.vazou(&saida).is_empty(),
        "{:?}:\n{saida}",
        p.vazou(&saida)
    );
    assert!(
        r.stdout.contains("fn soma"),
        "o projeto sumiu junto:\n{saida}"
    );
    assert!(r.stdout.contains("GRAVACAO-RECUSADA"), "{saida}");
    assert!(!agente.join("gravado").exists());

    let (spec, _) = phxclaw_agent::processo::espec_no_bwrap(&cmd, &extras).unwrap();
    let o = std::process::Command::new(&spec.executable)
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .env_clear()
        .envs(&spec.env)
        .output()
        .unwrap();
    let saida = format!(
        "{}\n{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(
        p.vazou(&saida).is_empty(),
        "{:?}:\n{saida}",
        p.vazou(&saida)
    );
    assert!(saida.contains("fn soma"), "{saida}");
}

/// (a) A excecao: a pasta de trabalho da tarefa mora dentro da do agente e continua sendo
/// a `/work` dela, gravavel -- a mesma excecao do `confine`. E a pasta do agente inteira
/// como pasta de trabalho NAO ganha a excecao: aparece vazia.
#[test]
fn a_pasta_da_tarefa_dentro_da_do_agente_continua_gravavel() {
    let _s = UM_DE_CADA_VEZ.lock().unwrap_or_else(|p| p.into_inner());
    let Some(bwrap) = sem_bwrap() else { return };
    let p = projeto("tarefa");
    let work = p.raiz.join("var/agente/tasks/t1/work");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("entrada.txt"), "da-tarefa\n").unwrap();
    let cmd = |workdir: PathBuf, script: &str| WorkdirCommand {
        workdir,
        script: script.into(),
        timeout: Duration::from_secs(30),
        network: false,
        max_output_bytes: 1 << 20,
    };
    let r = run_in_workdir_com(
        &bwrap,
        &cmd(
            work.clone(),
            "cat entrada.txt && echo gravou > saida.txt && cat saida.txt",
        ),
        &SandboxExtras::default(),
    )
    .unwrap();
    assert_eq!(r.exit_code, Some(0), "{r:?}");
    assert!(
        r.stdout.contains("da-tarefa") && r.stdout.contains("gravou"),
        "{r:?}"
    );
    assert_eq!(
        std::fs::read_to_string(work.join("saida.txt")).unwrap(),
        "gravou\n"
    );

    let r = run_in_workdir_com(
        &bwrap,
        &cmd(
            p.raiz.join("var/agente"),
            "echo \"LISTA[$(ls -A)]\"; cat segredos/master.key",
        ),
        &SandboxExtras::default(),
    )
    .unwrap();
    let saida = format!("{}\n{}", r.stdout, r.stderr);
    assert!(
        p.vazou(&saida).is_empty(),
        "{:?}:\n{saida}",
        p.vazou(&saida)
    );
}

/// (b) O terminal do IDE: o Helix sobe pelo bwrap (nunca direto), sem `--new-session` (que
/// mata o redimensionar), com o projeto em `/work` e a pasta do agente mascarada; o
/// ambiente herdado do agente nao passa, e o Bearer da completacao vai no ambiente, nunca
/// no argv. Roda-se o programa montado com `cat`/`ls`/`env` no lugar do hx, herdando o
/// ambiente do pai como o PTY herda.
#[test]
fn o_terminal_do_ide_roda_no_bwrap_com_a_pasta_do_agente_mascarada() {
    let _s = UM_DE_CADA_VEZ.lock().unwrap_or_else(|p| p.into_inner());
    let Some(_) = sem_bwrap() else { return };
    let p = projeto("terminal");
    unsafe {
        std::env::set_var("PROVA_HERANCA_DO_TERMINAL", "nao-pode-chegar-ao-hx");
    }
    let agente = p.agente();
    let raiz = std::fs::canonicalize(&p.raiz).unwrap();
    let programa = phxclaw_terminal::Programa {
        programa: "/bin/sh".into(),
        args: vec![
            "-c".into(),
            format!("{}; echo ---; env", script_que_tenta(&agente)),
        ],
        cwd: Some(p.raiz.clone()),
        env: vec![("PHXCLAW_API_TOKEN".into(), "bearer-do-terminal".into())],
    };
    let t =
        phxclaw_agent::processo::terminal_no_bwrap(programa, std::slice::from_ref(&raiz)).unwrap();
    assert!(t.programa.ends_with("bwrap"), "{}", t.programa);
    assert!(!t.args.iter().any(|a| a == "--new-session"), "{:?}", t.args);
    assert!(
        !t.args.iter().any(|a| a.contains("bearer-do-terminal")),
        "o Bearer foi para o argv: {:?}",
        t.args
    );
    let o = std::process::Command::new(&t.programa)
        .args(&t.args)
        .current_dir(t.cwd.as_ref().unwrap())
        .envs(t.env.iter().cloned())
        .output()
        .unwrap();
    unsafe {
        std::env::remove_var("PROVA_HERANCA_DO_TERMINAL");
    }
    let saida = format!(
        "{}\n{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(
        p.vazou(&saida).is_empty(),
        "{:?}:\n{saida}",
        p.vazou(&saida)
    );
    assert!(saida.contains("fn soma"), "{saida}");
    assert!(
        saida.contains("PHXCLAW_API_TOKEN=bearer-do-terminal"),
        "{saida}"
    );
    assert!(!saida.contains("PROVA_HERANCA_DO_TERMINAL"), "{saida}");
    assert!(!saida.contains("PHXCLAW_HOME"), "{saida}");
    assert!(saida.contains("HOME=/work"), "{saida}");
}

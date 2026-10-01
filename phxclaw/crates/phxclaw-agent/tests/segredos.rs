//! A varredura de segredos do `git_write` contra o que ela usa de verdade: o git e o bwrap
//! da maquina num repositorio temporario e o binario oficial do gitleaks, conferido por
//! SHA-256. As credenciais daqui tem o FORMATO real e valor inventado.
//!
//! Sem bwrap ou sem o binario, os testes dizem PULADO e saem -- o relatorio da frente tem
//! de contar isso como nao rodado, nunca como verde.

use phxclaw_agent::git::GitTool;
use phxclaw_agent::segredos::Varredura;
use phxclaw_agent_core::{Tool, ToolContext, ToolError};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// SHA-256 do `gitleaks` 8.28.0 linux_x64. O tarball
/// (`gitleaks_8.28.0_linux_x64.tar.gz`, a65b5253...40eb) confere com o
/// `gitleaks_8.28.0_checksums.txt` do release oficial em 01/10/2026; este e o hash do
/// executavel extraido dele. Vem de fora do teste: calcular aqui o hash do proprio arquivo
/// e conferir contra ele nao provaria nada.
const SHA_GITLEAKS_8_28_0: &str =
    "5fd1b3b0073269484d40078662e921d07427340ab9e6ed526ccd215a565b3298";

/// Chave de acesso da AWS com o formato real (AKIA + 16) e valor inventado.
const AWS_FALSA: &str = "AKIAZ5T7XQ3PLM2NB4VC";
/// Token de acesso pessoal do GitHub com o formato real (ghp_ + 36) e valor inventado.
const GITHUB_FALSO: &str = "ghp_8mQz3Xv1Lr7Tn2Wk9Pb4Yc6Hd0Fs5Ja1Ge3K";

fn gitleaks() -> Option<(PathBuf, PathBuf, String)> {
    let var = |n: &str, p: &str| std::env::var(n).unwrap_or_else(|_| p.into());
    let bin = PathBuf::from(var(
        "PHXCLAW_GITLEAKS_BIN",
        "/var/tmp/gitleaks-8.28.0/gitleaks",
    ));
    let sha = var("PHXCLAW_GITLEAKS_SHA256", SHA_GITLEAKS_8_28_0);
    let Some(bwrap) = phxclaw_agent::arquivos::achar_bwrap() else {
        eprintln!("PULADO: sem bwrap");
        return None;
    };
    if !bin.is_file() {
        eprintln!("PULADO: sem o gitleaks em {bin:?} (PHXCLAW_GITLEAKS_BIN)");
        return None;
    }
    Some((bwrap, bin, sha))
}

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-segredo-{nome}-{}",
        phxclaw_types::new_uuid_v7()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ctx(d: &Path) -> ToolContext {
    ToolContext {
        task_id: "t".into(),
        workdir: d.to_path_buf(),
        timeout: Duration::from_secs(60),
    }
}

fn ferramenta(bwrap: &Path, bin: &Path, sha: &str) -> GitTool {
    let mut g = GitTool::escrita(bwrap.to_path_buf());
    g.segredos = Varredura {
        bin: Some(bin.to_path_buf()),
        sha256: Some(sha.to_string()),
        exigir: false,
        erro: None,
    };
    g
}

async fn json_de(t: &dyn Tool, c: &ToolContext, args: Value) -> Result<Value, ToolError> {
    let r = t.run(args, c).await?;
    Ok(serde_json::from_str(&r.content).unwrap())
}

/// O indice de verdade, lido fora do agente: o que o `git add` recusado NAO pode ter mexido.
fn indice(repo: &Path) -> String {
    let s = std::process::Command::new("git")
        .args(["-C"])
        .arg(repo)
        .args(["ls-files", "--stage"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&s.stdout).into_owned()
}

fn git_fora(repo: &Path, args: &[&str]) {
    let s = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(s.status.success(), "{}", String::from_utf8_lossy(&s.stderr));
}

async fn repo_iniciado(g: &GitTool, d: &Path) -> (ToolContext, PathBuf) {
    let c = ctx(d);
    json_de(
        g,
        &c,
        json!({"action":"init","path":"repo","branch":"main"}),
    )
    .await
    .unwrap();
    (c, d.join("repo"))
}

/// Limpo passa e diz que varreu; segredo no `add` e no `commit` bloqueia com arquivo, linha
/// e regra, sem o segredo; o `add` recusado nao toca no indice.
#[tokio::test]
async fn credencial_falsa_bloqueia_add_e_commit_e_limpo_passa() {
    let Some((bwrap, bin, sha)) = gitleaks() else {
        return;
    };
    let d = tmp("bloqueia");
    let g = ferramenta(&bwrap, &bin, &sha);
    let (c, repo) = repo_iniciado(&g, &d).await;

    std::fs::write(repo.join("leia.txt"), "texto limpo\nsem nada\n").unwrap();
    let st = json_de(
        &g,
        &c,
        json!({"action":"add","path":"repo","paths":["leia.txt"]}),
    )
    .await
    .unwrap();
    assert_eq!(st["varredura_de_segredos"]["feita"], true, "{st}");
    assert_eq!(st["varredura_de_segredos"]["achados"], 0, "{st}");
    let ok = json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":"limpo"}),
    )
    .await
    .unwrap();
    assert!(ok["commit"]["commit"].is_string(), "{ok}");
    assert_eq!(ok["varredura_de_segredos"]["feita"], true, "{ok}");

    // add: bloqueado, e o indice de verdade igual ao de antes.
    std::fs::write(
        repo.join("conf.py"),
        format!("REGIAO = 'sa-east-1'\nCHAVE = \"{AWS_FALSA}\"\n"),
    )
    .unwrap();
    let antes = indice(&repo);
    let e = json_de(
        &g,
        &c,
        json!({"action":"add","path":"repo","paths":["conf.py"]}),
    )
    .await
    .unwrap_err();
    let m = e.to_string();
    assert!(matches!(e, ToolError::Denied(_)), "{m}");
    assert!(
        m.contains("conf.py:2") && m.contains("aws-access-token"),
        "{m}"
    );
    assert!(
        !m.contains(AWS_FALSA) && !m.contains("AKIA"),
        "segredo vazou: {m}"
    );
    assert_eq!(indice(&repo), antes, "add recusado mexeu no indice");

    // commit: o arquivo preparado por fora (o operador, um script) tambem nao passa.
    git_fora(&repo, &["add", "conf.py"]);
    let e = json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":"vaza"}),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(e.contains("conf.py:2") && !e.contains(AWS_FALSA), "{e}");

    // commit -a: o segredo so na arvore (arquivo ja rastreado, mudanca nao preparada).
    git_fora(&repo, &["rm", "-q", "--cached", "conf.py"]);
    std::fs::write(
        repo.join("leia.txt"),
        format!("texto limpo\nsem nada\ntoken = \"{GITHUB_FALSO}\"\n"),
    )
    .unwrap();
    let e = json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":"vaza","all":true}),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        e.contains("leia.txt:3") && e.contains("github-pat") && !e.contains(GITHUB_FALSO),
        "{e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// O que o modelo escreve na arvore nao desliga a guarda: nem o comentario
/// `gitleaks:allow`, nem um `.gitleaks.toml` sem regras, nem um `.gitleaksignore`.
#[tokio::test]
async fn a_arvore_nao_desliga_a_varredura() {
    let Some((bwrap, bin, sha)) = gitleaks() else {
        return;
    };
    let d = tmp("arvore");
    let g = ferramenta(&bwrap, &bin, &sha);
    let (c, repo) = repo_iniciado(&g, &d).await;
    std::fs::write(repo.join(".gitleaks.toml"), "title = \"vazio\"\n").unwrap();
    std::fs::write(repo.join(".gitleaksignore"), "*\n").unwrap();
    std::fs::write(
        repo.join("a.rs"),
        format!("let t = \"{GITHUB_FALSO}\"; // gitleaks:allow\n"),
    )
    .unwrap();
    let e = json_de(&g, &c, json!({"action":"add","path":"repo","paths":["."]}))
        .await
        .unwrap_err()
        .to_string();
    assert!(e.contains("a.rs:1") && e.contains("github-pat"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// A prova de que quem bloqueia e a varredura: o MESMO commit, com ela desligada, passa --
/// e o resultado diz que nao varreu. Com `exigir`, a falta do binario recusa; com o hash
/// errado, recusa mesmo sem `exigir`.
#[tokio::test]
async fn desligada_passa_avisando_exigida_e_hash_errado_recusam() {
    let Some((bwrap, bin, sha)) = gitleaks() else {
        return;
    };
    let d = tmp("desligada");
    let mut g = ferramenta(&bwrap, &bin, &sha);
    g.segredos = Varredura::default();
    let (c, repo) = repo_iniciado(&g, &d).await;
    std::fs::write(repo.join("conf.py"), format!("CHAVE = \"{AWS_FALSA}\"\n")).unwrap();
    let r = json_de(
        &g,
        &c,
        json!({"action":"add","path":"repo","paths":["conf.py"]}),
    )
    .await
    .unwrap();
    assert_eq!(r["varredura_de_segredos"]["feita"], false, "{r}");
    assert!(
        r["varredura_de_segredos"]["motivo"]
            .as_str()
            .unwrap()
            .contains("NAO feita"),
        "{r}"
    );

    g.segredos.exigir = true;
    let e = json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":"x"}),
    )
    .await
    .unwrap_err();
    assert!(matches!(e, ToolError::Denied(_)), "{e}");

    g.segredos = Varredura {
        bin: Some(bin.clone()),
        sha256: Some(format!("{}0", &sha[..63])),
        exigir: false,
        erro: None,
    };
    let e = json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":"x"}),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(e.contains("recusado"), "{e}");

    g.segredos = Varredura::default();
    let r = json_de(
        &g,
        &c,
        json!({"action":"commit","path":"repo","message":"passou"}),
    )
    .await
    .unwrap();
    assert!(r["commit"]["commit"].is_string(), "{r}");
    assert_eq!(r["varredura_de_segredos"]["feita"], false, "{r}");
    let _ = std::fs::remove_dir_all(&d);
}
